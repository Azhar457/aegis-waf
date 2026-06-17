pub mod headers;
pub mod uri;
pub mod body;

use std::collections::HashMap;
use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::time::Instant;
use once_cell::sync::Lazy;
use crate::config::Config;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Phase {
    Headers,
    Uri,
    Body,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Action {
    Block,
    Log,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[allow(dead_code)]
pub struct Rule {
    pub id: &'static str,
    pub name: &'static str,
    pub phase: Phase,
    pub action: Action,
    pub severity: Severity,
    pub description: &'static str,
    pub check: fn(&RequestInfo) -> bool,
}

#[allow(dead_code)]
pub struct RequestInfo<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub headers: &'a HashMap<String, String>,
    pub body: &'a str,
    pub ip: Option<IpAddr>,
}

pub struct RuleEngine {}

struct TokenBucket {
    tokens: f64,
    last_check: Instant,
    rate: f64, // tokens per second
    capacity: f64,
}

static RATE_LIMITER: Lazy<DashMap<IpAddr, TokenBucket>> = Lazy::new(DashMap::new);

impl RuleEngine {
    pub fn new(_cfg: &Config) -> Self {
        Self {}
    }

    /// Jalankan semua rule terhadap request yang sudah diparse.
    /// Return Option<(rule_id, message)> jika diblokir.
    pub fn check_request(
        &self,
        path: &str,
        query: &str,
        headers: &HashMap<String, String>,
        body: &str,
        ip: Option<IpAddr>,
        method: &str,
        enabled_rules: &[String],
    ) -> Option<(String, String)> {
        let req_info = RequestInfo {
            method,
            path,
            query,
            headers,
            body,
            ip,
        };

        // Phase 1: Headers
        for rule in headers::HEADER_RULES {
            if is_rule_enabled(rule.id, enabled_rules) && (rule.check)(&req_info) {
                return Some((rule.id.to_string(), format!("{}: {}", rule.name, rule.description)));
            }
        }

        // Phase 2: URI + Query
        for rule in uri::URI_RULES {
            if is_rule_enabled(rule.id, enabled_rules) && (rule.check)(&req_info) {
                return Some((rule.id.to_string(), format!("{}: {}", rule.name, rule.description)));
            }
        }

        // Phase 3: Body
        for rule in body::BODY_RULES {
            if is_rule_enabled(rule.id, enabled_rules) && (rule.check)(&req_info) {
                return Some((rule.id.to_string(), format!("{}: {}", rule.name, rule.description)));
            }
        }

        None
    }
}

fn is_rule_enabled(rule_id: &str, enabled_rules: &[String]) -> bool {
    if enabled_rules.is_empty() {
        return true;
    }
    let is_toggled_category = rule_id.starts_with("SQLI-")
        || rule_id.starts_with("XSS-")
        || rule_id.starts_with("LFI-")
        || rule_id.starts_with("RFI-")
        || rule_id.starts_with("CMDI-")
        || rule_id.starts_with("SSRF-")
        || rule_id.starts_with("BOT-");

    if !is_toggled_category {
        return true;
    }

    for pattern in enabled_rules {
        if pattern == "*" {
            return true;
        }
        if pattern.ends_with('*') {
            let prefix = pattern.trim_end_matches('*');
            if rule_id.starts_with(prefix) {
                return true;
            }
        } else if pattern.starts_with('*') {
            let suffix = pattern.trim_start_matches('*');
            if rule_id.ends_with(suffix) {
                return true;
            }
        } else if rule_id == pattern {
            return true;
        }
    }
    false
}

impl RuleEngine {
    /// Rate limiter check (token bucket). Return true jika diizinkan.
    pub fn check_rate_limit(&self, ip: IpAddr, limit: u32) -> bool {
        let rate = limit as f64 / 60.0; // req per detik
        let capacity = rate * 2.0; // burst 2x
        let mut bucket = RATE_LIMITER.entry(ip).or_insert_with(|| TokenBucket {
            tokens: capacity,
            last_check: Instant::now(),
            rate,
            capacity,
        });

        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_check).as_secs_f64();
        bucket.last_check = now;

        // Refill token
        bucket.tokens = (bucket.tokens + elapsed * bucket.rate).min(bucket.capacity);

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GlobalConfig;
    use crate::config::TlsConfig;

    fn test_config() -> Config {
        Config {
            global: GlobalConfig {
                port_http: 80,
                port_https: 443,
                max_body_size: 1024,
                default_rate_limit: 100,
                log_dir: "./logs".to_string(),
            },
            tls: TlsConfig {
                mode: "local_ca".to_string(),
                cert_dir: "./certs".to_string(),
            },
            vhosts: vec![],
        }
    }

    #[test]
    fn test_clean_request_passes() {
        let engine = RuleEngine::new(&test_config());
        let mut headers = HashMap::new();
        headers.insert("user-agent".to_string(), "Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string());
        headers.insert("host".to_string(), "example.com".to_string());

        let result = engine.check_request(
            "/index.html",
            "id=123&name=alice",
            &headers,
            "hello world",
            None,
            "GET",
            &[],
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_bot_001_blocked() {
        let engine = RuleEngine::new(&test_config());
        let mut headers = HashMap::new();
        headers.insert("user-agent".to_string(), "sqlmap/1.4.9".to_string());

        let result = engine.check_request(
            "/",
            "",
            &headers,
            "",
            None,
            "GET",
            &[],
        );
        assert!(result.is_some());
        let (rule_id, msg) = result.unwrap();
        assert_eq!(rule_id, "BOT-001");
        assert!(msg.contains("Bad User-Agent"));
    }

    #[test]
    fn test_sqli_001_blocked() {
        let engine = RuleEngine::new(&test_config());
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/x-www-form-urlencoded".to_string());

        let result = engine.check_request(
            "/login",
            "",
            &headers,
            "username=admin' OR 1=1 --",
            None,
            "POST",
            &[],
        );
        assert!(result.is_some());
        let (rule_id, msg) = result.unwrap();
        assert_eq!(rule_id, "SQLI-001");
        assert!(msg.contains("SQL Injection"));
    }

    #[test]
    fn test_lfi_001_blocked() {
        let engine = RuleEngine::new(&test_config());
        let headers = HashMap::new();

        let result = engine.check_request(
            "/../../etc/passwd",
            "",
            &headers,
            "",
            None,
            "GET",
            &[],
        );
        assert!(result.is_some());
        let (rule_id, msg) = result.unwrap();
        assert_eq!(rule_id, "LFI-001");
        assert!(msg.contains("Local File Inclusion"));
    }
}