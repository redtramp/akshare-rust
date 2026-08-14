//! HTTP 客户端封装。
//!
//! 对应 akshare `utils/request.py::request_with_retry` 与
//! `utils/func.py::fetch_paginated_data`：
//! - 指数退避 + 随机抖动重试
//! - 统一 UA / 可选 Referer / 可选代理
//! - JSON 与文本（自动字符集解码）响应
//! - 分页抓取合并

use crate::core::config::get_config;
use crate::core::error::{AkshareError, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use serde_json::{Map, Value};
use std::time::Duration;

/// 反爬特征关键字：命中即判定为被拦截。
const BLOCK_MARKERS: &[&str] = &["_waf", "alichlgref", "just a moment", "challenge-platform"];

/// 需要登录态的响应特征（雪球等）。
/// 注意：`400016` 仅作为雪球业务错误信封 `"error_code":400016` 的一部分匹配，
/// 不可作裸子串——东财等大报表的真实数据里数字字段可能巧合包含 `400016`，
/// 裸匹配会在大响应体上误报为登录态缺失（见 futures_comex_inventory 白银系列）。
const AUTH_MARKERS: &[&str] = &[
    "\"error_code\":400016",
    "\"error_code\": 400016",
    "xq_a_token",
    "需要登录",
    "login required",
];

/// 轻量 HTTP 客户端，围绕 `reqwest::blocking` 封装重试与解码。
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    user_agent: String,
    timeout_secs: u64,
    max_retries: u32,
    base_delay_secs: f64,
    random_delay_range: (f64, f64),
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::from_config(&get_config())
    }
}

impl HttpClient {
    /// 基于全局/指定配置构建客户端。
    pub fn from_config(cfg: &crate::core::config::AkshareConfig) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&cfg.user_agent).unwrap_or_else(|_| {
                HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            }),
        );

        let mut builder = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .gzip(true)
            // 会话 cookie：乐咕/雪球等两步流源依赖首次访问页面写入的 cookie
            .cookie_store(true)
            // 对应 akshare 51 处 verify=False 的自签证书接口
            .danger_accept_invalid_certs(true);

        if let Some(proxy) = &cfg.proxies {
            if let Some(p) = &proxy.http {
                if let Ok(pp) = reqwest::Proxy::http(p) {
                    builder = builder.proxy(pp);
                }
            }
        }

        Self {
            inner: builder.build().expect("构建 HTTP 客户端失败"),
            user_agent: cfg.user_agent.clone(),
            timeout_secs: cfg.timeout_secs,
            max_retries: cfg.max_retries,
            base_delay_secs: cfg.base_delay_secs,
            random_delay_range: cfg.random_delay_range,
        }
    }

    /// 带重试的 GET，返回原始 JSON。
    pub fn get_json(
        &self,
        url: &str,
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Value> {
        let text = self.get_text(url, params, referer)?;
        serde_json::from_str(&text).map_err(|e| AkshareError::json(url, e.to_string()))
    }

    /// 带重试的 GET，返回解析后的 JSON，**即使 HTTP 非 2xx 也返回响应体**
    /// （不 raise_for_status）。
    ///
    /// 用于检测业务级登录态错误：如雪球个股接口在无登录态时返回 HTTP 400 +
    /// `{"error_code": 400016, ...}` 业务 JSON。响应体仍经过 [`detect_block_or_auth`]，
    /// 命中 `400016` 等特征即返回 [`AkshareError::AuthRequired`]
    /// （对应 akshare 抛 `APIError`，PLAN §D2 不伪造数据）。网络/连接错误仍按
    /// 指数退避重试，仅业务级非 2xx 响应被原样返回供调用方判定。
    pub fn get_json_allow_status(
        &self,
        url: &str,
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Value> {
        let mut last_err: Option<AkshareError> = None;
        for attempt in 0..self.max_retries {
            let mut req = self.inner.get(url).query(params);
            if let Some(r) = referer {
                req = req.header(REFERER, r);
            }
            match req.send() {
                Ok(resp) => {
                    let bytes = resp.bytes().map_err(AkshareError::from)?;
                    let text = decode_body(&bytes);
                    // 反爬/登录态特征检测（命中 400016 即返回 AuthRequired）
                    detect_block_or_auth(url, &text)?;
                    return serde_json::from_str(&text)
                        .map_err(|e| AkshareError::json(url, e.to_string()));
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }
            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }
        Err(last_err.unwrap_or_else(|| {
            AkshareError::Blocked("GET(allow_status) 请求重试耗尽，未知错误".into())
        }))
    }

    /// 随机延迟（对应 akshare `time.sleep(random.uniform(...))`），用于分页抓取时降低封禁风险。
    pub fn random_delay(&self) {
        let jitter = rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
        std::thread::sleep(std::time::Duration::from_secs_f64(jitter));
    }

    /// 带重试的 GET（自定义请求头 + referer），返回解析后的 JSON。
    ///
    /// 乐咕等源需要 `X-CSRF-Token` 等自定义头配合会话 cookie。
    pub fn get_json_with_headers(
        &self,
        url: &str,
        params: &Map<String, Value>,
        headers: &[(&str, &str)],
        referer: Option<&str>,
    ) -> Result<Value> {
        let mut last_err: Option<AkshareError> = None;

        for attempt in 0..self.max_retries {
            let mut req = self.inner.get(url).query(params);
            for (k, v) in headers {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    req = req.header(*k, hv);
                }
            }
            if let Some(r) = referer {
                req = req.header(REFERER, r);
            }

            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let bytes = resp.bytes().map_err(AkshareError::from)?;
                        let text = decode_body(&bytes);
                        detect_block_or_auth(url, &text)?;
                        return serde_json::from_str(&text)
                            .map_err(|e| AkshareError::json(url, e.to_string()));
                    }
                    let err = AkshareError::Status {
                        status: status.as_u16(),
                        url: url.to_string(),
                    };
                    if status.is_client_error() {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }

            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AkshareError::Blocked("GET(带自定义头) 请求重试耗尽，未知错误".into())
        }))
    }

    /// 带重试的 GET，返回按字符集解码后的文本。
    pub fn get_text(
        &self,
        url: &str,
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<String> {
        let bytes = self.request_with_retry(url, params, referer)?;
        let text = decode_body(&bytes);
        detect_block_or_auth(url, &text)?;
        Ok(text)
    }

    /// 取响应中指定头的值（对应需要从响应头提取 token 的源，如 99qh 的 `_pcc`）。
    ///
    /// 仅做一次请求（不重试）：token 类接口对失败不敏感，调用方据此决定是否继续。
    pub fn get_response_header(
        &self,
        url: &str,
        params: &Map<String, Value>,
        header: &str,
    ) -> Result<Option<String>> {
        let req = self.inner.get(url).query(params);
        let resp = req.send().map_err(AkshareError::Http)?;
        let value = resp
            .headers()
            .get(header)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        Ok(value)
    }

    /// 带自定义请求头取响应中指定头的值（同 [`Self::get_response_header`]，但可附带
    /// `Origin`/`Referer` 等头——99qh 的 `_pcc` token 仅在带这些头时才返回）。
    pub fn get_response_header_with_headers(
        &self,
        url: &str,
        params: &Map<String, Value>,
        header: &str,
        headers: &[(&str, &str)],
    ) -> Result<Option<String>> {
        let mut req = self.inner.get(url).query(params);
        for (k, v) in headers {
            if let Ok(hv) = HeaderValue::from_str(v) {
                req = req.header(*k, hv);
            }
        }
        let resp = req.send().map_err(AkshareError::Http)?;
        let value = resp
            .headers()
            .get(header)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        Ok(value)
    }

    /// 带重试的 GET，返回文本但**跳过反爬/登录态检测**。
    ///
    /// 用于建立会话 cookie 的首页访问（如雪球 `xueqiu.com/`）：
    /// 该请求的目的只是写入 cookie，响应本身可能是 WAF 页或登录页，
    /// 对内容做检测会产生误报（akshare 对这类请求也不检查内容）。
    pub fn get_text_allow_blocked(
        &self,
        url: &str,
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<String> {
        let bytes = self.request_with_retry(url, params, referer)?;
        Ok(decode_body(&bytes))
    }

    /// 带重试的 GET（自定义请求头 + referer），返回按字符集解码后的文本。
    ///
    /// 同花顺等源需要 `Cookie: v=...` 等自定义头（与 akshare 的 `requests.get(headers=...)` 对应）。
    pub fn get_text_with_headers(
        &self,
        url: &str,
        params: &Map<String, Value>,
        headers: &[(&str, &str)],
        referer: Option<&str>,
    ) -> Result<String> {
        let mut last_err: Option<AkshareError> = None;

        for attempt in 0..self.max_retries {
            let mut req = self.inner.get(url).query(params);
            for (k, v) in headers {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    req = req.header(*k, hv);
                }
            }
            if let Some(r) = referer {
                req = req.header(REFERER, r);
            }

            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let bytes = resp.bytes().map_err(AkshareError::from)?;
                        let text = decode_body(&bytes);
                        detect_block_or_auth(url, &text)?;
                        return Ok(text);
                    }
                    let err = AkshareError::Status {
                        status: status.as_u16(),
                        url: url.to_string(),
                    };
                    if status.is_client_error() {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }

            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AkshareError::Blocked("GET(带自定义头) 请求重试耗尽，未知错误".into())
        }))
    }

    /// 带重试的 POST（query 参数 + 自定义请求头），返回解析后的 JSON。
    ///
    /// 巨潮 cninfo 等源要求 POST + 自定义头（如 `Accept-Enckey`）。
    /// 重试策略与 GET 一致：仅对 5xx 与连接错误重试，4xx 立即返回。
    pub fn post_json(
        &self,
        url: &str,
        params: &Map<String, Value>,
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut last_err: Option<AkshareError> = None;

        for attempt in 0..self.max_retries {
            let mut req = self.inner.post(url).query(params);
            for (k, v) in headers {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    req = req.header(*k, hv);
                }
            }

            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let bytes = resp.bytes().map_err(AkshareError::from)?;
                        let text = decode_body(&bytes);
                        detect_block_or_auth(url, &text)?;
                        return serde_json::from_str(&text)
                            .map_err(|e| AkshareError::json(url, e.to_string()));
                    }
                    let err = AkshareError::Status {
                        status: status.as_u16(),
                        url: url.to_string(),
                    };
                    if status.is_client_error() {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }

            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }

        Err(last_err.unwrap_or_else(|| AkshareError::Blocked("POST 请求重试耗尽，未知错误".into())))
    }

    /// 带重试的 POST（application/x-www-form-urlencoded 表单体 + 自定义请求头），返回解析后的 JSON。
    ///
    /// 对应 akshare `requests.post(url, data=payload, headers=...)`：
    /// 广期所等接口要求**表单体**（裸 query 参数会因无 Content-Length 被拒 411）。
    /// 重试策略与 GET 一致：仅对 5xx 与连接错误重试，4xx 立即返回。
    pub fn post_form(
        &self,
        url: &str,
        params: &Map<String, Value>,
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut last_err: Option<AkshareError> = None;

        for attempt in 0..self.max_retries {
            let mut req = self.inner.post(url).form(params);
            for (k, v) in headers {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    req = req.header(*k, hv);
                }
            }

            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let bytes = resp.bytes().map_err(AkshareError::from)?;
                        let text = decode_body(&bytes);
                        detect_block_or_auth(url, &text)?;
                        return serde_json::from_str(&text)
                            .map_err(|e| AkshareError::json(url, e.to_string()));
                    }
                    let err = AkshareError::Status {
                        status: status.as_u16(),
                        url: url.to_string(),
                    };
                    if status.is_client_error() {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }

            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }

        Err(last_err
            .unwrap_or_else(|| AkshareError::Blocked("POST(表单) 请求重试耗尽，未知错误".into())))
    }

    /// 带重试的 POST（application/x-www-form-urlencoded 表单体），返回按字符集解码后的文本。
    ///
    /// 对应 akshare `requests.post(url, data=payload)` 后取 `r.text()`，用于响应为
    /// HTML（而非 JSON）的接口，如外汇局 `RMBQuery.do`。重试策略与 `post_form` 一致。
    pub fn post_form_text(
        &self,
        url: &str,
        params: &Map<String, Value>,
        headers: &[(&str, &str)],
    ) -> Result<String> {
        let mut last_err: Option<AkshareError> = None;

        for attempt in 0..self.max_retries {
            let mut req = self.inner.post(url).form(params);
            for (k, v) in headers {
                if let Ok(hv) = HeaderValue::from_str(v) {
                    req = req.header(*k, hv);
                }
            }

            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let bytes = resp.bytes().map_err(AkshareError::from)?;
                        let text = decode_body(&bytes);
                        detect_block_or_auth(url, &text)?;
                        return Ok(text);
                    }
                    let err = AkshareError::Status {
                        status: status.as_u16(),
                        url: url.to_string(),
                    };
                    if status.is_client_error() {
                        return Err(err);
                    }
                    last_err = Some(err);
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }

            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AkshareError::Blocked("POST(表单-文本) 请求重试耗尽，未知错误".into())
        }))
    }

    /// 核心重试循环：对应 akshare `request_with_retry`。
    /// 指数退避 `base_delay * 2^attempt + uniform(随机延迟范围)`。
    /// 仅对 5xx 与连接错误重试；4xx 客户端错误立即返回（对应 akshare
    /// `raise_for_status` 语义，重试无意义）。
    fn request_with_retry(
        &self,
        url: &str,
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Vec<u8>> {
        let mut last_err: Option<AkshareError> = None;

        for attempt in 0..self.max_retries {
            let mut req = self.inner.get(url).query(params);
            if let Some(r) = referer {
                req = req.header(REFERER, r);
            }

            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp.bytes().map(|b| b.to_vec()).map_err(AkshareError::from);
                    }
                    let err = AkshareError::Status {
                        status: status.as_u16(),
                        url: url.to_string(),
                    };
                    if status.is_client_error() {
                        return Err(err);
                    }
                    // 5xx：服务端错误，可重试
                    last_err = Some(err);
                }
                Err(e) => last_err = Some(AkshareError::Http(e)),
            }

            if attempt + 1 < self.max_retries {
                let jitter: f64 =
                    rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
                let delay = self.base_delay_secs * (2u32.pow(attempt)) as f64 + jitter;
                std::thread::sleep(Duration::from_secs_f64(delay));
            }
        }

        Err(last_err.unwrap_or_else(|| AkshareError::Blocked("请求重试耗尽，未知错误".into())))
    }

    /// 单次 GET 并解析为 JSON（不重试）：多节点容灾时快速探测节点可用性。
    fn get_json_once(
        &self,
        url: &str,
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Value> {
        let mut req = self.inner.get(url).query(params);
        if let Some(r) = referer {
            req = req.header(REFERER, r);
        }
        let resp = req.send().map_err(AkshareError::Http)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(AkshareError::Status {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }
        let bytes = resp.bytes().map_err(AkshareError::from)?;
        let text = decode_body(&bytes);
        detect_block_or_auth(url, &text)?;
        serde_json::from_str(&text).map_err(|e| AkshareError::json(url, e.to_string()))
    }

    /// 多节点 GET：依次尝试候选 URL，返回首个成功响应。
    ///
    /// 第一轮每节点单次快速探测，失败立即切换；全部失败后按完整重试策略兜底。
    pub fn get_json_any(
        &self,
        urls: &[String],
        params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Value> {
        for url in urls {
            if let Ok(v) = self.get_json_once(url, params, referer) {
                return Ok(v);
            }
        }
        let mut last_err: Option<AkshareError> = None;
        for url in urls {
            match self.get_json(url, params, referer) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            AkshareError::Blocked("所有候选节点均请求失败（多节点容灾耗尽）".into())
        }))
    }

    /// 单主机分页抓取并合并（对应 akshare `fetch_paginated_data`）。
    ///
    /// - 首页确定 `total` 与每页条数，计算总页数
    /// - 后续页带随机延迟 0.5~1.5s（对应 akshare 限流）
    /// - 返回每页 `diff` 数组的拼接
    ///
    /// 注：东财等多节点源请优先使用 [`Self::fetch_paginated_diff_any`]（带节点容灾）。
    pub fn fetch_paginated_diff(
        &self,
        url: &str,
        base_params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Vec<Value>> {
        let first = self.get_json(url, base_params, referer)?;
        self.paginate_diff(first, url, base_params, referer)
    }

    /// 多节点分页抓取（东财 push2 多节点容灾）。
    ///
    /// 第一轮对每个候选节点做**单次**快速探测，连接/网络错误立即切换下一节点，
    /// 避免被限流的主节点拖慢整体延迟；取首个成功节点继续翻页。
    /// 若全部单次探测失败，第二轮按完整重试策略再走一遍（容忍瞬时抖动），
    /// 仍全部失败则返回最后一次错误。
    pub fn fetch_paginated_diff_any(
        &self,
        urls: &[String],
        base_params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Vec<Value>> {
        // 第一轮：每节点单次快速探测
        for url in urls {
            if let Ok(first) = self.get_json_once(url, base_params, referer) {
                return self.paginate_diff(first, url, base_params, referer);
            }
        }
        // 第二轮：完整重试策略兜底
        let mut last_err: Option<AkshareError> = None;
        for url in urls {
            match self.get_json(url, base_params, referer) {
                Ok(first) => return self.paginate_diff(first, url, base_params, referer),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            AkshareError::Blocked("所有候选节点均请求失败（东财多节点容灾耗尽）".into())
        }))
    }

    /// 分页合并：由首页响应确定 `total` 与每页条数，抓取其余页面。
    fn paginate_diff(
        &self,
        first: Value,
        url: &str,
        base_params: &Map<String, Value>,
        referer: Option<&str>,
    ) -> Result<Vec<Value>> {
        let data = first.get("data").cloned().unwrap_or(Value::Null);
        let total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
        let diff = data.get("diff").and_then(Value::as_array).cloned();
        let Some(mut acc) = diff else {
            return Ok(Vec::new());
        };
        let per_page = acc.len().max(1) as u64;
        let total_pages = total.div_ceil(per_page);

        for page in 2..=total_pages {
            let mut params = base_params.clone();
            params.insert("pn".to_string(), Value::from(page));
            // 随机延迟，避免请求过频
            let delay: f64 =
                rand::random_range(self.random_delay_range.0..self.random_delay_range.1);
            std::thread::sleep(Duration::from_secs_f64(delay));

            match self.get_json(url, &params, referer) {
                Ok(v) => {
                    if let Some(rows) = v
                        .get("data")
                        .and_then(|d| d.get("diff"))
                        .and_then(Value::as_array)
                    {
                        acc.extend(rows.iter().cloned());
                    } else {
                        break; // 数据提前结束
                    }
                }
                Err(_) => break, // 单页失败即停止，返回已获取部分
            }
        }
        Ok(acc)
    }

    /// 当前 UA（调试/日志用）。
    #[allow(dead_code)]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// 当前超时（调试/日志用）。
    #[allow(dead_code)]
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }
}

/// 按字符集解码响应体：优先严格 UTF-8（JSON 响应均为 UTF-8），
/// 失败则回退 GBK（交易所/新浪 HTML 页面常见编码）。
fn decode_body(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            // GBK 是中文财经站点最常见的非 UTF-8 编码
            let (cow, _, _) = encoding_rs::GBK.decode(bytes);
            cow.into_owned()
        }
    }
}

/// 检测反爬拦截与登录态特征，命中即报错（对应 akshare 抛异常语义，v1.0 无浏览器兜底）。
fn detect_block_or_auth(url: &str, text: &str) -> Result<()> {
    let lower = text.to_lowercase();
    for marker in AUTH_MARKERS {
        if lower.contains(&marker.to_lowercase()) && text.len() < 200_000 {
            return Err(AkshareError::AuthRequired(format!(
                "响应含登录态特征 '{marker}' (url: {url})"
            )));
        }
    }
    for marker in BLOCK_MARKERS {
        if lower.contains(marker) {
            return Err(AkshareError::Blocked(format!(
                "响应含反爬特征 '{marker}' (url: {url})"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_markers() {
        let r = detect_block_or_auth("http://x", "Just a moment... verifying you are human");
        assert!(matches!(r, Err(AkshareError::Blocked(_))));

        let r = detect_block_or_auth("http://x", r#"{"error_code":400016}"#);
        assert!(matches!(r, Err(AkshareError::AuthRequired(_))));

        let r = detect_block_or_auth("http://x", "正常数据");
        assert!(r.is_ok());
    }

    #[test]
    fn detect_400016_not_in_data() {
        // 大响应体中数字字段巧合包含 "400016" 子串（如 COMEX 白银库存报表），
        // 不得误报为登录态缺失（裸子串匹配的回归用例）。
        let big = format!(
            "{{\"result\":{{\"pages\":3,\"data\":[{{\"STORAGE_TON\":{}}}],\"count\":1}}}}",
            "123400016.78"
        );
        assert!(detect_block_or_auth("http://x", &big).is_ok());
        // 但真正的雪球错误信封仍须命中
        assert!(matches!(
            detect_block_or_auth("http://x", r#"{"error_code": 400016,"error_info":"need login"}"#),
            Err(AkshareError::AuthRequired(_))
        ));
    }

    #[test]
    fn gbk_decode() {
        // "东方财富" GBK 编码
        let gbk = vec![0xB6, 0xAB, 0xB7, 0xBD, 0xB2, 0xC6, 0xB8, 0xBB];
        let text = decode_body(&gbk);
        assert_eq!(text, "东方财富");
    }
}
