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
use crate::core::error::{AkshareError, Result};
use crate::sources::eastmoney::finalize_report;
use crate::sources::jin10::{fetch_jin10_cdn, fetch_jin10_cdn_text, macro_china_base};
use crate::stock_feature::{datacenter, report_extra};
use serde_json::Value;

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
macro_china_fn!(
    macro_china_cx_services_pmi_yearly,
    "中国财新服务业PMI报告",
    "67"
);
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

// ============ 东财 datacenter-web 海外宏观（RPT_ECONOMICVALUE_* 系列） ============

/// 东财 datacenter-web 海外宏观核心（对应 akshare `macro_australia_*` / `macro_canada_*` 等
/// `RPT_ECONOMICVALUE_*` 报表）。
///
/// 按 `INDICATOR_ID` 过滤；输出列序 `时间, 前值, 现值, 发布日期`
/// （`时间` = `REPORT_DATE_CH` 中文年月，`前值`/`现值` 数值化，`发布日期` 截断为
/// `YYYY-MM-DD`）。`sort` 为 `Some((列, 升序))` 时按该列排序；`None` 时保持服务端
/// `REPORT_DATE` 降序返回（对应 akshare 各指标是否二次排序的差异）。
pub(crate) fn macro_em_economic_core(
    report: &str,
    indicator_id: &str,
    sort: Option<(&str, bool)>,
) -> Result<Df> {
    let filter = format!("(INDICATOR_ID=\"{indicator_id}\")");
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter(report, "ALL", &extra, "2000")?;
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
    if let Some((col, asc)) = sort {
        df = df.sort_by(col, asc, false)?;
    }
    Ok(df)
}

macro_rules! macro_em_economic_fn {
    ($name:ident, $report:literal, $indicator:literal, $sort:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $name() -> Result<Df> {
            macro_em_economic_core($report, $indicator, $sort)
        }
    };
}

// 澳大利亚（RPT_ECONOMICVALUE_AUSTRALIA）：akshare 按 `发布日期` 升序。
macro_em_economic_fn!(
    macro_australia_bank_rate,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00342255",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-基准利率（对应 akshare [`akshare.macro_australia_bank_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_australia_cpi_quarterly,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00101104",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-CPI季率（对应 akshare [`akshare.macro_australia_cpi_quarterly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_australia_cpi_yearly,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00101093",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-CPI年率（对应 akshare [`akshare.macro_australia_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_australia_ppi_quarterly,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00152722",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-PPI季率（对应 akshare [`akshare.macro_australia_ppi_quarterly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_australia_retail_rate_monthly,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00152903",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-零售销售月率（对应 akshare [`akshare.macro_australia_retail_rate_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_australia_trade,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00152793",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-贸易帐（对应 akshare [`akshare.macro_australia_trade`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_australia_unemployment_rate,
    "RPT_ECONOMICVALUE_AUSTRALIA",
    "EMG00101141",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-澳大利亚-失业率（对应 akshare [`akshare.macro_australia_unemployment_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);

// 加拿大（RPT_ECONOMICVALUE_CA）：akshare 不二次排序（保持服务端 `REPORT_DATE` 降序）。
macro_em_economic_fn!(
    macro_canada_bank_rate,
    "RPT_ECONOMICVALUE_CA",
    "EMG00342248",
    None,
    "东方财富-经济数据一览-加拿大-基准利率（对应 akshare [`akshare.macro_canada_bank_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_core_cpi_monthly,
    "RPT_ECONOMICVALUE_CA",
    "EMG00102044",
    None,
    "东方财富-经济数据一览-加拿大-核心CPI月率（对应 akshare [`akshare.macro_canada_core_cpi_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_core_cpi_yearly,
    "RPT_ECONOMICVALUE_CA",
    "EMG00102030",
    None,
    "东方财富-经济数据一览-加拿大-核心CPI年率（对应 akshare [`akshare.macro_canada_core_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_cpi_monthly,
    "RPT_ECONOMICVALUE_CA",
    "EMG00158719",
    None,
    "东方财富-经济数据一览-加拿大-CPI月率（对应 akshare [`akshare.macro_canada_cpi_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_cpi_yearly,
    "RPT_ECONOMICVALUE_CA",
    "EMG00102029",
    None,
    "东方财富-经济数据一览-加拿大-CPI年率（对应 akshare [`akshare.macro_canada_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_gdp_monthly,
    "RPT_ECONOMICVALUE_CA",
    "EMG00159259",
    None,
    "东方财富-经济数据一览-加拿大-GDP月率（对应 akshare [`akshare.macro_canada_gdp_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_new_house_rate,
    "RPT_ECONOMICVALUE_CA",
    "EMG00342247",
    None,
    "东方财富-经济数据一览-加拿大-新屋开工（对应 akshare [`akshare.macro_canada_new_house_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_retail_rate_monthly,
    "RPT_ECONOMICVALUE_CA",
    "EMG01337094",
    None,
    "东方财富-经济数据一览-加拿大-零售销售月率（对应 akshare [`akshare.macro_canada_retail_rate_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_trade,
    "RPT_ECONOMICVALUE_CA",
    "EMG00102022",
    None,
    "东方财富-经济数据一览-加拿大-贸易帐（对应 akshare [`akshare.macro_canada_trade`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_canada_unemployment_rate,
    "RPT_ECONOMICVALUE_CA",
    "EMG00157746",
    None,
    "东方财富-经济数据一览-加拿大-失业率（对应 akshare [`akshare.macro_canada_unemployment_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);

// 德国（RPT_ECONOMICVALUE_GER）：akshare 按 `发布日期` 升序。
macro_em_economic_fn!(
    macro_germany_ifo,
    "RPT_ECONOMICVALUE_GER",
    "EMG00179154",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-IFO商业景气指数（对应 akshare [`akshare.macro_germany_ifo`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_cpi_monthly,
    "RPT_ECONOMICVALUE_GER",
    "EMG00009758",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-消费者物价指数月率终值（对应 akshare [`akshare.macro_germany_cpi_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_cpi_yearly,
    "RPT_ECONOMICVALUE_GER",
    "EMG00009756",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-消费者物价指数年率终值（对应 akshare [`akshare.macro_germany_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_trade_adjusted,
    "RPT_ECONOMICVALUE_GER",
    "EMG00009753",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-贸易帐(季调后)（对应 akshare [`akshare.macro_germany_trade_adjusted`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_gdp,
    "RPT_ECONOMICVALUE_GER",
    "EMG00009720",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-GDP（对应 akshare [`akshare.macro_germany_gdp`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_retail_sale_monthly,
    "RPT_ECONOMICVALUE_GER",
    "EMG01333186",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-实际零售销售月率（对应 akshare [`akshare.macro_germany_retail_sale_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_retail_sale_yearly,
    "RPT_ECONOMICVALUE_GER",
    "EMG01333192",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-实际零售销售年率（对应 akshare [`akshare.macro_germany_retail_sale_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_germany_zew,
    "RPT_ECONOMICVALUE_GER",
    "EMG00172577",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-德国-ZEW经济景气指数（对应 akshare [`akshare.macro_germany_zew`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);

// 日本（RPT_ECONOMICVALUE_JPAN）：akshare 按 `发布日期` 升序。
macro_em_economic_fn!(
    macro_japan_bank_rate,
    "RPT_ECONOMICVALUE_JPAN",
    "EMG00342252",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-日本-央行公布利率决议（对应 akshare [`akshare.macro_japan_bank_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_japan_cpi_yearly,
    "RPT_ECONOMICVALUE_JPAN",
    "EMG00005004",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-日本-全国消费者物价指数年率（对应 akshare [`akshare.macro_japan_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_japan_core_cpi_yearly,
    "RPT_ECONOMICVALUE_JPAN",
    "EMG00158099",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-日本-全国核心消费者物价指数年率（对应 akshare [`akshare.macro_japan_core_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_japan_unemployment_rate,
    "RPT_ECONOMICVALUE_JPAN",
    "EMG00005047",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-日本-失业率（对应 akshare [`akshare.macro_japan_unemployment_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_japan_head_indicator,
    "RPT_ECONOMICVALUE_JPAN",
    "EMG00005117",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-日本-领先指标终值（对应 akshare [`akshare.macro_japan_head_indicator`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);

// 瑞士（RPT_ECONOMICVALUE_CH，CH = Confoederatio Helvetica）：akshare 按 `发布日期` 升序。
macro_em_economic_fn!(
    macro_swiss_svme,
    "RPT_ECONOMICVALUE_CH",
    "EMG00341602",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-瑞士-SVME采购经理人指数（对应 akshare [`akshare.macro_swiss_svme`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_swiss_trade,
    "RPT_ECONOMICVALUE_CH",
    "EMG00341603",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-瑞士-贸易帐（对应 akshare [`akshare.macro_swiss_trade`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_swiss_cpi_yearly,
    "RPT_ECONOMICVALUE_CH",
    "EMG00341604",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-瑞士-消费者物价指数年率（对应 akshare [`akshare.macro_swiss_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_swiss_gdp_quarterly,
    "RPT_ECONOMICVALUE_CH",
    "EMG00341600",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-瑞士-GDP季率（对应 akshare [`akshare.macro_swiss_gdp_quarterly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_swiss_gbd_yearly,
    "RPT_ECONOMICVALUE_CH",
    "EMG00341601",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-瑞士-GDP年率（对应 akshare [`akshare.macro_swiss_gbd_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_swiss_gbd_bank_rate,
    "RPT_ECONOMICVALUE_CH",
    "EMG00341606",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-瑞士-央行公布利率决议（对应 akshare [`akshare.macro_swiss_gbd_bank_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);

// 英国（RPT_ECONOMICVALUE_BRITAIN）：akshare 按 `发布日期` 升序。
// 注：`macro_uk_cpi_monthly` 与 `macro_uk_core_cpi_monthly` 在 akshare 中均用
// INDICATOR_ID=EMG00010291（上游拷贝笔误），此处保持与 akshare 一致。
macro_em_economic_fn!(
    macro_uk_halifax_monthly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00342256",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-Halifax房价指数月率（对应 akshare [`akshare.macro_uk_halifax_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_halifax_yearly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00010370",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-Halifax房价指数年率（对应 akshare [`akshare.macro_uk_halifax_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_trade,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00158309",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-贸易帐（对应 akshare [`akshare.macro_uk_trade`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_bank_rate,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00342253",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-央行公布利率决议（对应 akshare [`akshare.macro_uk_bank_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_core_cpi_yearly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00010279",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-核心消费者物价指数年率（对应 akshare [`akshare.macro_uk_core_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_core_cpi_monthly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00010291",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-核心消费者物价指数月率（对应 akshare [`akshare.macro_uk_core_cpi_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_cpi_yearly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00010267",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-消费者物价指数年率（对应 akshare [`akshare.macro_uk_cpi_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_cpi_monthly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00010291",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-消费者物价指数月率（对应 akshare [`akshare.macro_uk_cpi_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_retail_monthly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00158298",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-零售销售月率（对应 akshare [`akshare.macro_uk_retail_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_retail_yearly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00158297",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-零售销售年率（对应 akshare [`akshare.macro_uk_retail_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_rightmove_yearly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00341608",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-Rightmove房价指数年率（对应 akshare [`akshare.macro_uk_rightmove_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_rightmove_monthly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00341607",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-Rightmove房价指数月率（对应 akshare [`akshare.macro_uk_rightmove_monthly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_gdp_quarterly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00158277",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-GDP季率初值（对应 akshare [`akshare.macro_uk_gdp_quarterly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_gdp_yearly,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00158276",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-GDP年率初值（对应 akshare [`akshare.macro_uk_gdp_yearly`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);
macro_em_economic_fn!(
    macro_uk_unemployment_rate,
    "RPT_ECONOMICVALUE_BRITAIN",
    "EMG00010348",
    Some(("发布日期", true)),
    "东方财富-经济数据一览-英国-失业率（对应 akshare [`akshare.macro_uk_unemployment_rate`]）。\n\n# 返回列\n`时间, 前值, 现值, 发布日期`"
);

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
    const NUMERIC: [&str; 5] = [
        "当月",
        "当月-同比增长",
        "当月-环比增长",
        "累计",
        "累计-同比增长",
    ];
    let extra = report_extra("REPORT_DATE", "-1", None, None, None, None);
    let rows = datacenter("RPT_ECONOMY_FDI", COLUMNS, &extra, "500")?;
    let mut df = finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)?;
    df = df.sort_by("月份", true, false)?;
    Ok(df)
}

// === BATCH3 ECONOMIC REMAINING (jin10/em datacenter) ===

/// 东财 datacenter-web 中国宏观指标报表通用落地（对应 akshare 各 `macro_china_*` 东财实现）。
///
/// 走 `datacenter-web.eastmoney.com/api/data/v1/get`（`reportName`），服务端按
/// `REPORT_DATE` 降序返回全部历史；本函数用 `columns=ALL` 拉取后按 `rename` 重命名、
/// `select` 选列、`numeric` 数值化，并对 `date_cols` 截断到 `YYYY-MM-DD`。
fn macro_china_em(
    report: &str,
    rename: &[(&str, &str)],
    select: &[&str],
    numeric: &[&str],
    date_cols: &[&str],
) -> Result<Df> {
    let extra = report_extra("REPORT_DATE", "-1", None, None, None, None);
    let rows = datacenter(report, "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, rename, select, numeric, None)?;
    if !date_cols.is_empty() {
        df.cast_date(date_cols)?;
    }
    Ok(df)
}

/// 东财 datacenter-web 行业指数报表（`RPT_INDUSTRY_INDEX`，按 `INDICATOR_ID` 过滤）。
///
/// 输出列序 `日期, 最新值, 涨跌幅, 近3月涨跌幅, 近6月涨跌幅, 近1年涨跌幅,
/// 近2年涨跌幅, 近3年涨跌幅`；`日期` 截断为 `YYYY-MM-DD` 并按升序返回。
fn macro_china_industry_index(symbol: &str) -> Result<Df> {
    let filter = format!("(INDICATOR_ID=\"{symbol}\")");
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_INDUSTRY_INDEX", "ALL", &extra, "500")?;
    const RENAME: [(&str, &str); 8] = [
        ("REPORT_DATE", "日期"),
        ("INDICATOR_VALUE", "最新值"),
        ("CHANGE_RATE", "涨跌幅"),
        ("CHANGERATE_3M", "近3月涨跌幅"),
        ("CHANGERATE_6M", "近6月涨跌幅"),
        ("CHANGERATE_1Y", "近1年涨跌幅"),
        ("CHANGERATE_2Y", "近2年涨跌幅"),
        ("CHANGERATE_3Y", "近3年涨跌幅"),
    ];
    const SELECT: [&str; 8] = [
        "日期",
        "最新值",
        "涨跌幅",
        "近3月涨跌幅",
        "近6月涨跌幅",
        "近1年涨跌幅",
        "近2年涨跌幅",
        "近3年涨跌幅",
    ];
    const NUMERIC: [&str; 7] = [
        "最新值",
        "涨跌幅",
        "近3月涨跌幅",
        "近6月涨跌幅",
        "近1年涨跌幅",
        "近2年涨跌幅",
        "近3年涨跌幅",
    ];
    let mut df = finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

macro_rules! macro_china_industry_fn {
    ($name:ident, $symbol:literal, $desc:literal) => {
        #[doc = $desc]
        pub fn $name() -> Result<Df> {
            macro_china_industry_index($symbol)
        }
    };
}

macro_china_industry_fn!(
    macro_china_agricultural_index,
    "EMI00662543",
    "东方财富-农副指数（对应 akshare `macro_china_agricultural_index`）。"
);
macro_china_industry_fn!(
    macro_china_agricultural_product,
    "EMI00009274",
    "东方财富-农产品批发价格总指数（对应 akshare `macro_china_agricultural_product`）。"
);
macro_china_industry_fn!(
    macro_china_bank_financing,
    "EMI01516267",
    "东方财富-银行理财产品发行数量（对应 akshare `macro_china_bank_financing`）。"
);
macro_china_industry_fn!(
    macro_china_bdti_index,
    "EMI00107668",
    "东方财富-原油运输指数（对应 akshare `macro_china_bdti_index`）。"
);
macro_china_industry_fn!(
    macro_china_bsi_index,
    "EMI00107667",
    "东方财富-超灵便型船运价指数（对应 akshare `macro_china_bsi_index`）。"
);
macro_china_industry_fn!(
    macro_china_commodity_price_index,
    "EMI00662535",
    "东方财富-大宗商品价格（对应 akshare `macro_china_commodity_price_index`）。"
);
macro_china_industry_fn!(
    macro_china_construction_index,
    "EMI00662541",
    "东方财富-建材指数（对应 akshare `macro_china_construction_index`）。"
);
macro_china_industry_fn!(
    macro_china_construction_price_index,
    "EMI00237146",
    "东方财富-建材价格指数（对应 akshare `macro_china_construction_price_index`）。"
);
macro_china_industry_fn!(
    macro_china_energy_index,
    "EMI00662539",
    "东方财富-能源指数（对应 akshare `macro_china_energy_index`）。"
);
macro_china_industry_fn!(
    macro_china_insurance_income,
    "EMM00088870",
    "东方财富-保险业经营情况（对应 akshare `macro_china_insurance_income`）。"
);
macro_china_industry_fn!(
    macro_china_lpi_index,
    "EMI00352262",
    "东方财富-物流业景气指数（对应 akshare `macro_china_lpi_index`）。"
);
macro_china_industry_fn!(
    macro_china_mobile_number,
    "EMI00225823",
    "东方财富-移动电话用户数（对应 akshare `macro_china_mobile_number`）。"
);
macro_china_industry_fn!(
    macro_china_real_estate,
    "EMM00121987",
    "东方财富-房地产开发景气指数（对应 akshare `macro_china_real_estate`）。"
);
macro_china_industry_fn!(
    macro_china_vegetable_basket,
    "EMI00009275",
    "东方财富-菜篮子产品批发价格（对应 akshare `macro_china_vegetable_basket`）。"
);
macro_china_industry_fn!(
    macro_china_yw_electronic_index,
    "EMI00055551",
    "东方财富-义乌小商品指数（对应 akshare `macro_china_yw_electronic_index`）。"
);

/// 东方财富-中国消费品零售总额（对应 akshare [`akshare.macro_china_consumer_goods_retail`]）。
///
/// 报表 `RPT_ECONOMY_TOTAL_RETAIL`。# 返回列
/// `月份, 当月, 同比增长, 环比增长, 累计, 累计-同比增长`
pub fn macro_china_consumer_goods_retail() -> Result<Df> {
    const RENAME: [(&str, &str); 6] = [
        ("TIME", "月份"),
        ("RETAIL_TOTAL", "当月"),
        ("RETAIL_TOTAL_SAME", "同比增长"),
        ("RETAIL_TOTAL_SEQUENTIAL", "环比增长"),
        ("RETAIL_TOTAL_ACCUMULATE", "累计"),
        ("RETAIL_ACCUMULATE_SAME", "累计-同比增长"),
    ];
    const SELECT: [&str; 6] = [
        "月份",
        "当月",
        "同比增长",
        "环比增长",
        "累计",
        "累计-同比增长",
    ];
    const NUMERIC: [&str; 5] = ["当月", "同比增长", "环比增长", "累计", "累计-同比增长"];
    macro_china_em("RPT_ECONOMY_TOTAL_RETAIL", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-中国居民消费价格指数 CPI（对应 akshare [`akshare.macro_china_cpi`]）。
///
/// 报表 `RPT_ECONOMY_CPI`。# 返回列
/// `月份, 全国-当月, 全国-同比增长, 全国-环比增长, 全国-累计, 城市-当月,
/// 城市-同比增长, 城市-环比增长, 城市-累计, 农村-当月, 农村-同比增长,
/// 农村-环比增长, 农村-累计`
pub fn macro_china_cpi() -> Result<Df> {
    const RENAME: [(&str, &str); 13] = [
        ("TIME", "月份"),
        ("NATIONAL_BASE", "全国-当月"),
        ("NATIONAL_SAME", "全国-同比增长"),
        ("NATIONAL_SEQUENTIAL", "全国-环比增长"),
        ("NATIONAL_ACCUMULATE", "全国-累计"),
        ("CITY_BASE", "城市-当月"),
        ("CITY_SAME", "城市-同比增长"),
        ("CITY_SEQUENTIAL", "城市-环比增长"),
        ("CITY_ACCUMULATE", "城市-累计"),
        ("RURAL_BASE", "农村-当月"),
        ("RURAL_SAME", "农村-同比增长"),
        ("RURAL_SEQUENTIAL", "农村-环比增长"),
        ("RURAL_ACCUMULATE", "农村-累计"),
    ];
    const SELECT: [&str; 13] = [
        "月份",
        "全国-当月",
        "全国-同比增长",
        "全国-环比增长",
        "全国-累计",
        "城市-当月",
        "城市-同比增长",
        "城市-环比增长",
        "城市-累计",
        "农村-当月",
        "农村-同比增长",
        "农村-环比增长",
        "农村-累计",
    ];
    const NUMERIC: [&str; 12] = [
        "全国-当月",
        "全国-同比增长",
        "全国-环比增长",
        "全国-累计",
        "城市-当月",
        "城市-同比增长",
        "城市-环比增长",
        "城市-累计",
        "农村-当月",
        "农村-同比增长",
        "农村-环比增长",
        "农村-累计",
    ];
    macro_china_em("RPT_ECONOMY_CPI", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-中国财政收入（对应 akshare [`akshare.macro_china_czsr`]）。
///
/// 报表 `RPT_ECONOMY_INCOME`。# 返回列
/// `月份, 当月, 当月-同比增长, 当月-环比增长, 累计, 累计-同比增长`
pub fn macro_china_czsr() -> Result<Df> {
    const RENAME: [(&str, &str); 6] = [
        ("TIME", "月份"),
        ("BASE", "当月"),
        ("BASE_SAME", "当月-同比增长"),
        ("BASE_SEQUENTIAL", "当月-环比增长"),
        ("BASE_ACCUMULATE", "累计"),
        ("ACCUMULATE_SAME", "累计-同比增长"),
    ];
    const SELECT: [&str; 6] = [
        "月份",
        "当月",
        "当月-同比增长",
        "当月-环比增长",
        "累计",
        "累计-同比增长",
    ];
    const NUMERIC: [&str; 5] = [
        "当月",
        "当月-同比增长",
        "当月-环比增长",
        "累计",
        "累计-同比增长",
    ];
    macro_china_em("RPT_ECONOMY_INCOME", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-企业景气指数（对应 akshare [`akshare.macro_china_enterprise_boom_index`]）。
///
/// 报表 `RPT_ECONOMY_BOOM_INDEX`。# 返回列
/// `季度, 企业景气指数-指数, 企业景气指数-同比, 企业景气指数-环比,
/// 企业家信心指数-指数, 企业家信心指数-同比, 企业家信心指数-环比`
pub fn macro_china_enterprise_boom_index() -> Result<Df> {
    const RENAME: [(&str, &str); 7] = [
        ("TIME", "季度"),
        ("BOOM_INDEX", "企业景气指数-指数"),
        ("BOOM_INDEX_SAME", "企业景气指数-同比"),
        ("BOOM_INDEX_SEQUENTIAL", "企业景气指数-环比"),
        ("FAITH_INDEX", "企业家信心指数-指数"),
        ("FAITH_INDEX_SAME", "企业家信心指数-同比"),
        ("FAITH_INDEX_SEQUENTIAL", "企业家信心指数-环比"),
    ];
    const SELECT: [&str; 7] = [
        "季度",
        "企业景气指数-指数",
        "企业景气指数-同比",
        "企业景气指数-环比",
        "企业家信心指数-指数",
        "企业家信心指数-同比",
        "企业家信心指数-环比",
    ];
    const NUMERIC: [&str; 6] = [
        "企业景气指数-指数",
        "企业景气指数-同比",
        "企业景气指数-环比",
        "企业家信心指数-指数",
        "企业家信心指数-同比",
        "企业家信心指数-环比",
    ];
    macro_china_em("RPT_ECONOMY_BOOM_INDEX", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-黄金和外汇储备（对应 akshare [`akshare.macro_china_fx_gold`]）。
///
/// 报表 `RPT_ECONOMY_GOLD_CURRENCY`。# 返回列
/// `月份, 黄金储备-数值, 黄金储备-同比, 黄金储备-环比, 国家外汇储备-数值,
/// 国家外汇储备-同比, 国家外汇储备-环比`
pub fn macro_china_fx_gold() -> Result<Df> {
    const RENAME: [(&str, &str); 7] = [
        ("TIME", "月份"),
        ("GOLD_RESERVES", "黄金储备-数值"),
        ("GOLD_RESERVES_SAME", "黄金储备-同比"),
        ("GOLD_RESERVES_SEQUENTIAL", "黄金储备-环比"),
        ("FOREX", "国家外汇储备-数值"),
        ("FOREX_SAME", "国家外汇储备-同比"),
        ("FOREX_SEQUENTIAL", "国家外汇储备-环比"),
    ];
    const SELECT: [&str; 7] = [
        "月份",
        "黄金储备-数值",
        "黄金储备-同比",
        "黄金储备-环比",
        "国家外汇储备-数值",
        "国家外汇储备-同比",
        "国家外汇储备-环比",
    ];
    const NUMERIC: [&str; 6] = [
        "黄金储备-数值",
        "黄金储备-同比",
        "黄金储备-环比",
        "国家外汇储备-数值",
        "国家外汇储备-同比",
        "国家外汇储备-环比",
    ];
    macro_china_em("RPT_ECONOMY_GOLD_CURRENCY", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-国内生产总值 GDP（对应 akshare [`akshare.macro_china_gdp`]）。
///
/// 报表 `RPT_ECONOMY_GDP`。# 返回列
/// `季度, 国内生产总值-绝对值, 国内生产总值-同比增长, 第一产业-绝对值,
/// 第一产业-同比增长, 第二产业-绝对值, 第二产业-同比增长, 第三产业-绝对值,
/// 第三产业-同比增长`
pub fn macro_china_gdp() -> Result<Df> {
    const RENAME: [(&str, &str); 9] = [
        ("TIME", "季度"),
        ("DOMESTICL_PRODUCT_BASE", "国内生产总值-绝对值"),
        ("SUM_SAME", "国内生产总值-同比增长"),
        ("FIRST_PRODUCT_BASE", "第一产业-绝对值"),
        ("FIRST_SAME", "第一产业-同比增长"),
        ("SECOND_PRODUCT_BASE", "第二产业-绝对值"),
        ("SECOND_SAME", "第二产业-同比增长"),
        ("THIRD_PRODUCT_BASE", "第三产业-绝对值"),
        ("THIRD_SAME", "第三产业-同比增长"),
    ];
    const SELECT: [&str; 9] = [
        "季度",
        "国内生产总值-绝对值",
        "国内生产总值-同比增长",
        "第一产业-绝对值",
        "第一产业-同比增长",
        "第二产业-绝对值",
        "第二产业-同比增长",
        "第三产业-绝对值",
        "第三产业-同比增长",
    ];
    const NUMERIC: [&str; 8] = [
        "国内生产总值-绝对值",
        "国内生产总值-同比增长",
        "第一产业-绝对值",
        "第一产业-同比增长",
        "第二产业-绝对值",
        "第二产业-同比增长",
        "第三产业-绝对值",
        "第三产业-同比增长",
    ];
    macro_china_em("RPT_ECONOMY_GDP", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-固定资产投资（对应 akshare [`akshare.macro_china_gdzctz`]）。
///
/// 报表 `RPT_ECONOMY_ASSET_INVEST`。# 返回列
/// `月份, 当月, 同比增长, 环比增长, 自年初累计`
pub fn macro_china_gdzctz() -> Result<Df> {
    const RENAME: [(&str, &str); 5] = [
        ("TIME", "月份"),
        ("BASE", "当月"),
        ("BASE_SAME", "同比增长"),
        ("BASE_SEQUENTIAL", "环比增长"),
        ("BASE_ACCUMULATE", "自年初累计"),
    ];
    const SELECT: [&str; 5] = ["月份", "当月", "同比增长", "环比增长", "自年初累计"];
    const NUMERIC: [&str; 4] = ["当月", "同比增长", "环比增长", "自年初累计"];
    macro_china_em("RPT_ECONOMY_ASSET_INVEST", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-规模以上工业增加值（对应 akshare [`akshare.macro_china_gyzjz`]）。
///
/// 报表 `RPT_ECONOMY_INDUS_GROW`。# 返回列
/// `月份, 同比增长, 累计增长, 发布时间`
pub fn macro_china_gyzjz() -> Result<Df> {
    const RENAME: [(&str, &str); 4] = [
        ("TIME", "月份"),
        ("BASE_SAME", "同比增长"),
        ("BASE_ACCUMULATE", "累计增长"),
        ("REPORT_DATE", "发布时间"),
    ];
    const SELECT: [&str; 4] = ["月份", "同比增长", "累计增长", "发布时间"];
    const NUMERIC: [&str; 2] = ["同比增长", "累计增长"];
    macro_china_em(
        "RPT_ECONOMY_INDUS_GROW",
        &RENAME,
        &SELECT,
        &NUMERIC,
        &["发布时间"],
    )
}

/// 东方财富-海关进出口（对应 akshare [`akshare.macro_china_hgjck`]）。
///
/// 报表 `RPT_ECONOMY_CUSTOMS`。# 返回列
/// `月份, 当月出口额-金额, 当月出口额-同比增长, 当月出口额-环比增长,
/// 当月进口额-金额, 当月进口额-同比增长, 当月进口额-环比增长,
/// 累计出口额-金额, 累计出口额-同比增长, 累计进口额-金额, 累计进口额-同比增长`
pub fn macro_china_hgjck() -> Result<Df> {
    const RENAME: [(&str, &str); 11] = [
        ("TIME", "月份"),
        ("EXIT_BASE", "当月出口额-金额"),
        ("EXIT_BASE_SAME", "当月出口额-同比增长"),
        ("EXIT_BASE_SEQUENTIAL", "当月出口额-环比增长"),
        ("IMPORT_BASE", "当月进口额-金额"),
        ("IMPORT_BASE_SAME", "当月进口额-同比增长"),
        ("IMPORT_BASE_SEQUENTIAL", "当月进口额-环比增长"),
        ("EXIT_ACCUMULATE", "累计出口额-金额"),
        ("EXIT_ACCUMULATE_SAME", "累计出口额-同比增长"),
        ("IMPORT_ACCUMULATE", "累计进口额-金额"),
        ("IMPORT_ACCUMULATE_SAME", "累计进口额-同比增长"),
    ];
    const SELECT: [&str; 11] = [
        "月份",
        "当月出口额-金额",
        "当月出口额-同比增长",
        "当月出口额-环比增长",
        "当月进口额-金额",
        "当月进口额-同比增长",
        "当月进口额-环比增长",
        "累计出口额-金额",
        "累计出口额-同比增长",
        "累计进口额-金额",
        "累计进口额-同比增长",
    ];
    const NUMERIC: [&str; 10] = [
        "当月出口额-金额",
        "当月出口额-同比增长",
        "当月出口额-环比增长",
        "当月进口额-金额",
        "当月进口额-同比增长",
        "当月进口额-环比增长",
        "累计出口额-金额",
        "累计出口额-同比增长",
        "累计进口额-金额",
        "累计进口额-同比增长",
    ];
    macro_china_em("RPT_ECONOMY_CUSTOMS", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-LPR 贷款市场报价利率（对应 akshare [`akshare.macro_china_lpr`]）。
///
/// 报表 `RPTA_WEB_RATE`（需 `token`）；`TRADE_DATE` 截断为 `YYYY-MM-DD` 并按升序。
/// # 返回列 `TRADE_DATE, LPR1Y, LPR5Y, RATE_1, RATE_2`
pub fn macro_china_lpr() -> Result<Df> {
    const RENAME: [(&str, &str); 5] = [
        ("TRADE_DATE", "TRADE_DATE"),
        ("LPR1Y", "LPR1Y"),
        ("LPR5Y", "LPR5Y"),
        ("RATE_1", "RATE_1"),
        ("RATE_2", "RATE_2"),
    ];
    const SELECT: [&str; 5] = ["TRADE_DATE", "LPR1Y", "LPR5Y", "RATE_1", "RATE_2"];
    const NUMERIC: [&str; 4] = ["LPR1Y", "LPR5Y", "RATE_1", "RATE_2"];
    let extra = report_extra(
        "TRADE_DATE",
        "-1",
        None,
        None,
        Some("894050c76af8597a853f5b408b759f5d"),
        None,
    );
    let rows = datacenter("RPTA_WEB_RATE", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)?;
    df.cast_date(&["TRADE_DATE"])?;
    df = df.sort_by("TRADE_DATE", true, false)?;
    Ok(df)
}

/// 东方财富-货币供应量（对应 akshare [`akshare.macro_china_money_supply`]）。
///
/// 报表 `RPT_ECONOMY_CURRENCY_SUPPLY`。# 返回列
/// `月份, 货币和准货币(M2)-数量(亿元), 货币和准货币(M2)-同比增长, 货币和准货币(M2)-环比增长,
/// 货币(M1)-数量(亿元), 货币(M1)-同比增长, 货币(M1)-环比增长,
/// 流通中的现金(M0)-数量(亿元), 流通中的现金(M0)-同比增长, 流通中的现金(M0)-环比增长`
pub fn macro_china_money_supply() -> Result<Df> {
    const RENAME: [(&str, &str); 10] = [
        ("TIME", "月份"),
        ("BASIC_CURRENCY", "货币和准货币(M2)-数量(亿元)"),
        ("BASIC_CURRENCY_SAME", "货币和准货币(M2)-同比增长"),
        ("BASIC_CURRENCY_SEQUENTIAL", "货币和准货币(M2)-环比增长"),
        ("CURRENCY", "货币(M1)-数量(亿元)"),
        ("CURRENCY_SAME", "货币(M1)-同比增长"),
        ("CURRENCY_SEQUENTIAL", "货币(M1)-环比增长"),
        ("FREE_CASH", "流通中的现金(M0)-数量(亿元)"),
        ("FREE_CASH_SAME", "流通中的现金(M0)-同比增长"),
        ("FREE_CASH_SEQUENTIAL", "流通中的现金(M0)-环比增长"),
    ];
    const SELECT: [&str; 10] = [
        "月份",
        "货币和准货币(M2)-数量(亿元)",
        "货币和准货币(M2)-同比增长",
        "货币和准货币(M2)-环比增长",
        "货币(M1)-数量(亿元)",
        "货币(M1)-同比增长",
        "货币(M1)-环比增长",
        "流通中的现金(M0)-数量(亿元)",
        "流通中的现金(M0)-同比增长",
        "流通中的现金(M0)-环比增长",
    ];
    const NUMERIC: [&str; 9] = [
        "货币和准货币(M2)-数量(亿元)",
        "货币和准货币(M2)-同比增长",
        "货币和准货币(M2)-环比增长",
        "货币(M1)-数量(亿元)",
        "货币(M1)-同比增长",
        "货币(M1)-环比增长",
        "流通中的现金(M0)-数量(亿元)",
        "流通中的现金(M0)-同比增长",
        "流通中的现金(M0)-环比增长",
    ];
    macro_china_em(
        "RPT_ECONOMY_CURRENCY_SUPPLY",
        &RENAME,
        &SELECT,
        &NUMERIC,
        &[],
    )
}

/// 东方财富-税收收入（对应 akshare [`akshare.macro_china_national_tax_receipts`]）。
///
/// 报表 `RPT_ECONOMY_TAX`。# 返回列 `季度, 税收收入合计, 较上年同期, 季度环比`
pub fn macro_china_national_tax_receipts() -> Result<Df> {
    const RENAME: [(&str, &str); 4] = [
        ("TIME", "季度"),
        ("TAX_INCOME", "税收收入合计"),
        ("TAX_INCOME_SAME", "较上年同期"),
        ("TAX_INCOME_SEQUENTIAL", "季度环比"),
    ];
    const SELECT: [&str; 4] = ["季度", "税收收入合计", "较上年同期", "季度环比"];
    const NUMERIC: [&str; 3] = ["税收收入合计", "较上年同期", "季度环比"];
    macro_china_em("RPT_ECONOMY_TAX", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-新增信贷（对应 akshare [`akshare.macro_china_new_financial_credit`]）。
///
/// 报表 `RPT_ECONOMY_RMB_LOAN`。# 返回列
/// `月份, 当月, 当月-同比增长, 当月-环比增长, 累计, 累计-同比增长`
pub fn macro_china_new_financial_credit() -> Result<Df> {
    const RENAME: [(&str, &str); 6] = [
        ("TIME", "月份"),
        ("RMB_LOAN", "当月"),
        ("RMB_LOAN_SAME", "当月-同比增长"),
        ("RMB_LOAN_SEQUENTIAL", "当月-环比增长"),
        ("RMB_LOAN_ACCUMULATE", "累计"),
        ("LOAN_ACCUMULATE_SAME", "累计-同比增长"),
    ];
    const SELECT: [&str; 6] = [
        "月份",
        "当月",
        "当月-同比增长",
        "当月-环比增长",
        "累计",
        "累计-同比增长",
    ];
    const NUMERIC: [&str; 5] = [
        "当月",
        "当月-同比增长",
        "当月-环比增长",
        "累计",
        "累计-同比增长",
    ];
    macro_china_em("RPT_ECONOMY_RMB_LOAN", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-新建商品住宅价格指数（对应 akshare [`akshare.macro_china_new_house_price`]）。
///
/// 报表 `RPT_ECONOMY_HOUSE_PRICE`；`日期` 截断为 `YYYY-MM-DD`。# 返回列
/// `日期, 城市, 新建商品住宅价格指数-同比, 新建商品住宅价格指数-环比,
/// 新建商品住宅价格指数-定基, 二手住宅价格指数-同比, 二手住宅价格指数-环比,
/// 二手住宅价格指数-定基`
pub fn macro_china_new_house_price() -> Result<Df> {
    const RENAME: [(&str, &str); 8] = [
        ("REPORT_DATE", "日期"),
        ("CITY", "城市"),
        ("FIRST_COMHOUSE_SAME", "新建商品住宅价格指数-同比"),
        ("FIRST_COMHOUSE_SEQUENTIAL", "新建商品住宅价格指数-环比"),
        ("FIRST_COMHOUSE_BASE", "新建商品住宅价格指数-定基"),
        ("SECOND_HOUSE_SAME", "二手住宅价格指数-同比"),
        ("SECOND_HOUSE_SEQUENTIAL", "二手住宅价格指数-环比"),
        ("SECOND_HOUSE_BASE", "二手住宅价格指数-定基"),
    ];
    const SELECT: [&str; 8] = [
        "日期",
        "城市",
        "新建商品住宅价格指数-同比",
        "新建商品住宅价格指数-环比",
        "新建商品住宅价格指数-定基",
        "二手住宅价格指数-同比",
        "二手住宅价格指数-环比",
        "二手住宅价格指数-定基",
    ];
    const NUMERIC: [&str; 6] = [
        "新建商品住宅价格指数-同比",
        "新建商品住宅价格指数-环比",
        "新建商品住宅价格指数-定基",
        "二手住宅价格指数-同比",
        "二手住宅价格指数-环比",
        "二手住宅价格指数-定基",
    ];
    macro_china_em(
        "RPT_ECONOMY_HOUSE_PRICE",
        &RENAME,
        &SELECT,
        &NUMERIC,
        &["日期"],
    )
}

/// 东方财富-制造业/非制造业 PMI（对应 akshare [`akshare.macro_china_pmi`]）。
///
/// 报表 `RPT_ECONOMY_PMI`。# 返回列
/// `月份, 制造业-指数, 制造业-同比增长, 非制造业-指数, 非制造业-同比增长`
pub fn macro_china_pmi() -> Result<Df> {
    const RENAME: [(&str, &str); 5] = [
        ("TIME", "月份"),
        ("MAKE_INDEX", "制造业-指数"),
        ("MAKE_SAME", "制造业-同比增长"),
        ("NMAKE_INDEX", "非制造业-指数"),
        ("NMAKE_SAME", "非制造业-同比增长"),
    ];
    const SELECT: [&str; 5] = [
        "月份",
        "制造业-指数",
        "制造业-同比增长",
        "非制造业-指数",
        "非制造业-同比增长",
    ];
    const NUMERIC: [&str; 4] = [
        "制造业-指数",
        "制造业-同比增长",
        "非制造业-指数",
        "非制造业-同比增长",
    ];
    macro_china_em("RPT_ECONOMY_PMI", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-工业生产者出厂价格指数 PPI（对应 akshare [`akshare.macro_china_ppi`]）。
///
/// 报表 `RPT_ECONOMY_PPI`。# 返回列 `月份, 当月, 当月同比增长, 累计`
pub fn macro_china_ppi() -> Result<Df> {
    const RENAME: [(&str, &str); 4] = [
        ("TIME", "月份"),
        ("BASE", "当月"),
        ("BASE_SAME", "当月同比增长"),
        ("BASE_ACCUMULATE", "累计"),
    ];
    const SELECT: [&str; 4] = ["月份", "当月", "当月同比增长", "累计"];
    const NUMERIC: [&str; 3] = ["当月", "当月同比增长", "累计"];
    macro_china_em("RPT_ECONOMY_PPI", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-存款准备金率（对应 akshare [`akshare.macro_china_reserve_requirement_ratio`]）。
///
/// 报表 `RPT_ECONOMY_DEPOSIT_RESERVE`；`公布时间`/`生效时间` 截断为 `YYYY-MM-DD`。
/// # 返回列 `公布时间, 生效时间, 大型金融机构-调整前, 大型金融机构-调整后,
/// 大型金融机构-调整幅度, 中小金融机构-调整前, 中小金融机构-调整后,
/// 中小金融机构-调整幅度, 消息公布次日指数涨跌-上证, 消息公布次日指数涨跌-深证, 备注`
pub fn macro_china_reserve_requirement_ratio() -> Result<Df> {
    const RENAME: [(&str, &str); 11] = [
        ("PUBLISH_DATE", "公布时间"),
        ("TRADE_DATE", "生效时间"),
        ("INTEREST_RATE_BB", "大型金融机构-调整前"),
        ("INTEREST_RATE_BA", "大型金融机构-调整后"),
        ("CHANGE_RATE_B", "大型金融机构-调整幅度"),
        ("INTEREST_RATE_SB", "中小金融机构-调整前"),
        ("INTEREST_RATE_SA", "中小金融机构-调整后"),
        ("CHANGE_RATE_S", "中小金融机构-调整幅度"),
        ("NEXT_SH_RATE", "消息公布次日指数涨跌-上证"),
        ("NEXT_SZ_RATE", "消息公布次日指数涨跌-深证"),
        ("REMARK", "备注"),
    ];
    const SELECT: [&str; 11] = [
        "公布时间",
        "生效时间",
        "大型金融机构-调整前",
        "大型金融机构-调整后",
        "大型金融机构-调整幅度",
        "中小金融机构-调整前",
        "中小金融机构-调整后",
        "中小金融机构-调整幅度",
        "消息公布次日指数涨跌-上证",
        "消息公布次日指数涨跌-深证",
        "备注",
    ];
    const NUMERIC: [&str; 8] = [
        "大型金融机构-调整前",
        "大型金融机构-调整后",
        "大型金融机构-调整幅度",
        "中小金融机构-调整前",
        "中小金融机构-调整后",
        "中小金融机构-调整幅度",
        "消息公布次日指数涨跌-上证",
        "消息公布次日指数涨跌-深证",
    ];
    macro_china_em(
        "RPT_ECONOMY_DEPOSIT_RESERVE",
        &RENAME,
        &SELECT,
        &NUMERIC,
        &["公布时间", "生效时间"],
    )
}

/// 东方财富-股票市场统计（对应 akshare [`akshare.macro_china_stock_market_cap`]）。
///
/// 报表 `RPT_ECONOMY_STOCK_STATISTICS`；`数据日期` 截断为 `YYYY-MM-DD`。# 返回列
/// `数据日期, 发行总股本-上海, 发行总股本-深圳, 市价总值-上海, 市价总值-深圳,
/// 成交金额-上海, 成交金额-深圳, 成交量-上海, 成交量-深圳,
/// A股最高综合股价指数-上海, A股最高综合股价指数-深圳,
/// A股最低综合股价指数-上海, A股最低综合股价指数-深圳`
pub fn macro_china_stock_market_cap() -> Result<Df> {
    const RENAME: [(&str, &str); 13] = [
        ("TIME", "数据日期"),
        ("TOTAL_SHARES_SH", "发行总股本-上海"),
        ("TOTAL_SZARES_SZ", "发行总股本-深圳"),
        ("TOTAL_MARKE_SH", "市价总值-上海"),
        ("TOTAL_MARKE_SZ", "市价总值-深圳"),
        ("DEAL_AMOUNT_SH", "成交金额-上海"),
        ("DEAL_AMOUNT_SZ", "成交金额-深圳"),
        ("VOLUME_SH", "成交量-上海"),
        ("VOLUME_SZ", "成交量-深圳"),
        ("HIGH_INDEX_SH", "A股最高综合股价指数-上海"),
        ("HIGH_INDEX_SZ", "A股最高综合股价指数-深圳"),
        ("LOW_INDEX_SH", "A股最低综合股价指数-上海"),
        ("LOW_INDEX_SZ", "A股最低综合股价指数-深圳"),
    ];
    const SELECT: [&str; 13] = [
        "数据日期",
        "发行总股本-上海",
        "发行总股本-深圳",
        "市价总值-上海",
        "市价总值-深圳",
        "成交金额-上海",
        "成交金额-深圳",
        "成交量-上海",
        "成交量-深圳",
        "A股最高综合股价指数-上海",
        "A股最高综合股价指数-深圳",
        "A股最低综合股价指数-上海",
        "A股最低综合股价指数-深圳",
    ];
    const NUMERIC: [&str; 12] = [
        "发行总股本-上海",
        "发行总股本-深圳",
        "市价总值-上海",
        "市价总值-深圳",
        "成交金额-上海",
        "成交金额-深圳",
        "成交量-上海",
        "成交量-深圳",
        "A股最高综合股价指数-上海",
        "A股最高综合股价指数-深圳",
        "A股最低综合股价指数-上海",
        "A股最低综合股价指数-深圳",
    ];
    macro_china_em(
        "RPT_ECONOMY_STOCK_STATISTICS",
        &RENAME,
        &SELECT,
        &NUMERIC,
        &["数据日期"],
    )
}

/// 东方财富-外汇存款（对应 akshare [`akshare.macro_china_wbck`]）。
///
/// 报表 `RPT_ECONOMY_FOREX_DEPOSIT`。# 返回列
/// `月份, 当月, 同比增长, 环比增长, 累计`
pub fn macro_china_wbck() -> Result<Df> {
    const RENAME: [(&str, &str); 5] = [
        ("TIME", "月份"),
        ("BASE", "当月"),
        ("BASE_SAME", "同比增长"),
        ("BASE_SEQUENTIAL", "环比增长"),
        ("BASE_ACCUMULATE", "累计"),
    ];
    const SELECT: [&str; 5] = ["月份", "当月", "同比增长", "环比增长", "累计"];
    const NUMERIC: [&str; 4] = ["当月", "同比增长", "环比增长", "累计"];
    macro_china_em("RPT_ECONOMY_FOREX_DEPOSIT", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-外汇贷款（对应 akshare [`akshare.macro_china_whxd`]）。
///
/// 报表 `RPT_ECONOMY_FOREX_LOAN`。# 返回列
/// `月份, 当月, 同比增长, 环比增长, 累计`
pub fn macro_china_whxd() -> Result<Df> {
    const RENAME: [(&str, &str); 5] = [
        ("TIME", "月份"),
        ("BASE", "当月"),
        ("BASE_SAME", "同比增长"),
        ("BASE_SEQUENTIAL", "环比增长"),
        ("BASE_ACCUMULATE", "累计"),
    ];
    const SELECT: [&str; 5] = ["月份", "当月", "同比增长", "环比增长", "累计"];
    const NUMERIC: [&str; 4] = ["当月", "同比增长", "环比增长", "累计"];
    macro_china_em("RPT_ECONOMY_FOREX_LOAN", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 东方财富-消费者信心指数（对应 akshare [`akshare.macro_china_xfzxx`]）。
///
/// 报表 `RPT_ECONOMY_FAITH_INDEX`。# 返回列
/// `月份, 消费者信心指数-指数值, 消费者信心指数-同比增长, 消费者信心指数-环比增长,
/// 消费者满意指数-指数值, 消费者满意指数-同比增长, 消费者满意指数-环比增长,
/// 消费者预期指数-指数值, 消费者预期指数-同比增长, 消费者预期指数-环比增长`
pub fn macro_china_xfzxx() -> Result<Df> {
    const RENAME: [(&str, &str); 10] = [
        ("TIME", "月份"),
        ("CONSUMERS_FAITH_INDEX", "消费者信心指数-指数值"),
        ("FAITH_INDEX_SAME", "消费者信心指数-同比增长"),
        ("FAITH_INDEX_SEQUENTIAL", "消费者信心指数-环比增长"),
        ("CONSUMERS_ASTIS_INDEX", "消费者满意指数-指数值"),
        ("ASTIS_INDEX_SAME", "消费者满意指数-同比增长"),
        ("ASTIS_INDEX_SEQUENTIAL", "消费者满意指数-环比增长"),
        ("CONSUMERS_EXPECT_INDEX", "消费者预期指数-指数值"),
        ("EXPECT_INDEX_SAME", "消费者预期指数-同比增长"),
        ("EXPECT_INDEX_SEQUENTIAL", "消费者预期指数-环比增长"),
    ];
    const SELECT: [&str; 10] = [
        "月份",
        "消费者信心指数-指数值",
        "消费者信心指数-同比增长",
        "消费者信心指数-环比增长",
        "消费者满意指数-指数值",
        "消费者满意指数-同比增长",
        "消费者满意指数-环比增长",
        "消费者预期指数-指数值",
        "消费者预期指数-同比增长",
        "消费者预期指数-环比增长",
    ];
    const NUMERIC: [&str; 9] = [
        "消费者信心指数-指数值",
        "消费者信心指数-同比增长",
        "消费者信心指数-环比增长",
        "消费者满意指数-指数值",
        "消费者满意指数-同比增长",
        "消费者满意指数-环比增长",
        "消费者预期指数-指数值",
        "消费者预期指数-同比增长",
        "消费者预期指数-环比增长",
    ];
    macro_china_em("RPT_ECONOMY_FAITH_INDEX", &RENAME, &SELECT, &NUMERIC, &[])
}

/// 金十 cdn 报表单元格 → `Option<String>`。
fn jin10_cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.trim().to_string()),
        other => Some(other.to_string()),
    }
}

/// 判断字符串行表中某一列是否「全部可解析为数值」（模仿 pandas 对未显式转换列的
/// dtype 自动推断：全数值 → float64，否则保持 str）。空值（`None`）不参与判定。
fn col_all_numeric(rows: &[Vec<Option<String>>], idx: usize) -> bool {
    for r in rows {
        if let Some(s) = r.get(idx).and_then(|x| x.as_ref()) {
            if s.trim().is_empty() {
                continue;
            }
            if s.trim().parse::<f64>().is_err() {
                return false;
            }
        }
    }
    true
}

/// `YYYYMMDD` → `YYYY-MM-DD`（金十能源报告日期为紧凑格式）。
fn ymd8(s: &str) -> String {
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
    } else {
        s.to_string()
    }
}

/// 金十-上海黄金交易所报告（对应 akshare [`akshare.macro_china_au_report`]）。
///
/// `cdn.jin10.com/data_center/reports/sge.json`：`values` 按日期分组、每日多行，
/// 每行 13 字段 `[商品, 开盘价, 最高价, 最低价, 收盘价, 涨跌, 涨跌幅, 加权平均价,
/// 成交量, 成交金额, 持仓量, 交收方向, 交收量]`。# 返回列
/// `日期, 商品, 开盘价, 最高价, 最低价, 收盘价, 涨跌, 涨跌幅, 加权平均价,
/// 成交量, 成交金额, 持仓量, 交收方向, 交收量`
pub fn macro_china_au_report() -> Result<Df> {
    let json = fetch_jin10_cdn("sge.json")?;
    const COLS: [&str; 14] = [
        "日期",
        "商品",
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "涨跌",
        "涨跌幅",
        "加权平均价",
        "成交量",
        "成交金额",
        "持仓量",
        "交收方向",
        "交收量",
    ];
    let values = json
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("金十 sge 报表缺少 values".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (date, v) in values {
        let Some(day_rows) = v.as_array() else {
            continue;
        };
        for r in day_rows {
            let Some(arr) = r.as_array() else {
                continue;
            };
            let mut row: Vec<Option<String>> = Vec::with_capacity(14);
            row.push(Some(date.clone()));
            for i in 0..13 {
                row.push(arr.get(i).and_then(jin10_cell));
            }
            rows.push(row);
        }
    }
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    // akshare 仅显式 `pd.to_numeric(errors="coerce")` 转换 `持仓量`/`交收量`，其余价格列
    // 由 pandas 自动推断 dtype：全数值 → float64，存在非数值（如休市 "-")→ str。
    // 这里复刻该语义：`持仓量`/`交收量` 强制数值，价格列仅在「全部可解析为数值」时转换。
    const FORCE_NUMERIC: [&str; 2] = ["持仓量", "交收量"];
    df.cast_numeric(&FORCE_NUMERIC)?;
    const INFER_NUMERIC: [&str; 8] = [
        "开盘价",
        "最高价",
        "最低价",
        "涨跌",
        "涨跌幅",
        "加权平均价",
        "成交量",
        "成交金额",
    ];
    let mut inferred: Vec<&str> = Vec::with_capacity(INFER_NUMERIC.len());
    for (i, name) in INFER_NUMERIC.iter().enumerate() {
        // `rows` 列序与 `COLS` 对齐：`INFER_NUMERIC` 起始于 `COLS` 下标 2。
        if col_all_numeric(&rows, 2 + i) {
            inferred.push(name);
        }
    }
    df.cast_numeric(&inferred)?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 金十-人民币汇率中间价报告（对应 akshare [`akshare.macro_china_rmb`]）。
///
/// `cdn.jin10.com/data_center/reports/exchange_rate.json`：`values` 按日期分组，
/// 每日期下 `{币种: [中间价, 涨跌幅]}`。输出列序 `日期` + 各币种 `_中间价`/`_涨跌幅`。
pub fn macro_china_rmb() -> Result<Df> {
    let json = fetch_jin10_cdn("exchange_rate.json")?;
    let values = json
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("金十 rmb 报表缺少 values".into()))?;
    let products = json
        .get("products")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("金十 rmb 报表缺少 products".into()))?;
    let mut cols: Vec<String> = vec!["日期".to_string()];
    for p in products {
        if let Some(n) = p.as_str() {
            cols.push(format!("{n}_中间价"));
            cols.push(format!("{n}_涨跌幅"));
        }
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (date, entry) in values {
        let obj = entry.as_object();
        let mut row: Vec<Option<String>> = vec![Some(date.clone())];
        for p in products {
            if let Some(n) = p.as_str() {
                let pair = obj.and_then(|o| o.get(n)).and_then(Value::as_array);
                row.push(pair.and_then(|a| a.first()).and_then(jin10_cell));
                row.push(pair.and_then(|a| a.get(1)).and_then(jin10_cell));
            }
        }
        rows.push(row);
    }
    let col_refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
    let mut df = Df::from_string_rows(&col_refs, &rows)?;
    let numeric: Vec<&str> = col_refs[1..].to_vec();
    df.cast_numeric(&numeric)?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 金十-上海银行业同业拆借报告（对应 akshare [`akshare.macro_china_shibor_all`]）。
///
/// `cdn.jin10.com/data_center/reports/il_1.json`：`values` 按日期分组，每日期下
/// `{期限: [定价, 涨跌幅]}`。输出列序 `日期` + 各期限 `-定价`/`-涨跌幅`。
pub fn macro_china_shibor_all() -> Result<Df> {
    let json = fetch_jin10_cdn("il_1.json")?;
    let values = json
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("金十 shibor 报表缺少 values".into()))?;
    let products = json
        .get("products")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("金十 shibor 报表缺少 products".into()))?;
    let mut cols: Vec<String> = vec!["日期".to_string()];
    for p in products {
        if let Some(n) = p.as_str() {
            cols.push(format!("{n}-定价"));
            cols.push(format!("{n}-涨跌幅"));
        }
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (date, entry) in values {
        let obj = entry.as_object();
        let mut row: Vec<Option<String>> = vec![Some(date.clone())];
        for p in products {
            if let Some(n) = p.as_str() {
                let pair = obj.and_then(|o| o.get(n)).and_then(Value::as_array);
                row.push(pair.and_then(|a| a.first()).and_then(jin10_cell));
                row.push(pair.and_then(|a| a.get(1)).and_then(jin10_cell));
            }
        }
        rows.push(row);
    }
    let col_refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
    let mut df = Df::from_string_rows(&col_refs, &rows)?;
    let numeric: Vec<&str> = col_refs[1..].to_vec();
    df.cast_numeric(&numeric)?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 金十-香港同业拆借报告（对应 akshare [`akshare.macro_china_hk_market_info`]）。
///
/// `cdn.jin10.com/data_center/reports/il_2.json`：`values` 按日期分组，每日期下
/// `{期限: [定价, 涨跌幅]}`；列序按 akshare 固定为 `1W,2W,1M,3M,6M,1Y,ON,2M`。
pub fn macro_china_hk_market_info() -> Result<Df> {
    const PRODUCTS: [&str; 8] = ["1W", "2W", "1M", "3M", "6M", "1Y", "ON", "2M"];
    let json = fetch_jin10_cdn("il_2.json")?;
    let values = json
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("金十 hk_market_info 报表缺少 values".into()))?;
    let mut cols: Vec<String> = vec!["日期".to_string()];
    for p in PRODUCTS {
        cols.push(format!("{p}-定价"));
        cols.push(format!("{p}-涨跌幅"));
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (date, entry) in values {
        let obj = entry.as_object();
        let mut row: Vec<Option<String>> = vec![Some(date.clone())];
        for p in PRODUCTS {
            let pair = obj.and_then(|o| o.get(p)).and_then(Value::as_array);
            row.push(pair.and_then(|a| a.first()).and_then(jin10_cell));
            row.push(pair.and_then(|a| a.get(1)).and_then(jin10_cell));
        }
        rows.push(row);
    }
    let col_refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
    let mut df = Df::from_string_rows(&col_refs, &rows)?;
    let numeric: Vec<&str> = col_refs[1..].to_vec();
    df.cast_numeric(&numeric)?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 金十-上海融资融券报告（对应 akshare [`akshare.macro_china_market_margin_sh`]）。
///
/// `cdn.jin10.com/data_center/reports/fs_1.json`：`values` 按日期分组，每日一行
/// 6 字段 `[融资买入额, 融资余额, 融券卖出量, 融券余量, 融券余额, 融资融券余额]`。
pub fn macro_china_market_margin_sh() -> Result<Df> {
    const COLS: [&str; 7] = [
        "日期",
        "融资买入额",
        "融资余额",
        "融券卖出量",
        "融券余量",
        "融券余额",
        "融资融券余额",
    ];
    let json = fetch_jin10_cdn("fs_1.json")?;
    let values = json
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("金十 fs_1 报表缺少 values".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (date, v) in values {
        let mut row: Vec<Option<String>> = vec![Some(date.clone())];
        if let Some(a) = v.as_array() {
            for i in 0..6 {
                row.push(a.get(i).and_then(jin10_cell));
            }
        }
        rows.push(row);
    }
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_numeric(&COLS[1..])?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 金十-深圳融资融券报告（对应 akshare [`akshare.macro_china_market_margin_sz`]）。
///
/// `cdn.jin10.com/data_center/reports/fs_2.json`：结构同 `fs_1.json`（`日期` 为键）。
pub fn macro_china_market_margin_sz() -> Result<Df> {
    const COLS: [&str; 7] = [
        "日期",
        "融资买入额",
        "融资余额",
        "融券卖出量",
        "融券余量",
        "融券余额",
        "融资融券余额",
    ];
    let json = fetch_jin10_cdn("fs_2.json")?;
    let values = json
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("金十 fs_2 报表缺少 values".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (date, v) in values {
        let mut row: Vec<Option<String>> = vec![Some(date.clone())];
        if let Some(a) = v.as_array() {
            for i in 0..6 {
                row.push(a.get(i).and_then(jin10_cell));
            }
        }
        rows.push(row);
    }
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_numeric(&COLS[1..])?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 金十-中国日度沿海六大电库存（对应 akshare [`akshare.macro_china_daily_energy`]）。
///
/// `cdn.jin10.com/dc/reports/dc_qihuo_energy_report_all.js`（`datas` 键为
/// `沿海六大电厂库存动态报告`，值为 `[库存, 日耗, 存煤可用天数]` 三项的字符串数组；
/// `date` 为 `YYYYMMDD` 紧凑格式）。数据区间自 20160101 至今。
///
/// # 返回列
/// `日期, 沿海六大电库存, 日耗, 存煤可用天数`
pub fn macro_china_daily_energy() -> Result<Df> {
    const COLS: [&str; 4] = ["日期", "沿海六大电库存", "日耗", "存煤可用天数"];
    let text = fetch_jin10_cdn_text("dc_qihuo_energy_report_all.js")?;
    let json: Value = serde_json::from_str(&text)
        .map_err(|e| AkshareError::json("金十能源日报", format!("JSON 解析失败: {e}")))?;
    let list = json
        .get("list")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("金十能源日报缺少 list".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(list.len());
    for item in list {
        let date = item
            .get("date")
            .and_then(Value::as_str)
            .ok_or_else(|| AkshareError::Empty("金十能源日报条目缺少 date".into()))?;
        let datas = item
            .get("datas")
            .and_then(Value::as_object)
            .and_then(|m| m.get("沿海六大电厂库存动态报告"))
            .and_then(Value::as_array)
            .ok_or_else(|| AkshareError::Empty("金十能源日报条目缺少 datas".into()))?;
        let mut row: Vec<Option<String>> = vec![Some(ymd8(date))];
        for i in 0..3 {
            row.push(datas.get(i).and_then(jin10_cell));
        }
        rows.push(row);
    }
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_numeric(&COLS[1..])?;
    df.cast_date(&["日期"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}
