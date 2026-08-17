//! REITs 行情模块（东财 REITs 行情中心）。
//!
//! 对应 akshare `reits/reits_basic.py`：
//! - [`reits_realtime_em`]：沪深 REITs 实时行情
//! - [`reits_hist_em`]：沪深 REITs 历史行情（日 K）
//! - [`reits_hist_min_em`]：沪深 REITs 历史分时
//!
//! 列名与 akshare 逐字一致。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use polars::prelude::{Int64Chunked, IntoSeries, NewChunkedArray};
use serde_json::{json, Map, Value};

const CLIST_URL: &str = "https://95.push2.eastmoney.com/api/qt/clist/get";
const CLIST_PARAMS: &[(&str, &str)] = &[
    ("pn", "1"),
    ("pz", "100"),
    ("po", "1"),
    ("np", "1"),
    ("ut", "bd1d9ddb04089700cf9c27f6f7426281"),
    ("fltt", "2"),
    ("invt", "2"),
    ("fid", "f3"),
    ("fs", "m:1 t:9 e:97,m:0 t:10 e:97"),
];

/// 沪深 REITs 代码 → 市场标识映射（对应 akshare `__reits_code_market_map`）。
fn reits_code_market_map() -> Result<Map<String, Value>> {
    let mut params = Map::new();
    for (k, v) in CLIST_PARAMS {
        params.insert((*k).into(), Value::String((*v).into()));
    }
    params.insert("fields".into(), Value::String("f12,f13".into()));

    let http = HttpClient::default();
    let value = http.get_json(CLIST_URL, &params, None)?;
    let diff = value
        .get("data")
        .and_then(|d| d.get("diff"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut map = Map::new();
    for row in diff {
        let Some(obj) = row.as_object() else { continue };
        let code = obj.get("f12").and_then(Value::as_str).unwrap_or_default();
        let market = obj.get("f13").and_then(Value::as_str).unwrap_or_default();
        if !code.is_empty() {
            map.insert(code.to_string(), Value::String(market.to_string()));
        }
    }
    if map.is_empty() {
        return Err(AkshareError::Empty("REITs 代码-市场映射为空".into()));
    }
    Ok(map)
}

/// 东财-沪深 REITs 实时行情（对应 akshare [`akshare.reits_realtime_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 成交量, 成交额, 开盘价, 最高价, 最低价, 昨收`
pub fn reits_realtime_em() -> Result<Df> {
    let mut params = Map::new();
    for (k, v) in CLIST_PARAMS {
        params.insert((*k).into(), Value::String((*v).into()));
    }
    params.insert(
        "fields".into(),
        Value::String("f2,f3,f4,f5,f6,f12,f14,f15,f16,f17,f18".into()),
    );

    let http = HttpClient::default();
    let value = http.get_json(CLIST_URL, &params, None)?;
    let diff = value
        .get("data")
        .and_then(|d| d.get("diff"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // akshare：不排序，reset_index 后 index = range(1, n+1) 作为 序号
    let mut df = Df::from_json_rows(&diff)?;
    if df.height() > 0 {
        let idx: Vec<Option<i64>> = (1..=df.height()).map(|i| Some(i as i64)).collect();
        df.inner_mut().insert_column(
            0,
            Int64Chunked::from_iter_options("index".into(), idx.iter().copied())
                .into_series()
                .into(),
        )?;
    }

    let df = df.select(&[
        "index", "f12", "f14", "f2", "f4", "f3", "f5", "f6", "f17", "f15", "f16", "f18",
    ])?;
    let mut df = df;
    df.rename_columns(&[
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "开盘价",
        "最高价",
        "最低价",
        "昨收",
    ])?;
    df.cast_numeric(&[
        "最新价",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "开盘价",
        "最高价",
        "最低价",
        "昨收",
    ])?;
    Ok(df)
}

/// 东财-沪深 REITs 历史行情（对应 akshare [`akshare.reits_hist_em`]）。
///
/// - `symbol`: REITs 代码，如 `"508097"`。
///
/// # 返回列
/// `日期, 今开, 最高, 最低, 最新价, 成交量, 成交额, 振幅, 换手`
pub fn reits_hist_em(symbol: &str) -> Result<Df> {
    let map = reits_code_market_map()?;
    let market = map
        .get(symbol)
        .and_then(Value::as_str)
        .ok_or_else(|| AkshareError::Param(format!("未知 REITs 代码: {symbol}")))?;
    let secid = format!("{market}.{symbol}");

    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let params = json!({
        "secid": secid,
        "klt": "101",
        "fqt": "1",
        "lmt": "10000",
        "end": "20500000",
        "iscca": "1",
        "fields1": "f1,f2,f3,f4,f5,f6,f7,f8",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64",
        "ut": "f057cbcbce2a86e2866ab8877db1d059",
        "forcect": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let http = HttpClient::default();
    let value = http.get_json(url, &params, None)?;
    let klines = value
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // 14 字段：日期,今开,最新价,最高,最低,成交量,成交额,振幅,涨跌幅,涨跌额,换手,-,-,-
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for line in klines.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        rows.push(vec![
            pick(0),  // 日期
            pick(1),  // 今开
            pick(3),  // 最高
            pick(4),  // 最低
            pick(2),  // 最新价
            pick(5),  // 成交量
            pick(6),  // 成交额
            pick(7),  // 振幅
            pick(10), // 换手
        ]);
    }
    let mut df = Df::from_string_rows(
        &[
            "日期",
            "今开",
            "最高",
            "最低",
            "最新价",
            "成交量",
            "成交额",
            "振幅",
            "换手",
        ],
        &rows,
    )?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&[
        "今开",
        "最高",
        "最低",
        "最新价",
        "成交量",
        "成交额",
        "振幅",
        "换手",
    ])?;
    Ok(df)
}

/// 东财-沪深 REITs 历史分时（对应 akshare [`akshare.reits_hist_min_em`]）。
///
/// - `symbol`: REITs 代码，如 `"508097"`。
///
/// # 返回列
/// `时间, 最新价, 最高, 最低, 成交量, 成交额, 昨收`
pub fn reits_hist_min_em(symbol: &str) -> Result<Df> {
    let map = reits_code_market_map()?;
    let market = map
        .get(symbol)
        .and_then(Value::as_str)
        .ok_or_else(|| AkshareError::Param(format!("未知 REITs 代码: {symbol}")))?;
    let secid = format!("{market}.{symbol}");

    let url = "https://push2.eastmoney.com/api/qt/stock/trends2/get";
    let params = json!({
        "secid": secid,
        "fields1": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,f13,f14,f17",
        "fields2": "f51,f53,f54,f55,f56,f57,f58",
        "iscr": "0",
        "iscca": "0",
        "ut": "f057cbcbce2a86e2866ab8877db1d059",
        "ndays": "5",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let http = HttpClient::default();
    let value = http.get_json(url, &params, None)?;
    let trends = value
        .get("data")
        .and_then(|d| d.get("trends"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // 7 字段：时间,最新价,最高,最低,成交量,成交额,昨收
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(trends.len());
    for line in trends.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        rows.push(vec![
            pick(0),
            pick(1),
            pick(2),
            pick(3),
            pick(4),
            pick(5),
            pick(6),
        ]);
    }
    let mut df = Df::from_string_rows(
        &["时间", "最新价", "最高", "最低", "成交量", "成交额", "昨收"],
        &rows,
    )?;
    df.cast_numeric(&["最新价", "最高", "最低", "成交量", "成交额"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realtime_select_order_offline() {
        // 模拟 clist diff 行，验证列抽取顺序与 akshare 一致
        let rows = vec![json!({
            "f2": "3.500", "f3": "1.12", "f4": "0.04", "f5": "12345",
            "f6": "43210000", "f12": "508097", "f14": "国金中国铁建高速REIT",
            "f15": "3.520", "f16": "3.480", "f17": "3.510", "f18": "3.460",
        })];
        let df = Df::from_json_rows(&rows).unwrap();
        let df = df
            .select(&[
                "f12", "f14", "f2", "f4", "f3", "f5", "f6", "f17", "f15", "f16", "f18",
            ])
            .unwrap();
        assert_eq!(
            df.column_names(),
            vec!["f12", "f14", "f2", "f4", "f3", "f5", "f6", "f17", "f15", "f16", "f18"]
        );
    }

    #[test]
    fn hist_parse_offline() {
        let line = "2026-08-14,3.510,3.500,3.520,3.480,12345,43210000,1.14,0.20,0.01,1.15,-,-,-";
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 14);
        assert_eq!(f[0], "2026-08-14");
        assert_eq!(f[7], "1.14"); // 振幅
        assert_eq!(f[10], "1.15"); // 换手
    }

    #[test]
    fn min_parse_offline() {
        let line = "2026-08-14 09:30,3.510,3.520,3.480,100,350000,3.460";
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 7);
        assert_eq!(f[0], "2026-08-14 09:30");
        assert_eq!(f[1], "3.510");
    }
}
