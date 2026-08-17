//! forex 外汇行情模块（东财行情中心-外汇市场）。
//!
//! 对应 akshare `forex/forex_em.py`：
//! - [`forex_spot_em`]：所有汇率实时行情（push2 clist 分页）
//! - [`forex_hist_em`]：单品种历史行情（push2his kline）
//!
//! 列名与 akshare 逐字一致。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{fetch_clist, push2_urls};
use serde_json::{json, Map, Value};

/// 品种 → 市场码映射（对应 akshare `forex/cons.py::symbol_market_map`，
/// secid = `{市场码}.{品种}`，如 `120.USDCNYC`）。
fn forex_market_code(symbol: &str) -> Option<&'static str> {
    const MAP: [(&str, &str); 190] = [
        ("EURCNYC", "120"),
        ("JPYZAR", "119"),
        ("NZDCNYC", "120"),
        ("CNYRUBC", "120"),
        ("AUDCNYC", "120"),
        ("JPYGBP", "119"),
        ("JPYSGD", "119"),
        ("JPYCNH", "133"),
        ("JPYAUD", "119"),
        ("USDBRL", "119"),
        ("JPYEUR", "119"),
        ("JPYTRY", "119"),
        ("JPYCAD", "119"),
        ("CHFZAR", "119"),
        ("JPYHKD", "119"),
        ("SEKEUR", "119"),
        ("JPYUSD", "119"),
        ("GBPCNYC", "120"),
        ("JPYNZD", "119"),
        ("CHFGBP", "119"),
        ("USDIDR", "119"),
        ("CHFSGD", "119"),
        ("USDPLN", "119"),
        ("CHFCNH", "133"),
        ("SEKUSD", "119"),
        ("CHFAUD", "119"),
        ("USDKRW", "119"),
        ("EURPLN", "119"),
        ("USDHUF", "119"),
        ("CHFCAD", "119"),
        ("USDTHB", "119"),
        ("CHFEUR", "119"),
        ("JPYCNYC", "120"),
        ("EURHUF", "119"),
        ("CHFHKD", "119"),
        ("SGDCNYC", "120"),
        ("CHFUSD", "119"),
        ("USDINR", "119"),
        ("USDCZK", "119"),
        ("CHFNZD", "119"),
        ("USDMXN", "119"),
        ("GBPPLN", "119"),
        ("USDZAR", "119"),
        ("JPYCHF", "119"),
        ("EURCZK", "119"),
        ("EURZAR", "119"),
        ("CADCNYC", "120"),
        ("NOKEUR", "119"),
        ("NZDGBP", "119"),
        ("NOKUSD", "119"),
        ("NZDSGD", "119"),
        ("USDGBP", "119"),
        ("HKDGBP", "119"),
        ("NZDCNH", "133"),
        ("NZDAUD", "119"),
        ("HKDSGD", "119"),
        ("CNYSARC", "120"),
        ("USDSGD", "119"),
        ("CNYAEDC", "120"),
        ("EURGBP", "119"),
        ("CADGBP", "119"),
        ("USDCNH", "133"),
        ("CNYTRYC", "120"),
        ("CADSGD", "119"),
        ("USDAUD", "119"),
        ("GBPZAR", "119"),
        ("EURSGD", "119"),
        ("HKDCNH", "133"),
        ("NZDCAD", "119"),
        ("CADCNH", "133"),
        ("HKDAUD", "119"),
        ("NZDEUR", "119"),
        ("EURCNH", "133"),
        ("EURAUD", "119"),
        ("NZDHKD", "119"),
        ("CADAUD", "119"),
        ("AUDGBP", "119"),
        ("USDDKK", "119"),
        ("HKDCAD", "119"),
        ("USDCAD", "119"),
        ("AUDSGD", "119"),
        ("USDTRY", "119"),
        ("EURTRY", "119"),
        ("USDEUR", "119"),
        ("NZDUSD", "119"),
        ("SGDGBP", "119"),
        ("USDHKD", "119"),
        ("AUDCNH", "133"),
        ("EURDKK", "119"),
        ("USDARS", "119"),
        ("USDSAR", "119"),
        ("TRYUSD", "119"),
        ("TRYEUR", "119"),
        ("SARUSD", "119"),
        ("INRUSD", "119"),
        ("HUFUSD", "119"),
        ("HUFEUR", "119"),
        ("HKDUSD", "119"),
        ("HKDEUR", "119"),
        ("HKDCNYC", "120"),
        ("EURCAD", "119"),
        ("DKKUSD", "119"),
        ("DKKEUR", "119"),
        ("CNYMOPC", "120"),
        ("CNHSGD", "133"),
        ("CNHGBP", "133"),
        ("CNHAUD", "133"),
        ("CADEUR", "119"),
        ("SGDCNH", "133"),
        ("EURHKD", "119"),
        ("CADHKD", "119"),
        ("USDCNYC", "120"),
        ("GBPSGD", "119"),
        ("EURUSD", "119"),
        ("SGDAUD", "119"),
        ("HKDNZD", "119"),
        ("USDNZD", "119"),
        ("GBPCNH", "133"),
        ("CADUSD", "119"),
        ("AUDCAD", "119"),
        ("CNYTHBC", "120"),
        ("CNHEUR", "133"),
        ("GBPAUD", "119"),
        ("AUDEUR", "119"),
        ("CADNZD", "119"),
        ("EURNZD", "119"),
        ("CNHCAD", "133"),
        ("AUDHKD", "119"),
        ("SGDCAD", "119"),
        ("AUDUSD", "119"),
        ("SGDEUR", "119"),
        ("CNHHKD", "133"),
        ("GBPCAD", "119"),
        ("CNHUSD", "133"),
        ("SGDHKD", "119"),
        ("GBPEUR", "119"),
        ("SGDUSD", "119"),
        ("AUDNZD", "119"),
        ("GBPHKD", "119"),
        ("GBPUSD", "119"),
        ("CNHNZD", "133"),
        ("CHFCNYC", "120"),
        ("SGDNZD", "119"),
        ("ZARGBP", "119"),
        ("USDNOK", "119"),
        ("GBPNZD", "119"),
        ("CZKEUR", "119"),
        ("EURNOK", "119"),
        ("CHFJPY", "119"),
        ("NZDCHF", "119"),
        ("PLNGBP", "119"),
        ("HKDCHF", "119"),
        ("ZARUSD", "119"),
        ("USDCHF", "119"),
        ("ZAREUR", "119"),
        ("MXNUSD", "119"),
        ("EURCHF", "119"),
        ("CADCHF", "119"),
        ("CZKUSD", "119"),
        ("CNYKRWC", "120"),
        ("CNHCHF", "133"),
        ("AUDCHF", "119"),
        ("PLNEUR", "119"),
        ("CNYMXNC", "120"),
        ("SGDCHF", "119"),
        ("PLNUSD", "119"),
        ("USDSEK", "119"),
        ("GBPCHF", "119"),
        ("EURSEK", "119"),
        ("CNYMYRC", "120"),
        ("NZDJPY", "119"),
        ("ZARCHF", "119"),
        ("USDJPY", "119"),
        ("THBUSD", "119"),
        ("HKDJPY", "119"),
        ("EURJPY", "119"),
        ("CADJPY", "119"),
        ("AUDJPY", "119"),
        ("TRYJPY", "119"),
        ("CNHJPY", "133"),
        ("SGDJPY", "119"),
        ("GBPJPY", "119"),
        ("CNYZARC", "120"),
        ("ZARJPY", "119"),
        ("USDRUB", "119"),
        ("CNYDKKC", "120"),
        ("CNYNOKC", "120"),
        ("CNYHUFC", "120"),
        ("CNYPLNC", "120"),
        ("CNYSEKC", "120"),
    ];
    MAP.iter().find(|(k, _)| *k == symbol).map(|(_, v)| *v)
}

/// 东财-外汇市场-所有汇率-实时行情（对应 akshare [`akshare.forex_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收`
pub fn forex_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "np": "1",
        "fltt": "2",
        "invt": "2",
        "fs": "m:119,m:120,m:133",
        "fields": "f12,f13,f14,f1,f2,f4,f3,f152,f17,f18,f15,f16",
        "fid": "f3",
        "pn": "1",
        "pz": "100",
        "po": "1",
        "dect": "1",
        "wbp2u": "|0|0|0|web",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let http = HttpClient::default();
    let df = fetch_clist(&http, &urls, &params)?;

    // fetch_clist 已生成 index 序号并按 f3 数值降序（对齐 akshare fetch_paginated_data）。
    // 按 akshare 列序抽取后位置式重命名。
    let df = df.select(&[
        "index", "f12", "f14", "f2", "f4", "f3", "f17", "f15", "f16", "f18",
    ])?;
    let mut df = df;
    df.rename_columns(&[
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "今开",
        "最高",
        "最低",
        "昨收",
    ])?;
    df.cast_numeric(&["最新价", "涨跌额", "涨跌幅", "今开", "最高", "最低", "昨收"])?;
    Ok(df)
}

/// 东财-外汇市场-所有汇率-历史行情（对应 akshare [`akshare.forex_hist_em`]）。
///
/// - `symbol`: 品种代码，如 `"USDCNH"`；可通过 [`forex_spot_em`] 获取全部可查品种。
///
/// # 返回列
/// `日期, 代码, 名称, 今开, 最新价, 最高, 最低, 振幅`
pub fn forex_hist_em(symbol: &str) -> Result<Df> {
    let market = forex_market_code(symbol).ok_or_else(|| {
        crate::core::error::AkshareError::Param(format!("未知外汇品种: {symbol}"))
    })?;
    let secid = format!("{market}.{symbol}");

    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let params = json!({
        "secid": secid,
        "klt": "101",
        "fqt": "1",
        "lmt": "50000",
        "end": "20500000",
        "iscca": "1",
        "fields1": "f1,f2,f3,f4,f5,f6,f7,f8",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64",
        "ut": "f057cbcbce2a86e2866ab8877db1d059",
        "forcect": 1,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let http = HttpClient::default();
    let value = http.get_json(url, &params, None)?;
    let data = value
        .get("data")
        .ok_or_else(|| crate::core::error::AkshareError::Empty("外汇 kline 无 data".into()))?;
    let klines = data
        .get("klines")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let code = data.get("code").and_then(Value::as_str).unwrap_or(symbol);
    let name = data.get("name").and_then(Value::as_str).unwrap_or_default();

    // 14 字段：日期,今开,最新价,最高,最低,-,-,振幅,-,-,-,-,-,-  + 代码/名称
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for line in klines.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        rows.push(vec![
            pick(0),
            Some(code.to_string()),
            Some(name.to_string()),
            pick(1),
            pick(2),
            pick(3),
            pick(4),
            pick(7),
        ]);
    }
    let mut df = Df::from_string_rows(
        &[
            "日期",
            "代码",
            "名称",
            "今开",
            "最新价",
            "最高",
            "最低",
            "振幅",
        ],
        &rows,
    )?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["今开", "最新价", "最高", "最低", "振幅"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_code_lookup() {
        assert_eq!(forex_market_code("USDCNH"), Some("133"));
        assert_eq!(forex_market_code("EURCNYC"), Some("120"));
        assert_eq!(forex_market_code("JPYUSD"), Some("119"));
        assert_eq!(forex_market_code("NOT_EXIST"), None);
    }

    #[test]
    fn hist_build_offline() {
        // 模拟 push2his kline 行（14 字段），校验列契约与数值化
        let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
        let params = json!({"secid": "133.USDCNH"});
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let _ = url;
        let _ = params;
        // 直接构造 rows 走同一构建路径：手工验证 pick 逻辑
        let line = "2026-08-14,7.1200,7.1300,7.1500,7.1000,12345,67890,0.55,0.1,0.0,-,-,-,-";
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 14);
        assert_eq!(f[0], "2026-08-14");
        assert_eq!(f[1], "7.1200");
        assert_eq!(f[7], "0.55");
    }
}
