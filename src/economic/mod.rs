//! 宏观经济数据（对应 akshare `economic/` 目录）。
//!
//! - 金十数据中心-中国宏观指标 14 个（对应 akshare `economic/macro_china.py`），
//!   走 `sources::jin10::macro_china_base`（`datacenter-api.jin10.com/reports/list_v2`，
//!   按 `attr_id` 分指标翻页抓取）。输出统一 5 列：
//!   `商品, 日期, 今值, 预测值, 前值`（商品 = 报表名，日期 `YYYY-MM-DD` 升序，
//!   今值/预测值/前值为数值列，缺失为 None）。
//! - 东财 datacenter-web 宏观 11 个：中国香港 9 个（`macro_china_hk_*`，
//!   `RPT_ECONOMICVALUE_HK` 报表按 `INDICATOR_ID` 过滤）+ 企业商品价格指数
//!   （`RPT_ECONOMY_GOODS_INDEX`）+ 外商直接投资（`RPT_ECONOMY_FDI`）。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::sources::eastmoney::finalize_report;
use crate::sources::jin10::macro_china_base;
use crate::stock_feature::{datacenter, report_extra};

macro_rules! macro_china_fn {
    ($name:ident, $symbol:literal, $attr:literal) => {
        /// 金十数据中心-中国宏观指标（对应 akshare [`akshare.$name`]）。
        ///
        /// 数据源 `datacenter-api.jin10.com/reports/list_v2`（`attr_id=$attr`）。
        ///
        /// # 返回列
        /// `商品, 日期, 今值, 预测值, 前值`
        pub fn $name() -> Result<Df> {
            macro_china_base($symbol, $attr)
        }
    };
}

macro_china_fn!(macro_china_gdp_yearly, "中国GDP年率报告", "57");
macro_china_fn!(macro_china_cpi_yearly, "中国CPI年率报告", "56");
macro_china_fn!(macro_china_cpi_monthly, "中国CPI月率报告", "72");
macro_china_fn!(macro_china_ppi_yearly, "中国PPI年率报告", "60");
macro_china_fn!(macro_china_exports_yoy, "中国以美元计算出口年率报告", "66");
macro_china_fn!(macro_china_imports_yoy, "中国以美元计算进口年率报告", "77");
macro_china_fn!(macro_china_trade_balance, "中国以美元计算贸易帐报告", "61");
macro_china_fn!(
    macro_china_industrial_production_yoy,
    "中国规模以上工业增加值年率报告",
    "58"
);
macro_china_fn!(macro_china_pmi_yearly, "中国官方制造业PMI", "65");
macro_china_fn!(macro_china_cx_pmi_yearly, "中国财新制造业PMI终值报告", "73");
macro_china_fn!(macro_china_cx_services_pmi_yearly, "中国财新服务业PMI报告", "67");
macro_china_fn!(macro_china_non_man_pmi, "中国官方非制造业PMI报告", "75");
macro_china_fn!(macro_china_fx_reserves_yearly, "中国外汇储备报告", "76");
macro_china_fn!(macro_china_m2_yearly, "中国M2货币供应年率报告", "59");

// ============ 东财 datacenter-web 宏观（11 个） ============

/// 中国香港宏观核心（对应 akshare `macro_china_hk_core`）。
///
/// 报表 `RPT_ECONOMICVALUE_HK`，按 `INDICATOR_ID` 过滤；输出列序
/// `时间, 前值, 现值, 发布日期`，`前值`/`现值` 数值化，`发布日期` 截断为
/// `YYYY-MM-DD`，最后按 `发布日期` 升序（对应 akshare `sort_values(["发布日期"])`）。
pub fn macro_china_hk_core(symbol: &str) -> Result<Df> {
    let filter = format!("(INDICATOR_ID=\"{symbol}\")");
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_ECONOMICVALUE_HK", "ALL", &extra, "500")?;
    const RENAME: [(&str, &str); 4] = [
        ("REPORT_DATE_CH", "时间"),
        ("PRE_VALUE", "前值"),
        ("VALUE", "现值"),
        ("PUBLISH_DATE", "发布日期"),
    ];
    const SELECT: [&str; 4] = ["时间", "前值", "现值", "发布日期"];
    const NUMERIC: [&str; 2] = ["前值", "现值"];
    let mut df = finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)?;
    df.cast_date(&["发布日期"])?;
    df = df.sort_by("发布日期", true, false)?;
    Ok(df)
}

macro_rules! macro_china_hk_fn {
    ($name:ident, $symbol:literal) => {
        /// 东方财富-经济数据一览-中国香港（对应 akshare [`akshare.$name`]）。
        ///
        /// # 返回列
        /// `时间, 前值, 现值, 发布日期`
        pub fn $name() -> Result<Df> {
            macro_china_hk_core($symbol)
        }
    };
}

macro_china_hk_fn!(macro_china_hk_cpi, "EMG01336996");
macro_china_hk_fn!(macro_china_hk_cpi_ratio, "EMG00059282");
macro_china_hk_fn!(macro_china_hk_rate_of_unemployment, "EMG00059647");
macro_china_hk_fn!(macro_china_hk_gbp, "EMG01337008");
macro_china_hk_fn!(macro_china_hk_gbp_ratio, "EMG01337009");
macro_china_hk_fn!(macro_china_hk_building_volume, "EMG00158055");
macro_china_hk_fn!(macro_china_hk_building_amount, "EMG00158066");
macro_china_hk_fn!(macro_china_hk_trade_diff_ratio, "EMG00157898");
macro_china_hk_fn!(macro_china_hk_ppi, "EMG00157818");

/// 东方财富-经济数据一览-中国-企业商品价格指数（对应 akshare [`akshare.macro_china_qyspjg`]）。
///
/// 报表 `RPT_ECONOMY_GOODS_INDEX`，服务端按 `REPORT_DATE` 降序返回（akshare 不再排序）。
///
/// # 返回列
/// `月份, 总指数-指数值, 总指数-同比增长, 总指数-环比增长, 农产品-指数值,
/// 农产品-同比增长, 农产品-环比增长, 矿产品-指数值, 矿产品-同比增长,
/// 矿产品-环比增长, 煤油电-指数值, 煤油电-同比增长, 煤油电-环比增长`
pub fn macro_china_qyspjg() -> Result<Df> {
    const COLUMNS: &str = "REPORT_DATE,TIME,BASE,BASE_SAME,BASE_SEQUENTIAL,FARM_BASE,FARM_BASE_SAME,FARM_BASE_SEQUENTIAL,MINERAL_BASE,MINERAL_BASE_SAME,MINERAL_BASE_SEQUENTIAL,ENERGY_BASE,ENERGY_BASE_SAME,ENERGY_BASE_SEQUENTIAL";
    const RENAME: [(&str, &str); 13] = [
        ("TIME", "月份"),
        ("BASE", "总指数-指数值"),
        ("BASE_SAME", "总指数-同比增长"),
        ("BASE_SEQUENTIAL", "总指数-环比增长"),
        ("FARM_BASE", "农产品-指数值"),
        ("FARM_BASE_SAME", "农产品-同比增长"),
        ("FARM_BASE_SEQUENTIAL", "农产品-环比增长"),
        ("MINERAL_BASE", "矿产品-指数值"),
        ("MINERAL_BASE_SAME", "矿产品-同比增长"),
        ("MINERAL_BASE_SEQUENTIAL", "矿产品-环比增长"),
        ("ENERGY_BASE", "煤油电-指数值"),
        ("ENERGY_BASE_SAME", "煤油电-同比增长"),
        ("ENERGY_BASE_SEQUENTIAL", "煤油电-环比增长"),
    ];
    const SELECT: [&str; 13] = [
        "月份",
        "总指数-指数值",
        "总指数-同比增长",
        "总指数-环比增长",
        "农产品-指数值",
        "农产品-同比增长",
        "农产品-环比增长",
        "矿产品-指数值",
        "矿产品-同比增长",
        "矿产品-环比增长",
        "煤油电-指数值",
        "煤油电-同比增长",
        "煤油电-环比增长",
    ];
    const NUMERIC: [&str; 12] = [
        "总指数-指数值",
        "总指数-同比增长",
        "总指数-环比增长",
        "农产品-指数值",
        "农产品-同比增长",
        "农产品-环比增长",
        "矿产品-指数值",
        "矿产品-同比增长",
        "矿产品-环比增长",
        "煤油电-指数值",
        "煤油电-同比增长",
        "煤油电-环比增长",
    ];
    let extra = report_extra("REPORT_DATE", "-1", None, None, None, None);
    let rows = datacenter("RPT_ECONOMY_GOODS_INDEX", COLUMNS, &extra, "500")?;
    let df = finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)?;
    Ok(df)
}

/// 东方财富-经济数据一览-中国-外商直接投资数据（对应 akshare [`akshare.macro_china_fdi`]）。
///
/// 报表 `RPT_ECONOMY_FDI`；`月份` 为字符串（`2023年07月份`），akshare 不解析日期，
/// 按 `月份` 字符串升序（固定宽度字符串序 = 时间序）。
///
/// # 返回列
/// `月份, 当月, 当月-同比增长, 当月-环比增长, 累计, 累计-同比增长`
pub fn macro_china_fdi() -> Result<Df> {
    const COLUMNS: &str = "REPORT_DATE,TIME,ACTUAL_FOREIGN,ACTUAL_FOREIGN_SAME,ACTUAL_FOREIGN_SEQUENTIAL,ACTUAL_FOREIGN_ACCUMULATE,FOREIGN_ACCUMULATE_SAME";
    const RENAME: [(&str, &str); 6] = [
        ("TIME", "月份"),
        ("ACTUAL_FOREIGN", "当月"),
        ("ACTUAL_FOREIGN_SAME", "当月-同比增长"),
        ("ACTUAL_FOREIGN_SEQUENTIAL", "当月-环比增长"),
        ("ACTUAL_FOREIGN_ACCUMULATE", "累计"),
        ("FOREIGN_ACCUMULATE_SAME", "累计-同比增长"),
    ];
    const SELECT: [&str; 6] = [
        "月份",
        "当月",
        "当月-同比增长",
        "当月-环比增长",
        "累计",
        "累计-同比增长",
    ];
    const NUMERIC: [&str; 5] = ["当月", "当月-同比增长", "当月-环比增长", "累计", "累计-同比增长"];
    let extra = report_extra("REPORT_DATE", "-1", None, None, None, None);
    let rows = datacenter("RPT_ECONOMY_FDI", COLUMNS, &extra, "500")?;
    let mut df = finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)?;
    df = df.sort_by("月份", true, false)?;
    Ok(df)
}
