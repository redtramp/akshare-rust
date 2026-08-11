//! 宏观经济数据（对应 akshare `economic/` 目录）。
//!
//! 首批实现：金十数据中心-中国宏观指标 14 个（对应 akshare `economic/macro_china.py`），
//! 全部走 `sources::jin10::macro_china_base`（`datacenter-api.jin10.com/reports/list_v2`，
//! 按 `attr_id` 分指标翻页抓取）。输出统一 5 列：
//! `商品, 日期, 今值, 预测值, 前值`（商品 = 报表名，日期 `YYYY-MM-DD` 升序，
//! 今值/预测值/前值为数值列，缺失为 None）。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::sources::jin10::macro_china_base;

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
