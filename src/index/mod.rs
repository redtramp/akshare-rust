//! 指数数据接口。
//!
//! 首批实现（对应 akshare `index/index_zh_em.py`）：
//! - [`index_zh_a_hist`]：中国股票指数历史行情

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    fetch_clist, fetch_kline, fetch_kline_min, fetch_trends, kline_to_df, min_kline_to_df,
    push2_urls, KLINE_COLS,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// 指数代码 → 市场标识 映射缓存（对应 akshare `index_code_id_map_em` 的 lru_cache）。
static INDEX_CODE_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

/// 获取指数代码 → 市场标识 映射（对应 akshare `index_code_id_map_em()`）。
pub fn index_code_id_map_em() -> Result<&'static HashMap<String, String>> {
    if let Some(map) = INDEX_CODE_MAP.get() {
        return Ok(map);
    }
    {
        let urls = push2_urls("/api/qt/clist/get");
        let params = json!({
            "pn": "1",
            "pz": "100",
            "po": "1",
            "np": "1",
            "ut": "bd1d9ddb04089700cf9c27f6f7426281",
            "fltt": "2",
            "invt": "2",
            "fid": "f3",
            "fs": "b:MK0010,m:1+t:1,m:0 t:5,m:1+s:3,m:0+t:5,m:2",
            "fields": "f3,f12,f13",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let http = HttpClient::default();
        let df = fetch_clist(&http, &urls, &params)?;

        let mut map = HashMap::new();
        let inner = df.inner();
        if let (Ok(codes), Ok(markets)) = (inner.column("f12"), inner.column("f13")) {
            if let (Ok(codes), Ok(markets)) = (codes.str(), markets.str()) {
                for (c, m) in codes.iter().zip(markets.iter()) {
                    if let (Some(c), Some(m)) = (c, m) {
                        map.insert(c.to_string(), m.to_string());
                    }
                }
            }
        }
        let _ = INDEX_CODE_MAP.set(map);
    }
    INDEX_CODE_MAP
        .get()
        .ok_or_else(|| AkshareError::empty("指数映射初始化失败"))
}

/// 中国股票指数历史行情。
///
/// 对应 akshare [`akshare.index_zh_a_hist`]。
///
/// # 参数
/// - `symbol`: 指数代码，如 `"000001"`（上证指数）
/// - `period`: `daily`/`weekly`/`monthly`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `日期, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 振幅, 涨跌幅, 涨跌额, 换手率`
pub fn index_zh_a_hist(symbol: &str, period: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        _ => return Err(AkshareError::Param(format!("无效 period: {period}"))),
    };
    let http = HttpClient::default();

    // 尝试市场标识：优先查映射，回退 1/0/2/47（对应 akshare 的 fallback 链）
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(map) = index_code_id_map_em() {
        if let Some(m) = map.get(symbol) {
            candidates.push(m.clone());
        }
    }
    for fallback in ["1", "0", "2", "47"] {
        candidates.push(fallback.to_string());
    }

    let mut last_err: Option<AkshareError> = None;
    for market in candidates {
        let secid = format!("{market}.{symbol}");
        match fetch_kline(&http, &secid, klt, "0", start_date, end_date) {
            Ok(klines) if !klines.is_empty() => {
                return kline_to_df(&KLINE_COLS, &klines, None);
            }
            Ok(_) => {
                last_err = Some(AkshareError::empty(format!("{symbol} 无 K 线数据")));
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| AkshareError::empty(format!("{symbol} 无 K 线数据"))))
}

/// 指数分钟级行情（对应 akshare [`akshare.index_zh_a_hist_min_em`]）。
///
/// # 参数
/// - `symbol`: 指数代码，如 `"399006"`（创业板指）
/// - `period`: `"1"`（当日分时）或 `"5"`/`"15"`/`"30"`/`"60"`（分钟 K 线，恒前复权）
/// - `start_date`/`end_date`: `YYYY-MM-DD HH:MM:SS` 区间（含边界）
///
/// # 返回列
/// period=1: `时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 均价`；
/// 其余: `时间, 开盘, 收盘, 最高, 最低, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 换手率`
pub fn index_zh_a_hist_min_em(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    if period != "1" && !matches!(period, "5" | "15" | "30" | "60") {
        return Err(AkshareError::Param(format!("无效 period: {period}")));
    }
    let http = HttpClient::default();

    // secid 候选：优先查映射，回退 1/0/47（对应 akshare 的 fallback 链）
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(map) = index_code_id_map_em() {
        if let Some(m) = map.get(symbol) {
            candidates.push(m.clone());
        }
    }
    for fb in ["1", "0", "47"] {
        candidates.push(fb.to_string());
    }

    let mut last_err: Option<AkshareError> = None;
    for market in candidates {
        let secid = format!("{market}.{symbol}");
        let result = if period == "1" {
            let lines = match fetch_trends(&http, &secid, "5", "0") {
                Ok(l) => l,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            if lines.is_empty() {
                last_err = Some(AkshareError::empty(format!("{symbol} 无分时数据")));
                continue;
            }
            let cols = [
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "成交量",
                "成交额",
                "均价",
            ];
            min_kline_to_df(&lines, start_date, end_date, &cols, &cols, &cols[1..])
        } else {
            let lines = match fetch_kline_min(&http, &secid, period, "1") {
                Ok(l) => l,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            if lines.is_empty() {
                last_err = Some(AkshareError::empty(format!("{symbol} 无分钟数据")));
                continue;
            }
            let src = [
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "成交量",
                "成交额",
                "振幅",
                "涨跌幅",
                "涨跌额",
                "换手率",
            ];
            let out = [
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "涨跌幅",
                "涨跌额",
                "成交量",
                "成交额",
                "振幅",
                "换手率",
            ];
            min_kline_to_df(&lines, start_date, end_date, &src, &out, &out[1..])
        };
        match result {
            Ok(df) if df.height() > 0 => return Ok(df),
            Ok(_) => last_err = Some(AkshareError::empty(format!("{symbol} 无分钟数据"))),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| AkshareError::empty(format!("{symbol} 无分钟数据"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klt_mapping_rejects_bad_period() {
        let r = index_zh_a_hist("000001", "bad", "20240101", "20240131");
        assert!(matches!(r, Err(AkshareError::Param(_))));
    }
}
