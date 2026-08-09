//! 全局配置：UA、超时、重试参数、代理。
//!
//! 对应 akshare 的 `utils/context.py`（`AkshareConfig`/`set_proxies`/`ProxyContext`）
//! 与 `utils/cons.py`（全局 UA）。

use once_cell::sync::Lazy;
use std::sync::RwLock;

/// 默认 UA，与 akshare `utils/cons.py` 保持一致。
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";

/// 默认请求超时（秒），对应 akshare `request_with_retry` 的 `timeout=15`。
pub const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// 默认最大重试次数，对应 akshare `max_retries=3`。
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// 默认基础退避延迟（秒），对应 akshare `base_delay=1.0`。
pub const DEFAULT_BASE_DELAY_SECS: f64 = 1.0;

/// 默认随机延迟范围（秒），对应 akshare `random_delay_range=(0.5, 1.5)`。
pub const DEFAULT_RANDOM_DELAY_RANGE: (f64, f64) = (0.5, 1.5);

/// 全局可配置项，对应 akshare 的 `AkshareConfig` 单例。
#[derive(Debug, Clone)]
pub struct AkshareConfig {
    /// 请求 UA。
    pub user_agent: String,
    /// 请求超时（秒）。
    pub timeout_secs: u64,
    /// 最大重试次数。
    pub max_retries: u32,
    /// 指数退避基础延迟（秒）。
    pub base_delay_secs: f64,
    /// 随机延迟范围（秒）。
    pub random_delay_range: (f64, f64),
    /// 代理配置（http/https），对应 `set_proxies`。
    pub proxies: Option<ProxyConfig>,
}

/// 代理配置。
#[derive(Debug, Clone, Default)]
pub struct ProxyConfig {
    /// HTTP 代理地址，如 `http://127.0.0.1:7890`。
    pub http: Option<String>,
    /// HTTPS 代理地址，如 `http://127.0.0.1:7890`。
    pub https: Option<String>,
}

impl Default for AkshareConfig {
    fn default() -> Self {
        Self {
            user_agent: DEFAULT_USER_AGENT.to_string(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_secs: DEFAULT_BASE_DELAY_SECS,
            random_delay_range: DEFAULT_RANDOM_DELAY_RANGE,
            proxies: None,
        }
    }
}

/// 进程级全局配置（对应 akshare 的 `AkshareConfig` 单例）。
static GLOBAL_CONFIG: Lazy<RwLock<AkshareConfig>> =
    Lazy::new(|| RwLock::new(AkshareConfig::default()));

/// 读取全局配置快照。
pub fn get_config() -> AkshareConfig {
    GLOBAL_CONFIG.read().expect("全局配置读锁").clone()
}

/// 写入全局配置。
pub fn set_config(config: AkshareConfig) {
    *GLOBAL_CONFIG.write().expect("全局配置写锁") = config;
}

/// 设置全局代理（对应 akshare `set_proxies`）。
pub fn set_proxies(proxies: Option<ProxyConfig>) {
    let mut guard = GLOBAL_CONFIG.write().expect("全局配置写锁");
    guard.proxies = proxies;
}

/// 读取全局代理（对应 akshare `get_proxies`）。
pub fn get_proxies() -> Option<ProxyConfig> {
    GLOBAL_CONFIG.read().expect("全局配置读锁").proxies.clone()
}

/// RAII 代理上下文：进入时设置代理，离开时恢复（对应 akshare `ProxyContext`）。
pub struct ProxyContext {
    old: Option<ProxyConfig>,
}

impl ProxyContext {
    /// 以指定代理建立上下文。
    pub fn new(proxies: Option<ProxyConfig>) -> Self {
        let old = get_proxies();
        set_proxies(proxies);
        Self { old }
    }
}

impl Drop for ProxyContext {
    fn drop(&mut self) {
        set_proxies(self.old.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_akshare() {
        let cfg = AkshareConfig::default();
        assert!(cfg.user_agent.contains("Chrome"));
        assert_eq!(cfg.timeout_secs, 15);
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.random_delay_range, (0.5, 1.5));
    }

    #[test]
    fn proxy_context_restores_old() {
        set_proxies(None);
        {
            let _ctx = ProxyContext::new(Some(ProxyConfig {
                http: Some("http://127.0.0.1:7890".into()),
                https: None,
            }));
            assert!(get_proxies().is_some());
        }
        assert!(get_proxies().is_none());
    }
}
