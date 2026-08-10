//! 股票特色数据接口（对应 akshare `stock_feature/*`）。
//!
//! 本期实现东财系接口：实时行情快照（push2 clist）+ 数据中心 `RPT_*` 报表
//! （datacenter-web）。复用 `sources::eastmoney` 的 `fetch_clist` /
//! `fetch_datacenter_pages` 与 `finalize_spot` / `finalize_report` 工具，
//! 列名与 akshare 逐字对齐（离线单测 + 与 Python akshare 实测列序核对）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    fetch_clist, fetch_datacenter_pages, finalize_report, finalize_spot, push2_urls,
};
use serde_json::{json, Map, Value};

/// 沪深京 A 股实时行情公共字段串（与 akshare `stock_zh_a_spot_em` 一致）。
const SPOT_FIELDS: &str = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152";
/// 港股实时行情字段串（与 akshare `stock_hk_spot_em` 一致）。
const HK_SPOT_FIELDS: &str = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152";

/// A 股快照列重命名（f-字段码 → 中文），含序号列。
const SPOT_RENAME: [(&str, &str); 23] = [
    ("index", "序号"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f5", "成交量"),
    ("f6", "成交额"),
    ("f7", "振幅"),
    ("f8", "换手率"),
    ("f9", "市盈率-动态"),
    ("f10", "量比"),
    ("f11", "5分钟涨跌"),
    ("f12", "代码"),
    ("f14", "名称"),
    ("f15", "最高"),
    ("f16", "最低"),
    ("f17", "今开"),
    ("f18", "昨收"),
    ("f20", "总市值"),
    ("f21", "流通市值"),
    ("f22", "60日涨跌幅"),
    ("f23", "年初至今涨跌幅"),
    ("f24", "涨速"),
    ("f25", "市净率"),
];
/// A 股快照输出列顺序。
const SPOT_SELECT: [&str; 23] = [
    "序号", "代码", "名称", "最新价", "涨跌幅", "涨跌额", "成交量", "成交额", "振幅", "最高", "最低",
    "今开", "昨收", "量比", "换手率", "市盈率-动态", "市净率", "总市值", "流通市值", "涨速", "5分钟涨跌",
    "60日涨跌幅", "年初至今涨跌幅",
];
/// A 股快照数值列。
const SPOT_NUMERIC: [&str; 20] = [
    "最新价", "涨跌幅", "涨跌额", "成交量", "成交额", "振幅", "最高", "最低", "今开", "昨收", "量比",
    "换手率", "市盈率-动态", "市净率", "总市值", "流通市值", "涨速", "5分钟涨跌", "60日涨跌幅",
    "年初至今涨跌幅",
];

/// 新股快照：在标准 A 股快照基础上增加「上市日期」(`f26`)。
const NEW_A_RENAME: [(&str, &str); 24] = [
    ("index", "序号"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f5", "成交量"),
    ("f6", "成交额"),
    ("f7", "振幅"),
    ("f8", "换手率"),
    ("f9", "市盈率-动态"),
    ("f10", "量比"),
    ("f11", "5分钟涨跌"),
    ("f12", "代码"),
    ("f14", "名称"),
    ("f15", "最高"),
    ("f16", "最低"),
    ("f17", "今开"),
    ("f18", "昨收"),
    ("f20", "总市值"),
    ("f21", "流通市值"),
    ("f22", "60日涨跌幅"),
    ("f23", "年初至今涨跌幅"),
    ("f24", "涨速"),
    ("f25", "市净率"),
    ("f26", "上市日期"),
];
/// 新股快照输出列顺序（上市日期在标准快照的「市净率」之后）。
const NEW_A_SELECT: [&str; 24] = [
    "序号", "代码", "名称", "最新价", "涨跌幅", "涨跌额", "成交量", "成交额", "振幅", "最高", "最低",
    "今开", "昨收", "量比", "换手率", "市盈率-动态", "市净率", "上市日期", "总市值", "流通市值", "涨速",
    "5分钟涨跌", "60日涨跌幅", "年初至今涨跌幅",
];
/// 新股快照数值列（上市日期为日期，不数值化）。
const NEW_A_NUMERIC: [&str; 20] = SPOT_NUMERIC;

/// 港股快照列重命名（f-字段码 → 中文），含序号列。
const HK_RENAME: [(&str, &str); 17] = [
    ("index", "序号"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f5", "成交量"),
    ("f6", "成交额"),
    ("f7", "振幅"),
    ("f8", "换手率"),
    ("f9", "市盈率-动态"),
    ("f10", "量比"),
    ("f12", "代码"),
    ("f14", "名称"),
    ("f15", "最高"),
    ("f16", "最低"),
    ("f17", "今开"),
    ("f18", "昨收"),
    ("f25", "市净率"),
];
/// 港股快照输出列顺序（与 akshare `stock_hk_spot_em` 一致）。
const HK_SELECT: [&str; 12] = [
    "序号", "代码", "名称", "最新价", "涨跌额", "涨跌幅", "今开", "最高", "最低", "昨收", "成交量",
    "成交额",
];
/// 港股快照数值列。
const HK_NUMERIC: [&str; 9] = [
    "最新价", "涨跌额", "涨跌幅", "今开", "最高", "最低", "昨收", "成交量", "成交额",
];

/// 股东户数（`RPT_HOLDERNUMLATEST` / `RPT_HOLDERNUM_DET`）字段键 → 中文列名。
const GDHS_RENAME: [(&str, &str); 16] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("END_DATE", "股东户数统计截止日-本次"),
    ("INTERVAL_CHRATE", "区间涨跌幅"),
    ("AVG_MARKET_CAP", "户均持股市值"),
    ("AVG_HOLD_NUM", "户均持股数量"),
    ("TOTAL_MARKET_CAP", "总市值"),
    ("TOTAL_A_SHARES", "总股本"),
    ("HOLD_NOTICE_DATE", "公告日期"),
    ("HOLDER_NUM", "股东户数-本次"),
    ("PRE_HOLDER_NUM", "股东户数-上次"),
    ("HOLDER_NUM_CHANGE", "股东户数-增减"),
    ("HOLDER_NUM_RATIO", "股东户数-增减比例"),
    ("PRE_END_DATE", "股东户数统计截止日-上次"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
];
/// 股东户数输出列顺序（与 akshare `stock_zh_a_gdhs` 一致）。
const GDHS_SELECT: [&str; 16] = [
    "代码", "名称", "最新价", "涨跌幅", "股东户数-本次", "股东户数-上次", "股东户数-增减",
    "股东户数-增减比例", "区间涨跌幅", "股东户数统计截止日-本次", "股东户数统计截止日-上次",
    "户均持股市值", "户均持股数量", "总市值", "总股本", "公告日期",
];
/// 股东户数数值列（日期列单独 `cast_date`）。
const GDHS_NUMERIC: [&str; 11] = [
    "最新价", "涨跌幅", "股东户数-本次", "股东户数-上次", "股东户数-增减", "股东户数-增减比例",
    "区间涨跌幅", "户均持股市值", "户均持股数量", "总市值", "总股本",
];
/// 股东户数日期列。
const GDHS_DATE: [&str; 3] = [
    "股东户数统计截止日-本次",
    "股东户数统计截止日-上次",
    "公告日期",
];

/// 构造 push2 clist 公共参数（pn/pz/po/np/fltt/invt 固定）。
fn clist_params(fs: &str, fid: &str, fields: &str) -> Map<String, Value> {
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2", "invt": "2", "fid": fid,
        "fs": fs, "fields": fields,
    });
    params.as_object().cloned().unwrap_or_default()
}

/// 创业板实时行情（对应 akshare [`akshare.stock_cy_a_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率, 总市值, 流通市值, 涨速,
/// 5分钟涨跌, 60日涨跌幅, 年初至今涨跌幅`
pub fn stock_cy_a_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &clist_params("m:0 t:80", "f12", SPOT_FIELDS))?;
    finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC)
}

/// 科创板实时行情（对应 akshare [`akshare.stock_kc_a_spot_em`]）。
///
/// # 返回列
/// 与 [`stock_cy_a_spot_em`] 一致。
pub fn stock_kc_a_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &clist_params("m:1 t:23", "f12", SPOT_FIELDS))?;
    finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC)
}

/// B 股实时行情（对应 akshare [`akshare.stock_zh_b_spot_em`]）。
///
/// # 返回列
/// 与 [`stock_cy_a_spot_em`] 一致。
pub fn stock_zh_b_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &clist_params("m:0 t:7,m:1 t:3", "f12", SPOT_FIELDS))?;
    finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC)
}

/// 新股实时行情（对应 akshare [`akshare.stock_new_a_spot_em`]）。
///
/// 在标准 A 股快照基础上增加「上市日期」列。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率, 上市日期, 总市值, 流通市值, 涨速,
/// 5分钟涨跌, 60日涨跌幅, 年初至今涨跌幅`
pub fn stock_new_a_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &clist_params("m:0 f:8,m:1 f:8", "f26", SPOT_FIELDS))?;
    finalize_spot(df, &NEW_A_RENAME, &NEW_A_SELECT, &NEW_A_NUMERIC)
}

/// 港股主板实时行情（对应 akshare [`akshare.stock_hk_main_board_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额`
pub fn stock_hk_main_board_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &clist_params("m:128 t:3", "f12", HK_SPOT_FIELDS))?;
    finalize_spot(df, &HK_RENAME, &HK_SELECT, &HK_NUMERIC)
}

/// 港股通成份股（对应 akshare [`akshare.stock_hk_ggt_components_em`]）。
///
/// # 返回列
/// 与 [`stock_hk_main_board_spot_em`] 一致。
pub fn stock_hk_ggt_components_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &clist_params("b:DLMK0146,b:DLMK0144", "f12", HK_SPOT_FIELDS))?;
    finalize_spot(df, &HK_RENAME, &HK_SELECT, &HK_NUMERIC)
}

/// 股东户数（对应 akshare [`akshare.stock_zh_a_gdhs`]）。
///
/// `symbol`：`"最新"` 或季度末日期 `YYYYMMDD`（如 `"20230930"`）。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌幅, 股东户数-本次, 股东户数-上次, 股东户数-增减,
/// 股东户数-增减比例, 区间涨跌幅, 股东户数统计截止日-本次, 股东户数统计截止日-上次,
/// 户均持股市值, 户均持股数量, 总市值, 总股本, 公告日期`
pub fn stock_zh_a_gdhs(symbol: &str) -> Result<Df> {
    let (report_name, filter) = if symbol == "最新" {
        ("RPT_HOLDERNUMLATEST", None)
    } else {
        if symbol.len() != 8 || !symbol.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 '最新' 或 YYYYMMDD）"
            )));
        }
        let d = format!("{}-{}-{}", &symbol[..4], &symbol[4..6], &symbol[6..]);
        ("RPT_HOLDERNUM_DET", Some(format!("(END_DATE='{d}')")))
    };
    // 注意：akshare 原版 `columns` 含重复的 `END_DATE`（服务端归一化为 `PRE_END_DATE`），
    // 必须原样发送；`f2,f3` 仅出现在 `quoteColumns`（见 `extra`），不可并入 `columns`，
    // 否则 datacenter 报 `f2返回字段不存在`（code 9501）。
    let columns = "SECURITY_CODE,SECURITY_NAME_ABBR,END_DATE,INTERVAL_CHRATE,AVG_MARKET_CAP,AVG_HOLD_NUM,TOTAL_MARKET_CAP,TOTAL_A_SHARES,HOLD_NOTICE_DATE,HOLDER_NUM,PRE_HOLDER_NUM,HOLDER_NUM_CHANGE,HOLDER_NUM_RATIO,END_DATE,PRE_END_DATE";
    let mut extra = Map::new();
    extra.insert("sortColumns".into(), json!("HOLD_NOTICE_DATE,SECURITY_CODE"));
    extra.insert("sortTypes".into(), json!("-1,-1"));
    if let Some(f) = filter {
        extra.insert("filter".into(), Value::String(f));
    }
    extra.insert("quoteColumns".into(), json!("f2,f3"));
    let http = HttpClient::default();
    let rows = fetch_datacenter_pages(&http, report_name, columns, &extra, "500")?;
    let mut df = finalize_report(&rows, &GDHS_RENAME, &GDHS_SELECT, &GDHS_NUMERIC)?;
    df.cast_date(&GDHS_DATE)?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 离线验证 A 股快照列契约（与 akshare 输出一致）。
    #[test]
    fn spot_contract_offline() {
        let rows = json!([
            {"f2":"10.5","f3":"9.9","f4":"0.9","f5":"100000","f6":"1050000","f7":"3.2",
             "f8":"0.5","f9":"8.1","f10":"1.2","f11":"0.1","f12":"000001","f14":"平安银行",
             "f15":"10.8","f16":"10.2","f17":"10.3","f18":"9.6","f20":"200000000000",
             "f21":"180000000000","f22":"5.0","f23":"12.0","f24":"0.05","f25":"0.9"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = crate::sources::eastmoney::finalize_clist(rows).unwrap();
        let df = finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC).unwrap();
        assert_eq!(df.column_names(), SPOT_SELECT);
        assert_eq!(df.height(), 1);
        let px = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(10.5));
        let pct = df.inner().column("涨跌幅").unwrap().f64().unwrap();
        assert_eq!(pct.get(0), Some(9.9));
    }

    /// 离线验证新股快照列契约（含上市日期）。
    #[test]
    fn new_a_contract_offline() {
        let rows = json!([
            {"f2":"10.5","f3":"9.9","f4":"0.9","f5":"100000","f6":"1050000","f7":"3.2",
             "f8":"0.5","f9":"8.1","f10":"1.2","f11":"0.1","f12":"001234","f14":"新股",
             "f15":"10.8","f16":"10.2","f17":"10.3","f18":"9.6","f20":"200000000000",
             "f21":"180000000000","f22":"5.0","f23":"12.0","f24":"0.05","f25":"0.9","f26":"20240101"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = crate::sources::eastmoney::finalize_clist(rows).unwrap();
        let df = finalize_spot(df, &NEW_A_RENAME, &NEW_A_SELECT, &NEW_A_NUMERIC).unwrap();
        assert_eq!(df.column_names(), NEW_A_SELECT);
        let list = df.inner().column("上市日期").unwrap().str().unwrap();
        assert_eq!(list.get(0), Some("20240101"));
    }

    /// 离线验证港股快照列契约。
    #[test]
    fn hk_spot_contract_offline() {
        let rows = json!([
            {"f2":"100.0","f3":"2.0","f4":"2.0","f5":"50000","f6":"5000000","f7":"1.5",
             "f8":"0.3","f9":"12.0","f10":"0.8","f12":"00700","f14":"腾讯控股",
             "f15":"101.0","f16":"99.0","f17":"99.5","f18":"98.0","f25":"3.0"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = crate::sources::eastmoney::finalize_clist(rows).unwrap();
        let df = finalize_spot(df, &HK_RENAME, &HK_SELECT, &HK_NUMERIC).unwrap();
        assert_eq!(df.column_names(), HK_SELECT);
        assert_eq!(df.height(), 1);
    }

    /// 离线验证股东户数报表列契约 + 数值化 + 日期归一化。
    #[test]
    fn gdhs_report_offline() {
        let rows = json!([{
            "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
            "END_DATE":"2023-09-30T00:00:00","INTERVAL_CHRATE":"-1.23",
            "AVG_MARKET_CAP":"123456.78","AVG_HOLD_NUM":"5000",
            "TOTAL_MARKET_CAP":"200000000000","TOTAL_A_SHARES":"190000000000",
            "HOLD_NOTICE_DATE":"2023-10-01T00:00:00","HOLDER_NUM":"120000",
            "PRE_HOLDER_NUM":"130000","HOLDER_NUM_CHANGE":"-10000",
            "HOLDER_NUM_RATIO":"-7.69","PRE_END_DATE":"2023-06-30T00:00:00",
            "f2":"10.5","f3":"1.2"
        }]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(&rows, &GDHS_RENAME, &GDHS_SELECT, &GDHS_NUMERIC).unwrap();
        df.cast_date(&GDHS_DATE).unwrap();
        assert_eq!(df.column_names(), GDHS_SELECT);
        assert_eq!(df.height(), 1);
        // 数值列已转 f64
        let holder = df.inner().column("股东户数-本次").unwrap().f64().unwrap();
        assert_eq!(holder.get(0), Some(120000.0));
        let chg = df.inner().column("股东户数-增减比例").unwrap().f64().unwrap();
        assert_eq!(chg.get(0), Some(-7.69));
        // 日期列归一化为 YYYY-MM-DD
        let d = df.inner().column("股东户数统计截止日-本次").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2023-09-30"));
        let notice = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2023-10-01"));
    }

    /// 错误路径：非法 symbol 应明确报错（合法 YYYYMMDD 格式即使月份无效也交由服务端返回空表）。
    #[test]
    fn gdhs_invalid_symbol() {
        assert!(stock_zh_a_gdhs("2023-09-30").is_err());
        assert!(stock_zh_a_gdhs("abc").is_err());
        assert!(stock_zh_a_gdhs("202399").is_err());
    }

    /// 真实网络冒烟：拉取实时列契约，与 akshare 实测列序核对（需联网，默认忽略）。
    /// 东财 push2 对本机 IP 偶发 TLS 重置（与 akshare Python 同样受影响），故不 `expect`，
    /// 仅打印结果以便人工核对。
    #[test]
    #[ignore]
    fn live_columns_smoke() {
        match stock_cy_a_spot_em() {
            Ok(df) => {
                println!("CY cols={:?} rows={}", df.column_names(), df.height());
                assert_eq!(df.column_names(), SPOT_SELECT);
            }
            Err(e) => println!("CY spot network/blocked: {e}"),
        }
        match stock_zh_a_gdhs("最新") {
            Ok(df) => {
                println!("GDHS cols={:?} rows={}", df.column_names(), df.height());
                assert_eq!(df.column_names(), GDHS_SELECT);
            }
            Err(e) => println!("GDHS network/blocked: {e}"),
        }
    }
}
