//! 中国外汇交易中心（chinamoney）外汇市场行情。
//!
//! 对应 akshare `fx/fx_quote.py` 与 `fx/fx_c_swap_cm.py`：
//! - 即期 / 远掉 / 外币对报价走 `www.chinamoney.com.cn` 的 JSON 接口
//!   （POST 表单体，参数 `t = 毫秒时间戳`）
//! - C-Swap 定盘曲线走 `www.chinamoney.org.cn` 的 JSON 接口（同上）
//!
//! 行情为实时数据，列契约稳定但数值随时间变化，parity 用例使用 `loose` 模式。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use serde_json::{Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

/// 当前毫秒时间戳（对应 akshare `str(int(round(time.time() * 1000)))`）。
fn now_millis() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    ms.to_string()
}

/// POST chinamoney 行情接口，返回 `records` 数组。
fn post_quote(url: &str) -> Result<Vec<Value>> {
    let mut params = Map::new();
    params.insert("t".into(), Value::String(now_millis()));
    let http = HttpClient::default();
    let data = http.post_form(url, &params, &[])?;
    Ok(data
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// 从行情记录中按 `(原始键, 输出列名)` 对直接抽取目标列（顺序=输出列序），
/// 缺失键记为 `None`（对应 akshare 取列缺失时的 NaN / 空值）。
fn extract(records: &[Value], picks: &[(&str, &str)]) -> Result<Df> {
    let col_names: Vec<&str> = picks.iter().map(|(_, cn)| *cn).collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(records.len());
    for r in records {
        let Some(obj) = r.as_object() else {
            rows.push(vec![None; picks.len()]);
            continue;
        };
        let row: Vec<Option<String>> = picks
            .iter()
            .map(|(k, _)| match obj.get(*k) {
                Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(other) => Some(other.to_string()),
                None => None,
            })
            .collect();
        rows.push(row);
    }
    Df::from_string_rows(&col_names, &rows)
}

/// 中国外汇交易中心-人民币外汇即期报价（对应 akshare [`akshare.fx_spot_quote`]）。
///
/// # 返回列
/// `货币对, 买报价, 卖报价`
pub fn fx_spot_quote() -> Result<Df> {
    const URL: &str = "http://www.chinamoney.com.cn/r/cms/www/chinamoney/data/fx/rfx-sp-quot.json";
    let records = post_quote(URL)?;
    const PICKS: [(&str, &str); 3] = [
        ("ccyPair", "货币对"),
        ("bidPrc", "买报价"),
        ("askPrc", "卖报价"),
    ];
    let mut df = extract(&records, &PICKS)?;
    df.cast_numeric(&["买报价", "卖报价"])?;
    Ok(df)
}

/// 中国外汇交易中心-人民币外汇远掉报价（对应 akshare [`akshare.fx_swap_quote`]）。
///
/// # 返回列
/// `货币对, 1周, 1月, 3月, 6月, 9月, 1年`
pub fn fx_swap_quote() -> Result<Df> {
    const URL: &str = "http://www.chinamoney.com.cn/r/cms/www/chinamoney/data/fx/rfx-sw-quot.json";
    let records = post_quote(URL)?;
    const PICKS: [(&str, &str); 7] = [
        ("ccyPair", "货币对"),
        ("label_1W", "1周"),
        ("label_1M", "1月"),
        ("label_3M", "3月"),
        ("label_6M", "6月"),
        ("label_9M", "9月"),
        ("label_1Y", "1年"),
    ];
    extract(&records, &PICKS)
}

/// 中国外汇交易中心-外币对即期报价（对应 akshare [`akshare.fx_pair_quote`]）。
///
/// # 返回列
/// `货币对, 买报价, 卖报价`
pub fn fx_pair_quote() -> Result<Df> {
    const URL: &str = "http://www.chinamoney.com.cn/r/cms/www/chinamoney/data/fx/cpair-quot.json";
    let records = post_quote(URL)?;
    const PICKS: [(&str, &str); 3] = [
        ("ccyPair", "货币对"),
        ("bidPrc", "买报价"),
        ("askPrc", "卖报价"),
    ];
    let mut df = extract(&records, &PICKS)?;
    df.cast_numeric(&["买报价", "卖报价"])?;
    Ok(df)
}

/// 中国外汇交易中心-外汇掉期 C-Swap 定盘曲线（对应 akshare [`akshare.fx_c_swap_cm`]）。
///
/// # 返回列
/// `日期时间, 期限品种, 掉期点(Pips), 掉期点数据源, 全价汇率`
pub fn fx_c_swap_cm() -> Result<Df> {
    const URL: &str =
        "https://www.chinamoney.org.cn/r/cms/www/chinamoney/data/fx/fx-c-sw-curv-USD.CNY.json";
    let records = post_quote(URL)?;
    const PICKS: [(&str, &str); 5] = [
        ("curveTime", "日期时间"),
        ("tenor", "期限品种"),
        ("swapPnt", "掉期点(Pips)"),
        ("dataSource", "掉期点数据源"),
        ("swapAllPrc", "全价汇率"),
    ];
    let mut df = extract(&records, &PICKS)?;
    df.cast_numeric(&["掉期点(Pips)", "全价汇率"])?;
    Ok(df)
}
