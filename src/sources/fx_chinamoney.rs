//! 中国外汇交易中心（chinamoney）外汇行情数据源。
//!
//! 对应 akshare `fx/fx_quote.py` 与 `fx/fx_c_swap_cm.py`：
//! 市场行情 JSON 接口（`fx-c-sw-curv-*` / `rfx-sp-quot` / `rfx-sw-quot` / `cpair-quot`），
//! 以 `t=毫秒时间戳` 表单参数 POST 返回 `records` 数组。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const C_SWAP_URL: &str =
    "https://www.chinamoney.org.cn/r/cms/www/chinamoney/data/fx/fx-c-sw-curv-USD.CNY.json";
const SPOT_URL: &str = "http://www.chinamoney.com.cn/r/cms/www/chinamoney/data/fx/rfx-sp-quot.json";
const SWAP_URL: &str = "http://www.chinamoney.com.cn/r/cms/www/chinamoney/data/fx/rfx-sw-quot.json";
const PAIR_URL: &str = "http://www.chinamoney.com.cn/r/cms/www/chinamoney/data/fx/cpair-quot.json";

/// 当前毫秒时间戳（对应 akshare `str(int(round(time.time() * 1000)))`）。
fn now_ms() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    ms.to_string()
}

/// 通用：POST 行情接口并取 `records` 数组。
fn fetch_records(url: &str) -> Result<Vec<Value>> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("t".into(), Value::String(now_ms()));
    let json = http.post_form(url, &params, &[])?;
    json.get("records")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty(format!("chinamoney {url} records 缺失")))
}

/// 字段提取辅助：records 中每行取指定键（缺失为 None）。
fn col(records: &[Value], keys: &[&str]) -> Vec<Vec<Option<String>>> {
    records
        .iter()
        .map(|r| {
            keys.iter()
                .map(|k| r.get(k).and_then(Value::as_str).map(str::to_string))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// 人民币外汇即期报价（对应 akshare [`fx_spot_quote`]）。
///
/// # 返回列
/// `货币对, 买报价, 卖报价`
pub fn fx_spot_quote() -> Result<Df> {
    let records = fetch_records(SPOT_URL)?;
    let rows = col(&records, &["ccyPair", "bidPrc", "askPrc"]);
    let mut df = Df::from_string_rows(&["货币对", "买报价", "卖报价"], &rows)?;
    df.cast_numeric(&["买报价", "卖报价"])?;
    Ok(df)
}

/// 外币对即期报价（对应 akshare [`fx_pair_quote`]）。
///
/// # 返回列
/// `货币对, 买报价, 卖报价`
pub fn fx_pair_quote() -> Result<Df> {
    let records = fetch_records(PAIR_URL)?;
    let rows = col(&records, &["ccyPair", "bidPrc", "askPrc"]);
    let mut df = Df::from_string_rows(&["货币对", "买报价", "卖报价"], &rows)?;
    df.cast_numeric(&["买报价", "卖报价"])?;
    Ok(df)
}

/// 人民币外汇远掉报价（对应 akshare [`fx_swap_quote`]）。
///
/// # 返回列
/// `货币对, 1周, 1月, 3月, 6月, 9月, 1年`
pub fn fx_swap_quote() -> Result<Df> {
    let records = fetch_records(SWAP_URL)?;
    let rows = col(
        &records,
        &["ccyPair", "label_1W", "label_1M", "label_3M", "label_6M", "label_9M", "label_1Y"],
    );
    Df::from_string_rows(
        &["货币对", "1周", "1月", "3月", "6月", "9月", "1年"],
        &rows,
    )
}

/// 外汇掉期 C-Swap 定盘曲线（对应 akshare [`fx_c_swap_cm`]）。
///
/// # 返回列
/// `日期时间, 期限品种, 掉期点(Pips), 掉期点数据源, 全价汇率`
pub fn fx_c_swap_cm() -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("t".into(), Value::String(now_ms()));
    let json = http.post_form(C_SWAP_URL, &params, &[])?;
    let records = json
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("chinamoney C-Swap records 缺失".into()))?;
    let rows = col(
        &records,
        &["curveTime", "tenor", "swapPnt", "dataSource", "swapAllPrc"],
    );
    let mut df = Df::from_string_rows(
        &["日期时间", "期限品种", "掉期点(Pips)", "掉期点数据源", "全价汇率"],
        &rows,
    )?;
    df.cast_numeric(&["掉期点(Pips)", "全价汇率"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_extracts_in_order() {
        let records = vec![
            serde_json::json!({"a": "x", "b": "1"}),
            serde_json::json!({"a": "y", "b": "2"}),
        ];
        let rows = col(&records, &["a", "b"]);
        assert_eq!(rows[0], vec![Some("x".into()), Some("1".into())]);
        assert_eq!(rows[1], vec![Some("y".into()), Some("2".into())]);
    }
}
