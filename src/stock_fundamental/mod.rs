//! 限售股解禁（对应 akshare `stock_restricted_release_*_em`）。
//!
//! 全部走东财 `datacenter-web` 的 `RPT_*` 报表（`RPT_LIFTDAY_STA` /
//! `RPT_LIFT_STAGE` / `RPT_LIFT_GD`），复用 `stock_feature` 的
//! `datacenter` / `report_extra` / `fmt_ymd` 与 `sources::eastmoney::finalize_report`
//! 工具，列名与 akshare 逐字对齐。数值列（数量/市值类）服务端以「股」为单位返回，
//! 与 akshare 一致地统一除以 10000 转为「万股/万元」；日期列截断为 `YYYY-MM-DD`。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::sources::eastmoney::finalize_report;
use crate::stock_feature::{datacenter, fmt_ymd, report_extra};

// ============ 1. stock_restricted_release_summary_em ============

const SUMMARY_RENAME: [(&str, &str); 7] = [
    ("FREE_DATE", "解禁时间"),
    ("LIFT_ORG_NUM", "当日解禁股票家数"),
    ("LIFT_NUM", "解禁数量"),
    ("MARKET_CAP", "实际解禁数量"),
    ("INDEX_PRICE", "实际解禁市值"),
    ("CHANGE_RATE", "沪深300指数"),
    ("PLAN_LIFT_NUM", "沪深300指数涨跌幅"),
];
const SUMMARY_SELECT: [&str; 7] = [
    "解禁时间",
    "当日解禁股票家数",
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "沪深300指数",
    "沪深300指数涨跌幅",
];
const SUMMARY_NUMERIC: [&str; 6] = [
    "当日解禁股票家数",
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "沪深300指数",
    "沪深300指数涨跌幅",
];
const SUMMARY_DATE: [&str; 1] = ["解禁时间"];

/// 限售股解禁汇总（对应 akshare [`akshare.stock_restricted_release_summary_em`]）。
///
/// `symbol`：板块（默认 `"全部股票"`，可选 沪市A股/科创板/深市A股/创业板/京市A股）；
/// `start_date` / `end_date`：区间 `YYYYMMDD`（默认 `"20221101"` / `"20221209"`）。
/// 报表 `RPT_LIFTDAY_STA`，按解禁日升序。
///
/// # 返回列
/// `序号, 解禁时间, 当日解禁股票家数, 解禁数量, 实际解禁数量, 实际解禁市值,
/// 沪深300指数, 沪深300指数涨跌幅`
pub fn stock_restricted_release_summary_em(
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    const SYMBOL_MAP: &[(&str, &str)] = &[
        ("全部股票", "000300"),
        ("沪市A股", "000001"),
        ("科创板", "000688"),
        ("深市A股", "399001"),
        ("创业板", "399001"),
        ("京市A股", "999999"),
    ];
    let code = SYMBOL_MAP
        .iter()
        .find(|(k, _)| *k == symbol)
        .map(|(_, v)| *v)
        .ok_or_else(|| AkshareError::Param(format!("未知板块 symbol: {symbol}")))?;
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!(r#"(INDEX_CODE="{code}")(FREE_DATE>='{sd}')(FREE_DATE<='{ed}')"#);
    let extra = report_extra("FREE_DATE", "1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_LIFTDAY_STA", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &SUMMARY_RENAME,
        &SUMMARY_SELECT,
        &SUMMARY_NUMERIC,
        Some("序号"),
    )?;
    df.scale("解禁数量", 10000.0)?
        .scale("实际解禁数量", 10000.0)?
        .scale("实际解禁市值", 10000.0)?
        .cast_date(&SUMMARY_DATE)?;
    Ok(df)
}

// ============ 2. stock_restricted_release_detail_em ============

const DETAIL_RENAME: [(&str, &str); 11] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("FREE_DATE", "解禁时间"),
    ("CURRENT_FREE_SHARES", "实际解禁数量"),
    ("ABLE_FREE_SHARES", "解禁数量"),
    ("LIFT_MARKET_CAP", "实际解禁市值"),
    ("FREE_RATIO", "占解禁前流通市值比例"),
    ("NEW", "解禁前一交易日收盘价"),
    ("B20_ADJCHRATE", "解禁前20日涨跌幅"),
    ("A20_ADJCHRATE", "解禁后20日涨跌幅"),
    ("FREE_SHARES_TYPE", "限售股类型"),
];
const DETAIL_SELECT: [&str; 11] = [
    "股票代码",
    "股票简称",
    "解禁时间",
    "限售股类型",
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "占解禁前流通市值比例",
    "解禁前一交易日收盘价",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
];
const DETAIL_NUMERIC: [&str; 7] = [
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "占解禁前流通市值比例",
    "解禁前一交易日收盘价",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
];
const DETAIL_DATE: [&str; 1] = ["解禁时间"];
const DETAIL_COLUMNS: &str = "SECURITY_CODE,SECURITY_NAME_ABBR,FREE_DATE,CURRENT_FREE_SHARES,ABLE_FREE_SHARES,LIFT_MARKET_CAP,FREE_RATIO,NEW,B20_ADJCHRATE,A20_ADJCHRATE,FREE_SHARES_TYPE,TOTAL_RATIO,NON_FREE_SHARES,BATCH_HOLDER_NUM";

/// 限售股解禁详情（对应 akshare [`akshare.stock_restricted_release_detail_em`]）。
///
/// `start_date` / `end_date`：区间 `YYYYMMDD`（默认 `"20221202"` / `"20241202"`）。
/// 报表 `RPT_LIFT_STAGE`，按解禁日、实际解禁数量降序。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 解禁时间, 限售股类型, 解禁数量, 实际解禁数量,
/// 实际解禁市值, 占解禁前流通市值比例, 解禁前一交易日收盘价, 解禁前20日涨跌幅,
/// 解禁后20日涨跌幅`
pub fn stock_restricted_release_detail_em(start_date: &str, end_date: &str) -> Result<Df> {
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!(r#"(FREE_DATE>='{sd}')(FREE_DATE<='{ed}')"#);
    let extra = report_extra(
        "FREE_DATE,CURRENT_FREE_SHARES",
        "1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_LIFT_STAGE", DETAIL_COLUMNS, &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &DETAIL_RENAME,
        &DETAIL_SELECT,
        &DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.scale("解禁数量", 10000.0)?
        .scale("实际解禁数量", 10000.0)?
        .scale("实际解禁市值", 10000.0)?
        .cast_date(&DETAIL_DATE)?;
    Ok(df)
}

// ============ 3. stock_restricted_release_queue_em ============

const QUEUE_RENAME: [(&str, &str); 12] = [
    ("FREE_DATE", "解禁时间"),
    ("CURRENT_FREE_SHARES", "实际解禁数量"),
    ("ABLE_FREE_SHARES", "解禁数量"),
    ("LIFT_MARKET_CAP", "实际解禁数量市值"),
    ("FREE_RATIO", "占流通市值比例"),
    ("NEW", "解禁前一交易日收盘价"),
    ("B20_ADJCHRATE", "解禁前20日涨跌幅"),
    ("A20_ADJCHRATE", "解禁后20日涨跌幅"),
    ("FREE_SHARES_TYPE", "限售股类型"),
    ("TOTAL_RATIO", "占总市值比例"),
    ("NON_FREE_SHARES", "未解禁数量"),
    ("BATCH_HOLDER_NUM", "解禁股东数"),
];
const QUEUE_SELECT: [&str; 12] = [
    "解禁时间",
    "解禁股东数",
    "解禁数量",
    "实际解禁数量",
    "未解禁数量",
    "实际解禁数量市值",
    "占总市值比例",
    "占流通市值比例",
    "解禁前一交易日收盘价",
    "限售股类型",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
];
const QUEUE_NUMERIC: [&str; 10] = [
    "解禁数量",
    "实际解禁数量",
    "未解禁数量",
    "实际解禁数量市值",
    "占总市值比例",
    "占流通市值比例",
    "解禁前一交易日收盘价",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
    "解禁股东数",
];
const QUEUE_DATE: [&str; 1] = ["解禁时间"];

/// 个股限售股解禁批次（对应 akshare [`akshare.stock_restricted_release_queue_em`]）。
///
/// `symbol`：股票代码（默认 `"600000"`）。同一张 `RPT_LIFT_STAGE` 报表，按解禁日降序。
///
/// # 返回列
/// `序号, 解禁时间, 解禁股东数, 解禁数量, 实际解禁数量, 未解禁数量, 实际解禁数量市值,
/// 占总市值比例, 占流通市值比例, 解禁前一交易日收盘价, 限售股类型, 解禁前20日涨跌幅,
/// 解禁后20日涨跌幅`
pub fn stock_restricted_release_queue_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let extra = report_extra("FREE_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_LIFT_STAGE", DETAIL_COLUMNS, &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &QUEUE_RENAME,
        &QUEUE_SELECT,
        &QUEUE_NUMERIC,
        Some("序号"),
    )?;
    df.scale("解禁数量", 10000.0)?
        .scale("实际解禁数量", 10000.0)?
        .scale("未解禁数量", 10000.0)?
        .scale("实际解禁数量市值", 10000.0)?
        .cast_date(&QUEUE_DATE)?;
    Ok(df)
}

// ============ 4. stock_restricted_release_stockholder_em ============

const STOCKHOLDER_RENAME: [(&str, &str); 8] = [
    ("LIMITED_HOLDER_NAME", "股东名称"),
    ("ADD_LISTING_SHARES", "解禁数量"),
    ("ACTUAL_LISTED_SHARES", "实际解禁数量"),
    ("ADD_LISTING_CAP", "解禁市值"),
    ("LOCK_MONTH", "锁定期"),
    ("RESIDUAL_LIMITED_SHARES", "剩余未解禁数量"),
    ("FREE_SHARES_TYPE", "限售股类型"),
    ("PLAN_FEATURE", "进度"),
];
const STOCKHOLDER_SELECT: [&str; 8] = [
    "股东名称",
    "解禁数量",
    "实际解禁数量",
    "解禁市值",
    "锁定期",
    "剩余未解禁数量",
    "限售股类型",
    "进度",
];
const STOCKHOLDER_NUMERIC: [&str; 5] = [
    "解禁数量",
    "实际解禁数量",
    "解禁市值",
    "锁定期",
    "剩余未解禁数量",
];

/// 限售股解禁股东明细（对应 akshare [`akshare.stock_restricted_release_stockholder_em`]）。
///
/// `symbol`：股票代码（默认 `"600000"`）；`date`：解禁日 `YYYYMMDD`（默认 `"20200904"`）。
/// 报表 `RPT_LIFT_GD`，按解禁数量降序。
///
/// # 返回列
/// `序号, 股东名称, 解禁数量, 实际解禁数量, 解禁市值, 锁定期, 剩余未解禁数量,
/// 限售股类型, 进度`
pub fn stock_restricted_release_stockholder_em(symbol: &str, date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(r#"(SECURITY_CODE="{symbol}")(FREE_DATE='{d}')"#);
    let extra = report_extra("ADD_LISTING_SHARES", "-1", Some(&filter), None, None, None);
    let rows = datacenter(
        "RPT_LIFT_GD",
        "LIMITED_HOLDER_NAME,ADD_LISTING_SHARES,ACTUAL_LISTED_SHARES,ADD_LISTING_CAP,LOCK_MONTH,RESIDUAL_LIMITED_SHARES,FREE_SHARES_TYPE,PLAN_FEATURE",
        &extra,
        "500",
    )?;
    let df = finalize_report(
        &rows,
        &STOCKHOLDER_RENAME,
        &STOCKHOLDER_SELECT,
        &STOCKHOLDER_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 抽取列名（与 parity export_parity 同口径），用于断言列契约顺序。
    fn col_names(df: &Df) -> Vec<String> {
        df.export_parity(0)["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn summary_offline_contract() {
        let rows = vec![json!({
            "FREE_DATE": "2022-11-01 00:00:00",
            "LIFT_ORG_NUM": 3,
            "LIFT_NUM": 123456789,
            "MARKET_CAP": 234567890,
            "INDEX_PRICE": 3500.12,
            "CHANGE_RATE": 1.23,
            "PLAN_LIFT_NUM": -0.45,
        })];
        let mut df = finalize_report(
            &rows,
            &SUMMARY_RENAME,
            &SUMMARY_SELECT,
            &SUMMARY_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.scale("解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量", 10000.0).unwrap();
        df.scale("实际解禁市值", 10000.0).unwrap();
        df.cast_date(&SUMMARY_DATE).unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "解禁时间",
                "当日解禁股票家数",
                "解禁数量",
                "实际解禁数量",
                "实际解禁市值",
                "沪深300指数",
                "沪深300指数涨跌幅",
            ]
        );
        // 序号 1 起始
        let idx = df.inner().column("序号").unwrap().f64().unwrap().get(0);
        assert_eq!(idx, Some(1.0));
        // 日期截断为 YYYY-MM-DD
        let d = df.inner().column("解禁时间").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2022-11-01"));
        // 数量列 ÷10000
        let n = df
            .inner()
            .column("解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(n, 12345.6789));
        let m = df
            .inner()
            .column("实际解禁市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(m, 0.350012));
        // 指数类列不缩放
        let i = df
            .inner()
            .column("沪深300指数")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(i, 1.23));
    }

    #[test]
    fn detail_offline_contract() {
        let rows = vec![json!({
            "SECURITY_CODE": "600000",
            "SECURITY_NAME_ABBR": "浦发银行",
            "FREE_DATE": "2022-12-02 00:00:00",
            "CURRENT_FREE_SHARES": 100000000,
            "ABLE_FREE_SHARES": 200000000,
            "LIFT_MARKET_CAP": 300000000,
            "FREE_RATIO": 12.5,
            "NEW": 7.5,
            "B20_ADJCHRATE": 3.2,
            "A20_ADJCHRATE": -2.1,
            "FREE_SHARES_TYPE": "首发原股东限售股份",
        })];
        let mut df = finalize_report(
            &rows,
            &DETAIL_RENAME,
            &DETAIL_SELECT,
            &DETAIL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.scale("解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量", 10000.0).unwrap();
        df.scale("实际解禁市值", 10000.0).unwrap();
        df.cast_date(&DETAIL_DATE).unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "股票代码",
                "股票简称",
                "解禁时间",
                "限售股类型",
                "解禁数量",
                "实际解禁数量",
                "实际解禁市值",
                "占解禁前流通市值比例",
                "解禁前一交易日收盘价",
                "解禁前20日涨跌幅",
                "解禁后20日涨跌幅",
            ]
        );
        let d = df.inner().column("解禁时间").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2022-12-02"));
        let qty = df
            .inner()
            .column("解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(qty, 20000.0));
        let actual = df
            .inner()
            .column("实际解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(actual, 10000.0));
        let cap = df
            .inner()
            .column("实际解禁市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(cap, 30000.0));
    }

    #[test]
    fn queue_offline_contract() {
        let rows = vec![json!({
            "FREE_DATE": "2022-12-02 00:00:00",
            "CURRENT_FREE_SHARES": 100000000,
            "ABLE_FREE_SHARES": 200000000,
            "LIFT_MARKET_CAP": 300000000,
            "FREE_RATIO": 12.5,
            "NEW": 7.5,
            "B20_ADJCHRATE": 3.2,
            "A20_ADJCHRATE": -2.1,
            "FREE_SHARES_TYPE": "首发原股东限售股份",
            "TOTAL_RATIO": 5.5,
            "NON_FREE_SHARES": 400000000,
            "BATCH_HOLDER_NUM": 8,
        })];
        let mut df = finalize_report(
            &rows,
            &QUEUE_RENAME,
            &QUEUE_SELECT,
            &QUEUE_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.scale("解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量", 10000.0).unwrap();
        df.scale("未解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量市值", 10000.0).unwrap();
        df.cast_date(&QUEUE_DATE).unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "解禁时间",
                "解禁股东数",
                "解禁数量",
                "实际解禁数量",
                "未解禁数量",
                "实际解禁数量市值",
                "占总市值比例",
                "占流通市值比例",
                "解禁前一交易日收盘价",
                "限售股类型",
                "解禁前20日涨跌幅",
                "解禁后20日涨跌幅",
            ]
        );
        let d = df.inner().column("解禁时间").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2022-12-02"));
        let holders = df
            .inner()
            .column("解禁股东数")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(holders, 8.0));
        let total = df
            .inner()
            .column("实际解禁数量市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(total, 30000.0));
        let residual = df
            .inner()
            .column("未解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(residual, 40000.0));
    }

    #[test]
    fn stockholder_offline_contract() {
        let rows = vec![json!({
            "LIMITED_HOLDER_NAME": "张三",
            "ADD_LISTING_SHARES": 100000,
            "ACTUAL_LISTED_SHARES": 90000,
            "ADD_LISTING_CAP": 200000,
            "LOCK_MONTH": 12,
            "RESIDUAL_LIMITED_SHARES": 50000,
            "FREE_SHARES_TYPE": "首发原股东限售股份",
            "PLAN_FEATURE": "已实施",
        })];
        let df = finalize_report(
            &rows,
            &STOCKHOLDER_RENAME,
            &STOCKHOLDER_SELECT,
            &STOCKHOLDER_NUMERIC,
            Some("序号"),
        )
        .unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "股东名称",
                "解禁数量",
                "实际解禁数量",
                "解禁市值",
                "锁定期",
                "剩余未解禁数量",
                "限售股类型",
                "进度",
            ]
        );
        // 无日期列、无缩放：数值保持原值
        let qty = df
            .inner()
            .column("解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(qty, 100000.0));
        let cap = df
            .inner()
            .column("解禁市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(cap, 200000.0));
        let lock = df
            .inner()
            .column("锁定期")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(lock, 12.0));
    }
}
