//! 东方财富-基本面 F10：股本结构 / 商誉明细 / A·港·美 财务分析主要指标。
//!
//! 对应 akshare：
//! - `stock_fundamental.stock_gbjg_em`（`stock_zh_a_gbjg_em`）
//! - `stock_feature.stock_sy_em`（`stock_sy_em`）
//! - `stock_fundamental.stock_finance_sina.stock_financial_analysis_indicator_em`
//!   （`stock_financial_analysis_indicator_em`）
//! - `stock_fundamental.stock_finance_hk_em.stock_financial_hk_analysis_indicator_em`
//!   （`stock_financial_hk_analysis_indicator_em`）
//! - `stock_fundamental.stock_finance_us_em.stock_financial_us_analysis_indicator_em`
//!   （`stock_financial_us_analysis_indicator_em`）
//!
//! 数据源均为东财 datacenter：
//! - A 股股本结构 / 财务分析「按单季度」/ 港股 / 美股 走 `securities/api/data/v1/get`
//!   （[`fetch_securities_pages`]，按报表分别用 `source=HSF10/F10/SECURITIES`）；
//! - 财务分析「按报告期」走 `securities/api/data/get`（非 `/v1`，[`fetch_securities_data_get`]）；
//! - 个股商誉明细走 datacenter-web `api/data/v1/get`（`fetch_datacenter_pages`，带 `token`）。
//!
//! 列名/数值化与 akshare 逐字对齐：财务分析三市场返回原生英文键（identity rename）；
//! 股本结构/商誉明细按 akshare 硬编码重命名数组还原；日期列保持字符串（与 akshare 一致）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    fetch_datacenter_pages, fetch_securities_data_get, fetch_securities_pages, finalize_report,
};
use crate::stock_feature::{fmt_ymd, report_extra};
use serde_json::{json, Value};

// === 自动生成的列契约（来源：akshare 实测，tests/golden_probe/batch26_spec.json） ===
const GBJG_COLS: [&str; 9] = [
    "变更日期",
    "总股本",
    "流通受限股份",
    "其他内资持股(受限)",
    "境内法人持股(受限)",
    "境内自然人持股(受限)",
    "已流通股份",
    "已上市流通A股",
    "变动原因",
];
const GBJG_NUMERIC: [&str; 7] = [
    "总股本",
    "流通受限股份",
    "其他内资持股(受限)",
    "境内法人持股(受限)",
    "境内自然人持股(受限)",
    "已流通股份",
    "已上市流通A股",
];

const SY_COLS: [&str; 9] = [
    "股票代码",
    "股票简称",
    "商誉",
    "商誉占净资产比例",
    "净利润",
    "净利润同比",
    "上年商誉",
    "公告日期",
    "交易市场",
];
const SY_NUMERIC: [&str; 5] = [
    "商誉",
    "商誉占净资产比例",
    "净利润",
    "净利润同比",
    "上年商誉",
];

const FIN_DJ_COLS: [&str; 26] = [
    "SECUCODE",
    "SECURITY_CODE",
    "SECURITY_NAME_ABBR",
    "ORG_CODE",
    "REPORT_DATE",
    "SECURITY_TYPE_CODE",
    "EPSJB",
    "BPS",
    "PER_CAPITAL_RESERVE",
    "PER_UNASSIGN_PROFIT",
    "PER_NETCASH",
    "TOTALOPERATEREVE",
    "GROSS_PROFIT",
    "PARENTNETPROFIT",
    "DEDU_PARENT_PROFIT",
    "TOTALOPERATEREVETZ",
    "PARENTNETPROFITTZ",
    "DPNP_YOY_RATIO",
    "YYZSRGDHBZC",
    "NETPROFITRPHBZC",
    "KFJLRGDHBZC",
    "ROE_DILUTED",
    "JROA",
    "GROSS_PROFIT_RATIO",
    "NET_PROFIT_RATIO",
    "SEASON_LABEL",
];
const FIN_DJ_NUMERIC: [&str; 19] = [
    "EPSJB",
    "BPS",
    "PER_CAPITAL_RESERVE",
    "PER_UNASSIGN_PROFIT",
    "PER_NETCASH",
    "TOTALOPERATEREVE",
    "GROSS_PROFIT",
    "PARENTNETPROFIT",
    "DEDU_PARENT_PROFIT",
    "TOTALOPERATEREVETZ",
    "PARENTNETPROFITTZ",
    "DPNP_YOY_RATIO",
    "YYZSRGDHBZC",
    "NETPROFITRPHBZC",
    "KFJLRGDHBZC",
    "ROE_DILUTED",
    "JROA",
    "GROSS_PROFIT_RATIO",
    "NET_PROFIT_RATIO",
];

const FIN_BG_COLS: [&str; 141] = [
    "SECUCODE",
    "SECURITY_CODE",
    "SECURITY_NAME_ABBR",
    "ORG_CODE",
    "ORG_TYPE",
    "REPORT_DATE",
    "REPORT_TYPE",
    "REPORT_DATE_NAME",
    "SECURITY_TYPE_CODE",
    "NOTICE_DATE",
    "UPDATE_DATE",
    "CURRENCY",
    "EPSJB",
    "EPSKCJB",
    "EPSXS",
    "BPS",
    "MGZBGJ",
    "MGWFPLR",
    "MGJYXJJE",
    "TOTALOPERATEREVE",
    "MLR",
    "PARENTNETPROFIT",
    "KCFJCXSYJLR",
    "TOTALOPERATEREVETZ",
    "PARENTNETPROFITTZ",
    "KCFJCXSYJLRTZ",
    "YYZSRGDHBZC",
    "NETPROFITRPHBZC",
    "KFJLRGDHBZC",
    "ROEJQ",
    "ROEKCJQ",
    "ZZCJLL",
    "XSJLL",
    "XSMLL",
    "YSZKYYSR",
    "XSJXLYYSR",
    "JYXJLYYSR",
    "TAXRATE",
    "LD",
    "SD",
    "XJLLB",
    "ZCFZL",
    "QYCS",
    "CQBL",
    "ZZCZZTS",
    "CHZZTS",
    "YSZKZZTS",
    "TOAZZL",
    "CHZZL",
    "YSZKZZL",
    "TOTALDEPOSITS",
    "GROSSLOANS",
    "LTDRR",
    "NEWCAPITALADER",
    "HXYJBCZL",
    "NONPERLOAN",
    "BLDKBBL",
    "NZBJE",
    "TOTAL_ROI",
    "NET_ROI",
    "EARNED_PREMIUM",
    "COMPENSATE_EXPENSE",
    "SURRENDER_RATE_LIFE",
    "SOLVENCY_AR",
    "JZB",
    "JZC",
    "JZBJZC",
    "ZYGPGMJZC",
    "ZYGDSYLZQJZB",
    "YYFXZB",
    "JJYWFXZB",
    "ZQZYYWFXZB",
    "ZQCXYWFXZB",
    "RZRQYWFXZB",
    "EPSJBTZ",
    "BPSTZ",
    "MGZBGJTZ",
    "MGWFPLRTZ",
    "MGJYXJJETZ",
    "ROEJQTZ",
    "ZZCJLLTZ",
    "ZCFZLTZ",
    "REPORT_YEAR",
    "ROIC",
    "ROICTZ",
    "NBV_LIFE",
    "NBV_RATE",
    "NHJZ_CURRENT_AMT",
    "DJD_TOI_YOY",
    "DJD_DPNP_YOY",
    "DJD_DEDUCTDPNP_YOY",
    "DJD_TOI_QOQ",
    "DJD_DPNP_QOQ",
    "DJD_DEDUCTDPNP_QOQ",
    "XSMLL_TB",
    "PER_TOI",
    "PER_OI",
    "PER_EBIT",
    "STAFF_NUM",
    "AVG_TOI",
    "AVG_NET_PROFIT",
    "PREPAID_ACCOUNTS_RATIO",
    "ACCOUNTS_PAYABLE_TR",
    "FIXED_ASSET_TR",
    "CURRENT_ASSET_TR",
    "PREPAID_ACCOUNTS_TDAYS",
    "PAYABLE_TDAYS",
    "OPERATE_CYCLE",
    "GUARD_SPEED_RATIO",
    "CASH_RATIO",
    "INTEREST_COVERAGE_RATIO",
    "CA_TA",
    "NCA_TA",
    "LIQUIDATION_RATIO",
    "INTEREST_DEBT_RATIO",
    "FC_LIABILITIES",
    "FCFF_FORWARD",
    "FCFF_BACK",
    "SS_OI",
    "SS_TA",
    "NCO_OP",
    "NCO_NETPROFIT",
    "NCO_FIXED",
    "FIRST_ADEQUACY_RATIO",
    "NET_INTEREST_SPREAD",
    "NET_INTEREST_MARGIN",
    "LOAN_ADVANCES",
    "NON_PERFORMING_LOAN",
    "OVERDUE_LOANS",
    "LOAN_PROVISION_RATIO",
    "REVENUE_RATIO",
    "LIABILITY",
    "CAPITAL_PROVISIONS_SUM",
    "RISK_COVERAGE",
    "CAPITAL_LEVERAGE_RATIO",
    "LIQUIDITY_COVERAGE_RATIO",
    "NET_FUNDING_RATIO",
    "NET_CAPITAL_LIABILITIES",
    "NET_ASSETS_LIABILITIES",
    "PROPRIETARY_CAPITAL",
    "IS_BZ",
];
const FIN_BG_NUMERIC: [&str; 84] = [
    "EPSJB",
    "EPSKCJB",
    "EPSXS",
    "BPS",
    "MGZBGJ",
    "MGWFPLR",
    "MGJYXJJE",
    "TOTALOPERATEREVE",
    "MLR",
    "PARENTNETPROFIT",
    "KCFJCXSYJLR",
    "TOTALOPERATEREVETZ",
    "PARENTNETPROFITTZ",
    "KCFJCXSYJLRTZ",
    "YYZSRGDHBZC",
    "NETPROFITRPHBZC",
    "KFJLRGDHBZC",
    "ROEJQ",
    "ROEKCJQ",
    "ZZCJLL",
    "XSJLL",
    "XSMLL",
    "YSZKYYSR",
    "XSJXLYYSR",
    "JYXJLYYSR",
    "TAXRATE",
    "LD",
    "SD",
    "XJLLB",
    "ZCFZL",
    "QYCS",
    "CQBL",
    "ZZCZZTS",
    "CHZZTS",
    "YSZKZZTS",
    "TOAZZL",
    "CHZZL",
    "YSZKZZL",
    "EPSJBTZ",
    "BPSTZ",
    "MGZBGJTZ",
    "MGWFPLRTZ",
    "MGJYXJJETZ",
    "ROEJQTZ",
    "ZZCJLLTZ",
    "ZCFZLTZ",
    "ROIC",
    "ROICTZ",
    "DJD_TOI_YOY",
    "DJD_DPNP_YOY",
    "DJD_DEDUCTDPNP_YOY",
    "DJD_TOI_QOQ",
    "DJD_DPNP_QOQ",
    "DJD_DEDUCTDPNP_QOQ",
    "XSMLL_TB",
    "PER_TOI",
    "PER_OI",
    "PER_EBIT",
    "STAFF_NUM",
    "AVG_TOI",
    "AVG_NET_PROFIT",
    "PREPAID_ACCOUNTS_RATIO",
    "ACCOUNTS_PAYABLE_TR",
    "FIXED_ASSET_TR",
    "CURRENT_ASSET_TR",
    "PREPAID_ACCOUNTS_TDAYS",
    "PAYABLE_TDAYS",
    "OPERATE_CYCLE",
    "GUARD_SPEED_RATIO",
    "CASH_RATIO",
    "INTEREST_COVERAGE_RATIO",
    "CA_TA",
    "NCA_TA",
    "LIQUIDATION_RATIO",
    "INTEREST_DEBT_RATIO",
    "FC_LIABILITIES",
    "FCFF_FORWARD",
    "FCFF_BACK",
    "SS_OI",
    "SS_TA",
    "NCO_OP",
    "NCO_NETPROFIT",
    "NCO_FIXED",
    "LIABILITY",
];

const HK_COLS: [&str; 36] = [
    "SECUCODE",
    "SECURITY_CODE",
    "SECURITY_NAME_ABBR",
    "ORG_CODE",
    "REPORT_DATE",
    "DATE_TYPE_CODE",
    "PER_NETCASH_OPERATE",
    "PER_OI",
    "BPS",
    "BASIC_EPS",
    "DILUTED_EPS",
    "OPERATE_INCOME",
    "OPERATE_INCOME_YOY",
    "GROSS_PROFIT",
    "GROSS_PROFIT_YOY",
    "HOLDER_PROFIT",
    "HOLDER_PROFIT_YOY",
    "GROSS_PROFIT_RATIO",
    "EPS_TTM",
    "OPERATE_INCOME_QOQ",
    "NET_PROFIT_RATIO",
    "ROE_AVG",
    "GROSS_PROFIT_QOQ",
    "ROA",
    "HOLDER_PROFIT_QOQ",
    "ROE_YEARLY",
    "ROIC_YEARLY",
    "TAX_EBT",
    "OCF_SALES",
    "DEBT_ASSET_RATIO",
    "CURRENT_RATIO",
    "CURRENTDEBT_DEBT",
    "START_DATE",
    "FISCAL_YEAR",
    "CURRENCY",
    "IS_CNY_CODE",
];
const HK_NUMERIC: [&str; 27] = [
    "PER_NETCASH_OPERATE",
    "PER_OI",
    "BPS",
    "BASIC_EPS",
    "DILUTED_EPS",
    "OPERATE_INCOME",
    "OPERATE_INCOME_YOY",
    "GROSS_PROFIT",
    "GROSS_PROFIT_YOY",
    "HOLDER_PROFIT",
    "HOLDER_PROFIT_YOY",
    "GROSS_PROFIT_RATIO",
    "EPS_TTM",
    "OPERATE_INCOME_QOQ",
    "NET_PROFIT_RATIO",
    "ROE_AVG",
    "GROSS_PROFIT_QOQ",
    "ROA",
    "HOLDER_PROFIT_QOQ",
    "ROE_YEARLY",
    "ROIC_YEARLY",
    "TAX_EBT",
    "OCF_SALES",
    "DEBT_ASSET_RATIO",
    "CURRENT_RATIO",
    "CURRENTDEBT_DEBT",
    "IS_CNY_CODE",
];

const US_COLS: [&str; 49] = [
    "SECUCODE",
    "SECURITY_CODE",
    "SECURITY_NAME_ABBR",
    "ORG_CODE",
    "SECURITY_INNER_CODE",
    "ACCOUNTING_STANDARDS",
    "NOTICE_DATE",
    "START_DATE",
    "REPORT_DATE",
    "FINANCIAL_DATE",
    "STD_REPORT_DATE",
    "CURRENCY",
    "DATE_TYPE",
    "DATE_TYPE_CODE",
    "REPORT_TYPE",
    "REPORT_DATA_TYPE",
    "ORGTYPE",
    "OPERATE_INCOME",
    "OPERATE_INCOME_YOY",
    "GROSS_PROFIT",
    "GROSS_PROFIT_YOY",
    "PARENT_HOLDER_NETPROFIT",
    "PARENT_HOLDER_NETPROFIT_YOY",
    "BASIC_EPS",
    "DILUTED_EPS",
    "GROSS_PROFIT_RATIO",
    "NET_PROFIT_RATIO",
    "ACCOUNTS_RECE_TR",
    "INVENTORY_TR",
    "TOTAL_ASSETS_TR",
    "ACCOUNTS_RECE_TDAYS",
    "INVENTORY_TDAYS",
    "TOTAL_ASSETS_TDAYS",
    "ROE_AVG",
    "ROA",
    "CURRENT_RATIO",
    "SPEED_RATIO",
    "OCF_LIQDEBT",
    "DEBT_ASSET_RATIO",
    "EQUITY_RATIO",
    "BASIC_EPS_YOY",
    "GROSS_PROFIT_RATIO_YOY",
    "NET_PROFIT_RATIO_YOY",
    "ROE_AVG_YOY",
    "ROA_YOY",
    "DEBT_ASSET_RATIO_YOY",
    "CURRENT_RATIO_YOY",
    "SPEED_RATIO_YOY",
    "CURRENCY_ABBR",
];
const US_NUMERIC: [&str; 31] = [
    "OPERATE_INCOME",
    "OPERATE_INCOME_YOY",
    "GROSS_PROFIT",
    "GROSS_PROFIT_YOY",
    "PARENT_HOLDER_NETPROFIT",
    "PARENT_HOLDER_NETPROFIT_YOY",
    "BASIC_EPS",
    "DILUTED_EPS",
    "GROSS_PROFIT_RATIO",
    "NET_PROFIT_RATIO",
    "ACCOUNTS_RECE_TR",
    "INVENTORY_TR",
    "TOTAL_ASSETS_TR",
    "ACCOUNTS_RECE_TDAYS",
    "INVENTORY_TDAYS",
    "TOTAL_ASSETS_TDAYS",
    "ROE_AVG",
    "ROA",
    "CURRENT_RATIO",
    "SPEED_RATIO",
    "OCF_LIQDEBT",
    "DEBT_ASSET_RATIO",
    "EQUITY_RATIO",
    "BASIC_EPS_YOY",
    "GROSS_PROFIT_RATIO_YOY",
    "NET_PROFIT_RATIO_YOY",
    "ROE_AVG_YOY",
    "ROA_YOY",
    "DEBT_ASSET_RATIO_YOY",
    "CURRENT_RATIO_YOY",
    "SPEED_RATIO_YOY",
];
/// 原生英文键 → 同名列（财务分析主要指标三市场均为原生键，无需重命名）。
fn identity_rename<'a>(cols: &'a [&'a str]) -> Vec<(&'a str, &'a str)> {
    cols.iter().map(|c| (*c, *c)).collect()
}

/// A 股代码规范化（对应 akshare `_normalize_em_secu_code`）：
/// `603392`→`603392.SH`、`SH603392`→`603392.SH`、`603392.SH` 保持。
fn normalize_em_secu_code(symbol: &str) -> Result<String> {
    let s = symbol.trim().to_uppercase();
    let is6 = |r: &str| r.len() == 6 && r.chars().all(|c| c.is_ascii_digit());
    // 6位.市场 形式（SH/SZ/BJ）
    if let Some(rest) = s.strip_suffix(".SH").filter(|r| is6(r)) {
        return Ok(format!("{rest}.SH"));
    }
    if let Some(rest) = s.strip_suffix(".SZ").filter(|r| is6(r)) {
        return Ok(format!("{rest}.SZ"));
    }
    if let Some(rest) = s.strip_suffix(".BJ").filter(|r| is6(r)) {
        return Ok(format!("{rest}.BJ"));
    }
    // 市场6位 形式
    if let Some(rest) = s.strip_prefix("SH").filter(|r| is6(r)) {
        return Ok(format!("{rest}.SH"));
    }
    if let Some(rest) = s.strip_prefix("SZ").filter(|r| is6(r)) {
        return Ok(format!("{rest}.SZ"));
    }
    if let Some(rest) = s.strip_prefix("BJ").filter(|r| is6(r)) {
        return Ok(format!("{rest}.BJ"));
    }
    // 纯 6 位，按首位推断市场
    if is6(&s) {
        let market = match s.chars().next().unwrap() {
            '4' | '8' => "BJ",
            '5' | '6' | '9' => "SH",
            _ => "SZ",
        };
        return Ok(format!("{s}.{market}"));
    }
    Err(AkshareError::Param(format!("无效股票代码: {symbol}")))
}

/// 个股商誉明细 `交易市场` 枚举映射（对应 akshare `.map({...})`）。
fn map_trade_board(v: &str) -> &str {
    match v {
        "shzb" => "沪市主板",
        "kcb" => "科创板",
        "szzb" => "深市主板",
        "cyb" => "创业板",
        other => other,
    }
}

// === 股本结构（stock_zh_a_gbjg_em）===
const GBJG_RENAME: [(&str, &str); 9] = [
    ("END_DATE", "变更日期"),
    ("TOTAL_SHARES", "总股本"),
    ("LISTED_A_SHARES", "已上市流通A股"),
    ("FREE_SHARES", "已流通股份"),
    ("CHANGE_REASON", "变动原因"),
    ("LIMITED_A_SHARES", "流通受限股份"),
    ("LIMITED_OTHARS", "其他内资持股(受限)"),
    ("LIMITED_DOMESTIC_NOSTATE", "境内法人持股(受限)"),
    ("LIMITED_DOMESTIC_NATURAL", "境内自然人持股(受限)"),
];
const GBJG_DATE: [&str; 1] = ["变更日期"];

// === 个股商誉明细（stock_sy_em）===
const SY_RENAME: [(&str, &str); 9] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("TRADE_BOARD", "交易市场"),
    ("GOODWILL", "商誉"),
    ("SUMSHEQUITY_RATIO", "商誉占净资产比例"),
    ("PARENTNETPROFIT", "净利润"),
    ("PNP_YOY_RATIO", "净利润同比"),
    ("GOODWILL_PRE", "上年商誉"),
    ("NOTICE_DATE", "公告日期"),
];
const SY_TOKEN: &str = "894050c76af8597a853f5b408b759f5d";

/// 东方财富-A股数据-股本结构（对应 akshare [`akshare.stock_zh_a_gbjg_em`]）。
///
/// `symbol`：股票代码（支持 `603392` / `SH603392` / `603392.SH`，自动规范化）。
///
/// # 返回列
/// `变更日期, 总股本, 流通受限股份, 其他内资持股(受限), 境内法人持股(受限),
/// 境内自然人持股(受限), 已流通股份, 已上市流通A股, 变动原因`
pub fn stock_zh_a_gbjg_em(symbol: &str) -> Result<Df> {
    let sym = normalize_em_secu_code(symbol)?;
    let http = HttpClient::default();
    let filter = format!("(SECUCODE=\"{sym}\")");
    let mut extra = report_extra("END_DATE", "-1", Some(&filter), None, None, None);
    extra.insert("source".into(), json!("HSF10"));
    extra.insert("client".into(), json!("PC"));
    let rows = fetch_securities_pages(
        &http,
        "RPT_F10_EH_EQUITY",
        "ALL",
        &extra,
        "500",
        "HSF10",
        "PC",
    )?;
    let mut df = finalize_report(&rows, &GBJG_RENAME, &GBJG_COLS, &GBJG_NUMERIC, None)?;
    df.cast_date(&GBJG_DATE)?;
    Ok(df)
}

/// 东方财富-数据中心-特色数据-商誉-个股商誉明细（对应 akshare [`akshare.stock_sy_em`]）。
///
/// `date`：`YYYYMMDD`（对应网站指定的数据日期，如 `20231231`）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 商誉, 商誉占净资产比例, 净利润, 净利润同比,
/// 上年商誉, 公告日期, 交易市场`
pub fn stock_sy_em(date: &str) -> Result<Df> {
    let ymd = fmt_ymd(date)?;
    let http = HttpClient::default();
    let filter = format!("(REPORT_DATE='{ymd}')");
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        Some(SY_TOKEN),
        None,
    );
    let rows = fetch_datacenter_pages(&http, "RPT_GOODWILL_STOCKDETAILS", "ALL", &extra, "5000")?;
    let mut df = finalize_report(&rows, &SY_RENAME, &SY_COLS, &SY_NUMERIC, Some("序号"))?;
    // 交易市场 枚举映射（对应 akshare `.map({"shzb":"沪市主板",...})`）
    let board = df
        .inner()
        .column("交易市场")
        .map_err(|e| AkshareError::Empty(e.to_string()))?
        .str()
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    let mapped: Vec<Option<String>> = (0..df.height())
        .map(|i| board.get(i).map(map_trade_board).map(str::to_string))
        .collect();
    df.with_column("交易市场", &mapped)?;
    Ok(df)
}

/// 东方财富-A股-财务分析-主要指标（对应 akshare
/// [`akshare.stock_financial_analysis_indicator_em`]）。
///
/// `symbol`：带市场标识的股票代码（如 `301389.SZ`）；`indicator`：`按报告期` / `按单季度`。
/// 两者均返回原生英文键（与 akshare 一致），分别走 `securities/api/data/get`（非 `/v1`）
/// 与 `securities/api/data/v1/get`。
pub fn stock_financial_analysis_indicator_em(symbol: &str, indicator: &str) -> Result<Df> {
    let http = HttpClient::default();
    let filter = format!("(SECUCODE=\"{symbol}\")");
    let mut extra = report_extra("REPORT_DATE", "-1", Some(&filter), None, None, None);
    extra.insert("source".into(), json!("HSF10"));
    extra.insert("client".into(), json!("PC"));
    let (rows, cols, numeric): (Vec<Value>, &[&str], &[&str]) = if indicator == "按报告期" {
        let rows = fetch_securities_data_get(
            &http,
            "RPT_F10_FINANCE_MAINFINADATA",
            "APP_F10_MAINFINADATA",
            &extra,
            "200",
            "HSF10",
            "PC",
        )?;
        (rows, &FIN_BG_COLS[..], &FIN_BG_NUMERIC[..])
    } else {
        let rows = fetch_securities_pages(
            &http,
            "RPT_F10_QTR_MAINFINADATA",
            "ALL",
            &extra,
            "200",
            "HSF10",
            "PC",
        )?;
        (rows, &FIN_DJ_COLS[..], &FIN_DJ_NUMERIC[..])
    };
    finalize_report(&rows, &identity_rename(cols), cols, numeric, None)
}

/// 东方财富-港股-财务分析-主要指标（对应 akshare
/// [`akshare.stock_financial_hk_analysis_indicator_em`]）。
///
/// `symbol`：港股代码（如 `00700`）；`indicator`：`年度` / `报告期`。
/// 返回原生英文键（与 akshare 一致）。
pub fn stock_financial_hk_analysis_indicator_em(symbol: &str, indicator: &str) -> Result<Df> {
    let http = HttpClient::default();
    let filter = if indicator == "年度" {
        format!("(SECUCODE=\"{symbol}.HK\")(DATE_TYPE_CODE=\"001\")")
    } else {
        format!("(SECUCODE=\"{symbol}.HK\")")
    };
    let mut extra = report_extra("STD_REPORT_DATE", "-1", Some(&filter), None, None, None);
    extra.insert("source".into(), json!("F10"));
    extra.insert("client".into(), json!("PC"));
    extra.insert("v".into(), json!("01975982096513973"));
    let rows = fetch_securities_pages(
        &http,
        "RPT_HKF10_FN_MAININDICATOR",
        "HKF10_FN_MAININDICATOR",
        &extra,
        "9",
        "F10",
        "PC",
    )?;
    finalize_report(
        &rows,
        &identity_rename(&HK_COLS),
        &HK_COLS,
        &HK_NUMERIC,
        None,
    )
}

/// 东方财富-美股-财务分析-主要指标（对应 akshare
/// [`akshare.stock_financial_us_analysis_indicator_em`]）。
///
/// `symbol`：美股代码（如 `TSLA`）；`indicator`：`年报` / `单季报` / `累计季报`。
/// 先经 `RPT_USF10_INFO_ORGPROFILE` 查询市场得到 `SECUCODE`，再拉取主要指标（原生英文键）。
pub fn stock_financial_us_analysis_indicator_em(symbol: &str, indicator: &str) -> Result<Df> {
    let http = HttpClient::default();
    // 1) 市场查询得到 SECUCODE
    let mkt_filter = format!("(SECURITY_CODE=\"{symbol}\")");
    let mut mkt_extra = report_extra("", "", Some(&mkt_filter), None, None, None);
    mkt_extra.insert("source".into(), json!("SECURITIES"));
    mkt_extra.insert("client".into(), json!("PC"));
    mkt_extra.insert("v".into(), json!("04406064331266868"));
    let mkt = fetch_securities_pages(
        &http,
        "RPT_USF10_INFO_ORGPROFILE",
        "SECUCODE,SECURITY_CODE,ORG_CODE,SECURITY_INNER_CODE,ORG_NAME,ORG_EN_ABBR,BELONG_INDUSTRY,FOUND_DATE,CHAIRMAN,REG_PLACE,ADDRESS,EMP_NUM,ORG_TEL,ORG_FAX,ORG_EMAIL,ORG_WEB,ORG_PROFILE",
        &mkt_extra,
        "200",
        "SECURITIES",
        "PC",
    )?;
    let secucode = mkt
        .first()
        .and_then(|v| v.get("SECUCODE"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AkshareError::Empty(format!("未找到美股代码: {symbol}")))?
        .to_string();
    // 2) 主要指标报表
    let filter = match indicator {
        "年报" => format!("(SECUCODE=\"{secucode}\")(DATE_TYPE_CODE=\"001\")"),
        "单季报" => format!(
            "(SECUCODE=\"{secucode}\")(DATE_TYPE_CODE in (\"003\",\"006\",\"007\",\"008\"))"
        ),
        "累计季报" => format!("(SECUCODE=\"{secucode}\")(DATE_TYPE_CODE in (\"002\",\"004\"))"),
        other => return Err(AkshareError::Param(format!("未知 indicator: {other}"))),
    };
    let mut extra = report_extra("REPORT_DATE", "-1", Some(&filter), None, None, None);
    extra.insert("source".into(), json!("SECURITIES"));
    extra.insert("client".into(), json!("PC"));
    let (report, columns) = if secucode.contains('_') {
        (
            "RPT_USF10_FN_IMAININDICATOR",
            "ORG_CODE,SECURITY_CODE,SECUCODE,SECURITY_NAME_ABBR,SECURITY_INNER_CODE,\
STD_REPORT_DATE,REPORT_DATE,DATE_TYPE,DATE_TYPE_CODE,REPORT_TYPE,REPORT_DATA_TYPE,\
FISCAL_YEAR,START_DATE,NOTICE_DATE,ACCOUNT_STANDARD,ACCOUNT_STANDARD_NAME,CURRENCY,\
CURRENCY_NAME,ORGTYPE,TOTAL_INCOME,TOTAL_INCOME_YOY,PREMIUM_INCOME,PREMIUM_INCOME_YOY,\
PARENT_HOLDER_NETPROFIT,PARENT_HOLDER_NETPROFIT_YOY,BASIC_EPS_CS,BASIC_EPS_CS_YOY,\
DILUTED_EPS_CS,PAYOUT_RATIO,CAPITIAL_RATIO,ROE,ROE_YOY,ROA,ROA_YOY,DEBT_RATIO,\
DEBT_RATIO_YOY,EQUITY_RATIO",
        )
    } else {
        ("RPT_USF10_FN_GMAININDICATOR", "USF10_FN_GMAININDICATOR")
    };
    let rows = fetch_securities_pages(&http, report, columns, &extra, "", "SECURITIES", "PC")?;
    finalize_report(
        &rows,
        &identity_rename(&US_COLS),
        &US_COLS,
        &US_NUMERIC,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gbjg_normalize_variants() {
        assert_eq!(normalize_em_secu_code("603392").unwrap(), "603392.SH");
        assert_eq!(normalize_em_secu_code("SH603392").unwrap(), "603392.SH");
        assert_eq!(normalize_em_secu_code("603392.SH").unwrap(), "603392.SH");
        assert_eq!(normalize_em_secu_code("000001").unwrap(), "000001.SZ");
        assert_eq!(normalize_em_secu_code("688041").unwrap(), "688041.SH");
        assert_eq!(normalize_em_secu_code("830799").unwrap(), "830799.BJ");
    }

    #[test]
    fn sy_trade_board_map() {
        assert_eq!(map_trade_board("shzb"), "沪市主板");
        assert_eq!(map_trade_board("kcb"), "科创板");
        assert_eq!(map_trade_board("szzb"), "深市主板");
        assert_eq!(map_trade_board("cyb"), "创业板");
        assert_eq!(map_trade_board("other"), "other");
    }

    #[test]
    fn gbjg_build_offline() {
        let rows = vec![serde_json::json!({
            "END_DATE": "2025-12-17 00:00:00",
            "TOTAL_SHARES": 1264392804,
            "LISTED_A_SHARES": 1264392804,
            "FREE_SHARES": 1264392804,
            "CHANGE_REASON": "回购",
            "LIMITED_A_SHARES": null,
            "LIMITED_OTHARS": null,
            "LIMITED_DOMESTIC_NOSTATE": null,
            "LIMITED_DOMESTIC_NATURAL": null,
        })];
        let mut df = finalize_report(&rows, &GBJG_RENAME, &GBJG_COLS, &GBJG_NUMERIC, None).unwrap();
        df.cast_date(&GBJG_DATE).unwrap();
        assert_eq!(df.column_names(), GBJG_COLS.to_vec());
        assert_eq!(
            df.inner().column("变更日期").unwrap().str().unwrap().get(0),
            Some("2025-12-17")
        );
        assert_eq!(
            df.inner().column("总股本").unwrap().f64().unwrap().get(0),
            Some(1264392804.0)
        );
        assert_eq!(
            df.inner().column("变动原因").unwrap().str().unwrap().get(0),
            Some("回购")
        );
        assert_eq!(
            df.inner()
                .column("流通受限股份")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            None
        );
    }

    #[test]
    fn sy_build_offline() {
        let rows = vec![serde_json::json!({
            "SECURITY_CODE": "000637",
            "SECURITY_NAME_ABBR": "茂化实华",
            "TRADE_BOARD": "szzb",
            "GOODWILL": 225901300.0,
            "SUMSHEQUITY_RATIO": 0.258494,
            "PARENTNETPROFIT": -127833700.0,
            "PNP_YOY_RATIO": 0.013795,
            "GOODWILL_PRE": 225901300.0,
            "NOTICE_DATE": "2026-08-06 00:00:00",
        })];
        let mut df =
            finalize_report(&rows, &SY_RENAME, &SY_COLS, &SY_NUMERIC, Some("序号")).unwrap();
        let board = df.inner().column("交易市场").unwrap().str().unwrap();
        let mapped: Vec<Option<String>> = (0..df.height())
            .map(|i| board.get(i).map(map_trade_board).map(str::to_string))
            .collect();
        df.with_column("交易市场", &mapped).unwrap();
        let mut expect = vec!["序号"];
        expect.extend(SY_COLS.iter().copied());
        assert_eq!(df.column_names(), expect);
        assert_eq!(
            df.inner().column("序号").unwrap().f64().unwrap().get(0),
            Some(1.0)
        );
        assert_eq!(
            df.inner().column("股票代码").unwrap().str().unwrap().get(0),
            Some("000637")
        );
        assert_eq!(
            df.inner().column("交易市场").unwrap().str().unwrap().get(0),
            Some("深市主板")
        );
        assert_eq!(
            df.inner().column("商誉").unwrap().f64().unwrap().get(0),
            Some(225901300.0)
        );
    }

    #[test]
    fn fin_dj_identity_offline() {
        let rows = vec![serde_json::json!({
            "SECUCODE": "301389.SZ",
            "SECURITY_CODE": "301389",
            "SECURITY_NAME_ABBR": "隆扬电子",
            "ORG_CODE": "10000214749",
            "REPORT_DATE": "2026-03-31 00:00:00",
            "SECURITY_TYPE_CODE": "058001001",
            "EPSJB": 0.09,
            "BPS": 7.920279,
            "SEASON_LABEL": "一季度",
        })];
        let df = finalize_report(
            &rows,
            &identity_rename(&FIN_DJ_COLS),
            &FIN_DJ_COLS,
            &FIN_DJ_NUMERIC,
            None,
        )
        .unwrap();
        assert_eq!(df.column_names(), FIN_DJ_COLS.to_vec());
        assert_eq!(
            df.inner().column("SECUCODE").unwrap().str().unwrap().get(0),
            Some("301389.SZ")
        );
        assert_eq!(
            df.inner().column("EPSJB").unwrap().f64().unwrap().get(0),
            Some(0.09)
        );
        assert_eq!(
            df.inner()
                .column("REPORT_DATE")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("2026-03-31 00:00:00")
        );
        assert_eq!(
            df.inner()
                .column("SEASON_LABEL")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("一季度")
        );
    }

    #[test]
    fn us_identity_offline() {
        let rows = vec![serde_json::json!({
            "SECUCODE": "TSLA.OQX",
            "SECURITY_CODE": "TSLA",
            "SECURITY_NAME_ABBR": "特斯拉",
            "OPERATE_INCOME": 23400000000.0,
            "GROSS_PROFIT": 5800000000.0,
            "PARENT_HOLDER_NETPROFIT": 1490000000.0,
            "BASIC_EPS": 4.5,
            "CURRENCY_ABBR": "USD",
        })];
        let df = finalize_report(
            &rows,
            &identity_rename(&US_COLS),
            &US_COLS,
            &US_NUMERIC,
            None,
        )
        .unwrap();
        assert_eq!(df.column_names(), US_COLS.to_vec());
        assert_eq!(
            df.inner().column("SECUCODE").unwrap().str().unwrap().get(0),
            Some("TSLA.OQX")
        );
        assert_eq!(
            df.inner()
                .column("OPERATE_INCOME")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(23400000000.0)
        );
        assert_eq!(
            df.inner()
                .column("CURRENCY_ABBR")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("USD")
        );
    }
}
