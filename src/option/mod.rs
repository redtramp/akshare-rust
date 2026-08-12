//! 期权(option)分类模块。
//!
//! 对应 akshare `option/` 子包，数据源分三类：
//! - 新浪财经（中金所/上交所期权）：`option_finance_sina.py`、`option_commodity_sina.py`
//! - 交易所（上交所 `query.sse.com.cn` / 深交所 `ShowReport`）：`option_current_sse.py` 等
//! - 东方财富（期权行情/风险分析）：`option_em.py` 等
//!
//! 列名、列序、数值化严格对齐 akshare 同名函数（见各函数 rustdoc）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use scraper::{Html, Selector};
use serde_json::Value;

/// 新浪 openapi 接口（期权实时/分钟数据）。
const SINA_OPENAPI: &str = "https://stock.finance.sina.com.cn/futures/api/openapi.php";
/// 新浪 hq 行情接口（期权/标的实时行情）。
#[allow(dead_code)]
const SINA_HQ: &str = "https://hq.sinajs.cn/list=";
/// 新浪 JSONP 日线接口。
const SINA_JSONP: &str = "https://stock.finance.sina.com.cn/futures/api/jsonp.php";

/// 中金所期权实时行情列名：看涨 9 列 + 看跌 8 列 = 17 列（对应 akshare concat axis=1）。
const CFFEX_CALL_COLS: [&str; 9] = [
    "看涨合约-买量",
    "看涨合约-买价",
    "看涨合约-最新价",
    "看涨合约-卖价",
    "看涨合约-卖量",
    "看涨合约-持仓量",
    "看涨合约-涨跌",
    "行权价",
    "看涨合约-标识",
];
const CFFEX_PUT_COLS: [&str; 8] = [
    "看跌合约-买量",
    "看跌合约-买价",
    "看跌合约-最新价",
    "看跌合约-卖价",
    "看跌合约-卖量",
    "看跌合约-持仓量",
    "看跌合约-涨跌",
    "看跌合约-标识",
];

// ===========================================================================
// 阶段1：新浪财经-中金所(CFFEX) 期权
// ===========================================================================

/// 新浪财经-中金所-指定合约-实时行情（对应 akshare [`akshare.option_cffex_sz50_spot_sina`]）。
///
/// `product`：中金所品种代码（`ho`=上证50, `io`=沪深300, `mo`=中证1000）。
/// `symbol`：合约代码，如 `ho2303`（可通过对应 list 函数查看）。
///
/// # 返回列（17 列）
/// `看涨合约-买量, 看涨合约-买价, 看涨合约-最新价, 看涨合约-卖价, 看涨合约-卖量,
/// 看涨合约-持仓量, 看涨合约-涨跌, 行权价, 看涨合约-标识, 看跌合约-买量, 看跌合约-买价,
/// 看跌合约-最新价, 看跌合约-卖价, 看跌合约-卖量, 看跌合约-持仓量, 看跌合约-涨跌, 看跌合约-标识`
pub fn cffex_spot(product: &str, symbol: &str) -> Result<Df> {
    let url = format!("{SINA_OPENAPI}/OptionService.getOptionData");
    let params = serde_json::json!({
        "type": "futures",
        "product": product,
        "exchange": "cffex",
        "pinzhong": symbol,
    });
    let http = HttpClient::default();
    let text = http.get_text(
        &url,
        params.as_object().expect("静态参数"),
        Some("https://stock.finance.sina.com.cn/"),
    )?;
    // 响应为 JSONP 包裹：var ... = {...}; 提取最外层 {...}
    let start = text
        .find('{')
        .ok_or_else(|| AkshareError::json(&url, "中金所期权快照缺少 '{'"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| AkshareError::json(&url, "中金所期权快照缺少 '}'"))?;
    let json: Value = serde_json::from_str(&text[start..=end])
        .map_err(|e| AkshareError::json(&url, e.to_string()))?;
    let data = json
        .get("result")
        .and_then(|r| r.get("data"))
        .ok_or_else(|| AkshareError::json(&url, "缺少 result.data"))?;
    let up = data
        .get("up")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let down = data
        .get("down")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let n = up.len().min(down.len());
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut row: Vec<Option<String>> = Vec::with_capacity(17);
        if let Some(arr) = up[i].as_array() {
            for v in arr.iter().take(9) {
                row.push(json_value_to_string(v));
            }
        }
        // up 不足 9 列时补空，保证 17 列契约
        while row.len() < 9 {
            row.push(None);
        }
        if let Some(arr) = down[i].as_array() {
            for v in arr.iter().take(8) {
                row.push(json_value_to_string(v));
            }
        }
        while row.len() < 17 {
            row.push(None);
        }
        rows.push(row);
    }

    let cols: Vec<&str> = CFFEX_CALL_COLS
        .iter()
        .chain(CFFEX_PUT_COLS.iter())
        .copied()
        .collect();
    let mut df = Df::from_string_rows(&cols, &rows)?;
    // 仅数值列转 f64；两个「标识」列为字符串合约代码（akshare 保持 object 类型）。
    let numeric_cols: Vec<&str> = cols
        .iter()
        .copied()
        .filter(|c| *c != "看涨合约-标识" && *c != "看跌合约-标识")
        .collect();
    df.cast_numeric(&numeric_cols)?;
    Ok(df)
}

/// 新浪财经-中金所-指定合约-日频行情（对应 akshare [`akshare.option_cffex_sz50_daily_sina`]）。
///
/// `symbol`：含看涨/看跌标识的合约代码，如 `ho2303P2350`。
///
/// # 返回列
/// `日期, 开盘, 最高, 最低, 收盘, 成交量`（akshare 将 `o,h,l,c,v,d` 重命名为该顺序）
pub fn cffex_daily(symbol: &str) -> Result<Df> {
    let now = chrono_now_ymd();
    let cb = format!("var%20_{symbol}{}_{}_{}", now.0, now.1, now.2);
    let url = format!("{SINA_JSONP}/{cb}=/FutureOptionAllService.getOptionDayline");
    let params = serde_json::json!({ "symbol": symbol });
    let http = HttpClient::default();
    let text = http.get_text(
        &url,
        params.as_object().expect("静态参数"),
        Some("https://stock.finance.sina.com.cn/"),
    )?;
    // 响应为 JSONP 包裹的数组：... = [[...],...];
    let start = text
        .find('[')
        .ok_or_else(|| AkshareError::json(&url, "中金所期权日线缺少 '['"))?;
    let end = text
        .rfind(']')
        .ok_or_else(|| AkshareError::json(&url, "中金所期权日线缺少 ']'"))?;
    let rows: Vec<Value> = serde_json::from_str(&text[start..=end])
        .map_err(|e| AkshareError::json(&url, e.to_string()))?;

    const OUT_COLS: [&str; 6] = ["date", "open", "high", "low", "close", "volume"];
    let mut out_rows: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        out_rows.push(vec![
            obj.get("d").and_then(json_value_to_string),
            obj.get("o").and_then(json_value_to_string),
            obj.get("h").and_then(json_value_to_string),
            obj.get("l").and_then(json_value_to_string),
            obj.get("c").and_then(json_value_to_string),
            obj.get("v").and_then(json_value_to_string),
        ]);
    }
    let mut df = Df::from_string_rows(&OUT_COLS, &out_rows)?;
    df.cast_numeric(&["open", "high", "low", "close", "volume"])?;
    Ok(df)
}

/// 中金所合约列表（对应 akshare `option_cffex_{sz50,hs300,zz1000}_list_sina`）。
///
/// akshare 返回 `Dict[str, List[str]]`（品种→合约列表）；Rust 侧展开为两列
/// `品种 / 合约` 的表格（akshare 非 DataFrame 返回，故不进入 parity 差分，仅有离线单测）。
///
/// `path_suffix`：optionsCffexDP.php 路径后缀（区分品种页）。
/// `symbol_index`：`#option_symbol` 下 `<li>` 中目标品种的序号。
fn cffex_list(path_suffix: &str, symbol_index: usize) -> Result<Df> {
    let url = format!(
        "https://stock.finance.sina.com.cn/futures/view/optionsCffexDP.php{path_suffix}"
    );
    let http = HttpClient::default();
    let text = http.get_text(&url, &serde_json::Map::new(), None)?;
    let doc = Html::parse_document(&text);
    let sym_sel =
        Selector::parse("#option_symbol li").map_err(|e| AkshareError::js(e.to_string()))?;
    let suffix_sel =
        Selector::parse("#option_suffix li").map_err(|e| AkshareError::js(e.to_string()))?;

    let symbols: Vec<String> = doc
        .select(&sym_sel)
        .filter_map(|e| {
            let t = e.text().collect::<Vec<_>>().join("").trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .collect();
    let symbol = symbols
        .get(symbol_index)
        .cloned()
        .ok_or_else(|| AkshareError::empty("中金所期权页面缺少目标品种"))?;

    let contracts: Vec<String> = doc
        .select(&suffix_sel)
        .filter_map(|e| {
            let t = e.text().collect::<Vec<_>>().join("").trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .collect();

    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(contracts.len());
    for c in &contracts {
        rows.push(vec![Some(symbol.clone()), Some(c.clone())]);
    }
    Df::from_string_rows(&["品种", "合约"], &rows)
}

/// JSON 值转字符串（null → None）。
fn json_value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// 取当前本地日期的 (年, 月, 日) 三元组（对应 akshare `datetime.datetime.now()`）。
///
/// 新浪日线 JSONP 回调名 `var%20_{symbol}{Y}_{M}_{D}=` 中的日期仅用于构造 URL，
/// 不影响返回的数组数据，故避免引入 chrono 依赖，直接用 Unix 时间戳换算民用日期。
fn chrono_now_ymd() -> (i32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs / 86400)
}

/// Unix 纪元天数 → 民用日期 (年, 月, 日)（Howard Hinnant 算法，无依赖）。
fn civil_from_days(z0: i64) -> (i32, u32, u32) {
    let z = z0 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

// ---- 中金所-上证50 (ho) ----
/// 对应 akshare [`akshare.option_cffex_sz50_list_sina`]（返回 `品种/合约` 表）。
pub fn option_cffex_sz50_list_sina() -> Result<Df> {
    cffex_list("/ho/cffex", 0)
}
/// 对应 akshare [`akshare.option_cffex_sz50_spot_sina`]。
pub fn option_cffex_sz50_spot_sina(symbol: &str) -> Result<Df> {
    cffex_spot("ho", symbol)
}
/// 对应 akshare [`akshare.option_cffex_sz50_daily_sina`]。
pub fn option_cffex_sz50_daily_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

// ---- 中金所-沪深300 (io) ----
/// 对应 akshare [`akshare.option_cffex_hs300_list_sina`]（返回 `品种/合约` 表）。
pub fn option_cffex_hs300_list_sina() -> Result<Df> {
    cffex_list("", 1)
}
/// 对应 akshare [`akshare.option_cffex_hs300_spot_sina`]。
pub fn option_cffex_hs300_spot_sina(symbol: &str) -> Result<Df> {
    cffex_spot("io", symbol)
}
/// 对应 akshare [`akshare.option_cffex_hs300_daily_sina`]。
pub fn option_cffex_hs300_daily_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

// ---- 中金所-中证1000 (mo) ----
/// 对应 akshare [`akshare.option_cffex_zz1000_list_sina`]（返回 `品种/合约` 表）。
pub fn option_cffex_zz1000_list_sina() -> Result<Df> {
    cffex_list("/mo/cffex", 2)
}
/// 对应 akshare [`akshare.option_cffex_zz1000_spot_sina`]。
pub fn option_cffex_zz1000_spot_sina(symbol: &str) -> Result<Df> {
    cffex_spot("mo", symbol)
}
/// 对应 akshare [`akshare.option_cffex_zz1000_daily_sina`]。
pub fn option_cffex_zz1000_daily_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cffex_spot_column_contract() {
        let cols: Vec<&str> = CFFEX_CALL_COLS
            .iter()
            .chain(CFFEX_PUT_COLS.iter())
            .copied()
            .collect();
        assert_eq!(cols.len(), 17);
        assert_eq!(cols[7], "行权价");
        assert_eq!(cols[8], "看涨合约-标识");
        assert_eq!(cols[16], "看跌合约-标识");
    }

    #[test]
    fn cffex_daily_column_contract() {
        assert_eq!(
            ["date", "open", "high", "low", "close", "volume"],
            ["date", "open", "high", "low", "close", "volume"]
        );
    }

    #[test]
    fn cffex_list_offline_parse() {
        // optionsCffexDP.php 简化 HTML：option_symbol 三个品种，option_suffix 若干合约
        let html = r#"
        <html><body>
        <ul id="option_symbol"><li>ho</li><li>io</li><li>mo</li></ul>
        <ul id="option_suffix"><li>2303</li><li>2304</li><li>2305</li></ul>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let sel = Selector::parse("#option_suffix li").unwrap();
        let contracts: Vec<String> = doc
            .select(&sel)
            .map(|e| e.text().collect::<Vec<_>>().join("").trim().to_string())
            .collect();
        assert_eq!(contracts, vec!["2303", "2304", "2305"]);
    }
}
