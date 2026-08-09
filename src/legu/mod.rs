//! 乐咕乐股（legulegu）数据源。
//!
//! 对应 akshare `stock_feature/stock_a_indicator.py::get_token_lg/get_cookie_csrf`
//! 与 `stock_gxl_lg.py`/`stock_ttm_lyr.py` 等模块。两步流：
//!
//! 1. `token = md5(YYYY-MM-DD)`（对应 [`get_token_lg`]，`md-5` crate）
//! 2. 先 GET 页面拿 `_csrf`（HTML `<meta name="_csrf" content="...">`）写入会话
//!    cookie，再用 `X-CSRF-Token` 头 + 会话 cookie 请求 API
//!
//! 注：本机当前对该站点 403（nginx 封禁），代码与 akshare 完全一致，换环境可用。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use md5::Digest;
use serde_json::{Map, Value};

/// 生成乐咕 token（对应 akshare `get_token_lg`：md5(今日日期)）。
///
/// 注意：akshare 用 `datetime.now()`（本地时区），本实现默认 Asia/Shanghai(+8)，
/// 与系统时区一致时输出与 akshare 逐字符相同。
pub fn get_token_lg() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // +8h 本地时区偏移（对应 akshare datetime.now()）
    let local = now + 8 * 3600;
    let days = local / 86_400;
    // 1970-01-01 起的天数 → 年/月/日（Howard Hinnant 算法）
    let (y, m, d) = civil_from_days(days as i64);
    let date_str = format!("{y:04}-{m:02}-{d:02}");
    let mut h = md5::Md5::new();
    h.update(date_str.as_bytes());
    format!("{:x}", h.finalize())
}

/// 天数 → 公历日期（Howard Hinnant `civil_from_days`）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 从页面 HTML 提取 `_csrf` token（对应 akshare BeautifulSoup 解析）。
fn extract_csrf(html: &str) -> Result<String> {
    // <meta name="_csrf" content="...">
    let mut best: Option<String> = None;
    let bytes = html.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 4] == b"csrf" {
            // 向前找 <meta，向后找 content="
            let window = &html[i.saturating_sub(60)..(i + 200).min(html.len())];
            if let Some(pos) = window.find("content=\"") {
                let rest = &window[pos + 9..];
                let end = rest.find('"').unwrap_or(0);
                if end > 0 {
                    best = Some(rest[..end].to_string());
                    break;
                }
            }
        }
        i += 1;
    }
    best.ok_or_else(|| AkshareError::Blocked("乐咕页面未找到 _csrf token".into()))
}

/// 两步流公共请求：GET 页面取 csrf（会话 cookie 自动保存）→ GET API。
///
/// 返回 API 响应 JSON。`page_url` 为页面地址（写 cookie + 提取 csrf），
/// 然后以 `X-CSRF-Token` 头 + 会话 cookie 请求 `api_url`（已含 `token` 参数）。
fn api_get(http: &HttpClient, page_url: &str, api_url: &str) -> Result<Value> {
    // 1) 访问页面拿 csrf + 写会话 cookie
    let page = http.get_text(page_url, &Map::new(), None)?;
    let csrf = extract_csrf(&page)?;
    // 2) API 请求（带 csrf 头 + 会话 cookie + referer）
    let headers = vec![("X-CSRF-Token", csrf.as_str())];
    let url = api_url.to_string();
    http.get_json_with_headers(&url, &Map::new(), &headers, Some(page_url))
}

/// 乐咕乐股-股息率-A 股股息率（对应 akshare [`akshare.stock_a_gxl_lg`]）。
///
/// `symbol`: `"上证A股"/"深证A股"/"创业板"/"科创板"`。
///
/// # 返回列
/// `日期, 股息率`
pub fn stock_a_gxl_lg(symbol: &str) -> Result<Df> {
    let symbol_map = match symbol {
        "上证A股" => "shangzheng",
        "深证A股" => "shenzheng",
        "创业板" => "chuangyeban",
        "科创板" => "kechuangban",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 上证A股/深证A股/创业板/科创板）"
            )))
        }
    };
    let http = HttpClient::default();
    let page_url = "https://legulegu.com/stockdata/guxilv";
    let token = get_token_lg();
    let url = format!("https://legulegu.com/api/stockdata/guxilv?token={token}");
    let data = api_get(&http, page_url, &url)?;
    let rows = data.get(symbol_map).and_then(Value::as_array).cloned();
    let rows = rows.unwrap_or_default();
    // 只取 date / addDvTtm 两列
    let df = Df::from_json_rows(&rows)?;
    let mut out = df.select(&["date", "addDvTtm"])?;
    out.rename_columns(&["日期", "股息率"])?;
    out.cast_date(&["日期"])?;
    out.cast_numeric(&["股息率"])?;
    Ok(out)
}

/// 乐咕乐股-股息率-恒生指数股息率（对应 akshare [`akshare.stock_hk_gxl_lg`]）。
///
/// # 返回列
/// `日期, 股息率`
pub fn stock_hk_gxl_lg() -> Result<Df> {
    let http = HttpClient::default();
    let page_url = "https://legulegu.com/stockdata/market/hk/dv/hsi";
    let token = get_token_lg();
    let url = format!("https://legulegu.com/api/stockdata/hs?token={token}&indexCode=HSI");
    let data = api_get(&http, page_url, &url)?;
    let rows = data.as_array().cloned().unwrap_or_default();
    let df = Df::from_json_rows(&rows)?;
    let mut out = df.select(&["date", "dvRatio"])?;
    out.rename_columns(&["日期", "股息率"])?;
    out.cast_date(&["日期"])?;
    out.cast_numeric(&["股息率"])?;
    Ok(out)
}

/// 乐咕乐股-全部 A 股等权重/中位数市盈率（对应 akshare [`akshare.stock_a_ttm_lyr`]）。
///
/// 直接返回响应 `data` 全列（akshare 不改列名），仅归一化 `date` 为日期。
pub fn stock_a_ttm_lyr() -> Result<Df> {
    let http = HttpClient::default();
    let page_url = "https://www.legulegu.com/stockdata/a-ttm-lyr";
    let token = get_token_lg();
    let url =
        format!("https://legulegu.com/api/stock-data/market-ttm-lyr?marketId=5&token={token}");
    let data = api_get(&http, page_url, &url)?;
    let rows = data
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = Df::from_json_rows(&rows)?;
    let has_date = out.column_names().iter().any(|n| n == "date");
    if has_date {
        out.cast_date(&["date"])?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_32_hex() {
        let t = get_token_lg();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn civil_from_days_epoch() {
        // 1970-01-01 = 0 days
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2024-01-01 = 19723 days
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        // 2000-02-29（闰日）
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn token_is_local_date_md5() {
        // 与 akshare get_token_lg 一致：md5(本地日期) 32 位 hex。
        // 用 `date` 系统命令交叉验证（= 本地时区日期），避免硬编码日期导致测试过期。
        let t = get_token_lg();
        assert_eq!(t.len(), 32);
        if let Ok(out) = std::process::Command::new("date").arg("+%Y-%m-%d").output() {
            if out.status.success() {
                let local_date = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let mut h = md5::Md5::new();
                h.update(local_date.as_bytes());
                let expected = format!("{:x}", h.finalize());
                assert_eq!(t, expected, "token 应等于 md5(本地日期 {local_date})");
            }
        }
    }

    #[test]
    fn extract_csrf_from_html() {
        let html = r#"<html><head><meta name="_csrf" content="abc123xyz"></head></html>"#;
        assert_eq!(extract_csrf(html).unwrap(), "abc123xyz");
    }

    #[test]
    fn extract_csrf_missing() {
        assert!(extract_csrf("<html>no csrf</html>").is_err());
    }
}
