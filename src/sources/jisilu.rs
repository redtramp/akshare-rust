//! 集思录（jisilu）数据源。
//!
//! 对应 akshare `bond/bond_cb_jsl.py` 等：可转债列表、等权指数、转股价调整记录、强赎。
//! 列表类接口走 POST（JSON 体 + `___jsl` 时间戳查询参数）；指数/调整记录走 GET。
//!
//! 注意：`core::http::post_json` 仅支持 query 参数、不携带 JSON 请求体，而集思录列表
//! 接口要求 `json=payload` 体，故此处直接用 `reqwest` 发送 JSON 体（自带指数退避重试）。

use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Map, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 集思录请求 UA（对应 akshare `utils/cons.headers`）。
pub const JSL_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36";

/// 复用单例客户端（与 `core::http` 一致开启 gzip / 容忍自签证书）。
static JSL_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .build()
        .expect("构建集思录 HTTP 客户端失败")
});

/// 当前毫秒时间戳（对应 akshare `int(time.time() * 1000)`）。
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// POST JSON 体到集思录接口（对应 akshare `requests.post(url, params=..., json=payload)`）。
///
/// 自带 5xx / 连接错误指数退避重试（仅对可重试错误重试，4xx 立即返回，对应 akshare 语义）。
pub fn jsl_post_json(url: &str, body: &Value, cookie: &str) -> Result<Value> {
    let url_q = format!("{url}?___jsl=LST___t={}", now_ms());
    let mut hm = HeaderMap::new();
    let hs: &[(&str, &str)] = &[
        ("User-Agent", JSL_UA),
        ("X-Requested-With", "XMLHttpRequest"),
        ("Referer", "https://www.jisilu.cn/data/cbnew/"),
        ("Content-Type", "application/json"),
    ];
    for (k, v) in hs {
        if let Ok(hv) = HeaderValue::from_str(v) {
            hm.insert(*k, hv);
        }
    }
    if !cookie.is_empty() {
        if let Ok(hv) = HeaderValue::from_str(cookie) {
            hm.insert("Cookie", hv);
        }
    }

    let max_retries = 3u32;
    let mut last: Option<AkshareError> = None;
    for attempt in 0..max_retries {
        match JSL_CLIENT.post(&url_q).headers(hm.clone()).json(body).send() {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let text = resp.text().unwrap_or_default();
                    return serde_json::from_str(&text)
                        .map_err(|e| AkshareError::json(url_q.as_str(), e.to_string()));
                }
                if status.is_client_error() {
                    return Err(AkshareError::Status {
                        status: status.as_u16(),
                        url: url_q.clone(),
                    });
                }
                last = Some(AkshareError::Status {
                    status: status.as_u16(),
                    url: url_q.clone(),
                });
            }
            Err(e) => last = Some(AkshareError::Http(e)),
        }
        if attempt + 1 < max_retries {
            let jitter: f64 = rand::random_range(0.5..1.5);
            let delay = 0.5f64 * 2f64.powi(attempt as i32) + jitter;
            std::thread::sleep(Duration::from_secs_f64(delay));
        }
    }
    Err(last.unwrap_or_else(|| AkshareError::Blocked("集思录 POST 请求重试耗尽".into())))
}

/// 集思录 GET 文本（对应 akshare `requests.get(...)` 返回 HTML，用于 `bond_cb_adj_logs_jsl`）。
pub fn jsl_get_text(url: &str) -> Result<String> {
    let params = Map::new();
    HttpClient::default().get_text_with_headers(url, &params, &[("User-Agent", JSL_UA)], None)
}

/// 集思录 GET JSON（对应 akshare `requests.get(...).json()`，用于 `bond_cb_index_jsl`）。
pub fn jsl_get_json(url: &str) -> Result<Value> {
    let params = Map::new();
    HttpClient::default().get_json_with_headers(url, &params, &[("User-Agent", JSL_UA)], None)
}
