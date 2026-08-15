//! IP 黑白名单过滤（≡ Node IpAccess）
//!
//! 受管服务与 Mock 服务的接入层共用同一套 IP 策略，避免各传输层复制漂移。
//! 原有逻辑散落在 WsServer / HttpServer / UnifiedState / MockAppState 的 `allow_ip`
//! 以及 mock `allow_ip_simple`，现统一收敛到此处（P1-1）。

/// 判断 `ip` 是否允许接入。
///
/// 规则（与历史实现完全一致）：
/// 1. 命中黑名单 → 拒绝；
/// 2. 白名单非空且未命中白名单 → 拒绝；
/// 3. 其余放行。
pub fn allow_ip(whitelist: &[String], blacklist: &[String], ip: &str) -> bool {
    if blacklist.iter().any(|b| b == ip) {
        return false;
    }
    if !whitelist.is_empty() && !whitelist.iter().any(|w| w == ip) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::allow_ip;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn blacklist_beats_whitelist() {
        assert!(!allow_ip(&s(&["10.0.0.1"]), &s(&["10.0.0.1"]), "10.0.0.1"));
    }

    #[test]
    fn blacklisted_ip_rejected() {
        assert!(!allow_ip(&s(&[]), &s(&["192.168.1.5"]), "192.168.1.5"));
    }

    #[test]
    fn whitelist_allows_only_match() {
        assert!(allow_ip(&s(&["10.0.0.1"]), &s(&[]), "10.0.0.1"));
        assert!(!allow_ip(&s(&["10.0.0.1"]), &s(&[]), "10.0.0.2"));
    }

    #[test]
    fn empty_whitelist_allows_any_non_blacklisted() {
        assert!(allow_ip(&s(&[]), &s(&[]), "203.0.113.7"));
    }
}
