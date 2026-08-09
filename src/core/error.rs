//! 统一错误类型。
//!
//! 对应 akshare 的异常语义：网络错误、数据为空、登录态缺失等，
//! 全部收敛为 [`AkshareError`] 并携带可读上下文（URL、函数名、诊断信息）。

/// 库内统一 Result 别名。
pub type Result<T> = std::result::Result<T, AkshareError>;

/// akshare-rust 统一错误类型。
#[derive(Debug, thiserror::Error)]
pub enum AkshareError {
    /// HTTP 传输层错误（DNS、连接、超时、TLS 等）。
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    /// 非 2xx 状态码。
    #[error("HTTP 状态码异常: {status} (url: {url})")]
    Status { status: u16, url: String },

    /// 响应不是预期的 JSON 结构。
    #[error("JSON 解析失败: {msg} (url: {url})")]
    Json { msg: String, url: String },

    /// 数据为空（akshare 返回空 DataFrame 的语义）。
    #[error("数据为空: {0}")]
    Empty(String),

    /// JS 引擎执行失败（对应 py_mini_racer 抛错）。
    #[error("JS 执行失败: {0}")]
    Js(String),

    /// 参数不合法（对应 akshare 的 ValueError）。
    #[error("参数错误: {0}")]
    Param(String),

    /// 被反爬拦截（WAF 挑战页 / 403 等）。
    #[error("疑似被反爬拦截: {0}")]
    Blocked(String),

    /// 需要登录态（对应雪球 400016 等）。
    #[error("需要登录态: {0}")]
    AuthRequired(String),

    /// IO 错误（资源文件等）。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// polars 数据表操作错误。
    #[error("polars 错误: {0}")]
    Polars(#[from] polars::error::PolarsError),
}

impl AkshareError {
    /// 构造 [`AkshareError::Json`]。
    pub fn json(url: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::Json {
            url: url.into(),
            msg: msg.into(),
        }
    }

    /// 构造 [`AkshareError::Empty`]。
    pub fn empty(msg: impl Into<String>) -> Self {
        Self::Empty(msg.into())
    }

    /// 构造 [`AkshareError::Js`]。
    pub fn js(msg: impl Into<String>) -> Self {
        Self::Js(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_contains_context() {
        let e = AkshareError::json("https://example.com", "missing field");
        let text = e.to_string();
        assert!(text.contains("example.com"));
        assert!(text.contains("missing field"));
    }

    #[test]
    fn error_from_io() {
        let e: AkshareError = std::io::Error::other("io fail").into();
        assert!(e.to_string().contains("IO"));
    }
}
