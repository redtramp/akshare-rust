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

/// 拉取乐咕数据 `data` 数组并归一化 `date` 列（对应 akshare `pd.to_datetime().dt.date`）。
///
/// `page_url` / `api_path` / `params` 由各函数提供；`select` 为输出列序。
fn fetch_legu_data(
    page_url: &str,
    api_path: &str,
    params: &[(&str, &str)],
    select: &[&str],
) -> Result<Df> {
    let http = HttpClient::default();
    let token = get_token_lg();
    let mut query: Vec<String> = vec![format!("token={token}")];
    for (k, v) in params {
        query.push(format!("{k}={v}"));
    }
    let api_url = format!("https://legulegu.com{api_path}?{}", query.join("&"));
    let data = api_get(&http, page_url, &api_url)?;
    let rows = data
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = Df::from_json_rows(&rows)?;
    out.cast_date(&["date"])?;
    out = out.select(select)?;
    Ok(out)
}

/// 乐咕乐股-主板市盈率（对应 akshare [`akshare.stock_market_pe_lg`]）。
///
/// `symbol`：`上证` / `深证` / `创业板` / `科创版`。科创版走独立接口，
/// 输出列名不同（`总市值`/`市盈率` vs `指数`/`平均市盈率`）。
///
/// # 返回列
/// - 上证/深证/创业板：`日期, 指数, 平均市盈率`
/// - 科创版：`日期, 总市值, 市盈率`
pub fn stock_market_pe_lg(symbol: &str) -> Result<Df> {
    let (market_id, page) = match symbol {
        "上证" => ("1", "https://legulegu.com/stockdata/shanghaiPE"),
        "深证" => ("2", "https://legulegu.com/stockdata/shenzhenPE"),
        "创业板" => ("4", "https://legulegu.com/stockdata/cybPE"),
        "科创版" => ("", "https://legulegu.com/stockdata/ke-chuang-ban-pe"),
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 上证/深证/创业板/科创版）"
            )))
        }
    };
    if market_id.is_empty() {
        let mut df = fetch_legu_data(page, "/api/stockdata/get-ke-chuang-ban-pe", &[], &["date", "close", "pe"])?;
        df.rename_columns(&["日期", "总市值", "市盈率"])?;
        df.cast_numeric(&["总市值", "市盈率"])?;
        return Ok(df);
    }
    let mut df = fetch_legu_data(
        page,
        "/api/stock-data/market-pe",
        &[("marketId", market_id)],
        &["date", "close", "pe"],
    )?;
    df.rename_columns(&["日期", "指数", "平均市盈率"])?;
    df.cast_numeric(&["指数", "平均市盈率"])?;
    Ok(df)
}

/// 乐咕乐股-指数市盈率（对应 akshare [`akshare.stock_index_pe_lg`]）。
///
/// `symbol`：`上证50/沪深300/上证380/创业板50/中证500/上证180/深证红利/深证100/
/// 中证1000/上证红利/中证100/中证800`。
///
/// # 返回列
/// `日期, 指数, 等权静态市盈率, 静态市盈率, 静态市盈率中位数,
/// 等权滚动市盈率, 滚动市盈率, 滚动市盈率中位数`
pub fn stock_index_pe_lg(symbol: &str) -> Result<Df> {
    let code = match symbol {
        "上证50" => "000016.SH",
        "沪深300" => "000300.SH",
        "上证380" => "000009.SH",
        "创业板50" => "399673.SZ",
        "中证500" => "000905.SH",
        "上证180" => "000010.SH",
        "深证红利" => "399324.SZ",
        "深证100" => "399330.SZ",
        "中证1000" => "000852.SH",
        "上证红利" => "000015.SH",
        "中证100" => "000903.SH",
        "中证800" => "000906.SH",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 上证50/沪深300/上证380/创业板50/中证500/上证180/深证红利/深证100/中证1000/上证红利/中证100/中证800）"
            )))
        }
    };
    let mut df = fetch_legu_data(
        "https://legulegu.com/stockdata/sz50-ttm-lyr",
        "/api/stockdata/index-basic-pe",
        &[("indexCode", code)],
        &["date", "close", "lyrPe", "addLyrPe", "middleLyrPe", "ttmPe", "addTtmPe", "middleTtmPe"],
    )?;
    df.rename_columns(&[
        "日期",
        "指数",
        "等权静态市盈率",
        "静态市盈率",
        "静态市盈率中位数",
        "等权滚动市盈率",
        "滚动市盈率",
        "滚动市盈率中位数",
    ])?;
    df.cast_numeric(&[
        "指数",
        "等权静态市盈率",
        "静态市盈率",
        "静态市盈率中位数",
        "等权滚动市盈率",
        "滚动市盈率",
        "滚动市盈率中位数",
    ])?;
    Ok(df)
}

/// 乐咕乐股-主板市净率（对应 akshare [`akshare.stock_market_pb_lg`]）。
///
/// `symbol`：`上证` / `深证` / `创业板` / `科创版`。
///
/// # 返回列
/// `日期, 指数, 市净率, 等权市净率, 市净率中位数`
pub fn stock_market_pb_lg(symbol: &str) -> Result<Df> {
    let (code, page) = match symbol {
        "上证" => ("1", "https://legulegu.com/stockdata/shanghaiPB"),
        "深证" => ("2", "https://legulegu.com/stockdata/shenzhenPB"),
        "创业板" => ("4", "https://legulegu.com/stockdata/cybPB"),
        "科创版" => ("7", "https://legulegu.com/stockdata/ke-chuang-ban-pb"),
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 上证/深证/创业板/科创版）"
            )))
        }
    };
    let mut df = fetch_legu_data(
        page,
        "/api/stockdata/index-basic-pb",
        &[("indexCode", code)],
        &["date", "close", "addPb", "pb", "middlePb"],
    )?;
    df.rename_columns(&["日期", "指数", "市净率", "等权市净率", "市净率中位数"])?;
    df.cast_numeric(&["指数", "市净率", "等权市净率", "市净率中位数"])?;
    Ok(df)
}

/// 乐咕乐股-指数市净率（对应 akshare [`akshare.stock_index_pb_lg`]）。
///
/// `symbol`：同 [`stock_index_pe_lg`] 的 12 个指数。
///
/// # 返回列
/// `日期, 指数, 市净率, 等权市净率, 市净率中位数`
pub fn stock_index_pb_lg(symbol: &str) -> Result<Df> {
    let code = match symbol {
        "上证50" => "000016.SH",
        "沪深300" => "000300.SH",
        "上证380" => "000009.SH",
        "创业板50" => "399673.SZ",
        "中证500" => "000905.SH",
        "上证180" => "000010.SH",
        "深证红利" => "399324.SZ",
        "深证100" => "399330.SZ",
        "中证1000" => "000852.SH",
        "上证红利" => "000015.SH",
        "中证100" => "000903.SH",
        "中证800" => "000906.SH",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 上证50/沪深300/上证380/创业板50/中证500/上证180/深证红利/深证100/中证1000/上证红利/中证100/中证800）"
            )))
        }
    };
    let mut df = fetch_legu_data(
        "https://legulegu.com/stockdata/zz500-ttm-lyr",
        "/api/stockdata/index-basic-pb",
        &[("indexCode", code)],
        &["date", "close", "addPb", "pb", "middlePb"],
    )?;
    df.rename_columns(&["日期", "指数", "市净率", "等权市净率", "市净率中位数"])?;
    df.cast_numeric(&["指数", "市净率", "等权市净率", "市净率中位数"])?;
    Ok(df)
}

/// 乐咕乐股-大盘拥挤度（对应 akshare [`akshare.stock_a_congestion_lg`]）。
///
/// 响应为 `items` 数组，akshare 不改列名（保留英文键）。
///
/// # 返回列
/// `date, close, congestion`
pub fn stock_a_congestion_lg() -> Result<Df> {
    let http = HttpClient::default();
    let token = get_token_lg();
    let page_url = "https://legulegu.com/stockdata/ashares-congestion";
    let api_url = format!("https://legulegu.com/api/stockdata/ashares-congestion?token={token}");
    let data = api_get(&http, page_url, &api_url)?;
    let rows = data
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = Df::from_json_rows(&rows)?;
    out.cast_date(&["date"])?;
    out = out.select(&["date", "close", "congestion"])?;
    out.cast_numeric(&["close", "congestion"])?;
    Ok(out)
}

/// 乐咕乐股-巴菲特指标（对应 akshare [`akshare.stock_buffett_index_lg`]）。
///
/// # 返回列
/// `日期, 收盘价, 总市值, GDP`（可选 `近十年分位数, 总历史分位数`）
pub fn stock_buffett_index_lg() -> Result<Df> {
    let http = HttpClient::default();
    let token = get_token_lg();
    let page_url = "https://legulegu.com/stockdata/marketcap-gdp";
    let api_url = format!(
        "https://legulegu.com/api/stockdata/marketcap-gdp/get-marketcap-gdp?token={token}"
    );
    let data = api_get(&http, page_url, &api_url)?;
    let rows = data
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let out = Df::from_json_rows(&rows)?;
    // akshare 条件重命名 + 可选列（quantile 列存在才输出）
    let rename_map = [
        ("marketCap", "总市值"),
        ("gdp", "GDP"),
        ("close", "收盘价"),
        ("date", "日期"),
        ("quantileInAllHistory", "总历史分位数"),
        ("quantileInRecent10Years", "近十年分位数"),
    ];
    let mut names: Vec<String> = out.column_names();
    let mut has: Vec<&str> = Vec::new();
    for (from, to) in rename_map {
        if let Some(pos) = names.iter().position(|n| n == from) {
            names[pos] = to.to_string();
            has.push(to);
        }
    }
    // 输出列序 = akshare base_cols + 可选列（近十年分位数/总历史分位数）
    let mut final_cols: Vec<&str> = Vec::new();
    for c in ["日期", "收盘价", "总市值", "GDP", "近十年分位数", "总历史分位数"] {
        if has.contains(&c) {
            final_cols.push(c);
        }
    }
    let names_ref: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut out2 = out.clone();
    let _ = out2.rename_columns(&names_ref);
    out2.cast_date(&["日期"])?;
    out2 = out2.select(&final_cols)?;
    let numeric: Vec<&str> = final_cols
        .iter()
        .copied()
        .filter(|c| *c != "日期")
        .collect();
    out2.cast_numeric(&numeric)?;
    Ok(out2)
}

/// 乐咕乐股-股债利差（对应 akshare [`akshare.stock_ebs_lg`]）。
///
/// # 返回列
/// `日期, 沪深300指数, 股债利差, 股债利差均线`
pub fn stock_ebs_lg() -> Result<Df> {
    let mut df = fetch_legu_data(
        "https://legulegu.com/stockdata/equity-bond-spread",
        "/api/stockdata/equity-bond-spread",
        &[("code", "000300.SH")],
        &["date", "close", "peSpread", "peSpreadAverage"],
    )?;
    df.rename_columns(&["日期", "沪深300指数", "股债利差", "股债利差均线"])?;
    df.cast_numeric(&["沪深300指数", "股债利差", "股债利差均线"])?;
    Ok(df)
}

/// 基金仓位公共实现（`type` 区分 股票型/平衡混合型/灵活配置型）。
///
/// 响应为顶层数组，akshare 不改列名。
fn fund_position_lg_impl(page_suffix: &str, pos_type: &str) -> Result<Df> {
    let http = HttpClient::default();
    let token = get_token_lg();
    let page_url = format!("https://legulegu.com/stockdata/fund-position/{page_suffix}");
    let api_url = format!(
        "https://legulegu.com/api/stockdata/fund-position?token={token}&type={pos_type}&category=%E6%80%BB%E4%BB%93%E4%BD%8D&marketId=5"
    );
    let data = api_get(&http, &page_url, &api_url)?;
    let rows = data.as_array().cloned().unwrap_or_default();
    let mut out = Df::from_json_rows(&rows)?;
    out.cast_date(&["date"])?;
    out = out.select(&["date", "close", "position"])?;
    out.cast_numeric(&["close", "position"])?;
    Ok(out)
}

/// 乐咕乐股-基金仓位-股票型基金仓位（对应 akshare [`akshare.fund_stock_position_lg`]）。
///
/// # 返回列
/// `date, close, position`
pub fn fund_stock_position_lg() -> Result<Df> {
    fund_position_lg_impl("pos-stock", "pos_stock")
}

/// 乐咕乐股-基金仓位-平衡混合型基金仓位（对应 akshare [`akshare.fund_balance_position_lg`]）。
///
/// # 返回列
/// `date, close, position`
pub fn fund_balance_position_lg() -> Result<Df> {
    fund_position_lg_impl("pos-pingheng", "pos_pingheng")
}

/// 乐咕乐股-基金仓位-灵活配置型基金仓位（对应 akshare [`akshare.fund_linghuo_position_lg`]）。
///
/// # 返回列
/// `date, close, position`
pub fn fund_linghuo_position_lg() -> Result<Df> {
    fund_position_lg_impl("pos-linghuo", "pos_linghuo")
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
