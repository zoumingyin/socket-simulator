//! matcher.rs —— Mock 规则匹配器
//!
//! 支持：
//! - 路径模式：精确 `/users`、前缀 `/users/*`、参数 `/users/:id`
//! - HTTP 方法：精确（含 ANY 通配）
//! - Header/Query 匹配：exact / contains / regex / exists
//! - 请求体匹配：包含子串

use axum::http::HeaderMap;
use regex::Regex;
use serde_json::Value;

use crate::backend::types::{HttpMethod, MockMatchCondition, MockRule};

/// 匹配结果
#[derive(Debug, Clone)]
pub struct MatchHit {
    /// 预留：命中的规则 ID。当前调用方仅用 `is_some()` 判定命中与否，
    /// 字段待 mock 审计 / 日志展示消费（届时移除 allow）
    #[allow(dead_code)]
    pub rule_id: String,
    /// 预留：路径参数（同上）
    #[allow(dead_code)]
    pub path_params: Vec<(String, String)>,
}

/// 单条条件是否命中
pub fn cond_match(cond: &MockMatchCondition, actual: Option<&str>) -> bool {
    if !cond.enabled {
        return true; // 未启用的条件视为通过
    }
    let v = actual.unwrap_or("");
    match cond.match_kind.as_str() {
        "exists" => !actual.is_some_and(str::is_empty),
        "contains" => v.contains(&cond.value),
        "regex" => match Regex::new(&cond.value) {
            Ok(re) => re.is_match(v),
            Err(_) => false,
        },
        // exact 或默认
        _ => v == cond.value,
    }
}

/// 头部/查询整组匹配（空 = 全通过）
pub fn conds_all_pass(conds: &[MockMatchCondition], values: &dyn Fn(&str) -> Option<String>) -> bool {
    for c in conds {
        if !c.enabled {
            continue;
        }
        let actual = values(&c.key);
        if !cond_match(c, actual.as_deref()) {
            return false;
        }
    }
    true
}

/// 路径模式匹配，返回捕获的命名参数
///
/// 规则：
/// - `/users` 精确
/// - `/users/*` 前缀（* 必须出现在末段且独占一段）
/// - `/users/:id` 参数（:name 占位一段）
/// - `/users/:id/posts` 多个参数
/// - 忽略首尾 `/` 不一致（统一 strip）
pub fn path_match(pattern: &str, actual: &str) -> Option<Vec<(String, String)>> {
    let norm = |s: &str| {
        let t = s.trim_matches('/').to_string();
        if t.is_empty() { String::new() } else { t }
    };
    let p = norm(pattern);
    let a = norm(actual);
    if p.is_empty() && a.is_empty() {
        return Some(vec![]);
    }
    let p_segs: Vec<&str> = if p.is_empty() { vec![] } else { p.split('/').collect() };
    let a_segs: Vec<&str> = if a.is_empty() { vec![] } else { a.split('/').collect() };

    let mut params: Vec<(String, String)> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < p_segs.len() && j < a_segs.len() {
        let p_seg = p_segs[i];
        let a_seg = a_segs[j];

        // 通配 * 出现时必须独占一段，且吞掉剩余所有段
        if p_seg == "*" {
            // 剩余全部归入"通配段"，记录为 params[("*", joined)]
            let rest: Vec<&str> = a_segs[j..].to_vec();
            params.push(("*".to_string(), rest.join("/")));
            return Some(params);
        }
        if let Some(name) = p_seg.strip_prefix(':') {
            params.push((name.to_string(), a_seg.to_string()));
        } else if p_seg != a_seg {
            return None;
        }
        i += 1;
        j += 1;
    }
    if i == p_segs.len() && j == a_segs.len() {
        Some(params)
    } else {
        None
    }
}

/// 方法匹配（含 ANY）
pub fn method_match(rule_method: HttpMethod, req_method: &str) -> bool {
    if rule_method == HttpMethod::Any {
        return true;
    }
    rule_method.as_str().eq_ignore_ascii_case(req_method)
}

/// Body 匹配（包含子串；空 = 全通过）
pub fn body_match(needle: Option<&str>, body: &[u8]) -> bool {
    match needle {
        None | Some("") => true,
        Some(n) => std::str::from_utf8(body).map(|s| s.contains(n)).unwrap_or(false),
    }
}

/// 完整匹配：用于路由分发
pub fn match_rule<'a>(
    rule: &'a MockRule,
    method: &str,
    full_path: &str,
    headers: &HeaderMap,
    query: &serde_json::Map<String, Value>,
    body: &[u8],
) -> Option<MatchHit> {
    if !rule.enabled {
        return None;
    }
    if !method_match(rule.method, method) {
        return None;
    }
    let path_params = path_match(&rule.path_pattern, full_path)?;
    // 去掉 basePath 前缀：path_match 已经 strip 了首尾 `/`，所以这里需要消费 basePath
    // 但规则内的 path_pattern 通常是相对路径；调用方应在调用前把 base_path 前缀去掉
    // 这里我们直接用 full_path；如果带 base_path，调用方负责预处理
    let _ = path_params.len();

    // headers 匹配
    let header_lookup = |k: &str| -> Option<String> {
        headers
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    };
    if !conds_all_pass(&rule.match_headers, &header_lookup) {
        return None;
    }

    // query 匹配
    let query_lookup = |k: &str| -> Option<String> {
        query.get(k).and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        })
    };
    if !conds_all_pass(&rule.match_query, &query_lookup) {
        return None;
    }

    // body 匹配
    if !body_match(rule.match_body.as_deref(), body) {
        return None;
    }

    Some(MatchHit {
        rule_id: rule.id.clone(),
        path_params: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_exact_match() {
        assert!(path_match("/users", "/users").is_some());
        assert!(path_match("/users", "/users/").is_some());
        assert!(path_match("/users/", "/users").is_some());
        assert!(path_match("/users", "/users/123").is_none());
        assert!(path_match("/", "/").is_some());
    }

    #[test]
    fn path_wildcard_match() {
        let hit = path_match("/users/*", "/users/123/posts/1").unwrap();
        assert_eq!(hit[0].0, "*");
        assert_eq!(hit[0].1, "123/posts/1");
    }

    #[test]
    fn path_param_match() {
        let hit = path_match("/users/:id/posts/:pid", "/users/42/posts/7").unwrap();
        assert_eq!(hit[0], ("id".to_string(), "42".to_string()));
        assert_eq!(hit[1], ("pid".to_string(), "7".to_string()));
    }

    #[test]
    fn cond_exact_contains_regex() {
        let mut h = MockMatchCondition::default();
        h.key = "X-Auth".into();
        h.value = "abc".into();
        h.match_kind = "exact".into();
        h.enabled = true;
        assert!(cond_match(&h, Some("abc")));
        assert!(!cond_match(&h, Some("abcd")));

        h.match_kind = "contains".into();
        assert!(cond_match(&h, Some("xxabcxx")));

        h.match_kind = "regex".into();
        h.value = r"^a.c$".into();
        assert!(cond_match(&h, Some("abc")));
        assert!(!cond_match(&h, Some("ac")));
    }

    #[test]
    fn body_substring_match() {
        assert!(body_match(Some("hello"), b"say hello world"));
        assert!(!body_match(Some("xxx"), b"hi"));
        assert!(body_match(None, b"any"));
        assert!(body_match(Some(""), b"any"));
    }
}