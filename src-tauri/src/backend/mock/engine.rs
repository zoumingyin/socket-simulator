//! engine.rs —— Mock 匹配/响应唯一入口（P2-2）
//!
//! 方案 B（双配置、单引擎）：独立 Mock 与共端口 Mock 两套持久化并存，
//! 但**匹配 → 响应**的编排逻辑只有这一处权威实现。
//!
//! 调用方职责：
//! - 解析请求（method / path / headers / query / body）
//! - 把「规则匹配用的相对路径」填进 `MockRequest.path`：
//!   - 独立主端口/自定义端口：先 `strip_base(full, base_path)`
//!   - 共端口：即请求完整路径（无 base_path 概念）
//! - IP 过滤由调用方中间件负责，引擎不感知
//!
//! 规则级 `enabled` 由 `matcher::match_rule` 内部统一过滤，
//! 因此独立与共端口行为天然一致（见 `docs/plans/2026-08-14-p2-mock-model-decision.md`）。

use std::time::Duration;

use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::Map;

use crate::backend::mock::matcher::match_rule;
use crate::backend::mock::responder::{default_response, rule_response};
use crate::backend::types::{MockRule, MockServiceConfig};

/// Mock 端点配置（引擎输入，与来源无关）。
pub struct MockEndpoint {
    pub rules: Vec<MockRule>,
    pub default_status_code: u16,
    pub default_response_body: String,
    pub default_delay_ms: u32,
}

/// 已解析的请求元数据（path 已是「规则匹配用的相对路径」）。
pub struct MockRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub headers: &'a HeaderMap,
    pub query: &'a Map<String, serde_json::Value>,
    pub body: &'a [u8],
}

/// 唯一匹配/响应入口：按规则顺序匹配，首个命中即返回；无命中走默认响应。
pub async fn dispatch(endpoint: &MockEndpoint, req: &MockRequest<'_>) -> Response {
    for rule in &endpoint.rules {
        if match_rule(rule, req.method, req.path, req.headers, req.query, req.body).is_some() {
            if rule.response_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(rule.response_delay_ms as u64)).await;
            }
            return rule_response(
                rule.response_status_code,
                &rule.response_headers,
                &rule.response_body,
                rule.response_delay_ms,
            );
        }
    }

    if endpoint.default_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(endpoint.default_delay_ms as u64)).await;
    }
    let tmp = MockServiceConfig {
        default_status_code: endpoint.default_status_code,
        default_response_body: endpoint.default_response_body.clone(),
        default_delay_ms: endpoint.default_delay_ms,
        ..Default::default()
    };
    default_response(&tmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::types::{HttpMethod, MockMatchCondition, MockRule};

    fn rule_get_users() -> MockRule {
        MockRule {
            id: "r1".to_string(),
            method: HttpMethod::Get,
            path_pattern: "/users".to_string(),
            response_status_code: 200,
            response_body: "{\"ok\":true}".to_string(),
            enabled: true,
            ..Default::default()
        }
    }

    fn endpoint_with(rule: MockRule) -> MockEndpoint {
        MockEndpoint {
            rules: vec![rule],
            default_status_code: 404,
            default_response_body: "{\"error\":\"not found\"}".to_string(),
            default_delay_ms: 0,
        }
    }

    fn status_of(resp: &Response) -> u16 {
        resp.status().as_u16()
    }

    #[tokio::test]
    async fn engine_matches_rule_and_returns_response() {
        let ep = endpoint_with(rule_get_users());
        let headers = HeaderMap::new();
        let query = Map::new();
        let req = MockRequest {
            method: "GET",
            path: "/users",
            headers: &headers,
            query: &query,
            body: &[],
        };
        let resp = dispatch(&ep, &req).await;
        assert_eq!(status_of(&resp), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("ok"));
    }

    #[tokio::test]
    async fn engine_disabled_rule_falls_through_to_default() {
        let mut r = rule_get_users();
        r.enabled = false;
        let ep = endpoint_with(r);
        let headers = HeaderMap::new();
        let query = Map::new();
        let req = MockRequest {
            method: "GET",
            path: "/users",
            headers: &headers,
            query: &query,
            body: &[],
        };
        let resp = dispatch(&ep, &req).await;
        assert_eq!(status_of(&resp), 404);
    }

    #[tokio::test]
    async fn engine_no_rule_match_returns_default() {
        let ep = endpoint_with(rule_get_users());
        let headers = HeaderMap::new();
        let query = Map::new();
        let req = MockRequest {
            method: "GET",
            path: "/posts",
            headers: &headers,
            query: &query,
            body: &[],
        };
        let resp = dispatch(&ep, &req).await;
        assert_eq!(status_of(&resp), 404);
    }

    #[tokio::test]
    async fn engine_method_mismatch_returns_default() {
        let ep = endpoint_with(rule_get_users());
        let headers = HeaderMap::new();
        let query = Map::new();
        let req = MockRequest {
            method: "POST",
            path: "/users",
            headers: &headers,
            query: &query,
            body: &[],
        };
        let resp = dispatch(&ep, &req).await;
        assert_eq!(status_of(&resp), 404);
    }

    #[tokio::test]
    async fn engine_header_condition_gates_match() {
        let mut r = rule_get_users();
        r.match_headers = vec![MockMatchCondition {
            key: "X-Auth".to_string(),
            value: "abc".to_string(),
            match_kind: "exact".to_string(),
            enabled: true,
            ..Default::default()
        }];
        let ep = endpoint_with(r);

        // 无匹配头 → 默认
        let headers = HeaderMap::new();
        let query = Map::new();
        let miss = dispatch(
            &ep,
            &MockRequest {
                method: "GET",
                path: "/users",
                headers: &headers,
                query: &query,
                body: &[],
            },
        )
        .await;
        assert_eq!(status_of(&miss), 404);

        // 带匹配头 → 命中
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth", "abc".parse().unwrap());
        let query = Map::new();
        let hit = dispatch(
            &ep,
            &MockRequest {
                method: "GET",
                path: "/users",
                headers: &headers,
                query: &query,
                body: &[],
            },
        )
        .await;
        assert_eq!(status_of(&hit), 200);
    }

    #[tokio::test]
    async fn engine_path_is_caller_relative_contract() {
        // 引擎对 path 不做 base_path 处理：独立端口由调用方 strip，共端口直接传全路径。
        // 同一规则 + 同一相对 path，引擎结果一致（三路一致的核心契约）。
        let ep = endpoint_with(rule_get_users());
        let headers = HeaderMap::new();
        let query = Map::new();
        let co_port = dispatch(
            &ep,
            &MockRequest {
                method: "GET",
                path: "/users",
                headers: &headers,
                query: &query,
                body: &[],
            },
        )
        .await;
        let independent = dispatch(
            &ep,
            &MockRequest {
                method: "GET",
                path: "/users",
                headers: &headers,
                query: &query,
                body: &[],
            },
        )
        .await;
        assert_eq!(status_of(&co_port), status_of(&independent));
        assert_eq!(status_of(&co_port), 200);
    }
}
