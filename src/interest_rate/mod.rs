//! 利率数据（对应 akshare `interest_rate/` 目录）。
//!
//! 已实现：
//! - 银行间拆借利率 [`rate_interbank`]（对应 akshare `interest_rate/interbank_rate_em.py`，
//!   东财 datacenter `RPT_IMP_INTRESTRATEN`）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::sources::eastmoney::finalize_report;
use crate::stock_feature::{datacenter, report_extra};

/// `rate_interbank` 市场映射（对应 akshare `market_map`）。
const MARKET_MAP: &[(&str, &str)] = &[
    ("上海银行同业拆借市场", "001"),
    ("中国银行同业拆借市场", "002"),
    ("伦敦银行同业拆借市场", "003"),
    ("欧洲银行同业拆借市场", "004"),
    ("香港银行同业拆借市场", "005"),
    ("新加坡银行同业拆借市场", "006"),
];

/// `rate_interbank` 品种（货币）映射（对应 akshare `symbol_map`）。
const SYMBOL_MAP: &[(&str, &str)] = &[
    ("Shibor人民币", "CNY"),
    ("Chibor人民币", "CNY"),
    ("Libor英镑", "GBP"),
    ("Libor欧元", "EUR"),
    ("Libor美元", "USD"),
    ("Libor日元", "JPY"),
    ("Euribor欧元", "EUR"),
    ("Hibor美元", "USD"),
    ("Hibor人民币", "CNH"),
    ("Hibor港币", "HKD"),
    ("Sibor星元", "SGD"),
    ("Sibor美元", "USD"),
];

/// `rate_interbank` 期限指标映射（对应 akshare `indicator_map`）。
const INDICATOR_MAP: &[(&str, &str)] = &[
    ("隔夜", "001"),
    ("1周", "101"),
    ("2周", "102"),
    ("3周", "103"),
    ("1月", "201"),
    ("2月", "202"),
    ("3月", "203"),
    ("4月", "204"),
    ("5月", "205"),
    ("6月", "206"),
    ("7月", "207"),
    ("8月", "208"),
    ("9月", "209"),
    ("10月", "210"),
    ("11月", "211"),
    ("1年", "301"),
];

/// 银行间拆借利率（对应 akshare [`akshare.rate_interbank`]）。
///
/// `market`：市场（默认 `"上海银行同业拆借市场"`）；`symbol`：品种/货币
/// （默认 `"Shibor人民币"`）；`indicator`：期限（默认 `"隔夜"`）。
/// 报表 `RPT_IMP_INTRESTRATEN`，按 `MARKET_CODE/CURRENCY_CODE/INDICATOR_ID` 过滤，按日期降序。
///
/// # 返回列
/// `报告日, 利率, 涨跌`（`报告日` 归一化为 `YYYY-MM-DD`；`利率`/`涨跌` 转 float64）。
pub fn rate_interbank(market: &str, symbol: &str, indicator: &str) -> Result<Df> {
    let market_code = MARKET_MAP
        .iter()
        .find(|(m, _)| m == &market)
        .map(|(_, c)| *c)
        .ok_or_else(|| {
            AkshareError::Param(format!(
                "未知 market: {market}（可选：{}）",
                MARKET_MAP
                    .iter()
                    .map(|(m, _)| *m)
                    .collect::<Vec<_>>()
                    .join("/")
            ))
        })?;
    let currency_code = SYMBOL_MAP
        .iter()
        .find(|(s, _)| s == &symbol)
        .map(|(_, c)| *c)
        .ok_or_else(|| {
            AkshareError::Param(format!(
                "未知 symbol: {symbol}（可选：{}）",
                SYMBOL_MAP
                    .iter()
                    .map(|(s, _)| *s)
                    .collect::<Vec<_>>()
                    .join("/")
            ))
        })?;
    let indicator_code = INDICATOR_MAP
        .iter()
        .find(|(i, _)| i == &indicator)
        .map(|(_, c)| *c)
        .ok_or_else(|| {
            AkshareError::Param(format!(
                "未知 indicator: {indicator}（可选：{}）",
                INDICATOR_MAP
                    .iter()
                    .map(|(i, _)| *i)
                    .collect::<Vec<_>>()
                    .join("/")
            ))
        })?;

    let filter = format!(
        r#"(MARKET_CODE="{market_code}")(CURRENCY_CODE="{currency_code}")(INDICATOR_ID="{indicator_code}")"#
    );
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), Some(""), None, None);
    let columns = "REPORT_DATE,REPORT_PERIOD,IR_RATE,CHANGE_RATE,INDICATOR_ID,LATEST_RECORD,MARKET,MARKET_CODE,CURRENCY,CURRENCY_CODE";
    let rows = datacenter("RPT_IMP_INTRESTRATEN", columns, &extra, "500")?;

    let rename: [(&str, &str); 3] = [
        ("REPORT_DATE", "报告日"),
        ("IR_RATE", "利率"),
        ("CHANGE_RATE", "涨跌"),
    ];
    let select: [&str; 3] = ["报告日", "利率", "涨跌"];
    let numeric: [&str; 2] = ["利率", "涨跌"];
    let mut df = finalize_report(&rows, &rename, &select, &numeric, None)?;
    df.cast_date(&["报告日"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rate_interbank_offline_contract() {
        // 复刻 rate_interbank 的 finalize 契约：报告日 + 利率 + 涨跌
        let rows = vec![json!({
            "REPORT_DATE": "2024-03-15 00:00:00",
            "IR_RATE": 2.31,
            "CHANGE_RATE": -0.05,
        })];
        let rename: [(&str, &str); 3] = [
            ("REPORT_DATE", "报告日"),
            ("IR_RATE", "利率"),
            ("CHANGE_RATE", "涨跌"),
        ];
        let select: [&str; 3] = ["报告日", "利率", "涨跌"];
        let numeric: [&str; 2] = ["利率", "涨跌"];
        let mut df = finalize_report(&rows, &rename, &select, &numeric, None).unwrap();
        df.cast_date(&["报告日"]).unwrap();

        assert_eq!(col_names(&df), vec!["报告日", "利率", "涨跌"]);
        let d = df.inner().column("报告日").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2024-03-15"));
        let r = df
            .inner()
            .column("利率")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!((r - 2.31).abs() < 1e-6);
        let c = df
            .inner()
            .column("涨跌")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!((c - (-0.05)).abs() < 1e-6);
    }

    fn col_names(df: &Df) -> Vec<String> {
        df.export_parity(0)["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    }
}
