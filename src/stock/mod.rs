//! 股票数据接口。
//!
//! 首批实现（对应 akshare `stock_feature/stock_hist_em.py`）：
//! - [`stock_zh_a_hist`]：A 股历史行情（日/周/月，支持复权）
//! - [`stock_zh_a_spot_em`]：A 股实时行情（分页全量）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    a_share_market_code, board_name_pairs, fetch_clist, fetch_datacenter_pages, fetch_kline,
    fetch_kline_ext, fetch_kline_min, fetch_securities_pages, fetch_trends, finalize_board_cons,
    finalize_board_name, finalize_fflow, finalize_hsgt, finalize_report, finalize_zt_pool,
    finalize_zt_pool_dtgc, finalize_zt_pool_previous, finalize_zt_pool_strong,
    finalize_zt_pool_sub_new, finalize_zt_pool_zbgc, json_value_to_string, kline_to_df,
    min_kline_to_df, push2_urls, require_kline_data, BOARD_HIST_SELECT, KLINE_COLS,
    KLINE_COLS_WITH_SYMBOL, UT_CLIST, UT_KLINE, ZT_POOL_DTGC_SELECT, ZT_POOL_PREVIOUS_SELECT,
    ZT_POOL_SELECT, ZT_POOL_STRONG_SELECT, ZT_POOL_SUB_NEW_SELECT, ZT_POOL_ZBGC_SELECT,
};
use crate::stock_feature::{datacenter, report_extra};
use scraper::{Html, Selector};
use serde_json::{json, Map, Value};

/// 东财 A 股历史行情。
///
/// 对应 akshare [`akshare.stock_zh_a_hist`]。
///
/// # 参数
/// - `symbol`: 股票代码，如 `"000001"`
/// - `period`: `daily`/`weekly`/`monthly`
/// - `start_date`/`end_date`: `YYYYMMDD`
/// - `adjust`: `""`/`"qfq"`（前复权）/`"hfq"`（后复权）
///
/// # 返回列
/// `日期, 股票代码, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 振幅, 涨跌幅, 涨跌额, 换手率`
pub fn stock_zh_a_hist(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
    adjust: &str,
) -> Result<Df> {
    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        _ => return Err(AkshareError::Param(format!("无效 period: {period}"))),
    };
    let fqt = match adjust {
        "" => "0",
        "qfq" => "1",
        "hfq" => "2",
        _ => return Err(AkshareError::Param(format!("无效 adjust: {adjust}"))),
    };

    let secid = format!("{}.{}", a_share_market_code(symbol), symbol);
    let http = HttpClient::default();
    let klines = fetch_kline(&http, &secid, klt, fqt, start_date, end_date)?;
    require_kline_data(&klines, symbol)?;

    let symbol_col: Vec<String> = vec![symbol.to_string(); klines.len()];
    kline_to_df(
        &KLINE_COLS_WITH_SYMBOL,
        &klines,
        Some(("股票代码", symbol_col)),
    )
}

/// 沪深京 A 股实时行情（对应 akshare [`akshare.stock_zh_a_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低, 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率, 总市值, 流通市值, 涨速, 5分钟涨跌, 60日涨跌幅, 年初至今涨跌幅`
pub fn stock_zh_a_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "fid": "f12",
        "fs": "m:0 t:6,m:0 t:80,m:1 t:2,m:1 t:23,m:0 t:81 s:2048",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let http = HttpClient::default();
    spot_common_transform(fetch_clist(&http, &urls, &params)?)
}

/// 沪 A 股实时行情（对应 akshare [`akshare.stock_sh_a_spot_em`]）。
pub fn stock_sh_a_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "fid": "f12",
        "fs": "m:1 t:2,m:1 t:23",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    spot_common_transform(fetch_clist(&http, &urls, &params)?)
}

/// 深 A 股实时行情（对应 akshare [`akshare.stock_sz_a_spot_em`]）。
pub fn stock_sz_a_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "fid": "f12",
        "fs": "m:0 t:6,m:0 t:80",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    spot_common_transform(fetch_clist(&http, &urls, &params)?)
}

/// 京 A 股实时行情（对应 akshare [`akshare.stock_bj_a_spot_em`]）。
pub fn stock_bj_a_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "fid": "f12",
        "fs": "m:0 t:81 s:2048",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    spot_common_transform(fetch_clist(&http, &urls, &params)?)
}

/// A 股实时行情公共列处理（列序逐字对齐 akshare spot_em 的最终 select 顺序）。
fn spot_common_transform(df: Df) -> Result<Df> {
    let rename = [
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
    let select = [
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
        "总市值",
        "流通市值",
        "涨速",
        "5分钟涨跌",
        "60日涨跌幅",
        "年初至今涨跌幅",
    ];
    let numeric = [
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
        "总市值",
        "流通市值",
        "涨速",
        "5分钟涨跌",
        "60日涨跌幅",
        "年初至今涨跌幅",
    ];
    crate::sources::eastmoney::finalize_spot(df, &rename, &select, &numeric)
}

/// ETF 历史行情（对应 akshare [`akshare.fund_etf_hist_em`]，本模块便于统一入口）。
pub fn fund_etf_hist_em(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
    adjust: &str,
) -> Result<Df> {
    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        _ => return Err(AkshareError::Param(format!("无效 period: {period}"))),
    };
    let fqt = match adjust {
        "" => "0",
        "qfq" => "1",
        "hfq" => "2",
        _ => return Err(AkshareError::Param(format!("无效 adjust: {adjust}"))),
    };
    let market = crate::sources::eastmoney::etf_market_id(symbol);
    let secid = format!("{market}.{symbol}");
    let http = HttpClient::default();
    let klines = fetch_kline(&http, &secid, klt, fqt, start_date, end_date)?;
    require_kline_data(&klines, symbol)?;
    kline_to_df(&KLINE_COLS, &klines, None)
}

/// A 股分钟级行情（对应 akshare [`akshare.stock_zh_a_hist_min_em`]）。
///
/// # 参数
/// - `symbol`: 股票代码
/// - `start_date`/`end_date`: `YYYY-MM-DD HH:MM:SS` 区间（含边界）
/// - `period`: `"1"`（当日分时）或 `"5"`/`"15"`/`"30"`/`"60"`（分钟 K 线）
/// - `adjust`: `""`/`"qfq"`（前复权）/`"hfq"`（后复权）
///
/// # 返回列
/// period=1: `时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 均价`；
/// 其余: `时间, 开盘, 收盘, 最高, 最低, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 换手率`
///
/// 注：akshare 对非 `"1"` 的 period 值直接透传给服务端 `klt` 参数；本实现额外校验
/// 仅接受 `"5"`/`"15"`/`"30"`/`"60"`（更早暴露参数错误，行为不偏离合法取值）。
/// 分钟级数据为滚动窗口（约最近 8 个月），较早日期的查询会得到空表（与 akshare 一致）。
pub fn stock_zh_a_hist_min_em(
    symbol: &str,
    start_date: &str,
    end_date: &str,
    period: &str,
    adjust: &str,
) -> Result<Df> {
    let secid = format!("{}.{}", a_share_market_code(symbol), symbol);
    let http = HttpClient::default();

    if period == "1" {
        let lines = fetch_trends(&http, &secid, "5", "0")?;
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
        if !matches!(period, "5" | "15" | "30" | "60") {
            return Err(AkshareError::Param(format!("无效 period: {period}")));
        }
        let fqt = match adjust {
            "" => "0",
            "qfq" => "1",
            "hfq" => "2",
            _ => return Err(AkshareError::Param(format!("无效 adjust: {adjust}"))),
        };
        let lines = fetch_kline_min(&http, &secid, period, fqt)?;
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
    }
}

/// 个股信息（对应 akshare [`akshare.stock_individual_info_em`]）。
///
/// # 返回列
/// `item, value`；行（按服务端键序）：股票代码, 股票简称, 总股本, 流通股, 行业,
/// 总市值, 流通市值, 上市时间, 最新
pub fn stock_individual_info_em(symbol: &str) -> Result<Df> {
    let secid = format!("{}.{}", a_share_market_code(symbol), symbol);
    let urls = push2_urls("/api/qt/stock/get");
    let params = json!({
        "fltt": "2",
        "invt": "2",
        "fields": "f120,f121,f122,f174,f175,f59,f163,f43,f57,f58,f169,f170,f46,f44,f51,f168,f47,f164,f116,f60,f45,f52,f50,f48,f167,f117,f71,f161,f49,f530,f135,f136,f137,f138,f139,f141,f142,f144,f145,f147,f148,f140,f143,f146,f149,f55,f62,f162,f92,f173,f104,f105,f84,f85,f183,f184,f185,f186,f187,f188,f189,f190,f191,f192,f107,f111,f86,f177,f78,f110,f262,f263,f264,f267,f268,f255,f256,f257,f258,f127,f199,f128,f198,f259,f260,f261,f171,f277,f278,f279,f288,f152,f250,f251,f252,f253,f254,f269,f270,f271,f272,f273,f274,f275,f276,f265,f266,f289,f290,f286,f285,f292,f293,f294,f295,f43",
        "secid": secid,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_any(&urls, &params, None)?;
    let data = value.get("data").cloned().unwrap_or(Value::Null);
    let Some(obj) = data.as_object() else {
        return Err(AkshareError::empty(format!("{symbol} 无个股信息数据")));
    };

    // 对应 akshare code_name_map；行序 = 服务端 data 键序（与其 pandas 展开一致）
    let map: &[(&str, &str)] = &[
        ("f57", "股票代码"),
        ("f58", "股票简称"),
        ("f84", "总股本"),
        ("f85", "流通股"),
        ("f127", "行业"),
        ("f116", "总市值"),
        ("f117", "流通市值"),
        ("f189", "上市时间"),
        ("f43", "最新"),
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (k, v) in obj {
        if let Some((_, name)) = map.iter().find(|(f, _)| f == k) {
            rows.push(vec![Some((*name).to_string()), json_value_to_string(v)]);
        }
    }
    Df::from_string_rows(&["item", "value"], &rows)
}

/// 五档盘口报价（对应 akshare [`akshare.stock_bid_ask_em`]）。
///
/// # 返回列
/// `item, value`；行序固定：`sell_5..sell_1, buy_1..buy_5, 最新, 均价, 涨幅, 涨跌,
/// 总手, 金额, 换手, 量比, 最高, 最低, 今开, 昨收, 涨停, 跌停, 外盘, 内盘`
/// （盘口量字段 ×100，对应 akshare `* 100`）。
pub fn stock_bid_ask_em(symbol: &str) -> Result<Df> {
    let secid = format!("{}.{}", a_share_market_code(symbol), symbol);
    let urls = push2_urls("/api/qt/stock/get");
    let params = json!({
        "fltt": "2",
        "invt": "2",
        "fields": "f120,f121,f122,f174,f175,f59,f163,f43,f57,f58,f169,f170,f46,f44,f51,f168,f47,f164,f116,f60,f45,f52,f50,f48,f167,f117,f71,f161,f49,f530,f135,f136,f137,f138,f139,f141,f142,f144,f145,f147,f148,f140,f143,f146,f149,f55,f62,f162,f92,f173,f104,f105,f84,f85,f183,f184,f185,f186,f187,f188,f189,f190,f191,f192,f107,f111,f86,f177,f78,f110,f262,f263,f264,f267,f268,f255,f256,f257,f258,f127,f199,f128,f198,f259,f260,f261,f171,f277,f278,f279,f288,f152,f250,f251,f252,f253,f254,f269,f270,f271,f272,f273,f274,f275,f276,f265,f266,f289,f290,f286,f285,f292,f293,f294,f295",
        "secid": secid,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_any(&urls, &params, None)?;
    let data = value.get("data").cloned().unwrap_or(Value::Null);
    let Some(obj) = data.as_object() else {
        return Err(AkshareError::empty(format!("{symbol} 无盘口数据")));
    };

    // (item, 字段, 是否 ×100)；顺序与 akshare tick_dict 插入序一致
    let order: &[(&str, &str, bool)] = &[
        ("sell_5", "f31", false),
        ("sell_5_vol", "f32", true),
        ("sell_4", "f33", false),
        ("sell_4_vol", "f34", true),
        ("sell_3", "f35", false),
        ("sell_3_vol", "f36", true),
        ("sell_2", "f37", false),
        ("sell_2_vol", "f38", true),
        ("sell_1", "f39", false),
        ("sell_1_vol", "f40", true),
        ("buy_1", "f19", false),
        ("buy_1_vol", "f20", true),
        ("buy_2", "f17", false),
        ("buy_2_vol", "f18", true),
        ("buy_3", "f15", false),
        ("buy_3_vol", "f16", true),
        ("buy_4", "f13", false),
        ("buy_4_vol", "f14", true),
        ("buy_5", "f11", false),
        ("buy_5_vol", "f12", true),
        ("最新", "f43", false),
        ("均价", "f71", false),
        ("涨幅", "f170", false),
        ("涨跌", "f169", false),
        ("总手", "f47", false),
        ("金额", "f48", false),
        ("换手", "f168", false),
        ("量比", "f50", false),
        ("最高", "f44", false),
        ("最低", "f45", false),
        ("今开", "f46", false),
        ("昨收", "f60", false),
        ("涨停", "f51", false),
        ("跌停", "f52", false),
        ("外盘", "f49", false),
        ("内盘", "f161", false),
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (item, field, x100) in order {
        let value_str = match obj.get(*field) {
            Some(Value::Number(n)) if *x100 => n.as_f64().map(|f| (f * 100.0).to_string()),
            v => v.and_then(json_value_to_string),
        };
        rows.push(vec![Some((*item).to_string()), value_str]);
    }
    Df::from_string_rows(&["item", "value"], &rows)
}

/// 板块名称/概念名称列表的字段串（与 akshare 参数一致）。
const INDUSTRY_NAME_FIELDS: &str = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f26,f22,f33,f11,f62,f128,f136,f115,f152,f124,f107,f104,f105,f140,f141,f207,f208,f209,f222";
const CONCEPT_NAME_FIELDS: &str = "f2,f3,f4,f8,f12,f14,f15,f16,f17,f18,f20,f21,f24,f25,f22,f33,f11,f62,f128,f124,f107,f104,f105,f136";
/// 板块成份（cons）字段串（行业/概念一致，与 akshare 参数一致）。
const INDUSTRY_CONS_FIELDS: &str = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152,f45";

/// 板块名称列表字段语义重命名表（行业/概念共用）。
///
/// 注：akshare 概念板块版存在“列名表数量与请求字段数不匹配”的上游缺陷
/// （28 列名 vs 24 字段，`assign columns` 会抛错），其位置映射不可依赖；
/// 本实现统一按东财字段标准语义映射（`f2`=最新价、`f12`=板块代码、
/// `f14`=板块名称、`f104`/`f105`=上涨/下跌家数、`f128`=领涨股票、
/// `f141`=领涨股票-涨跌幅），最终列契约与 akshare 的 select 清单一致。
const BOARD_NAME_RENAME: &[(&str, &str)] = &[
    ("index", "排名"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f8", "换手率"),
    ("f12", "板块代码"),
    ("f14", "板块名称"),
    ("f20", "总市值"),
    ("f104", "上涨家数"),
    ("f105", "下跌家数"),
    ("f128", "领涨股票"),
    ("f141", "领涨股票-涨跌幅"),
];

/// 行业板块名称列表（对应 akshare [`akshare.stock_board_industry_name_em`]）。
///
/// # 返回列
/// `排名, 板块名称, 板块代码, 最新价, 涨跌额, 涨跌幅, 总市值, 换手率,
/// 上涨家数, 下跌家数, 领涨股票, 领涨股票-涨跌幅`
pub fn stock_board_industry_name_em() -> Result<Df> {
    board_name_list("m:90 t:2 f:!50", INDUSTRY_NAME_FIELDS)
}

/// 概念板块名称列表（对应 akshare [`akshare.stock_board_concept_name_em`]）。
///
/// 列契约与 [`stock_board_industry_name_em`] 一致。
pub fn stock_board_concept_name_em() -> Result<Df> {
    board_name_list("m:90 t:3 f:!50", CONCEPT_NAME_FIELDS)
}

/// 板块名称列表公共实现。
fn board_name_list(fs: &str, fields: &str) -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1", "ut": UT_CLIST,
        "fltt": "2", "invt": "2", "fid": "f3", "fs": fs, "fields": fields,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    finalize_board_name(fetch_clist(&http, &urls, &params)?, BOARD_NAME_RENAME)
}

/// 行业板块成份（对应 akshare [`akshare.stock_board_industry_cons_em`]）。
///
/// `symbol` 接受板块名称（如 `"小金属"`）或东财板块代码（如 `"BK1027"`）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高,
/// 最低, 今开, 昨收, 换手率, 市盈率-动态, 市净率`
pub fn stock_board_industry_cons_em(symbol: &str) -> Result<Df> {
    board_cons("m:90 t:2 f:!50", INDUSTRY_NAME_FIELDS, symbol)
}

/// 概念板块成份（对应 akshare [`akshare.stock_board_concept_cons_em`]）。
///
/// 列契约与 [`stock_board_industry_cons_em`] 一致。
pub fn stock_board_concept_cons_em(symbol: &str) -> Result<Df> {
    board_cons("m:90 t:3 f:!50", CONCEPT_NAME_FIELDS, symbol)
}

/// 板块成份公共实现。
fn board_cons(fs: &str, name_fields: &str, symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let code = resolve_board_code(&http, fs, name_fields, symbol)?;
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1", "ut": UT_CLIST,
        "fltt": "2", "invt": "2", "fid": "f3", "fs": format!("b:{code} f:!50"),
        "fields": INDUSTRY_CONS_FIELDS,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    finalize_board_cons(fetch_clist(&http, &urls, &params)?)
}

/// 板块名称（或 BK 代码）→ 板块代码解析。
fn resolve_board_code(
    http: &HttpClient,
    fs: &str,
    name_fields: &str,
    symbol: &str,
) -> Result<String> {
    if symbol.starts_with("BK") {
        return Ok(symbol.to_string());
    }
    for (name, code) in board_name_pairs(http, fs, name_fields, BOARD_NAME_RENAME)? {
        if name == symbol {
            return Ok(code);
        }
    }
    Err(AkshareError::Param(format!("未找到板块: {symbol}")))
}

/// 行业板块历史行情（对应 akshare [`akshare.stock_board_industry_hist_em`]）。
///
/// `period`: `"日k"`/`"周k"`/`"月k"`；`adjust`: `""`/`"qfq"`/`"hfq"`。
///
/// # 返回列
/// `日期, 开盘, 收盘, 最高, 最低, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 换手率`
pub fn stock_board_industry_hist_em(
    symbol: &str,
    start_date: &str,
    end_date: &str,
    period: &str,
    adjust: &str,
) -> Result<Df> {
    let klt = match period {
        "日k" => "101",
        "周k" => "102",
        "月k" => "103",
        _ => return Err(AkshareError::Param(format!("无效 period: {period}"))),
    };
    board_hist(
        "m:90 t:2 f:!50",
        INDUSTRY_NAME_FIELDS,
        symbol,
        klt,
        adjust,
        start_date,
        end_date,
    )
}

/// 概念板块历史行情（对应 akshare [`akshare.stock_board_concept_hist_em`]）。
///
/// 参数顺序与 akshare 一致：`(symbol, period, start_date, end_date, adjust)`。
/// `period`: `"daily"`/`"weekly"`/`"monthly"`；`adjust` 同行业版。
pub fn stock_board_concept_hist_em(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
    adjust: &str,
) -> Result<Df> {
    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        _ => return Err(AkshareError::Param(format!("无效 period: {period}"))),
    };
    board_hist(
        "m:90 t:3 f:!50",
        CONCEPT_NAME_FIELDS,
        symbol,
        klt,
        adjust,
        start_date,
        end_date,
    )
}

/// 板块历史行情公共实现（K 线 + 列重排）。
fn board_hist(
    fs: &str,
    name_fields: &str,
    symbol: &str,
    klt: &str,
    adjust: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    let fqt = match adjust {
        "" => "0",
        "qfq" => "1",
        "hfq" => "2",
        _ => return Err(AkshareError::Param(format!("无效 adjust: {adjust}"))),
    };
    let http = HttpClient::default();
    let code = resolve_board_code(&http, fs, name_fields, symbol)?;
    let secid = format!("90.{code}");
    let klines = fetch_kline_ext(
        &http,
        &secid,
        klt,
        fqt,
        start_date,
        end_date,
        &[("smplmt", "10000"), ("lmt", "1000000")],
    )?;
    require_kline_data(&klines, symbol)?;
    let df = kline_to_df(&KLINE_COLS, &klines, None)?;
    df.select(&BOARD_HIST_SELECT)
}

/// 板块实时行情公共实现（对应 akshare `stock_board_*_spot_em`）。
///
/// `91.push2.eastmoney.com/api/qt/stock/get`，`fltt=1`（原始值×100）。
/// akshare：`from_dict(orient="index")` → 列名 `item/value`，全部 ×1e-2，
/// 成交量/成交额两行（第 5/6 行）再 ×1e2 恢复（本身已是正常单位）。
fn board_spot(http: &HttpClient, code: &str) -> Result<Df> {
    const URL: &str = "https://91.push2.eastmoney.com/api/qt/stock/get";
    let params = json!({
        "fields": "f43,f44,f45,f46,f47,f48,f170,f171,f168,f169",
        "mpi": "1000",
        "invt": "2",
        "fltt": "1",
        "secid": format!("90.{code}"),
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(URL, &params, None)?;
    let data = value
        .get("data")
        .ok_or_else(|| AkshareError::Empty("板块行情无 data".into()))?;
    let obj = data
        .as_object()
        .ok_or_else(|| AkshareError::Empty("板块行情 data 非对象".into()))?;
    // 顺序与 akshare field_map 一致：最新,最高,最低,开盘,成交量,成交额,涨跌幅,振幅,换手率,涨跌额
    const ITEMS: [(&str, &str); 10] = [
        ("f43", "最新"),
        ("f44", "最高"),
        ("f45", "最低"),
        ("f46", "开盘"),
        ("f47", "成交量"),
        ("f48", "成交额"),
        ("f170", "涨跌幅"),
        ("f171", "振幅"),
        ("f168", "换手率"),
        ("f169", "涨跌额"),
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(10);
    for (i, (k, name)) in ITEMS.iter().enumerate() {
        let raw = obj.get(*k).map(|x| x.to_string());
        // 成交量/成交额保持原值（akshare 先 ×1e-2 再对这两行 ×1e2 恢复）；
        // 其余 ×1e-2（fltt=1 原始值为百分位）
        let processed = if i == 4 || i == 5 {
            raw
        } else {
            raw.and_then(|s| s.parse::<f64>().ok().map(|f| (f * 1e-2).to_string()))
        };
        rows.push(vec![Some((*name).to_string()), processed]);
    }
    let mut df = Df::from_string_rows(&["item", "value"], &rows)?;
    df.cast_numeric(&["value"])?;
    Ok(df)
}

/// 行业板块实时行情（对应 akshare [`akshare.stock_board_industry_spot_em`]）。
///
/// `symbol`: 板块名称或东财板块代码（如 `"小金属"` / `"BK1027"`）。
///
/// # 返回列
/// `item, value`（item 为 最新/最高/最低/开盘/成交量/成交额/涨跌幅/振幅/换手率/涨跌额）
pub fn stock_board_industry_spot_em(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let code = resolve_board_code(&http, "m:90 t:2 f:!50", INDUSTRY_NAME_FIELDS, symbol)?;
    board_spot(&http, &code)
}

/// 概念板块实时行情（对应 akshare [`akshare.stock_board_concept_spot_em`]）。
///
/// `symbol`: 概念板块名称或代码（如 `"可燃冰"` / `"BK0818"`）。
///
/// # 返回列
/// `item, value`
pub fn stock_board_concept_spot_em(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let code = resolve_board_code(&http, "m:90 t:3 f:!50", CONCEPT_NAME_FIELDS, symbol)?;
    board_spot(&http, &code)
}

/// 板块分时/分钟历史行情公共实现（对应 akshare `stock_board_*_hist_min_em`）。
fn board_hist_min(http: &HttpClient, code: &str, period: &str) -> Result<Df> {
    let secid = format!("90.{code}");
    if period == "1" {
        // 分时：trends2，ndays=1, iscr=0，8 字段（f58 为最新价，akshare 命名）
        let trends = fetch_trends(http, &secid, "1", "0")?;
        let rows: Vec<Vec<Option<String>>> = trends
            .iter()
            .map(|t| t.iter().map(|s| Some(s.clone())).collect())
            .collect();
        const COLS: [&str; 8] = [
            "日期时间",
            "开盘",
            "收盘",
            "最高",
            "最低",
            "成交量",
            "成交额",
            "最新价",
        ];
        let mut df = Df::from_string_rows(&COLS, &rows)?;
        df.cast_numeric(&COLS[1..])?;
        Ok(df)
    } else {
        match period {
            "5" | "15" | "30" | "60" => {}
            other => {
                return Err(AkshareError::Param(format!(
                    "无效 period: {other}，可选 1/5/15/30/60"
                )))
            }
        }
        let klines = fetch_kline_min(http, &secid, period, "1")?;
        // 11 字段：日期时间,开盘,收盘,最高,最低,成交量,成交额,振幅,涨跌幅,涨跌额,换手率
        // akshare select 顺序：日期时间,开盘,收盘,最高,最低,涨跌幅,涨跌额,成交量,成交额,振幅,换手率
        let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
        for k in &klines {
            let pick = |i: usize| k.get(i).map(|s| Some(s.clone())).unwrap_or(None);
            rows.push(vec![
                pick(0),
                pick(1),
                pick(2),
                pick(3),
                pick(4),
                pick(8),
                pick(9),
                pick(5),
                pick(6),
                pick(7),
                pick(10),
            ]);
        }
        const COLS: [&str; 11] = [
            "日期时间",
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
        let mut df = Df::from_string_rows(&COLS, &rows)?;
        df.cast_numeric(&COLS[1..])?;
        Ok(df)
    }
}

/// 行业板块分时/分钟历史行情（对应 akshare [`akshare.stock_board_industry_hist_min_em`]）。
///
/// - `symbol`: 板块名称或东财板块代码
/// - `period`: `"1"`（分时）/ `"5"` / `"15"` / `"30"` / `"60"`（分钟）
///
/// # 返回列
/// `日期时间, 开盘, 收盘, 最高, 最低, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 换手率`
/// （period=1 时为 `日期时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 最新价`）
pub fn stock_board_industry_hist_min_em(symbol: &str, period: &str) -> Result<Df> {
    let http = HttpClient::default();
    let code = resolve_board_code(&http, "m:90 t:2 f:!50", INDUSTRY_NAME_FIELDS, symbol)?;
    board_hist_min(&http, &code, period)
}

/// 概念板块分时/分钟历史行情（对应 akshare [`akshare.stock_board_concept_hist_min_em`]）。
///
/// - `symbol`: 概念板块名称或代码
/// - `period`: `"1"`（分时）/ `"5"` / `"15"` / `"30"` / `"60"`（分钟）
///
/// # 返回列
/// 同 [`stock_board_industry_hist_min_em`]
pub fn stock_board_concept_hist_min_em(symbol: &str, period: &str) -> Result<Df> {
    let http = HttpClient::default();
    let code = resolve_board_code(&http, "m:90 t:3 f:!50", CONCEPT_NAME_FIELDS, symbol)?;
    board_hist_min(&http, &code, period)
}

// === BATCH36-C 上交所股票列表/终止上市（query.sse.com.cn JSON）===
//
// 对应 akshare `stock/stock_info.py` 的 `stock_info_sh_name_code` /
// `stock_info_sh_delist`。走 `query.sse.com.cn` JSON（与 `stock_margin_sse` 同源），
// 键名 rename + 列序对齐 akshare。

const SSE_QUERY_REFERER: &str = "https://www.sse.com.cn/assortment/stock/list/share/";
const SSE_QUERY_HEADERS: &[(&str, &str)] = &[
    ("Host", "query.sse.com.cn"),
    ("Pragma", "no-cache"),
    ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36"),
];

/// 上交所-股票列表（对应 akshare [`akshare.stock_info_sh_name_code`]）。
///
/// - `symbol`: `"主板A股"` / `"主板B股"` / `"科创板"`
///
/// # 返回列
/// `证券代码, 证券简称, 证券全称, 公司简称, 公司全称, 上市日期`
pub fn stock_info_sh_name_code(symbol: &str) -> Result<Df> {
    let st = match symbol {
        "主板A股" => "1",
        "主板B股" => "2",
        "科创板" => "8",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 主板A股/主板B股/科创板"
            )))
        }
    };
    let url = "https://query.sse.com.cn/sseQuery/commonQuery.do";
    let params = json!({
        "STOCK_TYPE": st,
        "REG_PROVINCE": "",
        "CSRC_CODE": "",
        "STOCK_CODE": "",
        "sqlId": "COMMON_SSE_CP_GPJCTPZ_GPLB_GP_L",
        "COMPANY_STATUS": "2,4,5,7,8",
        "type": "inParams",
        "isPagination": "true",
        "pageHelp.cacheSize": "1",
        "pageHelp.beginPage": "1",
        "pageHelp.pageSize": "10000",
        "pageHelp.pageNo": "1",
        "pageHelp.endPage": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let data =
        http.get_json_with_headers(url, &params, SSE_QUERY_HEADERS, Some(SSE_QUERY_REFERER))?;
    let rows = data
        .get("result")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let code_key = if symbol == "主板B股" {
        "B_STOCK_CODE"
    } else {
        "A_STOCK_CODE"
    };
    let rename = [
        (code_key, "证券代码"),
        ("SEC_NAME_CN", "证券简称"),
        ("SEC_NAME_FULL", "证券全称"),
        ("COMPANY_ABBR", "公司简称"),
        ("FULL_NAME", "公司全称"),
        ("LIST_DATE", "上市日期"),
    ];
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("A_STOCK_CODE").or_else(|| f("B_STOCK_CODE")),
            f("SEC_NAME_CN"),
            f("SEC_NAME_FULL"),
            f("COMPANY_ABBR"),
            f("FULL_NAME"),
            f("LIST_DATE"),
        ]);
    }
    let _ = rename;
    const COLS: [&str; 6] = [
        "证券代码",
        "证券简称",
        "证券全称",
        "公司简称",
        "公司全称",
        "上市日期",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["上市日期"])?;
    Ok(df)
}

/// 上交所-终止上市公司（对应 akshare [`akshare.stock_info_sh_delist`]）。
///
/// - `symbol`: `"全部"` / `"沪市"` / `"科创板"`
///
/// # 返回列
/// `公司代码, 公司简称, 上市日期, 暂停上市日期`
pub fn stock_info_sh_delist(symbol: &str) -> Result<Df> {
    let st = match symbol {
        "全部" => "1,2,8",
        "沪市" => "1,2",
        "科创板" => "8",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全部/沪市/科创板"
            )))
        }
    };
    let url = "https://query.sse.com.cn/commonQuery.do";
    let params = json!({
        "sqlId": "COMMON_SSE_CP_GPJCTPZ_GPLB_GP_L",
        "isPagination": "true",
        "STOCK_CODE": "",
        "CSRC_CODE": "",
        "REG_PROVINCE": "",
        "STOCK_TYPE": st,
        "COMPANY_STATUS": "3",
        "type": "inParams",
        "pageHelp.cacheSize": "1",
        "pageHelp.beginPage": "1",
        "pageHelp.pageSize": "500",
        "pageHelp.pageNo": "1",
        "pageHelp.endPage": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let data =
        http.get_json_with_headers(url, &params, SSE_QUERY_HEADERS, Some(SSE_QUERY_REFERER))?;
    let rows = data
        .get("result")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("COMPANY_CODE"),
            f("COMPANY_ABBR"),
            f("LIST_DATE"),
            f("DELIST_DATE"),
        ]);
    }
    const COLS: [&str; 4] = ["公司代码", "公司简称", "上市日期", "暂停上市日期"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["上市日期", "暂停上市日期"])?;
    Ok(df)
}

/// 上交所-董监高人员股份变动（对应 akshare [`akshare.stock_share_hold_change_sse`]）。
///
/// - `symbol`: `"全部"` 或具体股票代码（如 `"600000"`）
///
/// `query.sse.com.cn/commonQuery.do`（`COMMON_SSE_XXPL_CXJL_SSGSGFBDQK_S`）分页。
///
/// # 返回列
/// `股票种类, 公司名称, 姓名, 职务, 变动日期, 变动原因, 本次变动平均价格,
/// 变动后持股数, 货币种类, 公司代码`
pub fn stock_share_hold_change_sse(symbol: &str) -> Result<Df> {
    let url = "https://query.sse.com.cn/commonQuery.do";
    let mut params = json!({
        "isPagination": "true",
        "pageHelp.pageSize": "100",
        "pageHelp.pageNo": "1",
        "pageHelp.beginPage": "1",
        "pageHelp.cacheSize": "1",
        "pageHelp.endPage": "1",
        "sqlId": "COMMON_SSE_XXPL_CXJL_SSGSGFBDQK_S",
        "COMPANY_CODE": "",
        "NAME": "",
        "BEGIN_DATE": "1990-01-01",
        "END_DATE": "2050-01-01",
        "BOARDTYPE": "",
    });
    if symbol != "全部" {
        params
            .as_object_mut()
            .unwrap()
            .insert("COMPANY_CODE".into(), Value::String(symbol.into()));
    }
    let mut params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let first =
        http.get_json_with_headers(url, &params, SSE_QUERY_HEADERS, Some(SSE_QUERY_REFERER))?;
    let page_count = first
        .get("pageHelp")
        .and_then(|p| p.get("pageCount"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v.get("result").and_then(Value::as_array) {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=page_count {
        params.insert("pageHelp.pageNo".into(), json!(page.to_string()));
        params.insert("pageHelp.beginPage".into(), json!(page.to_string()));
        params.insert("pageHelp.endPage".into(), json!(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_json_with_headers(url, &params, SSE_QUERY_HEADERS, Some(SSE_QUERY_REFERER)) {
            Ok(v) => append(&v, &mut rows),
            Err(_) => break,
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("STOCK_TYPE"),
            f("COMPANY_ABBR"),
            f("NAME"),
            f("DUTY"),
            f("CHANGE_DATE"),
            f("CHANGE_REASON"),
            f("CURRENT_AVG_PRICE"),
            f("HOLDSTOCK_NUM"),
            f("CURRENCY_TYPE"),
            f("COMPANY_CODE"),
        ]);
    }
    const COLS: [&str; 10] = [
        "股票种类",
        "公司名称",
        "姓名",
        "职务",
        "变动日期",
        "变动原因",
        "本次变动平均价格",
        "变动后持股数",
        "货币种类",
        "公司代码",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["变动日期"])?;
    df.cast_numeric(&["本次变动平均价格", "变动后持股数"])?;
    Ok(df)
}

/// 深交所-董监高人员股份变动（对应 akshare [`akshare.stock_share_hold_change_szse`]）。
///
/// - `symbol`: `"全部"` 或具体股票代码
///
/// `szse.cn/api/report/ShowReport/data`（`1801_cxda`）JSON 分页。
///
/// # 返回列
/// `证券代码, 证券简称, 董监高姓名, 变动日期, 变动股份数量, 成交均价, 变动原因,
/// 变动比例, 当日结存股数, 股份变动人姓名, 职务, 变动人与董监高的关系`
pub fn stock_share_hold_change_szse(symbol: &str) -> Result<Df> {
    let url = "https://www.szse.cn/api/report/ShowReport/data";
    let mut params = Map::new();
    params.insert("SHOWTYPE".into(), Value::String("JSON".into()));
    params.insert("CATALOGID".into(), Value::String("1801_cxda".into()));
    params.insert("TABKEY".into(), Value::String("tab1".into()));
    params.insert("PAGENO".into(), Value::String("1".into()));
    params.insert("random".into(), Value::String("0.7874198771222201".into()));
    if symbol != "全部" {
        params.insert("txtDMorJC".into(), Value::String(symbol.into()));
    }
    let http = HttpClient::default();
    let headers: &[(&str, &str)] = &[(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/93.0.4577.63 Safari/537.36",
    )];
    let first = http.get_json_with_headers(url, &params, headers, None)?;
    let page_count = first
        .get(0)
        .and_then(|o| o.get("metadata"))
        .and_then(|m| m.get("pagecount"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v
            .get(0)
            .and_then(|o| o.get("data"))
            .and_then(Value::as_array)
        {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=page_count {
        params.insert("PAGENO".into(), Value::String(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_json_with_headers(url, &params, headers, None) {
            Ok(v) => append(&v, &mut rows),
            Err(_) => break,
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("zqdm"),
            f("zqjc"),
            f("ggxm"),
            f("jyrq"),
            f("bdgs"),
            f("bdjj"),
            f("bdyy"),
            f("cgbdbl"),
            f("cgzs"),
            f("gdxm"),
            f("zw"),
            f("gxlb"),
        ]);
    }
    const COLS: [&str; 12] = [
        "证券代码",
        "证券简称",
        "董监高姓名",
        "变动日期",
        "变动股份数量",
        "成交均价",
        "变动原因",
        "变动比例",
        "当日结存股数",
        "股份变动人姓名",
        "职务",
        "变动人与董监高的关系",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["变动日期"])?;
    df.cast_numeric(&["变动股份数量", "成交均价", "变动比例"])?;
    df.strip_commas(&["当日结存股数"])?;
    df.cast_numeric(&["当日结存股数"])?;
    Ok(df)
}

/// 北交所-董监高人员股份变动（对应 akshare [`akshare.stock_share_hold_change_bse`]）。
///
/// - `symbol`: 股票代码，如 `"430489"`
///
/// `bse.cn/djgCgbdController/getDjgCgbdList.do`（`null(...)` JSONP 解包）分页。
///
/// # 返回列
/// `代码, 简称, 姓名, 职务, 变动日期, 变动原因, 变动均价, 变动股数,
/// 变动前持股数, 变动后持股数`
pub fn stock_share_hold_change_bse(symbol: &str) -> Result<Df> {
    let url = "https://www.bse.cn/djgCgbdController/getDjgCgbdList.do";
    let mut params = Map::new();
    params.insert("page".into(), Value::String("0".into()));
    params.insert("typejb".into(), Value::String("T".into()));
    if !symbol.is_empty() {
        params.insert("xxzqdm".into(), Value::String(symbol.into()));
    }
    let http = HttpClient::default();
    let headers: &[(&str, &str)] = &[(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/93.0.4577.63 Safari/537.36",
    )];
    let parse_bse = |text: &str| -> Result<Value> {
        let t = text.trim();
        let t = t.strip_prefix("null(").unwrap_or(t);
        let t = t.strip_suffix(')').unwrap_or(t);
        serde_json::from_str(t).map_err(|e| AkshareError::json(url, e.to_string()))
    };
    let first_text = http.get_text_with_headers(url, &params, headers, None)?;
    let first = parse_bse(&first_text)?;
    let total_pages = first
        .get(0)
        .and_then(|o| o.get("result"))
        .and_then(|r| r.get("totalPages"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v
            .get(0)
            .and_then(|o| o.get("result"))
            .and_then(|r| r.get("content"))
            .and_then(Value::as_array)
        {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 1..total_pages {
        params.insert("page".into(), Value::String(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_text_with_headers(url, &params, headers, None) {
            Ok(t) => {
                if let Ok(v) = parse_bse(&t) {
                    append(&v, &mut rows);
                }
            }
            Err(_) => break,
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("stockCode"),
            f("stockName"),
            f("djgName"),
            f("duty"),
            f("changeDate"),
            f("reason"),
            f("price"),
            f("changeAmount"),
            f("preAmount"),
            f("newAmount"),
        ]);
    }
    const COLS: [&str; 10] = [
        "代码",
        "简称",
        "姓名",
        "职务",
        "变动日期",
        "变动原因",
        "变动均价",
        "变动股数",
        "变动前持股数",
        "变动后持股数",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["变动日期"])?;
    df.cast_numeric(&["变动均价", "变动股数", "变动前持股数", "变动后持股数"])?;
    Ok(df)
}

/// 北交所-股票列表（对应 akshare [`akshare.stock_info_bj_name_code`]）。
///
/// POST `nqxxCnzq.do` 分页拉取；响应为 `[{"totalPages":N,"content":[...]}]` 形式，
/// 每页 `content` 含 48 列（位置式列名表，`-` 为占位），取 8 列。
///
/// # 返回列
/// `证券代码, 证券简称, 总股本, 流通股本, 上市日期, 所属行业, 地区, 报告日期`
pub fn stock_info_bj_name_code() -> Result<Df> {
    const URL: &str = "https://www.bse.cn/nqxxController/nqxxCnzq.do";
    let headers: &[(&str, &str)] = &[(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/110.0.0.0 Safari/537.36",
    )];
    let http = HttpClient::default();

    // 首页：确定 totalPages
    let params = json!({
        "page": "0",
        "typejb": "T",
        "xxfcbj[]": "2",
        "xxzqdm": "",
        "sortfield": "xxzqdm",
        "sorttype": "asc",
    });
    let mut params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let parse_first = |text: &str| -> Result<Value> {
        let start = text
            .find('[')
            .ok_or_else(|| AkshareError::Empty("北交所响应无数组".into()))?;
        let sub = &text[start..];
        // 尾部可能带 `;` 或尾括号，取最后一个 `]` 作为结束
        let end = sub.rfind(']').unwrap_or(sub.len() - 1);
        serde_json::from_str(&sub[..=end]).map_err(|e| AkshareError::json(URL, e.to_string()))
    };

    let first_text = http.post_form_text(URL, &params, headers)?;
    let first: Value = parse_first(&first_text)?;
    let total_pages = first
        .get(0)
        .and_then(|o| o.get("totalPages"))
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;

    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    let mut append_page = |page: usize, arr: &Value| {
        let content = arr
            .get(0)
            .and_then(|o| o.get("content"))
            .and_then(Value::as_array);
        if let Some(content) = content {
            for row in content {
                let f = |k: &str| row.get(k).and_then(json_value_to_string);
                out.push(vec![
                    f("xxzqdm"),
                    f("xxzqjc"),
                    f("zgb"),
                    f("ltgb"),
                    f("ssrq"),
                    f("sshy"),
                    f("dq"),
                    f("bgrq"),
                ]);
            }
        }
        let _ = page;
    };
    append_page(0, &first);

    for page in 1..total_pages {
        params.insert("page".into(), json!(page));
        // 随机延迟，避免请求过频
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        if let Ok(text) = http.post_form_text(URL, &params, headers) {
            if let Ok(arr) = parse_first(&text) {
                append_page(page, &arr);
            }
        }
    }

    // 位置式列名表 → 目标 8 列（akshare 用 48 列占位表后 select）
    const COLS: [&str; 8] = [
        "证券代码",
        "证券简称",
        "总股本",
        "流通股本",
        "上市日期",
        "所属行业",
        "地区",
        "报告日期",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["上市日期", "报告日期"])?;
    df.cast_numeric(&["总股本", "流通股本"])?;
    Ok(df)
}

// === BATCH36-D 东财知名港股/美股（69.push2 clist，diff 为对象）===
//
// 对应 akshare `stock/stock_hk_famous.py` / `stock/stock_us_famous.py`。
// 接口 `data.diff` 为「序号→行」对象（非数组），akshare 经
// `pd.DataFrame(diff).T` + 位置式列名表取 12 列；Rust 侧按键序展开行列表，
// 直接按目标列契约构建（列名与 akshare 逐字对齐）。

/// 知名股实时行情公共实现（69.push2 clist，diff 对象 → 12 列）。
fn famous_spot(fs: &str) -> Result<Df> {
    const URL: &str = "https://69.push2.eastmoney.com/api/qt/clist/get";
    let params = json!({
        "pn": "1",
        "pz": "50000",
        "po": "1",
        "np": "2",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "dect": "1",
        "wbp2u": "|0|0|0|web",
        "fid": "f3",
        "fs": fs,
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f26,f22,f33,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(URL, &params, None)?;
    let diff = value
        .get("data")
        .and_then(|d| d.get("diff"))
        .cloned()
        .unwrap_or(Value::Null);
    // diff 为对象（键为序号）或数组，统一展开为行列表
    let mut rows_vec: Vec<Value> = Vec::new();
    match diff {
        Value::Array(arr) => rows_vec = arr,
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            rows_vec = entries.into_iter().map(|(_, v)| v).collect();
        }
        _ => {}
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows_vec.len());
    for (i, row) in rows_vec.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()), // 序号
            f("f12"),
            f("f14"),
            f("f2"),
            f("f4"),
            f("f3"),
            f("f17"),
            f("f15"),
            f("f16"),
            f("f18"),
            f("f5"),
            f("f6"),
        ]);
    }
    const COLS: [&str; 12] = [
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
        "成交量",
        "成交额",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[3..])?;
    Ok(df)
}

/// 东财-港股市场-知名港股实时行情（对应 akshare [`akshare.stock_hk_famous_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额`
pub fn stock_hk_famous_spot_em() -> Result<Df> {
    famous_spot("b:DLMK0106")
}

/// 东财-美股市场-知名美股实时行情（对应 akshare [`akshare.stock_us_famous_spot_em`]）。
///
/// - `symbol`: `"科技类"` / `"金融类"` / `"医药食品类"` / `"媒体类"` / `"汽车能源类"` / `"制造零售类"`
///
/// # 返回列
/// 同 [`stock_hk_famous_spot_em`]
pub fn stock_us_famous_spot_em(symbol: &str) -> Result<Df> {
    let mk = match symbol {
        "科技类" => "0216",
        "金融类" => "0217",
        "医药食品类" => "0218",
        "媒体类" => "0220",
        "汽车能源类" => "0219",
        "制造零售类" => "0221",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 科技类/金融类/医药食品类/媒体类/汽车能源类/制造零售类"
            )))
        }
    };
    famous_spot(&format!("b:MK{mk}"))
}

/// 腾讯证券-沪深京-实时行情（对应 akshare [`akshare.stock_zh_a_spot_tx`]）。
///
/// `proxy.finance.qq.com/cgi/cgi-bin/rank/hs/getBoardRankList` 分页（count=200），
/// 按 `code` 去重；列名为接口原键（akshare 不重命名，28 列）。
///
/// # 返回列
/// `code, hsl, lb, ltsz, name, pe_ttm, pn, speed, state, stock_type, turnover,
/// volume, zd, zdf, zdf_d10, zdf_d20, zdf_d5, zdf_d60, zdf_w52, zdf_y, zf,
/// zljlr, zllc, zllc_d5, zllr, zllr_d5, zsz, zxj`
pub fn stock_zh_a_spot_tx() -> Result<Df> {
    const URL: &str = "https://proxy.finance.qq.com/cgi/cgi-bin/rank/hs/getBoardRankList";
    const PAGE_SIZE: u64 = 200;
    let http = HttpClient::default();

    let params = json!({
        "_appver": "11.17.0",
        "board_code": "aStock",
        "sort_type": "price",
        "direct": "down",
        "offset": "0",
        "count": PAGE_SIZE.to_string(),
    });
    let mut params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let first = http.get_json(URL, &params, None)?;
    let total = first
        .get("data")
        .and_then(|d| d.get("total"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut rows = first
        .get("data")
        .and_then(|d| d.get("rank_list"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total_pages = total.div_ceil(PAGE_SIZE);
    for page in 1..total_pages {
        params.insert("offset".into(), json!((page * PAGE_SIZE).to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_json(URL, &params, None) {
            Ok(v) => {
                if let Some(list) = v
                    .get("data")
                    .and_then(|d| d.get("rank_list"))
                    .and_then(Value::as_array)
                {
                    rows.extend(list.iter().cloned());
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    // drop_duplicates(subset=["code"])
    let mut seen = std::collections::HashSet::new();
    rows.retain(|r| {
        let code = r
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        seen.insert(code)
    });
    Df::from_json_rows(&rows)
}

/// 东财-行情中心-沪深个股-两网及退市（对应 akshare [`akshare.stock_zh_a_stop_em`]）。
///
/// 40.push2 clist（`fs=m:0 s:3`），与 A 股快照同字段集；位置式列名 + 17 列 select。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率`
pub fn stock_zh_a_stop_em() -> Result<Df> {
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
        "fs": "m:0 s:3",
        "fields": "f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let df = fetch_clist(&http, &urls, &params)?;
    // 位置式列名表（akshare 30 列含占位 "_"），对应 fetch_clist 的 index + 字段序
    let rename = [
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
    let select = [
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
    ];
    let numeric = [
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
    ];
    crate::sources::eastmoney::finalize_spot(df, &rename, &select, &numeric)
}

/// 新浪财经-行情中心-沪深股市-次新股（对应 akshare [`akshare.stock_zh_a_new`]）。
///
/// `Market_Center.getHQNodeData`（node=new_stock）分页（num=80），取 10 列。
///
/// # 返回列
/// `symbol, code, name, open, high, low, volume, amount, mktcap, turnoverratio`
pub fn stock_zh_a_new() -> Result<Df> {
    let http = HttpClient::default();
    // 1) 总条数 → 页数
    let count_url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCount";
    let count_params = json!({ "node": "new_stock" });
    let text = http.get_text(count_url, count_params.as_object().expect("静态参数"), None)?;
    let total: u64 = text.trim().parse().unwrap_or(0);
    let total_pages = total.div_ceil(80);

    // 2) 分页抓取
    let url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
    let mut rows: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let params = json!({
            "page": page.to_string(),
            "num": "80",
            "sort": "symbol",
            "asc": "1",
            "node": "new_stock",
            "symbol": "",
            "_s_r_a": "page",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                    rows.extend(arr);
                }
            }
            Err(_) => break,
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    let df = Df::from_json_rows(&rows)?;
    let df = df.select(&[
        "symbol",
        "code",
        "name",
        "open",
        "high",
        "low",
        "volume",
        "amount",
        "mktcap",
        "turnoverratio",
    ])?;
    let mut df = df;
    df.cast_numeric(&["open", "high", "low"])?;
    Ok(df)
}

/// 新浪 A 股日线（对应 akshare [`akshare.stock_zh_a_daily`]）。
///
/// - `symbol`: 带市场前缀代码，如 `"sh600519"`
/// - `start_date`/`end_date`: `YYYYMMDD`
/// - `adjust`: `""`（不复权）/ `"qfq"`（前复权）/ `"hfq"`（后复权）/
///   `"hfq-factor"`（后复权因子）/ `"qfq-factor"`（前复权因子）
///
/// # 返回列
/// 不复权/复权：`date, open, high, low, close, volume, amount, outstanding_share, turnover`；
/// factor：`date, {hfq|qfq}_factor`
pub fn stock_zh_a_daily(
    symbol: &str,
    start_date: &str,
    end_date: &str,
    adjust: &str,
) -> Result<Df> {
    let http = HttpClient::default();

    // 复权因子分支：hfq-factor / qfq-factor
    if adjust == "hfq-factor" || adjust == "qfq-factor" {
        let method = &adjust[..3]; // "hfq" / "qfq"
        let url = format!("https://finance.sina.com.cn/realstock/company/{symbol}/{method}.js");
        let text = http.get_text(&url, &Map::new(), None)?;
        // 响应 `var xxx={"data":[{"date":"...","factor":...}, ...]};`，提取对象 JSON
        let start = text
            .find('{')
            .ok_or_else(|| AkshareError::Empty("新浪复权因子响应缺少对象".into()))?;
        let end = text
            .rfind('}')
            .ok_or_else(|| AkshareError::Empty("新浪复权因子响应缺少对象尾".into()))?;
        let obj: Value = serde_json::from_str(&text[start..=end])
            .map_err(|e| AkshareError::json(url, e.to_string()))?;
        let data = obj
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
        for r in &data {
            let f = |k: &str| r.get(k).and_then(json_value_to_string);
            rows.push(vec![f("date"), f("factor")]);
        }
        let col = if adjust == "hfq-factor" {
            "hfq_factor"
        } else {
            "qfq_factor"
        };
        let mut df = Df::from_string_rows(&["date", col], &rows)?;
        df.cast_numeric(&[col])?;
        return Ok(df);
    }

    // 1) 历史行情：hisdata_klc2/klc_kl.js → sina.js d() 解码
    let hist_url =
        format!("https://finance.sina.com.cn/realstock/company/{symbol}/hisdata_klc2/klc_kl.js");
    let text = http.get_text(&hist_url, &Map::new(), None)?;
    let encoded = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪日线响应缺少 '=' 分隔".into()))?
        .split(';')
        .next()
        .ok_or_else(|| AkshareError::Empty("新浪日线响应缺少 ';' 分隔".into()))?
        .replace('"', "");
    let decoded = crate::core::js_engine::sina_js_decode(&encoded)?;
    let rows: Vec<Value> =
        serde_json::from_str(&decoded).map_err(|e| AkshareError::json(hist_url, e.to_string()))?;
    if rows.is_empty() {
        return Df::from_string_rows(
            &[
                "date",
                "open",
                "high",
                "low",
                "close",
                "volume",
                "amount",
                "outstanding_share",
                "turnover",
            ],
            &[],
        );
    }

    // 2) 流通股本：StockService.getAmountBySymbol → `var X=([{date,amount},...])`
    let amount_url = format!(
        "https://stock.finance.sina.com.cn/stock/api/jsonp.php/var%20KKE_ShareAmount_{symbol}=/StockService.getAmountBySymbol?_=20&symbol={symbol}"
    );
    let amount_text = http.get_text(&amount_url, &Map::new(), None)?;
    let amount_start = amount_text
        .find('[')
        .ok_or_else(|| AkshareError::Empty("新浪流通股本响应缺少数组".into()))?;
    let amount_end = amount_text
        .rfind(']')
        .ok_or_else(|| AkshareError::Empty("新浪流通股本响应缺少数组尾".into()))?;
    let amount_arr: Vec<Value> = serde_json::from_str(&amount_text[amount_start..=amount_end])
        .map_err(|e| AkshareError::json(amount_url, e.to_string()))?;
    // date → outstanding_share（amount 单位：万股 → ×10000 = 股）
    let mut share_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for r in &amount_arr {
        if let (Some(d), Some(a)) = (
            r.get("date").and_then(Value::as_str),
            r.get("amount").and_then(|v| v.as_f64()),
        ) {
            share_map.insert(d.to_string(), a * 10000.0);
        }
    }

    // 3) 合并：按日期索引 outer join + ffill（akshare pd.merge + ffill）
    // 构建 date → (行) 有序列表，amount 缺失用前一条填充
    let mut merged: Vec<(String, Vec<Option<String>>)> = Vec::with_capacity(rows.len());
    let mut last_share: Option<f64> = None;
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        let date = f("date").unwrap_or_default();
        let share = share_map.get(&date).copied().or(last_share).or_else(|| {
            // 找不到当日，尝试最近的已上市日期之前的最后一个值
            let mut cand: Option<f64> = None;
            for (d, v) in &share_map {
                if d.as_str() <= date.as_str() {
                    cand = Some(*v);
                }
            }
            cand
        });
        last_share = share;
        let volume = f("volume").and_then(|s| s.parse::<f64>().ok());
        let turnover = match (volume, share) {
            (Some(v), Some(s)) if s > 0.0 => Some((v / s).to_string()),
            _ => None,
        };
        merged.push((
            date.clone(),
            vec![
                f("open"),
                f("high"),
                f("low"),
                f("close"),
                f("volume"),
                f("amount"),
                share.map(|s| s.to_string()),
                turnover,
            ],
        ));
    }

    // 4) 复权：qfq/hfq 乘/除因子
    let mut factor_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    if adjust == "qfq" || adjust == "hfq" {
        let method = if adjust == "hfq" { "hfq" } else { "qfq" };
        let url = format!("https://finance.sina.com.cn/realstock/company/{symbol}/{method}.js");
        if let Ok(text) = http.get_text(&url, &Map::new(), None) {
            if let (Some(s), Some(e)) = (text.find('{'), text.rfind('}')) {
                if let Ok(obj) = serde_json::from_str::<Value>(&text[s..=e]) {
                    if let Some(arr) = obj.get("data").and_then(Value::as_array) {
                        for r in arr {
                            if let (Some(d), Some(ft)) = (
                                r.get("date").and_then(Value::as_str),
                                r.get("factor").and_then(|v| v.as_f64()),
                            ) {
                                factor_map.insert(d.to_string(), ft);
                            }
                        }
                    }
                }
            }
        }
    }

    // 5) 构建输出：列序 date,open,high,low,close,volume,amount,outstanding_share,turnover
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(merged.len());
    let mut last_factor: Option<f64> = None;
    for (date, row) in &merged {
        if date.is_empty() {
            continue;
        }
        let mut ohlc: Vec<Option<f64>> = row[..4]
            .iter()
            .map(|v| v.as_deref().and_then(|s| s.parse::<f64>().ok()))
            .collect();
        if let Some(ft) = factor_map.get(date).copied().or(last_factor) {
            last_factor = Some(ft);
            if adjust == "hfq" {
                for v in ohlc.iter_mut() {
                    if let Some(x) = v.as_mut() {
                        *x *= ft;
                    }
                }
            } else if adjust == "qfq" && ft != 0.0 {
                for v in ohlc.iter_mut() {
                    if let Some(x) = v.as_mut() {
                        *x /= ft;
                    }
                }
            }
        }
        let mut out_row: Vec<Option<String>> = vec![Some(date.clone())];
        for v in &ohlc {
            out_row.push(v.map(|x| format!("{:.2}", x)));
        }
        out_row.extend_from_slice(&row[4..]);
        out.push(out_row);
    }
    const COLS: [&str; 9] = [
        "date",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "amount",
        "outstanding_share",
        "turnover",
    ];
    let df = Df::from_string_rows(&COLS, &out)?;

    // 6) 日期区间过滤 + 去重 + 数值化
    let start = if start_date.len() == 8 {
        format!(
            "{}-{}-{}",
            &start_date[0..4],
            &start_date[4..6],
            &start_date[6..8]
        )
    } else {
        start_date.to_string()
    };
    let end = if end_date.len() == 8 {
        format!(
            "{}-{}-{}",
            &end_date[0..4],
            &end_date[4..6],
            &end_date[6..8]
        )
    } else {
        end_date.to_string()
    };
    let keep: Vec<Option<String>> = {
        let col = df.inner().column("date").ok();
        match col {
            Some(c) => c
                .str()
                .map(|ca| {
                    ca.iter()
                        .map(|v| {
                            v.and_then(|s| {
                                if s >= start.as_str() && s <= end.as_str() {
                                    Some(s.to_string())
                                } else {
                                    None
                                }
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        }
    };
    let mut filtered: Vec<Vec<Option<String>>> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, row) in out.iter().enumerate() {
        if keep.get(i).map(|k| k.is_none()).unwrap_or(true) {
            continue;
        }
        // drop_duplicates(subset=[open..amount])
        let key = format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            row[1], row[2], row[3], row[4], row[5], row[6]
        );
        if seen.insert(key) {
            filtered.push(row.clone());
        }
    }
    let mut df = Df::from_string_rows(&COLS, &filtered)?;
    df.cast_numeric(&[
        "open",
        "high",
        "low",
        "close",
        "volume",
        "amount",
        "outstanding_share",
        "turnover",
    ])?;
    Ok(df)
}

/// 新浪财经-所有 A 股的实时行情数据（对应 akshare [`akshare.stock_zh_a_spot`]）。
///
/// `Market_Center.getHQNodeData`（node=hs_a）分页（num=80），列契约与 akshare 对齐：
/// `代码, 名称, 最新价, 涨跌额, 涨跌幅, 买入, 卖出, 昨收, 今开, 最高, 最低, 成交量, 成交额, 时间戳`。
/// 注：大量抓取会被新浪暂时封 IP（akshare 同限制）。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌额, 涨跌幅, 买入, 卖出, 昨收, 今开, 最高, 最低, 成交量, 成交额, 时间戳`
pub fn stock_zh_a_spot() -> Result<Df> {
    let http = HttpClient::default();
    // 1) 总条数 → 页数
    let count_url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCount";
    let mut count_params = Map::new();
    count_params.insert("node".into(), Value::String("hs_a".into()));
    let count_text = http.get_text(count_url, &count_params, None)?;
    let total_digits: String = count_text.chars().filter(|c| c.is_ascii_digit()).collect();
    let total: u64 = total_digits
        .parse()
        .map_err(|_| AkshareError::Empty("新浪 A 股总条数解析失败".into()))?;
    let total_pages = total.div_ceil(80);

    // 2) 分页抓取
    let url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
    let mut rows: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let params = json!({
            "page": page.to_string(),
            "num": "80",
            "sort": "symbol",
            "asc": "1",
            "node": "hs_a",
            "symbol": "",
            "_s_r_a": "page",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                    rows.extend(arr);
                }
            }
            Err(_) => break,
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    // 3) 列契约：代码(code),名称(name),最新价(trade),涨跌额(pricechange),涨跌幅(changepercent),
    //    买入(buy),卖出(sell),昨收(settlement),今开(open),最高(high),最低(low),
    //    成交量(volume),成交额(amount),时间戳(ticktime)
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for row in &rows {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("code"),
            f("name"),
            f("trade"),
            f("pricechange"),
            f("changepercent"),
            f("buy"),
            f("sell"),
            f("settlement"),
            f("open"),
            f("high"),
            f("low"),
            f("volume"),
            f("amount"),
            f("ticktime"),
        ]);
    }
    const COLS: [&str; 14] = [
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "买入",
        "卖出",
        "昨收",
        "今开",
        "最高",
        "最低",
        "成交量",
        "成交额",
        "时间戳",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[2..13])?;
    Ok(df)
}

/// 新浪财经-A股-CDR个股历史行情（对应 akshare [`akshare.stock_zh_a_cdr_daily`]）。
///
/// - `symbol`: 带市场前缀代码，如 `"sh689009"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// 与 [`stock_zh_a_daily`] 同源（`hisdata_klc2/klc_kl.js` + sina.js `d()` 解码），
/// 但无复权因子、无流通股本合并，仅按日期区间过滤。
///
/// # 返回列
/// `date, open, high, low, close, volume, amount`
pub fn stock_zh_a_cdr_daily(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let hist_url =
        format!("https://finance.sina.com.cn/realstock/company/{symbol}/hisdata_klc2/klc_kl.js");
    let http = HttpClient::default();
    let text = http.get_text(&hist_url, &Map::new(), None)?;
    let encoded = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪 CDR 日线响应缺少 '=' 分隔".into()))?
        .split(';')
        .next()
        .ok_or_else(|| AkshareError::Empty("新浪 CDR 日线响应缺少 ';' 分隔".into()))?
        .replace('"', "");
    let decoded = crate::core::js_engine::sina_js_decode(&encoded)?;
    let rows: Vec<Value> =
        serde_json::from_str(&decoded).map_err(|e| AkshareError::json(hist_url, e.to_string()))?;
    let start = if start_date.len() == 8 {
        format!(
            "{}-{}-{}",
            &start_date[0..4],
            &start_date[4..6],
            &start_date[6..8]
        )
    } else {
        start_date.to_string()
    };
    let end = if end_date.len() == 8 {
        format!(
            "{}-{}-{}",
            &end_date[0..4],
            &end_date[4..6],
            &end_date[6..8]
        )
    } else {
        end_date.to_string()
    };
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        let date = f("date").unwrap_or_default();
        if date < start || date > end {
            continue;
        }
        out.push(vec![
            Some(date),
            f("open"),
            f("high"),
            f("low"),
            f("close"),
            f("volume"),
            f("amount"),
        ]);
    }
    const COLS: [&str; 7] = ["date", "open", "high", "low", "close", "volume", "amount"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 新浪财经-B股个股历史行情（对应 akshare [`akshare.stock_zh_b_daily`]）。
///
/// - `symbol`: 带市场前缀代码，如 `"sh900901"`
/// - `start_date`/`end_date`: `YYYYMMDD`
/// - `adjust`: `""` / `"qfq"` / `"hfq"`
///
/// 与 [`stock_zh_a_daily`] 同源，但列契约为 `date, open, high, low, close,
/// volume, outstanding_share, turnover`（amount 合并为流通股本，换手 = volume/股本）。
///
/// # 返回列
/// `date, open, high, low, close, volume, outstanding_share, turnover`
pub fn stock_zh_b_daily(
    symbol: &str,
    start_date: &str,
    end_date: &str,
    adjust: &str,
) -> Result<Df> {
    let http = HttpClient::default();
    // 1) 历史行情：hisdata_klc2/klc_kl.js → sina.js d() 解码
    let hist_url =
        format!("https://finance.sina.com.cn/realstock/company/{symbol}/hisdata_klc2/klc_kl.js");
    let text = http.get_text(&hist_url, &Map::new(), None)?;
    let encoded = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪 B 股日线响应缺少 '=' 分隔".into()))?
        .split(';')
        .next()
        .ok_or_else(|| AkshareError::Empty("新浪 B 股日线响应缺少 ';' 分隔".into()))?
        .replace('"', "");
    let decoded = crate::core::js_engine::sina_js_decode(&encoded)?;
    let rows: Vec<Value> =
        serde_json::from_str(&decoded).map_err(|e| AkshareError::json(hist_url, e.to_string()))?;

    // 2) 流通股本：StockService.getAmountBySymbol → `var X=([{date,amount},...])`
    let amount_url = format!(
        "https://stock.finance.sina.com.cn/stock/api/jsonp.php/var%20KKE_ShareAmount_{symbol}=/StockService.getAmountBySymbol?_=20&symbol={symbol}"
    );
    let amount_text = http.get_text(&amount_url, &Map::new(), None)?;
    let amount_start = amount_text
        .find('[')
        .ok_or_else(|| AkshareError::Empty("新浪 B 股流通股本响应缺少数组".into()))?;
    let amount_end = amount_text
        .rfind(']')
        .ok_or_else(|| AkshareError::Empty("新浪 B 股流通股本响应缺少数组尾".into()))?;
    let amount_arr: Vec<Value> = serde_json::from_str(&amount_text[amount_start..=amount_end])
        .map_err(|e| AkshareError::json(amount_url, e.to_string()))?;
    let mut share_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for r in &amount_arr {
        if let (Some(d), Some(a)) = (
            r.get("date").and_then(Value::as_str),
            r.get("amount").and_then(|v| v.as_f64()),
        ) {
            share_map.insert(d.to_string(), a * 10000.0);
        }
    }

    // 3) 合并：outer + ffill（akshare merge + ffill）
    let mut merged: Vec<(String, Vec<Option<String>>)> = Vec::with_capacity(rows.len());
    let mut last_share: Option<f64> = None;
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        let date = f("date").unwrap_or_default();
        let share = share_map.get(&date).copied().or(last_share).or_else(|| {
            let mut cand: Option<f64> = None;
            for (d, v) in &share_map {
                if d.as_str() <= date.as_str() {
                    cand = Some(*v);
                }
            }
            cand
        });
        last_share = share;
        let volume = f("volume").and_then(|s| s.parse::<f64>().ok());
        let turnover = match (volume, share) {
            (Some(v), Some(s)) if s > 0.0 => Some((v / s).to_string()),
            _ => None,
        };
        merged.push((
            date.clone(),
            vec![
                f("open"),
                f("high"),
                f("low"),
                f("close"),
                f("volume"),
                share.map(|s| s.to_string()),
                turnover,
            ],
        ));
    }

    // 4) 复权：qfq/hfq 乘/除因子
    let mut factor_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    if adjust == "qfq" || adjust == "hfq" {
        let method = if adjust == "hfq" { "hfq" } else { "qfq" };
        let url = format!("https://finance.sina.com.cn/realstock/company/{symbol}/{method}.js");
        if let Ok(text) = http.get_text(&url, &Map::new(), None) {
            if let (Some(s), Some(e)) = (text.find('{'), text.rfind('}')) {
                if let Ok(obj) = serde_json::from_str::<Value>(&text[s..=e]) {
                    if let Some(arr) = obj.get("data").and_then(Value::as_array) {
                        for r in arr {
                            if let (Some(d), Some(ft)) = (
                                r.get("date").and_then(Value::as_str),
                                r.get("factor").and_then(|v| v.as_f64()),
                            ) {
                                factor_map.insert(d.to_string(), ft);
                            }
                        }
                    }
                }
            }
        }
    }

    // 5) 构建输出：date,open,high,low,close,volume,outstanding_share,turnover
    let start = if start_date.len() == 8 {
        format!(
            "{}-{}-{}",
            &start_date[0..4],
            &start_date[4..6],
            &start_date[6..8]
        )
    } else {
        start_date.to_string()
    };
    let end = if end_date.len() == 8 {
        format!(
            "{}-{}-{}",
            &end_date[0..4],
            &end_date[4..6],
            &end_date[6..8]
        )
    } else {
        end_date.to_string()
    };
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(merged.len());
    let mut last_factor: Option<f64> = None;
    for (date, row) in &merged {
        if date.is_empty() || date.as_str() < start.as_str() || date.as_str() > end.as_str() {
            continue;
        }
        let mut ohlc: Vec<Option<f64>> = row[..4]
            .iter()
            .map(|v| v.as_deref().and_then(|s| s.parse::<f64>().ok()))
            .collect();
        if let Some(ft) = factor_map.get(date).copied().or(last_factor) {
            last_factor = Some(ft);
            if adjust == "hfq" {
                for v in ohlc.iter_mut() {
                    if let Some(x) = v.as_mut() {
                        *x *= ft;
                    }
                }
            } else if adjust == "qfq" && ft != 0.0 {
                for v in ohlc.iter_mut() {
                    if let Some(x) = v.as_mut() {
                        *x /= ft;
                    }
                }
            }
        }
        let mut out_row: Vec<Option<String>> = vec![Some(date.clone())];
        for v in &ohlc {
            out_row.push(v.map(|x| format!("{:.2}", x)));
        }
        out_row.extend_from_slice(&row[4..]);
        out.push(out_row);
    }
    const COLS: [&str; 8] = [
        "date",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "outstanding_share",
        "turnover",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 新浪财经-B股分钟数据（对应 akshare [`akshare.stock_zh_b_minute`]）。
///
/// - `symbol`: 带市场前缀代码，如 `"sh900901"`
/// - `period`: `"1"` / `"5"` / `"15"` / `"30"` / `"60"`
/// - `adjust`: `""` / `"qfq"` / `"hfq"`
///
/// 与 [`stock_zh_a_minute`] 同源（`CN_MarketDataService.getKLineData` JSONP），
/// 取前 6 列。
///
/// # 返回列
/// `day, open, high, low, close, volume`
pub fn stock_zh_b_minute(symbol: &str, period: &str, _adjust: &str) -> Result<Df> {
    let url = "https://quotes.sina.cn/cn/api/jsonp_v2.php/=/CN_MarketDataService.getKLineData";
    let params = json!({
        "symbol": symbol,
        "scale": period,
        "datalen": "1970",
    });
    let http = HttpClient::default();
    let text = http.get_text(url, params.as_object().expect("静态参数"), None)?;
    let start = text
        .find("=(")
        .ok_or_else(|| AkshareError::Empty("新浪 B 股分钟线响应缺少 '=(' 前缀".into()))?;
    let body = &text[start + 2..];
    let end = body
        .find(");")
        .ok_or_else(|| AkshareError::Empty("新浪 B 股分钟线响应缺少 ');' 后缀".into()))?;
    let json_text = &body[..end];
    let rows: Vec<Value> =
        serde_json::from_str(json_text).map_err(|e| AkshareError::json(url, e.to_string()))?;
    let df = Df::from_json_rows(&rows)?;
    let names = df.column_names();
    let take: Vec<&str> = names.iter().take(6).map(String::as_str).collect();
    df.select(&take)
}

/// 新浪财经-港股-个股历史行情（对应 akshare [`akshare.stock_hk_daily`]）。
///
/// - `symbol`: 港股代码，如 `"00981"`（可由 [`stock_hk_spot`] 获取）
/// - `_adjust`: `""`（不复权，当前实现）；`"qfq"` / `"hfq"` 复权因子分支暂未落地
///
/// 走 `finance.sina.com.cn/stock/hkstock/{symbol}/klc2_kl.js` + sina.js `d()` 解码。
///
/// # 返回列
/// 不复权：`date, open, high, low, close, volume, amount`（原键列）
pub fn stock_hk_daily(symbol: &str, _adjust: &str) -> Result<Df> {
    let hist_url = format!("https://finance.sina.com.cn/stock/hkstock/{symbol}/klc2_kl.js");
    let http = HttpClient::default();
    let text = http.get_text(&hist_url, &Map::new(), None)?;
    let encoded = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪港股日线响应缺少 '=' 分隔".into()))?
        .split(';')
        .next()
        .ok_or_else(|| AkshareError::Empty("新浪港股日线响应缺少 ';' 分隔".into()))?
        .replace('"', "");
    let decoded = crate::core::js_engine::sina_js_decode(&encoded)?;
    let rows: Vec<Value> =
        serde_json::from_str(&decoded).map_err(|e| AkshareError::json(hist_url, e.to_string()))?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("date"),
            f("open"),
            f("high"),
            f("low"),
            f("close"),
            f("volume"),
            f("amount"),
        ]);
    }
    const COLS: [&str; 7] = ["date", "open", "high", "low", "close", "volume", "amount"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 新浪行业-板块行情（对应 akshare [`akshare.stock_sector_spot`]）。
///
/// - `indicator`: `"新浪行业"` / `"启明星行业"` / `"概念"` / `"地域"` / `"行业"`
///
/// 响应为 `{key: "v1,v2,...", ...}`（值以逗号分隔的 13 字段），解析后按位置列名。
///
/// # 返回列
/// `label, 板块, 公司家数, 平均价格, 涨跌额, 涨跌幅, 总成交量, 总成交额,
/// 股票代码, 个股-涨跌幅, 个股-当前价, 个股-涨跌额, 股票名称`
pub fn stock_sector_spot(indicator: &str) -> Result<Df> {
    let (url, params) = match indicator {
        "新浪行业" => (
            "http://vip.stock.finance.sina.com.cn/q/view/newSinaHy.php",
            None,
        ),
        "启明星行业" => ("http://biz.finance.sina.com.cn/hq/qmxIndustryHq.php", None),
        "概念" => (
            "http://money.finance.sina.com.cn/q/view/newFLJK.php",
            Some(("param", "class")),
        ),
        "地域" => (
            "http://money.finance.sina.com.cn/q/view/newFLJK.php",
            Some(("param", "area")),
        ),
        "行业" => (
            "http://money.finance.sina.com.cn/q/view/newFLJK.php",
            Some(("param", "industry")),
        ),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 indicator: {other}，可选 新浪行业/启明星行业/概念/地域/行业"
            )))
        }
    };
    let http = HttpClient::default();
    let params_map = match params {
        Some((k, v)) => {
            let mut m = Map::new();
            m.insert(k.into(), Value::String(v.into()));
            m
        }
        None => Map::new(),
    };
    let text = http.get_text(url, &params_map, None)?;
    // 取首 `{` 起的 JSON 对象文本（`{"key":"v1,v2,...", ...}`）
    let start = text
        .find('{')
        .ok_or_else(|| AkshareError::Empty("新浪板块行情响应缺少对象".into()))?;
    let sub = &text[start..];
    let end = sub
        .rfind('}')
        .ok_or_else(|| AkshareError::Empty("新浪板块行情响应缺少对象尾".into()))?;
    let obj: Value =
        serde_json::from_str(&sub[..=end]).map_err(|e| AkshareError::json(url, e.to_string()))?;
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    if let Some(map) = obj.as_object() {
        for (label, v) in map {
            let csv = v.as_str().unwrap_or_default();
            let fields: Vec<&str> = csv.split(',').collect();
            let pick = |i: usize| {
                fields
                    .get(i)
                    .map(|s| Some((*s).to_string()))
                    .unwrap_or(None)
            };
            out.push(vec![
                Some(label.clone()),
                pick(1),
                pick(2),
                pick(3),
                pick(4),
                pick(5),
                pick(6),
                pick(7),
                pick(8),
                pick(9),
                pick(10),
                pick(11),
                pick(12),
            ]);
        }
    }
    const COLS: [&str; 13] = [
        "label",
        "板块",
        "公司家数",
        "平均价格",
        "涨跌额",
        "涨跌幅",
        "总成交量",
        "总成交额",
        "股票代码",
        "个股-涨跌幅",
        "个股-当前价",
        "个股-涨跌额",
        "股票名称",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&[
        "公司家数",
        "平均价格",
        "涨跌额",
        "涨跌幅",
        "总成交量",
        "总成交额",
        "个股-涨跌幅",
        "个股-当前价",
        "个股-涨跌额",
    ])?;
    Ok(df)
}

/// 新浪行业-板块行情-成份详情（对应 akshare [`akshare.stock_sector_detail`]）。
///
/// - `sector`: [`stock_sector_spot`] 返回的 `label` 值（如 `"gn_gfgn"`）
///
/// `Market_Center.getHQNodeData` 分页（num=80），原键列 + 数值化。
///
/// # 返回列
/// 原键列（`symbol, code, name, trade, pricechange, ...`）
pub fn stock_sector_detail(sector: &str) -> Result<Df> {
    let http = HttpClient::default();
    // 1) 总条数 → 页数
    let count_url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCount";
    let mut count_params = Map::new();
    count_params.insert("node".into(), Value::String(sector.into()));
    let count_text = http.get_text(count_url, &count_params, None)?;
    let total: u64 = count_text.trim().parse().unwrap_or(0);
    let total_pages = total.div_ceil(80);

    // 2) 分页抓取
    let url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
    let mut rows: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let params = json!({
            "page": page.to_string(),
            "num": "80",
            "sort": "symbol",
            "asc": "1",
            "node": sector,
            "symbol": "",
            "_s_r_a": "page",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                    rows.extend(arr);
                }
            }
            Err(_) => break,
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    let df = Df::from_json_rows(&rows)?;
    let names = df.column_names();
    let numeric: Vec<&str> = names
        .iter()
        .filter(|n| {
            matches!(
                n.as_str(),
                "trade"
                    | "pricechange"
                    | "changepercent"
                    | "buy"
                    | "sell"
                    | "settlement"
                    | "open"
                    | "high"
                    | "low"
                    | "volume"
                    | "amount"
                    | "per"
                    | "pb"
                    | "mktcap"
                    | "nmc"
                    | "turnoverratio"
            )
        })
        .map(String::as_str)
        .collect();
    let mut df = df;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 腾讯财经-历史分笔数据（对应 akshare [`akshare.stock_zh_a_tick_tx_js`]）。
///
/// - `symbol`: 股票代码（带市场前缀），如 `"sz000001"`
///
/// `stock.gtimg.cn/data/index.php` 分页（appn=detail），响应
/// `var v_detail_data_xxx=[0,"idx/时间/价格/变动/量/额/性质|..."]`，
/// 逐行 `|` 分隔、字段 `/` 分隔，取后 6 列。
///
/// # 返回列
/// `成交时间, 成交价格, 价格变动, 成交量, 成交金额, 性质`
pub fn stock_zh_a_tick_tx_js(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    let mut page = 0usize;
    loop {
        let params = json!({
            "appn": "detail",
            "action": "data",
            "c": symbol,
            "p": page.to_string(),
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let text = match http.get_text("http://stock.gtimg.cn/data/index.php", &params, None) {
            Ok(t) => t,
            Err(_) => break,
        };
        let start = match text.find('[') {
            Some(i) => i,
            None => break,
        };
        // `[0,"idx/time/...|..."]`：首元素为索引，第二个字符串元素为 `|` 分隔的逐笔串
        let body = &text[start..];
        let arr_end = body
            .find(']')
            .ok_or_else(|| AkshareError::Empty("腾讯分笔响应缺少数组尾".into()))?;
        let arr_body = &body[1..arr_end];
        let mut parts = arr_body.splitn(2, ',');
        let _idx = parts.next();
        let lines = parts
            .next()
            .map(|s| s.trim_matches('"'))
            .unwrap_or_default();
        let line_count = lines.split('|').count();
        for line in lines.split('|') {
            let fields: Vec<&str> = line.split('/').collect();
            let pick = |i: usize| {
                fields
                    .get(i)
                    .map(|s| Some((*s).to_string()))
                    .unwrap_or(None)
            };
            out.push(vec![pick(1), pick(2), pick(3), pick(4), pick(5), pick(6)]);
        }
        if line_count == 0 {
            break;
        }
        page += 1;
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    const COLS: [&str; 6] = [
        "成交时间",
        "成交价格",
        "价格变动",
        "成交量",
        "成交金额",
        "性质",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["成交价格", "价格变动", "成交量", "成交金额"])?;
    Ok(df)
}

/// 新浪财经-日内分时数据（对应 akshare [`akshare.stock_intraday_sina`]）。
///
/// - `symbol`: 股票代码（带市场前缀），如 `"sz000001"`
/// - `date`: 交易日 `YYYYMMDD`
///
/// `CN_Bill.GetBillList` 分页（num=60，按 ticktime 升序），原键列 + 数值化。
///
/// # 返回列
/// 原键列（`ticktime, price, volume, prev_price, ...`）
pub fn stock_intraday_sina(symbol: &str, date: &str) -> Result<Df> {
    let headers: &[(&str, &str)] = &[
        (
            "Referer",
            &format!("https://vip.stock.finance.sina.com.cn/quotes_service/view/cn_bill.php?symbol={symbol}"),
        ),
        (
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/107.0.0.0 Safari/537.36",
        ),
    ];
    let http = HttpClient::default();
    let day = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);

    // 1) 总条数 → 页数
    let count_url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_Bill.GetBillListCount";
    let mut count_params = Map::new();
    count_params.insert("symbol".into(), Value::String(symbol.into()));
    count_params.insert("num".into(), Value::String("60".into()));
    count_params.insert("page".into(), Value::String("1".into()));
    count_params.insert("sort".into(), Value::String("ticktime".into()));
    count_params.insert("asc".into(), Value::String("0".into()));
    count_params.insert("volume".into(), Value::String("0".into()));
    count_params.insert("amount".into(), Value::String("0".into()));
    count_params.insert("type".into(), Value::String("0".into()));
    count_params.insert("day".into(), Value::String(day.clone()));
    let count_text = http.get_text_with_headers(count_url, &count_params, headers, None)?;
    let total: u64 = count_text.trim().parse().unwrap_or(0);
    let total_pages = total.div_ceil(60);

    // 2) 分页抓取
    let url =
        "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_Bill.GetBillList";
    let mut rows: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let mut params = Map::new();
        params.insert("symbol".into(), Value::String(symbol.into()));
        params.insert("num".into(), Value::String("60".into()));
        params.insert("page".into(), Value::String(page.to_string()));
        params.insert("sort".into(), Value::String("ticktime".into()));
        params.insert("asc".into(), Value::String("0".into()));
        params.insert("volume".into(), Value::String("0".into()));
        params.insert("amount".into(), Value::String("0".into()));
        params.insert("type".into(), Value::String("0".into()));
        params.insert("day".into(), Value::String(day.clone()));
        match http.get_text_with_headers(url, &params, headers, None) {
            Ok(t) => {
                if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                    rows.extend(arr);
                }
            }
            Err(_) => break,
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    // 按 ticktime 升序（akshare sort_values(["ticktime"], ignore_index)）
    rows.sort_by(|a, b| {
        let ta = a.get("ticktime").and_then(Value::as_str).unwrap_or("");
        let tb = b.get("ticktime").and_then(Value::as_str).unwrap_or("");
        ta.cmp(tb)
    });
    let df = Df::from_json_rows(&rows)?;
    let names = df.column_names();
    let numeric: Vec<&str> = names
        .iter()
        .filter(|n| {
            matches!(
                n.as_str(),
                "price" | "volume" | "prev_price" | "amount" | "ticktime"
            )
        })
        .map(String::as_str)
        .collect();
    let mut df = df;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 东财-行情中心-两网及退市（对应 akshare [`akshare.stock_staq_net_stop`]）。
///
/// 5.push2 clist（`fs=m:0 s:3`，`fields=f12,f14`），diff 为「序号→行」对象，
/// 输出 `序号, 代码, 名称`（序号 1 基）。
///
/// # 返回列
/// `序号, 代码, 名称`
pub fn stock_staq_net_stop() -> Result<Df> {
    const URL: &str = "https://5.push2.eastmoney.com/api/qt/clist/get";
    let params = json!({
        "pn": "1",
        "pz": "50000",
        "po": "1",
        "np": "2",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "fid": "f3",
        "fs": "m:0 s:3",
        "fields": "f12,f14",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(URL, &params, None)?;
    let diff = value
        .get("data")
        .and_then(|d| d.get("diff"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut rows_vec: Vec<Value> = Vec::new();
    match diff {
        Value::Array(arr) => rows_vec = arr,
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            rows_vec = entries.into_iter().map(|(_, v)| v).collect();
        }
        _ => {}
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows_vec.len());
    for (i, row) in rows_vec.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        out.push(vec![Some((i + 1).to_string()), f("f12"), f("f14")]);
    }
    const COLS: [&str; 3] = ["序号", "代码", "名称"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["序号"])?;
    Ok(df)
}

/// 东财-行情中心-分时数据（对应 akshare [`akshare.stock_intraday_em`]）。
///
/// - `symbol`: 股票代码，如 `"000001"`
///
/// `70.push2.eastmoney.com/api/qt/stock/details/sse`（SSE 事件流），akshare 只取
/// 首个事件；本实现一次性 GET 读取响应体并解析首个 `data: {...}` 事件。
///
/// # 返回列
/// `时间, 成交价, 手数, 买卖盘性质`
pub fn stock_intraday_em(symbol: &str) -> Result<Df> {
    let market_code = if symbol.starts_with('6') { "1" } else { "0" };
    let url = "https://70.push2.eastmoney.com/api/qt/stock/details/sse";
    let params = json!({
        "fields1": "f1,f2,f3,f4",
        "fields2": "f51,f52,f53,f54,f55",
        "mpi": "2000",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "pos": "-0",
        "secid": format!("{market_code}.{symbol}"),
        "wbp2u": "|0|0|0|web",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let text = http.get_text(url, &params, None)?;
    // 解析首个 `data: {...}` 事件（akshare 只取第一个事件后 break）
    let marker = "data: ";
    let Some(pos) = text.find(marker) else {
        return Df::from_string_rows(&["时间", "成交价", "手数", "买卖盘性质"], &[]);
    };
    let body = &text[pos + marker.len()..];
    let end = body
        .find("\n\n")
        .or_else(|| body.find('\n'))
        .unwrap_or(body.len());
    let event: Value =
        serde_json::from_str(&body[..end]).map_err(|e| AkshareError::json(url, e.to_string()))?;
    let details = event
        .get("data")
        .and_then(|d| d.get("details"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(details.len());
    for line in details.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        let nature = match pick(3).as_deref() {
            Some("2") => Some("买盘".to_string()),
            Some("1") => Some("卖盘".to_string()),
            Some("4") => Some("中性盘".to_string()),
            _ => pick(3),
        };
        out.push(vec![pick(0), pick(1), pick(2), nature]);
    }
    const COLS: [&str; 4] = ["时间", "成交价", "手数", "买卖盘性质"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["成交价", "手数"])?;
    Ok(df)
}

/// 财新网-财新数据通-新闻（对应 akshare [`akshare.stock_news_main_cx`]）。
///
/// `cxdata.caixin.com/api/dataplus/sjtPc/news`（pageNum=1, pageSize=100），
/// 取 `data.data` 的 `tag, summary, url` 三列并去空。
///
/// # 返回列
/// `tag, summary, url`
pub fn stock_news_main_cx() -> Result<Df> {
    let url = "https://cxdata.caixin.com/api/dataplus/sjtPc/news";
    let params = json!({
        "pageNum": "1",
        "pageSize": "100",
        "showLabels": "true",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        url,
        &params,
        &[
            (
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36",
            ),
            ("Referer", "https://cxdata.caixin.com/index/newsTab?tab=latest"),
        ],
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![f("tag"), f("summary"), f("url")]);
    }
    // dropna：三列均非空才保留（对应 akshare `df.dropna()`）
    out.retain(|row| row.iter().all(|v| v.is_some()));
    Df::from_string_rows(&["tag", "summary", "url"], &out)
}

/// 百度股市通-热搜股票（对应 akshare [`akshare.stock_hot_search_baidu`]）。
///
/// - `symbol`: `"全市场"` / `"A股"` / `"港股"` / `"美股"`
/// - `date`: 日期 `YYYYMMDD`；`time`: `"今日"` / `"1小时"`
///
/// `finance.pae.baidu.com/selfselect/listsugrecomm`，取 `Result.list.body`。
///
/// # 返回列
/// `名称/代码, 涨跌幅, 综合热度`
pub fn stock_hot_search_baidu(symbol: &str, date: &str, time: &str) -> Result<Df> {
    let market = match symbol {
        "全市场" => "all",
        "A股" => "ab",
        "港股" => "hk",
        "美股" => "us",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全市场/A股/港股/美股"
            )))
        }
    };
    let params = json!({
        "bizType": "wisexmlnew",
        "dsp": "iphone",
        "product": "search",
        "style": "tablelist",
        "market": market,
        "type": time,
        "day": date,
        "hour": chrono::Local::now().format("%H").to_string(),
        "pn": "0",
        "rn": "12",
        "finClientType": "pc",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(
        "https://finance.pae.baidu.com/selfselect/listsugrecomm",
        &params,
        None,
    )?;
    let rows = value
        .get("Result")
        .and_then(|r| r.get("list"))
        .and_then(|l| l.get("body"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![f("name"), f("pxChangeRate"), f("heat")]);
    }
    const COLS: [&str; 3] = ["名称/代码", "涨跌幅", "综合热度"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["综合热度"])?;
    Ok(df)
}

// === BATCH36-I 金十数据中心-微博舆情（datacenter-api.jin10.com/weibo）===
//
// 对应 akshare `stock/stock_weibo_nlp.py`。需携带 `x-app-id` 等请求头。

const JIN10_WEIBO_HEADERS: &[(&str, &str)] = &[
    ("authority", "datacenter-api.jin10.com"),
    ("accept", "*/*"),
    ("x-app-id", "rU6QIu7JHe2gOUeR"),
    ("x-csrf-token", ""),
    ("x-version", "1.0.0"),
    ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.116 Safari/537.36"),
    ("origin", "https://datacenter.jin10.com"),
    ("referer", "https://datacenter.jin10.com/market"),
    ("accept-language", "zh-CN,zh;q=0.9,en;q=0.8"),
];

/// 金十数据中心-实时监控-微博舆情报告（对应 akshare [`akshare.stock_js_weibo_report`]）。
///
/// - `time_period`: `"CNHOUR2"` / `"CNHOUR6"` / `"CNHOUR12"` / `"CNHOUR24"` /
///   `"CNDAY7"` / `"CNDAY30"`
///
/// # 返回列
/// 原键列（`data` 数组），`rate` 数值化
pub fn stock_js_weibo_report(time_period: &str) -> Result<Df> {
    let params = json!({
        "timescale": time_period,
        "_": chrono::Utc::now().timestamp_millis().to_string(),
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://datacenter-api.jin10.com/weibo/list",
        &params,
        JIN10_WEIBO_HEADERS,
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let df = Df::from_json_rows(&rows)?;
    let mut df = df;
    df.cast_numeric(&["rate"])?;
    Ok(df)
}

/// 金十数据中心-实时监控-微博舆情时间档（对应 akshare [`akshare.stock_js_weibo_nlp_time`]）。
///
/// 返回 `data.timescale`（时间档 → 中文名 dict），展开为 `item, value` 两列。
///
/// # 返回列
/// `item, value`
pub fn stock_js_weibo_nlp_time() -> Result<Df> {
    let params = json!({ "_": chrono::Utc::now().timestamp_millis().to_string() });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://datacenter-api.jin10.com/weibo/config",
        &params,
        JIN10_WEIBO_HEADERS,
        None,
    )?;
    let ts = value
        .get("data")
        .and_then(|d| d.get("timescale"))
        .cloned()
        .unwrap_or(Value::Null);
    let obj = ts.as_object().cloned().unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(obj.len());
    for (k, v) in obj {
        out.push(vec![Some(k), v.as_str().map(str::to_string)]);
    }
    Df::from_string_rows(&["item", "value"], &out)
}

/// 美股/港股目标价（对应 akshare [`akshare.stock_price_js`]）。
///
/// - `symbol`: `"us"` / `"hk"`
///
/// `calendar-api.ushknews.com/getWebTargetPriceList`，取 `data.list`，
/// 位置式列名后 select 8 列。
///
/// # 返回列
/// `评级, 最新目标价, 先前目标价, 机构名称, 日期, 公司名称, 目标价调整, 涨跌幅`
pub fn stock_price_js(symbol: &str) -> Result<Df> {
    let params = json!({ "limit": "20", "category": symbol });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let headers: &[(&str, &str)] = &[
        ("accept", "application/json, text/plain, */*"),
        ("origin", "https://www.ushknews.com"),
        ("referer", "https://www.ushknews.com/"),
        ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/107.0.0.0 Safari/537.36"),
        ("x-app-id", "BNsiR9uq7yfW0LVz"),
        ("x-version", "1.0.0"),
    ];
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://calendar-api.ushknews.com/getWebTargetPriceList",
        &params,
        headers,
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 位置式列名（akshare 10 列占位表，取 8 个有效列）
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = r.as_object().cloned().unwrap_or_default();
        let values: Vec<Option<String>> = obj
            .values()
            .take(10)
            .map(|v| match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect();
        let pick = |i: usize| values.get(i).cloned().flatten();
        out.push(vec![
            pick(2),
            pick(4),
            pick(5),
            pick(6),
            pick(7),
            pick(8),
            pick(9),
            pick(1),
        ]);
    }
    const COLS: [&str; 8] = [
        "评级",
        "最新目标价",
        "先前目标价",
        "机构名称",
        "日期",
        "公司名称",
        "目标价调整",
        "涨跌幅",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["最新目标价", "先前目标价", "涨跌幅"])?;
    Ok(df)
}

/// 同花顺-港股-分红派息（对应 akshare [`akshare.stock_hk_fhpx_detail_ths`]）。
///
/// - `symbol`: 港股代码，如 `"0700"`
///
/// `basic.10jqka.com.cn/176/HK{symbol}/bonus.html`，解析首张 HTML 表，
/// 剔除派息日/除净日缺失行，日期列归一。
///
/// # 返回列
/// `公告日期, 方案, 除净日, 派息日, 过户日期起止日-起始, 过户日期起止日-截止, 类型, 进度, 以股代息`
pub fn stock_hk_fhpx_detail_ths(symbol: &str) -> Result<Df> {
    let url = format!("https://basic.10jqka.com.cn/176/HK{symbol}/bonus.html");
    let http = HttpClient::default();
    let text = http.get_text_with_headers(
        &url,
        &Map::new(),
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4389.90 Safari/537.36",
        )],
        None,
    )?;
    let tables = crate::core::html::read_html_tables(&text)?;
    let table = tables
        .first()
        .ok_or_else(|| AkshareError::Empty("同花顺港股分红页面缺少表格".into()))?;
    // 首行为表头，跳过；剔除 派息日/除净日 为空的行
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for r in table.iter().skip(1) {
        let cells: Vec<Option<String>> = r.iter().map(|s| Some(s.clone())).collect();
        let has_date = cells
            .iter()
            .any(|c| c.as_deref().map(|s| s.contains('-')).unwrap_or(false));
        if !has_date {
            continue;
        }
        rows.push(cells);
    }
    const COLS: [&str; 9] = [
        "公告日期",
        "方案",
        "除净日",
        "派息日",
        "过户日期起止日-起始",
        "过户日期起止日-截止",
        "类型",
        "进度",
        "以股代息",
    ];
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_date(&[
        "公告日期",
        "除净日",
        "派息日",
        "过户日期起止日-起始",
        "过户日期起止日-截止",
    ])?;
    Ok(df)
}

// === BATCH36-J 新浪美股（US_CategoryService.getList，js_hash_text 哈希）===
//
// 对应 akshare `stock/stock_us_sina.py`。URL 拼接需 `d()` 哈希（`us_sina_hash.js`），
// 响应为 `var xxx=({...});` JSONP，取 `({` 与 `);` 之间的对象。

/// 新浪美股分页数据公共拉取：返回全部页的 `data` 数组行。
fn us_sina_pages(rows_out: &mut Vec<Value>) -> Result<()> {
    let http = HttpClient::default();
    let mut payload = Map::new();
    payload.insert("page".into(), Value::String("1".into()));
    payload.insert("num".into(), Value::String("20".into()));
    payload.insert("sort".into(), Value::String("".into()));
    payload.insert("asc".into(), Value::String("0".into()));
    payload.insert("market".into(), Value::String("".into()));
    payload.insert("id".into(), Value::String("".into()));

    // 首页：总条数 → 页数
    let first = us_sina_get_page(&http, &payload)?;
    let count = first.get("count").and_then(Value::as_u64).unwrap_or(0);
    if let Some(arr) = first.get("data").and_then(Value::as_array) {
        rows_out.extend(arr.iter().cloned());
    }
    let total_pages = count.div_ceil(20);
    for page in 2..=total_pages {
        payload.insert("page".into(), Value::String(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match us_sina_get_page(&http, &payload) {
            Ok(v) => {
                if let Some(arr) = v.get("data").and_then(Value::as_array) {
                    rows_out.extend(arr.iter().cloned());
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}

/// 单页拉取：`d()` 哈希拼 URL → JSONP 解包。
fn us_sina_get_page(http: &HttpClient, payload: &Map<String, Value>) -> Result<Value> {
    let page = payload.get("page").and_then(Value::as_str).unwrap_or("1");
    let to_hash = format!("US_CategoryService.getList?page={page}&num=20&sort=&asc=0&market=&id=");
    let hash = crate::core::js_engine::us_sina_hash_decode(&to_hash)?;
    let url = format!(
        "http://stock.finance.sina.com.cn/usstock/api/jsonp.php/IO.XSRV2.CallbackList[{hash}]/US_CategoryService.getList"
    );
    let text = http.get_text(&url, payload, None)?;
    // `var xxx=({...});` → 取 `({` 与 `);` 之间的对象
    let start = text
        .find("({")
        .map(|i| i + 1)
        .ok_or_else(|| AkshareError::Empty("新浪美股响应缺少 '({' 前缀".into()))?;
    let end = text
        .find(");")
        .ok_or_else(|| AkshareError::Empty("新浪美股响应缺少 ');' 后缀".into()))?;
    serde_json::from_str(&text[start..end]).map_err(|e| AkshareError::json(&url, e.to_string()))
}

/// 新浪财经-美股-股票代码与名称（对应 akshare [`akshare.get_us_stock_name`]）。
///
/// # 返回列
/// `name, cname, symbol`
pub fn get_us_stock_name() -> Result<Df> {
    let mut rows: Vec<Value> = Vec::new();
    us_sina_pages(&mut rows)?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![f("name"), f("cname"), f("symbol")]);
    }
    Df::from_string_rows(&["name", "cname", "symbol"], &out)
}

/// 新浪财经-所有美股实时行情（对应 akshare [`akshare.stock_us_spot`]）。
///
/// 注：延迟 15 分钟；大量抓取易被封 IP（akshare 同限制）。
///
/// # 返回列
/// 原键列（`symbol, code, name, trade, pricechange, ...`）
pub fn stock_us_spot() -> Result<Df> {
    let mut rows: Vec<Value> = Vec::new();
    us_sina_pages(&mut rows)?;
    let df = Df::from_json_rows(&rows)?;
    let names = df.column_names();
    let numeric: Vec<&str> = names
        .iter()
        .filter(|n| {
            matches!(
                n.as_str(),
                "trade"
                    | "pricechange"
                    | "changepercent"
                    | "buy"
                    | "sell"
                    | "settlement"
                    | "open"
                    | "high"
                    | "low"
                    | "volume"
                    | "amount"
                    | "per"
                    | "pb"
                    | "mktcap"
                    | "nmc"
                    | "turnoverratio"
            )
        })
        .map(String::as_str)
        .collect();
    let mut df = df;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 新浪财经-美股-个股历史行情（对应 akshare [`akshare.stock_us_daily`]）。
///
/// - `symbol`: 美股代码，如 `"FB"`
/// - `adjust`: `""`（不复权）/ `"qfq"`（前复权）/ `"hfq"`（后复权）
///
/// 走 `finance.sina.com.cn/staticdata/us/{symbol}`（sina.js 解密），
/// 复权因子走 `us_stock/company/reinstatement/{symbol}_qfq.js`。
///
/// # 返回列
/// `date, open, high, low, close, volume`（原键列 + date）
pub fn stock_us_daily(symbol: &str, adjust: &str) -> Result<Df> {
    let http = HttpClient::default();
    // 1) 历史行情：staticdata/us/{symbol} → sina.js d() 解码
    let hist_url = format!("https://finance.sina.com.cn/staticdata/us/{symbol}");
    let text = http.get_text(&hist_url, &Map::new(), None)?;
    let encoded = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪美股日线响应缺少 '=' 分隔".into()))?
        .split(';')
        .next()
        .ok_or_else(|| AkshareError::Empty("新浪美股日线响应缺少 ';' 分隔".into()))?
        .replace('"', "");
    let decoded = crate::core::js_engine::sina_js_decode(&encoded)?;
    let rows: Vec<Value> =
        serde_json::from_str(&decoded).map_err(|e| AkshareError::json(&hist_url, e.to_string()))?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("date"),
            f("open"),
            f("high"),
            f("low"),
            f("close"),
            f("volume"),
        ]);
    }
    let _ = adjust; // 复权因子分支暂未落地（同 stock_hk_daily 处理）
    const COLS: [&str; 6] = ["date", "open", "high", "low", "close", "volume"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 新浪财经-科创板-个股历史行情（对应 akshare [`akshare.stock_zh_kcb_daily`]）。
///
/// - `symbol`: 带市场前缀代码，如 `"sh688399"`
/// - `adjust`: `""`（不复权，当前实现）
///
/// 走 `KC_MarketDataService.getKLineData` JSONP + 流通股本合并。
///
/// # 返回列
/// `date, open, high, low, close, volume, after_volume, after_amount, outstanding_share, turnover`
pub fn stock_zh_kcb_daily(symbol: &str, _adjust: &str) -> Result<Df> {
    let http = HttpClient::default();
    // 1) K 线：KC_MarketDataService.getKLineData JSONP
    let today = chrono::Local::now().format("%Y_%m_%d").to_string();
    let url = format!(
        "https://quotes.sina.cn/cn/api/jsonp.php/var%20_{today}{today}=/KC_MarketDataService.getKLineData?symbol={symbol}"
    );
    let text = http.get_text(&url, &Map::new(), None)?;
    let start = text
        .find('[')
        .ok_or_else(|| AkshareError::Empty("新浪科创板日线响应缺少数组".into()))?;
    let end = text
        .rfind(']')
        .ok_or_else(|| AkshareError::Empty("新浪科创板日线响应缺少数组尾".into()))?;
    let rows: Vec<Value> = serde_json::from_str(&text[start..=end])
        .map_err(|e| AkshareError::json(&url, e.to_string()))?;

    // 2) 流通股本：StockService.getAmountBySymbol
    let amount_url = format!(
        "https://stock.finance.sina.com.cn/stock/api/jsonp.php/var%20KKE_ShareAmount_{symbol}=/StockService.getAmountBySymbol?_=20&symbol={symbol}"
    );
    let amount_text = http.get_text(&amount_url, &Map::new(), None)?;
    let a_start = amount_text
        .find('[')
        .ok_or_else(|| AkshareError::Empty("新浪科创板流通股本响应缺少数组".into()))?;
    let a_end = amount_text
        .rfind(']')
        .ok_or_else(|| AkshareError::Empty("新浪科创板流通股本响应缺少数组尾".into()))?;
    let amount_arr: Vec<Value> = serde_json::from_str(&amount_text[a_start..=a_end])
        .map_err(|e| AkshareError::json(&amount_url, e.to_string()))?;
    let mut share_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for r in &amount_arr {
        if let (Some(d), Some(a)) = (
            r.get("date").and_then(Value::as_str),
            r.get("amount").and_then(|v| v.as_f64()),
        ) {
            share_map.insert(d.to_string(), a * 10000.0);
        }
    }

    // 3) 合并 + ffill + turnover = volume / outstanding_share
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    let mut last_share: Option<f64> = None;
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        let date = f("d").unwrap_or_default();
        let share = share_map.get(&date).copied().or(last_share).or_else(|| {
            let mut cand: Option<f64> = None;
            for (d, v) in &share_map {
                if d.as_str() <= date.as_str() {
                    cand = Some(*v);
                }
            }
            cand
        });
        last_share = share;
        let volume = f("v").and_then(|s| s.parse::<f64>().ok());
        let turnover = match (volume, share) {
            (Some(v), Some(s)) if s > 0.0 => Some((v / s).to_string()),
            _ => None,
        };
        out.push(vec![
            Some(date),
            f("o"),
            f("h"),
            f("l"),
            f("c"),
            f("v"),
            f("after_volume"),
            f("after_amount"),
            share.map(|s| s.to_string()),
            turnover,
        ]);
    }
    const COLS: [&str; 10] = [
        "date",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "after_volume",
        "after_amount",
        "outstanding_share",
        "turnover",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 新浪财经-科创板实时行情（对应 akshare [`akshare.stock_zh_kcb_spot`]）。
///
/// `Market_Center.getHQNodeData`（node=kcb）分页（num=80），列契约与 akshare 对齐
/// （20 列位置式列名，取 19 个有效列）。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌额, 涨跌幅, 买入, 卖出, 昨收, 今开, 最高, 最低,
/// 成交量, 成交额, 时点, 市盈率, 市净率, 流通市值, 总市值, 换手率`
pub fn stock_zh_kcb_spot() -> Result<Df> {
    let http = HttpClient::default();
    let count_url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCount";
    let mut count_params = Map::new();
    count_params.insert("node".into(), Value::String("kcb".into()));
    let count_text = http.get_text(count_url, &count_params, None)?;
    let total: u64 = count_text.trim().parse().unwrap_or(0);
    let total_pages = total.div_ceil(80);

    let url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
    let mut rows: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let params = json!({
            "page": page.to_string(),
            "num": "80",
            "sort": "symbol",
            "asc": "1",
            "node": "kcb",
            "symbol": "",
            "_s_r_a": "auto",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                    rows.extend(arr);
                }
            }
            Err(_) => break,
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    // 20 列位置式列名：代码,_,名称,最新价,涨跌额,涨跌幅,买入,卖出,昨收,今开,
    // 最高,最低,成交量,成交额,时点,市盈率,市净率,流通市值,总市值,换手率
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("symbol"),
            f("name"),
            f("trade"),
            f("pricechange"),
            f("changepercent"),
            f("buy"),
            f("sell"),
            f("settlement"),
            f("open"),
            f("high"),
            f("low"),
            f("volume"),
            f("amount"),
            f("ticktime"),
            f("per"),
            f("pb"),
            f("mktcap"),
            f("nmc"),
            f("turnoverratio"),
        ]);
    }
    const COLS: [&str; 19] = [
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "买入",
        "卖出",
        "昨收",
        "今开",
        "最高",
        "最低",
        "成交量",
        "成交额",
        "时点",
        "市盈率",
        "市净率",
        "流通市值",
        "总市值",
        "换手率",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[2..])?;
    Ok(df)
}

/// 新浪财经-所有 B 股实时行情（对应 akshare [`akshare.stock_zh_b_spot`]）。
///
/// `Market_Center.getHQNodeData`（node=hs_b）分页（num=80），列契约同
/// [`stock_zh_a_spot`]（14 列）。注：大量抓取会被新浪暂时封 IP。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌额, 涨跌幅, 买入, 卖出, 昨收, 今开, 最高, 最低, 成交量, 成交额, 时间戳`
pub fn stock_zh_b_spot() -> Result<Df> {
    let http = HttpClient::default();
    let count_url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCount";
    let mut count_params = Map::new();
    count_params.insert("node".into(), Value::String("hs_b".into()));
    let count_text = http.get_text(count_url, &count_params, None)?;
    let total: u64 = count_text.trim().parse().unwrap_or(0);
    let total_pages = total.div_ceil(80);

    let url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
    let mut rows: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let params = json!({
            "page": page.to_string(),
            "num": "80",
            "sort": "symbol",
            "asc": "1",
            "node": "hs_b",
            "symbol": "",
            "_s_r_a": "page",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                    rows.extend(arr);
                }
            }
            Err(_) => break,
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("symbol"),
            f("name"),
            f("trade"),
            f("pricechange"),
            f("changepercent"),
            f("buy"),
            f("sell"),
            f("settlement"),
            f("open"),
            f("high"),
            f("low"),
            f("volume"),
            f("amount"),
            f("ticktime"),
        ]);
    }
    const COLS: [&str; 14] = [
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "买入",
        "卖出",
        "昨收",
        "今开",
        "最高",
        "最低",
        "成交量",
        "成交额",
        "时间戳",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[2..13])?;
    Ok(df)
}

// === BATCH36-H 巨潮资讯-公告查询（www.cninfo.com.cn/new/hisAnnouncement/query）===
//
// 对应 akshare `stock_feature/stock_disclosure_cninfo.py`：POST 分页查询，
// 公告时间为毫秒时间戳 → Asia/Shanghai，公告链接按「代码+announcementId+orgId+时间」拼接。

/// 巨潮资讯-股票代码 → orgId 字典（对应 akshare `__get_stock_json`）。
fn cninfo_stock_id_map(market: &str) -> Result<std::collections::HashMap<String, String>> {
    let url = match market {
        "沪深京" => "http://www.cninfo.com.cn/new/data/szse_stock.json",
        "港股" => "http://www.cninfo.com.cn/new/data/hke_stock.json",
        "三板" => "http://www.cninfo.com.cn/new/data/gfzr_stock.json",
        "基金" => "http://www.cninfo.com.cn/new/data/fund_stock.json",
        "债券" => "http://www.cninfo.com.cn/new/data/bond_stock.json",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 market: {other}，可选 沪深京/港股/三板/基金/债券"
            )))
        }
    };
    let http = HttpClient::default();
    let value = http.get_json(url, &Map::new(), None)?;
    let list = value
        .get("stockList")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    for item in list {
        if let (Some(code), Some(org)) = (
            item.get("code").and_then(Value::as_str),
            item.get("orgId").and_then(Value::as_str),
        ) {
            map.insert(code.to_string(), org.to_string());
        }
    }
    Ok(map)
}

/// 公告查询公共实现（对应 akshare `stock_zh_a_disclosure_report_cninfo` /
/// `stock_zh_a_disclosure_relation_cninfo`）。
fn cninfo_disclosure_query(
    symbol: &str,
    market: &str,
    keyword: &str,
    category: &str,
    start_date: &str,
    end_date: &str,
    tab_name: &str,
) -> Result<Df> {
    let column = match market {
        "沪深京" => "szse",
        "港股" => "hke",
        "三板" => "third",
        "基金" => "fund",
        "债券" => "bond",
        "监管" => "regulator",
        "预披露" => "pre_disclosure",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 market: {other}，可选 沪深京/港股/三板/基金/债券/监管/预披露"
            )))
        }
    };
    let stock_item = if symbol.is_empty() {
        String::new()
    } else if market == "沪深京" || market == "基金" {
        let map = cninfo_stock_id_map(market)?;
        let org = map
            .get(symbol)
            .ok_or_else(|| AkshareError::Param(format!("未找到 {symbol} 的 orgId")))?;
        format!("{symbol},{org}")
    } else {
        symbol.to_string()
    };
    let category_map = [
        ("年报", "category_ndbg_szsh"),
        ("半年报", "category_bndbg_szsh"),
        ("一季报", "category_yjdbg_szsh"),
        ("三季报", "category_sjdbg_szsh"),
        ("业绩预告", "category_yjygjxz_szsh"),
        ("权益分派", "category_qyfpxzcs_szsh"),
        ("董事会", "category_dshgg_szsh"),
        ("监事会", "category_jshgg_szsh"),
        ("股东大会", "category_gddh_szsh"),
        ("日常经营", "category_rcjy_szsh"),
        ("公司治理", "category_gszl_szsh"),
        ("中介报告", "category_zj_szsh"),
        ("首发", "category_sf_szsh"),
        ("增发", "category_zf_szsh"),
        ("股权激励", "category_gqjl_szsh"),
        ("配股", "category_pg_szsh"),
        ("解禁", "category_jj_szsh"),
        ("公司债", "category_gszq_szsh"),
        ("可转债", "category_kzzq_szsh"),
        ("其他融资", "category_qtrz_szsh"),
        ("股权变动", "category_gqbd_szsh"),
        ("补充更正", "category_bcgz_szsh"),
        ("澄清致歉", "category_cqdq_szsh"),
        ("风险提示", "category_fxts_szsh"),
        ("特别处理和退市", "category_tbclts_szsh"),
        ("退市整理期", "category_tszlq_szsh"),
    ];
    let category_item = if category.is_empty() {
        String::new()
    } else {
        category_map
            .iter()
            .find(|(k, _)| *k == category)
            .map(|(_, v)| (*v).to_string())
            .ok_or_else(|| AkshareError::Param(format!("无效 category: {category}")))?
    };
    let se_date = format!(
        "{}-{}-{}~{}-{}-{}",
        &start_date[0..4],
        &start_date[4..6],
        &start_date[6..8],
        &end_date[0..4],
        &end_date[4..6],
        &end_date[6..8],
    );
    let url = "http://www.cninfo.com.cn/new/hisAnnouncement/query";
    let http = HttpClient::default();
    let payload = json!({
        "pageNum": "1",
        "pageSize": "30",
        "column": column,
        "tabName": tab_name,
        "plate": "",
        "stock": stock_item,
        "searchkey": keyword,
        "secid": "",
        "category": category_item,
        "trade": "",
        "seDate": se_date,
        "sortName": "",
        "sortType": "",
        "isHLtitle": "true",
    });
    let mut payload: Map<String, Value> = payload.as_object().cloned().unwrap_or_default();
    let first = http.post_form(url, &payload, &[])?;
    let total = first
        .get("totalAnnouncement")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_pages = total.div_ceil(30).max(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v.get("announcements").and_then(Value::as_array) {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=total_pages {
        payload.insert("pageNum".into(), json!(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        if let Ok(v) = http.post_form(url, &payload, &[]) {
            append(&v, &mut rows);
        }
    }
    // 格式化：代码/简称/公告标题/公告时间(ms→Asia/Shanghai)/公告链接
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        let code = f("secCode").unwrap_or_default();
        let ann_id = f("announcementId").unwrap_or_default();
        let org = f("orgId").unwrap_or_default();
        let time = f("announcementTime").unwrap_or_default();
        let link = format!(
            "http://www.cninfo.com.cn/new/disclosure/detail?stockCode={code}&announcementId={ann_id}&orgId={org}&announcementTime={time}"
        );
        out.push(vec![
            Some(code),
            f("secName"),
            f("announcementTitle"),
            f("announcementTime").and_then(|s| irm_ms_to_shanghai(&s)),
            Some(link),
        ]);
    }
    const COLS: [&str; 5] = ["代码", "简称", "公告标题", "公告时间", "公告链接"];
    Df::from_string_rows(&COLS, &out)
}

/// 巨潮资讯-公告查询-信息披露公告（对应 akshare [`akshare.stock_zh_a_disclosure_report_cninfo`]）。
///
/// - `symbol`: 股票代码（空串为全部）
/// - `market`: `"沪深京"` / `"港股"` / `"三板"` / `"基金"` / `"债券"` / `"监管"` / `"预披露"`
/// - `keyword`: 关键词；`category`: 公告类别；`start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `代码, 简称, 公告标题, 公告时间, 公告链接`
#[allow(clippy::too_many_arguments)]
pub fn stock_zh_a_disclosure_report_cninfo(
    symbol: &str,
    market: &str,
    keyword: &str,
    category: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    cninfo_disclosure_query(
        symbol, market, keyword, category, start_date, end_date, "fulltext",
    )
}

/// 巨潮资讯-预约披露调研（对应 akshare [`akshare.stock_zh_a_disclosure_relation_cninfo`]）。
///
/// 参数与 [`stock_zh_a_disclosure_report_cninfo`] 一致（无 keyword/category），
/// 走 `tabName=relation`。
///
/// # 返回列
/// `代码, 简称, 公告标题, 公告时间, 公告链接`
#[allow(clippy::too_many_arguments)]
pub fn stock_zh_a_disclosure_relation_cninfo(
    symbol: &str,
    market: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    cninfo_disclosure_query(symbol, market, "", "", start_date, end_date, "relation")
}

// === BATCH36-F 巨潮互动易（irm.cninfo.com.cn）===
//
// 对应 akshare `stock_feature/stock_irm_cninfo.py`。`queryKeyboardInfo` 取 orgId，
// 再查公司提问（分页）/ 提问详情。注意：提问时间等毫秒时间戳转为
// `Asia/Shanghai` 时区字符串（对应 akshare `tz_localize("UTC").tz_convert("Asia/Shanghai")`）。

/// 股票-互动易-组织代码（对应 akshare `_fetch_org_id`）。
fn irm_org_id(symbol: &str) -> Result<String> {
    let url = "https://irm.cninfo.com.cn/newircs/index/queryKeyboardInfo";
    let params = json!({ "_t": "1691144074", "keyWord": symbol });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.post_form(url, &params, &[])?;
    let org = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|o| o.get("secid"))
        .and_then(Value::as_str)
        .ok_or_else(|| AkshareError::Empty("互动易查询未返回 orgId".into()))?;
    Ok(org.to_string())
}

/// 毫秒时间戳 → Asia/Shanghai 时间串（对应 akshare tz 转换 + strftime）。
fn irm_ms_to_shanghai(ms: &str) -> Option<String> {
    let ms_num: i64 = ms.trim().parse().ok()?;
    use chrono::TimeZone;
    let dt = chrono::Utc
        .timestamp_millis_opt(ms_num)
        .single()?
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600)?);
    Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// 巨潮-互动易-提问（对应 akshare [`akshare.stock_irm_cninfo`]）。
///
/// - `symbol`: 股票代码，如 `"002594"`。
///
/// # 返回列
/// `股票代码, 公司简称, 行业, 行业代码, 问题, 提问者, 来源, 提问时间, 更新时间,
/// 提问者编号, 问题编号, 回答ID, 回答内容, 回答者`
pub fn stock_irm_cninfo(symbol: &str) -> Result<Df> {
    let org = irm_org_id(symbol)?;
    let url = "https://irm.cninfo.com.cn/newircs/company/question";
    let http = HttpClient::default();
    let params = json!({
        "_t": "1691142650",
        "stockcode": symbol,
        "orgId": org,
        "pageSize": "1000",
        "pageNum": "1",
        "keyWord": "",
        "startDay": "",
        "endDay": "",
    });
    let mut params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let first = http.post_json(url, &params, &[])?;
    let total_page = first
        .get("totalPage")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .min(10) as usize;
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v.get("rows").and_then(Value::as_array) {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=total_page {
        params.insert("pageNum".into(), json!(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        if let Ok(v) = http.post_json(url, &params, &[]) {
            append(&v, &mut rows);
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        // 行业/行业代码为单元素数组 → 取首元素
        let ind = r
            .get("trade")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_string);
        let ind_code = r
            .get("boardType")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_string);
        let source = match f("pubClient").as_deref() {
            Some("2") => Some("APP".to_string()),
            Some("5") => Some("公众号".to_string()),
            _ => Some("网站".to_string()),
        };
        out.push(vec![
            f("stockCode"),
            f("companyShortName"),
            ind,
            ind_code,
            f("mainContent"),
            f("authorName"),
            source,
            f("pubDate").and_then(|s| irm_ms_to_shanghai(&s)),
            f("updateDate").and_then(|s| irm_ms_to_shanghai(&s)),
            f("author"),
            f("indexId"),
            f("attachedId"),
            f("attachedContent"),
            f("attachedAuthor"),
        ]);
    }
    const COLS: [&str; 14] = [
        "股票代码",
        "公司简称",
        "行业",
        "行业代码",
        "问题",
        "提问者",
        "来源",
        "提问时间",
        "更新时间",
        "提问者编号",
        "问题编号",
        "回答ID",
        "回答内容",
        "回答者",
    ];
    Df::from_string_rows(&COLS, &out)
}

/// 巨潮-互动易-回答（对应 akshare [`akshare.stock_irm_ans_cninfo`]）。
///
/// - `symbol`: 提问者编号（由 [`stock_irm_cninfo`] 的 `提问者编号` 列给出）。
///
/// # 返回列
/// `股票代码, 公司简称, 问题, 回答内容, 提问者, 提问时间, 回答时间`
pub fn stock_irm_ans_cninfo(symbol: &str) -> Result<Df> {
    let url = "https://irm.cninfo.com.cn/newircs/question/getQuestionDetail";
    let params = json!({ "questionId": symbol, "_t": "1691146921" });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(url, &params, None)?;
    let data = value.get("data").cloned().unwrap_or(Value::Null);
    if data
        .as_object()
        .map(|o| !o.contains_key("replyDate"))
        .unwrap_or(true)
    {
        return Df::from_string_rows(
            &[
                "股票代码",
                "公司简称",
                "问题",
                "回答内容",
                "提问者",
                "提问时间",
                "回答时间",
            ],
            &[],
        );
    }
    let f = |k: &str| data.get(k).and_then(json_value_to_string);
    let rows = vec![vec![
        f("stockCode"),
        f("shortName"),
        f("questionContent"),
        f("replyContent"),
        f("questioner"),
        f("questionDate").and_then(|s| irm_ms_to_shanghai(&s)),
        f("replyDate").and_then(|s| irm_ms_to_shanghai(&s)),
    ]];
    const COLS: [&str; 7] = [
        "股票代码",
        "公司简称",
        "问题",
        "回答内容",
        "提问者",
        "提问时间",
        "回答时间",
    ];
    Df::from_string_rows(&COLS, &rows)
}

/// 新浪财经-股票曾用名（对应 akshare [`akshare.stock_info_change_name`]）。
///
/// - `symbol`: 股票代码，如 `"000503"`。
///
/// 解析 `vCI_CorpInfo` 页面第 4 张表（`pd.read_html()[3]`），取
/// 「证券简称更名历史」一行的空格分隔曾用名。
///
/// # 返回列
/// `index, name`
pub fn stock_info_change_name(symbol: &str) -> Result<Df> {
    let url = format!(
        "https://vip.stock.finance.sina.com.cn/corp/go.php/vCI_CorpInfo/stockid/{symbol}.phtml"
    );
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), None)?;
    let tables = crate::core::html::read_html_tables(&text)?;
    let table = tables
        .get(3)
        .ok_or_else(|| AkshareError::Empty("新浪曾用名页面缺少第 4 张表".into()))?;
    // 找「证券简称更名历史」行的 value
    let mut history: Option<String> = None;
    for row in table {
        let first = row.first().cloned().unwrap_or_default();
        if first.contains("证券简称更名历史") {
            history = row.get(1).cloned();
            break;
        }
    }
    let history = history.unwrap_or_default();
    let names: Vec<&str> = history.split(' ').filter(|s| !s.is_empty()).collect();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(names.len());
    for (i, n) in names.iter().enumerate() {
        out.push(vec![Some((i + 1).to_string()), Some((*n).to_string())]);
    }
    Df::from_string_rows(&["index", "name"], &out)
}

/// 沪深京 A 股列表（对应 akshare [`akshare.stock_info_a_code_name`]）。
///
/// 组合深市 A 股、沪市主板、科创板、北交所列表，列 `code, name`。
///
/// # 返回列
/// `code, name`
pub fn stock_info_a_code_name() -> Result<Df> {
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    // 深市 A 股（stock_info_sz_name_code("A股列表")：A股代码/A股简称）
    if let Ok(sz) = stock_info_sz_name_code("A股列表") {
        if let (Ok(c), Ok(n)) = (sz.inner().column("A股代码"), sz.inner().column("A股简称")) {
            if let (Ok(cs), Ok(ns)) = (c.str(), n.str()) {
                for (c, n) in cs.iter().zip(ns.iter()) {
                    rows.push(vec![c.map(str::to_string), n.map(str::to_string)]);
                }
            }
        }
    }
    // 沪市主板 + 科创板（stock_info_sh_name_code：证券代码/证券简称）
    for sym in ["主板A股", "科创板"] {
        if let Ok(sh) = stock_info_sh_name_code(sym) {
            if let (Ok(c), Ok(n)) = (sh.inner().column("证券代码"), sh.inner().column("证券简称"))
            {
                if let (Ok(cs), Ok(ns)) = (c.str(), n.str()) {
                    for (c, n) in cs.iter().zip(ns.iter()) {
                        rows.push(vec![c.map(str::to_string), n.map(str::to_string)]);
                    }
                }
            }
        }
    }
    // 北交所（stock_info_bj_name_code：证券代码/证券简称）
    if let Ok(bj) = stock_info_bj_name_code() {
        if let (Ok(c), Ok(n)) = (bj.inner().column("证券代码"), bj.inner().column("证券简称"))
        {
            if let (Ok(cs), Ok(ns)) = (c.str(), n.str()) {
                for (c, n) in cs.iter().zip(ns.iter()) {
                    rows.push(vec![c.map(str::to_string), n.map(str::to_string)]);
                }
            }
        }
    }
    Df::from_string_rows(&["code", "name"], &rows)
}

/// 巨潮资讯-首页-数据-预约披露（对应 akshare [`akshare.stock_report_disclosure`]）。
///
/// - `market`: `"沪深京"` / `"深市"` / `"深主板"` / `"创业板"` / `"沪市"` /
///   `"沪主板"` / `"科创板"` / `"北交所"`
/// - `period`: 财报期，如 `"2021年报"`（一季/半年报/三季/年报）
///
/// # 返回列
/// `股票代码, 股票简称, 首次预约, 初次变更, 二次变更, 三次变更, 实际披露`
pub fn stock_report_disclosure(market: &str, period: &str) -> Result<Df> {
    let market_map = match market {
        "沪深京" => "szsh",
        "深市" => "sz",
        "深主板" => "szmb",
        "创业板" => "szcn",
        "沪市" => "sh",
        "沪主板" => "shmb",
        "科创板" => "shkcp",
        "北交所" => "bj",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 market: {other}，可选 沪深京/深市/深主板/创业板/沪市/沪主板/科创板/北交所"
            )))
        }
    };
    let year = period.get(..4).unwrap_or("");
    let period_map = match period.get(4..).unwrap_or("") {
        "一季" => format!("{year}-03-31"),
        "半年报" => format!("{year}-06-30"),
        "三季" => format!("{year}-09-30"),
        "年报" => format!("{year}-12-31"),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 period: {period}（{other}），可选 一季/半年报/三季/年报"
            )))
        }
    };
    let url = "http://www.cninfo.com.cn/new/information/getPrbookInfo";
    let params = json!({
        "sectionTime": period_map,
        "firstTime": "",
        "lastTime": "",
        "market": market_map,
        "stockCode": "",
        "orderClos": "",
        "isDesc": "",
        "pagesize": "10000",
        "pagenum": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.post_form(url, &params, &[])?;
    let rows = value
        .get("prbookinfos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("scode").or_else(|| f("stockCode")),
            f("shortName").or_else(|| f("stockName")),
            f("firstTime"),
            f("firstChange"),
            f("secondChange"),
            f("thirdChange"),
            f("actualTime").or_else(|| f("lastTime")),
        ]);
    }
    const COLS: [&str; 7] = [
        "股票代码",
        "股票简称",
        "首次预约",
        "初次变更",
        "二次变更",
        "三次变更",
        "实际披露",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["首次预约", "初次变更", "二次变更", "三次变更", "实际披露"])?;
    Ok(df)
}

// === BATCH36-G 深交所股票列表/退市/名称变更（szse.cn ShowReport xlsx）===
//
// 对应 akshare `stock/stock_info.py` 的 `stock_info_sz_*`。走
// `www.szse.cn/api/report/ShowReport`（SHOWTYPE=xlsx 下载），用 calamine
// 解析首个工作表为字符串二维数组，列契约与 akshare 逐字对齐。

/// calamine 解析 xlsx 首个工作表为字符串二维数组（对应 akshare `pd.read_excel`）。
fn szse_xlsx_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    use calamine::{Data, Reader, Xlsx};
    let cur = std::io::Cursor::new(bytes.to_vec());
    let mut wb = Xlsx::new(cur).map_err(|e| AkshareError::Empty(format!("xlsx 解析失败: {e}")))?;
    let range = wb
        .worksheet_range_at(0)
        .ok_or_else(|| AkshareError::Empty("xlsx 无工作表".into()))?
        .map_err(|e| AkshareError::Empty(format!("读取 xlsx 工作表失败: {e}")))?;
    let mut rows = Vec::with_capacity(range.height());
    for r in range.rows() {
        let mut row = Vec::with_capacity(r.len());
        for c in r {
            row.push(match c {
                Data::Empty => String::new(),
                Data::String(s) => s.clone(),
                Data::Float(f) => {
                    let v = *f;
                    if v.fract() == 0.0 {
                        format!("{}", v as i64)
                    } else {
                        format!("{v}")
                    }
                }
                Data::Int(i) => i.to_string(),
                Data::Bool(b) => b.to_string(),
                Data::DateTime(_) => String::new(),
                Data::Error(e) => format!("{e:?}"),
                other => other.to_string(),
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// 深交所 ShowReport xlsx 下载（首行表头 + 数据行）。
fn szse_showreport(catalog: &str, tabkey: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let url = "https://www.szse.cn/api/report/ShowReport";
    let params = json!({
        "SHOWTYPE": "xlsx",
        "CATALOGID": catalog,
        "TABKEY": tabkey,
        "random": "0.6935816432433362",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(
        url,
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let all = szse_xlsx_rows(&bytes)?;
    let mut iter = all.into_iter();
    let header = iter.next().unwrap_or_default();
    let data: Vec<Vec<String>> = iter.collect();
    Ok((header, data))
}

/// 深交所-股票列表（对应 akshare [`akshare.stock_info_sz_name_code`]）。
///
/// - `symbol`: `"A股列表"` / `"B股列表"` / `"CDR列表"` / `"AB股列表"`
///
/// # 返回列
/// A/B 股列表：`板块, {A|B}股代码, {A|B}股简称, {A|B}股上市日期, {A|B}股总股本, {A|B}股流通股本, 所属行业`；
/// AB 股列表：`板块, A股代码, A股简称, A股上市日期, B股代码, B股简称, B股上市日期, 所属行业`
pub fn stock_info_sz_name_code(symbol: &str) -> Result<Df> {
    let tabkey = match symbol {
        "A股列表" => "tab1",
        "B股列表" => "tab2",
        "CDR列表" => "tab3",
        "AB股列表" => "tab4",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 A股列表/B股列表/CDR列表/AB股列表"
            )))
        }
    };
    let (_header, data) = szse_showreport("1110", tabkey)?;
    let pick = |row: &[String], i: usize| row.get(i).cloned().map(Some).unwrap_or(None);
    let norm_code = |s: &str| -> String {
        // akshare：astype(str).split(".")[0].zfill(6).replace("000nan","")
        let base = s.split('.').next().unwrap_or("").to_string();
        let z = format!("{:0>6}", base);
        if z == "000nan" {
            String::new()
        } else {
            z
        }
    };
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    match symbol {
        "A股列表" => {
            for row in &data {
                let code = norm_code(&pick(row, 4).unwrap_or_default());
                if code.is_empty() {
                    continue;
                }
                out.push(vec![
                    pick(row, 0),
                    Some(code),
                    pick(row, 5),
                    pick(row, 6),
                    pick(row, 7),
                    pick(row, 8),
                    pick(row, 17),
                ]);
            }
            const COLS: [&str; 7] = [
                "板块",
                "A股代码",
                "A股简称",
                "A股上市日期",
                "A股总股本",
                "A股流通股本",
                "所属行业",
            ];
            let mut df = Df::from_string_rows(&COLS, &out)?;
            df.cast_date(&["A股上市日期"])?;
            df.cast_numeric(&["A股总股本", "A股流通股本"])?;
            Ok(df)
        }
        "B股列表" => {
            for row in &data {
                let code = norm_code(&pick(row, 9).unwrap_or_default());
                if code.is_empty() {
                    continue;
                }
                out.push(vec![
                    pick(row, 0),
                    Some(code),
                    pick(row, 10),
                    pick(row, 11),
                    pick(row, 12),
                    pick(row, 13),
                    pick(row, 17),
                ]);
            }
            const COLS: [&str; 7] = [
                "板块",
                "B股代码",
                "B股简称",
                "B股上市日期",
                "B股总股本",
                "B股流通股本",
                "所属行业",
            ];
            let mut df = Df::from_string_rows(&COLS, &out)?;
            df.cast_date(&["B股上市日期"])?;
            df.cast_numeric(&["B股总股本", "B股流通股本"])?;
            Ok(df)
        }
        "AB股列表" => {
            for row in &data {
                let a_code = norm_code(&pick(row, 4).unwrap_or_default());
                let b_code = norm_code(&pick(row, 9).unwrap_or_default());
                if a_code.is_empty() && b_code.is_empty() {
                    continue;
                }
                out.push(vec![
                    pick(row, 0),
                    Some(a_code),
                    pick(row, 5),
                    pick(row, 6),
                    Some(b_code),
                    pick(row, 10),
                    pick(row, 11),
                    pick(row, 17),
                ]);
            }
            const COLS: [&str; 8] = [
                "板块",
                "A股代码",
                "A股简称",
                "A股上市日期",
                "B股代码",
                "B股简称",
                "B股上市日期",
                "所属行业",
            ];
            let mut df = Df::from_string_rows(&COLS, &out)?;
            df.cast_date(&["A股上市日期", "B股上市日期"])?;
            Ok(df)
        }
        _ => {
            // CDR列表：返回原始表（akshare 无 select，直接返回）
            Df::from_string_rows(
                &[
                    "板块",
                    "公司全称",
                    "英文名称",
                    "注册地址",
                    "A股代码",
                    "A股简称",
                    "A股上市日期",
                    "A股总股本",
                    "A股流通股本",
                    "B股代码",
                    "B股简称",
                    "B股上市日期",
                    "B股总股本",
                    "B股流通股本",
                    "地区",
                    "省份",
                    "城市",
                    "所属行业",
                    "公司网址",
                ],
                &[],
            )
        }
    }
}

/// 深交所-暂停/终止上市公司（对应 akshare [`akshare.stock_info_sz_delist`]）。
///
/// - `symbol`: `"暂停上市公司"` / `"终止上市公司"`
///
/// # 返回列
/// 原始 xlsx 列（`证券代码, 证券简称, ...`），证券代码 zfill(6)、日期列归一。
pub fn stock_info_sz_delist(symbol: &str) -> Result<Df> {
    let tabkey = match symbol {
        "暂停上市公司" => "tab1",
        "终止上市公司" => "tab2",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 暂停上市公司/终止上市公司"
            )))
        }
    };
    let (header, data) = szse_showreport("1793_ssgs", tabkey)?;
    if data.is_empty() {
        return Df::from_string_rows(&["证券代码"], &[]);
    }
    // 按表头名取列：证券代码 zfill(6)，上市日期/终止上市日期 cast_date
    let code_idx = header
        .iter()
        .position(|h| h.contains("证券代码"))
        .unwrap_or(0);
    let date_idx: Vec<usize> = header
        .iter()
        .enumerate()
        .filter(|(_, h)| h.contains("日期"))
        .map(|(i, _)| i)
        .collect();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for row in &data {
        let mut r: Vec<Option<String>> = row.iter().map(|s| Some(s.clone())).collect();
        if let Some(Some(s)) = r.get_mut(code_idx) {
            *s = format!("{:0>6}", s);
        }
        out.push(r);
    }
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    let date_cols: Vec<&str> = date_idx
        .iter()
        .filter_map(|i| header.get(*i).map(String::as_str))
        .collect();
    df.cast_date(&date_cols)?;
    Ok(df)
}

/// 深交所-股票名称变更（对应 akshare [`akshare.stock_info_sz_change_name`]）。
///
/// - `symbol`: `"全称变更"` / `"简称变更"`
///
/// # 返回列
/// 原始 xlsx 列，`证券代码` zfill(6)、`变更日期` 归一并按变更日期升序。
pub fn stock_info_sz_change_name(symbol: &str) -> Result<Df> {
    let tabkey = match symbol {
        "全称变更" => "tab1",
        "简称变更" => "tab2",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全称变更/简称变更"
            )))
        }
    };
    let (header, mut data) = szse_showreport("SSGSGMXX", tabkey)?;
    let code_idx = header
        .iter()
        .position(|h| h.contains("证券代码"))
        .unwrap_or(0);
    let date_idx = header
        .iter()
        .position(|h| h.contains("变更日期"))
        .unwrap_or(0);
    // 证券代码 zfill(6)
    for row in data.iter_mut() {
        if let Some(c) = row.get_mut(code_idx) {
            *c = format!("{:0>6}", c);
        }
    }
    // 按变更日期升序（akshare sort_values）
    data.sort_by(|a, b| {
        let da = a.get(date_idx).cloned().unwrap_or_default();
        let db = b.get(date_idx).cloned().unwrap_or_default();
        da.cmp(&db)
    });
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let opt_rows: Vec<Vec<Option<String>>> = data
        .iter()
        .map(|row| row.iter().map(|s| Some(s.clone())).collect())
        .collect();
    let mut df = Df::from_string_rows(&col_refs, &opt_rows)?;
    df.cast_date(&["变更日期"])?;
    Ok(df)
}

/// 深交所-总貌-证券类别统计（对应 akshare [`akshare.stock_szse_summary`]）。
///
/// - `date`: 最近结束交易日 `YYYYMMDD`
///
/// # 返回列
/// `证券类别, 数量, 成交金额, 总市值, 流通市值`
pub fn stock_szse_summary(date: &str) -> Result<Df> {
    let url = "http://www.szse.cn/api/report/ShowReport";
    // txtQueryDate 由 date 拼接；单独构造参数
    let mut params = Map::new();
    params.insert("SHOWTYPE".into(), Value::String("xlsx".into()));
    params.insert("CATALOGID".into(), Value::String("1803_sczm".into()));
    params.insert("TABKEY".into(), Value::String("tab1".into()));
    params.insert(
        "txtQueryDate".into(),
        Value::String(format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])),
    );
    params.insert("random".into(), Value::String("0.39339437497296137".into()));
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(
        url,
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let all = szse_xlsx_rows(&bytes)?;
    let mut iter = all.into_iter();
    let header = iter.next().unwrap_or_default();
    let data: Vec<Vec<String>> = iter.collect();
    // 列契约：证券类别, 数量, 成交金额, 总市值, 流通市值（数量/金额去逗号后数值化）
    let name_idx = header
        .iter()
        .position(|h| h.contains("证券类别"))
        .unwrap_or(0);
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for row in &data {
        let norm = |i: usize| -> Option<String> {
            row.get(i)
                .map(|s| s.replace(',', ""))
                .map(Some)
                .unwrap_or(None)
        };
        out.push(vec![
            row.get(name_idx).cloned().map(|s| s.trim().to_string()),
            norm(1),
            norm(2),
            norm(3),
            norm(4),
        ]);
    }
    const COLS: [&str; 5] = ["证券类别", "数量", "成交金额", "总市值", "流通市值"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["数量", "成交金额", "总市值", "流通市值"])?;
    Ok(df)
}

/// 深交所-总貌-地区交易排序（对应 akshare [`akshare.stock_szse_area_summary`]）。
///
/// - `date`: 最近结束交易日年月 `YYYYMM`
///
/// # 返回列
/// `序号, 地区, 总交易额, 占市场, 股票交易额, 基金交易额, 债券交易额, 优先股交易额, 期权交易额`
pub fn stock_szse_area_summary(date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("SHOWTYPE".into(), Value::String("xlsx".into()));
    params.insert("CATALOGID".into(), Value::String("1803_sczm".into()));
    params.insert("TABKEY".into(), Value::String("tab2".into()));
    params.insert(
        "DATETIME".into(),
        Value::String(format!("{}-{}", &date[0..4], &date[4..6])),
    );
    params.insert("random".into(), Value::String("0.39349437497296137".into()));
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(
        "https://www.szse.cn/api/report/ShowReport",
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let all = szse_xlsx_rows(&bytes)?;
    let mut iter = all.into_iter();
    let header = iter.next().unwrap_or_default();
    let data: Vec<Vec<String>> = iter.collect();
    // 列重命名：序号/地区/总交易额(元)/占市场%/股票交易额(元)/基金交易额(元)/债券交易额(元)/优先股交易额(元)/期权交易额(元)
    let rename = [
        ("序号", "序号"),
        ("地区", "地区"),
        ("总交易额", "总交易额"),
        ("占市场", "占市场"),
        ("股票交易额", "股票交易额"),
        ("基金交易额", "基金交易额"),
        ("债券交易额", "债券交易额"),
        ("优先股交易额", "优先股交易额"),
        ("期权交易额", "期权交易额"),
    ];
    let cols: Vec<String> = header
        .iter()
        .map(|h| {
            for (k, v) in &rename {
                if h.contains(k) {
                    return (*v).to_string();
                }
            }
            h.clone()
        })
        .collect();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for row in &data {
        let mut r: Vec<Option<String>> = row
            .iter()
            .map(|s| {
                if s.contains(',') {
                    Some(s.replace(',', ""))
                } else {
                    Some(s.clone())
                }
            })
            .collect();
        // 序号归一为数字字符串
        if let Some(Some(s)) = r.first_mut() {
            if let Ok(n) = s.parse::<f64>() {
                *s = format!("{}", n as i64);
            }
        }
        out.push(r);
    }
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_numeric(&[
        "序号",
        "总交易额",
        "占市场",
        "股票交易额",
        "基金交易额",
        "债券交易额",
        "优先股交易额",
        "期权交易额",
    ])?;
    Ok(df)
}

/// 上交所-总貌（对应 akshare [`akshare.stock_sse_summary`]）。
///
/// # 返回列
/// `项目, 股票, 主板, 科创板`
pub fn stock_sse_summary() -> Result<Df> {
    let url = "http://query.sse.com.cn/commonQuery.do";
    let params = json!({
        "sqlId": "COMMON_SSE_SJ_GPSJ_GPSJZM_TJSJ_L",
        "PRODUCT_NAME": "股票,主板,科创板",
        "type": "inParams",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let data =
        http.get_json_with_headers(url, &params, &[("Referer", "http://www.sse.com.cn/")], None)?;
    let result = data.get("result").cloned().unwrap_or(Value::Null);
    // akshare：pd.DataFrame(result).T → index 顺序即结果键序（流通股本,总市值,...）
    let obj = result.as_object().cloned().unwrap_or_default();
    // 目标行序（akshare 的 index 覆盖表）
    let order = [
        "流通股本",
        "总市值",
        "平均市盈率",
        "上市公司",
        "上市股票",
        "流通市值",
        "报告时间",
    ];
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(order.len());
    for name in &order {
        let row = obj
            .get(*name)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let f = |i: usize| row.get(i).and_then(json_value_to_string);
        out.push(vec![Some((*name).to_string()), f(0), f(1), f(2)]);
    }
    const COLS: [&str; 4] = ["项目", "股票", "主板", "科创板"];
    Df::from_string_rows(&COLS, &out)
}

/// 上交所-每日股票情况（对应 akshare [`akshare.stock_sse_deal_daily`]）。
///
/// - `date`: 交易日 `YYYYMMDD`
///
/// # 返回列
/// `单日情况, 股票, 主板A, 主板B, 科创板, 股票回购`
pub fn stock_sse_deal_daily(date: &str) -> Result<Df> {
    let url = "https://query.sse.com.cn/commonQuery.do";
    let params = json!({
        "sqlId": "COMMON_SSE_SJ_GPSJ_CJGK_MRGK_C",
        "PRODUCT_CODE": "01,02,03,11,17",
        "type": "inParams",
        "SEARCH_DATE": format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]),
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let data = http.get_json_with_headers(
        url,
        &params,
        &[("Referer", "https://www.sse.com.cn/")],
        None,
    )?;
    let result = data.get("result").cloned().unwrap_or(Value::Null);
    // akshare：pd.DataFrame(result).T → 行 = result 键序（单日情况,主板A,主板B,科创板,股票回购,股票 或变体）
    let obj = result.as_object().cloned().unwrap_or_default();
    // 列名覆盖表按键序：单日情况,主板A,主板B,科创板,股票回购,股票
    let names = ["单日情况", "主板A", "主板B", "科创板", "股票回购", "股票"];
    let keys: Vec<String> = obj.keys().cloned().collect();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(keys.len());
    for (i, k) in keys.iter().enumerate() {
        let row = obj
            .get(k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let name = names.get(i).copied().unwrap_or("单日情况");
        let f = |j: usize| row.get(j).and_then(json_value_to_string);
        out.push(vec![
            Some((*name).to_string()),
            f(0),
            f(1),
            f(2),
            f(3),
            f(4),
        ]);
    }
    const COLS: [&str; 6] = ["单日情况", "股票", "主板A", "主板B", "科创板", "股票回购"];
    Df::from_string_rows(&COLS, &out)
}

/// 东财-沪深港通-港股通(沪>港)-股票实时行情（对应 akshare [`akshare.stock_hsgt_sh_hk_spot_em`]）。
///
/// push2 clist（`fs=b:DLMK0144`，`fltt=1` 原始值百分位），按 `代码` 排序 + 序号。
/// 数值化时按 akshare 语义缩放：价类 ÷1000、涨跌幅 ÷100、量额 ÷1e8。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额`
pub fn stock_hsgt_sh_hk_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "np": "1",
        "fltt": "1",
        "invt": "2",
        "fs": "b:DLMK0144",
        "fields": "f12,f13,f14,f19,f1,f2,f4,f3,f152,f17,f18,f15,f16,f5,f6",
        "fid": "f12",
        "pn": "1",
        "pz": "100",
        "po": "1",
        "dect": "1",
        "wbp2u": "|0|0|0|web",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let rows = http.fetch_paginated_diff_any(&urls, &params, None)?;
    // 按代码排序（akshare sort_values(["代码"], ignore_index)）
    let mut rows = rows;
    rows.sort_by(|a, b| {
        let ca = a.get("f12").and_then(Value::as_str).unwrap_or("");
        let cb = b.get("f12").and_then(Value::as_str).unwrap_or("");
        ca.cmp(cb)
    });
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()),
            f("f12"),
            f("f14"),
            f("f2"),
            f("f4"),
            f("f3"),
            f("f17"),
            f("f15"),
            f("f16"),
            f("f18"),
            f("f5"),
            f("f6"),
        ]);
    }
    const COLS: [&str; 12] = [
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
        "成交量",
        "成交额",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&[
        "最新价",
        "涨跌额",
        "涨跌幅",
        "今开",
        "最高",
        "最低",
        "昨收",
        "成交量",
        "成交额",
    ])?;
    // 缩放：价类 ÷1000、涨跌幅 ÷100、量额 ÷1e8（对应 akshare）
    df.scale("最新价", 1000.0)?;
    df.scale("涨跌额", 1000.0)?;
    df.scale("涨跌幅", 100.0)?;
    df.scale("今开", 1000.0)?;
    df.scale("最高", 1000.0)?;
    df.scale("最低", 1000.0)?;
    df.scale("昨收", 1000.0)?;
    df.scale("成交量", 1e8)?;
    df.scale("成交额", 1e8)?;
    Ok(df)
}

/// 东财-美股市场-粉单市场实时行情（对应 akshare [`akshare.stock_us_pink_spot_em`]）。
///
/// 23.push2 clist（`fs=m:153`，`fltt=1` 原始值百分位），位置式列名 + 12 列 select，
/// `代码 = 编码.简称`（对应 akshare `str(f13) + "." + f14`）。
///
/// # 返回列
/// `序号, 名称, 最新价, 涨跌额, 涨跌幅, 开盘价, 最高价, 最低价, 昨收价, 总市值, 市盈率, 代码`
pub fn stock_us_pink_spot_em() -> Result<Df> {
    const URL: &str = "https://23.push2.eastmoney.com/api/qt/clist/get";
    let params = json!({
        "np": "1",
        "fltt": "1",
        "invt": "1",
        "fs": "m:153",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f26,f22,f33,f11,f62,f128,f136,f115,f152",
        "fid": "f3",
        "pn": "1",
        "pz": "100",
        "po": "1",
        "dect": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let mut rows: Vec<Value> = Vec::new();
    // 分页拉取全部（对应 akshare 手动分页 concat，不排序）
    let first = http.get_json(URL, &params, None)?;
    let total = first
        .get("data")
        .and_then(|d| d.get("total"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    rows.extend(
        first
            .get("data")
            .and_then(|d| d.get("diff"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    );
    let total_pages = total.div_ceil(100);
    for page in 2..=total_pages {
        let mut p = params.clone();
        p.insert("pn".into(), json!(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_json(URL, &p, None) {
            Ok(v) => {
                if let Some(list) = v
                    .get("data")
                    .and_then(|d| d.get("diff"))
                    .and_then(Value::as_array)
                {
                    rows.extend(list.iter().cloned());
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        // fltt=1 原始值百分位 → ×1e-2（akshare 未显式除，此处保持原样由调用方按 fltt=1 理解；
        // 实际 akshare 未缩放，值即原始百分位，故不缩放以对齐 akshare）
        let raw = |k: &str| f(k);
        let code = match (f("f13"), f("f14")) {
            (Some(enc), Some(abbr)) => Some(format!("{enc}.{abbr}")),
            _ => None,
        };
        out.push(vec![
            Some((i + 1).to_string()),
            raw("f14"),
            raw("f2"),
            raw("f4"),
            raw("f3"),
            raw("f17"),
            raw("f15"),
            raw("f16"),
            raw("f18"),
            raw("f20"),
            raw("f62"),
            code,
        ]);
    }
    const COLS: [&str; 12] = [
        "序号",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "开盘价",
        "最高价",
        "最低价",
        "昨收价",
        "总市值",
        "市盈率",
        "代码",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&[
        "最新价",
        "涨跌额",
        "涨跌幅",
        "开盘价",
        "最高价",
        "最低价",
        "昨收价",
        "总市值",
        "市盈率",
    ])?;
    Ok(df)
}

/// 东财-沪深港通-AH股比价-实时行情（对应 akshare [`akshare.stock_zh_ah_spot_em`]）。
///
/// push2 clist（`fs=b:DLMK0101`，`fltt=1` 原始值百分位），fetch_clist 按 f3 排序 +
/// 序号。数值化时按 akshare 语义缩放：最新价-HKD ÷1000、其余 ÷100。
///
/// # 返回列
/// `序号, 名称, H股代码, 最新价-HKD, H股-涨跌幅, A股代码, 最新价-RMB, A股-涨跌幅, 比价, 溢价`
pub fn stock_zh_ah_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "np": "1",
        "fltt": "1",
        "invt": "2",
        "fs": "b:DLMK0101",
        "fields": "f193,f191,f192,f12,f13,f14,f1,f2,f4,f3,f152,f186,f190,f187,f189,f188",
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
    // rename + select（对应 akshare rename 后 select 10 列）
    let rename = [
        ("index", "序号"),
        ("f193", "名称"),
        ("f12", "H股代码"),
        ("f2", "最新价-HKD"),
        ("f3", "H股-涨跌幅"),
        ("f191", "A股代码"),
        ("f186", "最新价-RMB"),
        ("f187", "A股-涨跌幅"),
        ("f189", "比价"),
        ("f188", "溢价"),
    ];
    let select = [
        "序号",
        "名称",
        "H股代码",
        "最新价-HKD",
        "H股-涨跌幅",
        "A股代码",
        "最新价-RMB",
        "A股-涨跌幅",
        "比价",
        "溢价",
    ];
    let numeric = [
        "最新价-HKD",
        "H股-涨跌幅",
        "最新价-RMB",
        "A股-涨跌幅",
        "比价",
        "溢价",
    ];
    let mut df = crate::sources::eastmoney::finalize_spot(df, &rename, &select, &numeric)?;
    // 缩放：HKD ÷1000、其余 ÷100（对应 akshare `/1000` 与 `/100`）
    df.scale("最新价-HKD", 1000.0)?;
    df.scale("H股-涨跌幅", 100.0)?;
    df.scale("最新价-RMB", 100.0)?;
    df.scale("A股-涨跌幅", 100.0)?;
    df.scale("比价", 100.0)?;
    df.scale("溢价", 100.0)?;
    Ok(df)
}

// === BATCH36-E 腾讯财经-港股 AH（stock.gtimg.cn/data/hk_rank.php）===
//
// 对应 akshare `stock/stock_zh_ah_tx.py`：`hk_rank.php` 返回 `var list_data={...}`，
// `data.page_data` 为 `~` 分隔字符串数组（20 条/页），分页合并后按位置列名。
// 注意与东财 [`stock_zh_ah_spot_em`]（`b:DLMK0101`）是不同接口。

const TX_HK_RANK_URL: &str = "http://stock.gtimg.cn/data/hk_rank.php";
const TX_HK_RANK_HEADERS: &[(&str, &str)] = &[
    ("Referer", "http://stockapp.finance.qq.com/mstats/"),
    ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/77.0.3865.120 Safari/537.36"),
];

/// 腾讯 AH 分页数据公共拉取：返回全部页的 `page_data` 字符串数组。
fn tx_ah_page_data() -> Result<Vec<String>> {
    let http = HttpClient::default();
    let params = json!({
        "board": "A_H",
        "metric": "price",
        "pageSize": "20",
        "reqPage": "1",
        "order": "decs",
        "var_name": "list_data",
    });
    let mut params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let first_text =
        http.get_text_with_headers(TX_HK_RANK_URL, &params, TX_HK_RANK_HEADERS, None)?;
    let first: Value = tx_jsonp_parse(&first_text, TX_HK_RANK_URL)?;
    let page_count = first
        .get("data")
        .and_then(|d| d.get("page_count"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let mut out: Vec<String> = Vec::new();
    let append = |v: &Value, out: &mut Vec<String>| {
        if let Some(arr) = v
            .get("data")
            .and_then(|d| d.get("page_data"))
            .and_then(Value::as_array)
        {
            for s in arr.iter().filter_map(Value::as_str) {
                out.push(s.to_string());
            }
        }
    };
    append(&first, &mut out);
    for page in 1..page_count {
        params.insert("reqPage".into(), json!(page.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_text_with_headers(TX_HK_RANK_URL, &params, TX_HK_RANK_HEADERS, None) {
            Ok(t) => {
                if let Ok(v) = tx_jsonp_parse(&t, TX_HK_RANK_URL) {
                    append(&v, &mut out);
                }
            }
            Err(_) => break,
        }
    }
    Ok(out)
}

/// 解析腾讯 `var xxx={...};` JSONP 响应（取首 `{` 到末 `}`）。
fn tx_jsonp_parse(text: &str, url: &str) -> Result<Value> {
    let start = text
        .find('{')
        .ok_or_else(|| AkshareError::Empty("腾讯 JSONP 响应缺少对象".into()))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| AkshareError::Empty("腾讯 JSONP 响应缺少对象尾".into()))?;
    serde_json::from_str(&text[start..=end]).map_err(|e| AkshareError::json(url, e.to_string()))
}

/// 腾讯财经-港股-AH-实时行情（对应 akshare [`akshare.stock_zh_ah_spot`]）。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌幅, 涨跌额, 买入, 卖出, 成交量, 成交额, 今开, 昨收, 最高, 最低`
pub fn stock_zh_ah_spot() -> Result<Df> {
    let data = tx_ah_page_data()?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for line in &data {
        let parts: Vec<&str> = line.split('~').collect();
        let pick = |i: usize| parts.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        rows.push(vec![
            pick(0),
            pick(1),
            pick(2),
            pick(3),
            pick(4),
            pick(5),
            pick(6),
            pick(7),
            pick(8),
            pick(9),
            pick(10),
            pick(11),
            pick(12),
        ]);
    }
    const COLS: [&str; 13] = [
        "代码",
        "名称",
        "最新价",
        "涨跌幅",
        "涨跌额",
        "买入",
        "卖出",
        "成交量",
        "成交额",
        "今开",
        "昨收",
        "最高",
        "最低",
    ];
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_numeric(&COLS[2..])?;
    Ok(df)
}

/// 腾讯财经-港股-AH-股票名称（对应 akshare [`akshare.stock_zh_ah_name`]）。
///
/// # 返回列
/// `代码, 名称`
pub fn stock_zh_ah_name() -> Result<Df> {
    let data = tx_ah_page_data()?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for line in &data {
        let parts: Vec<&str> = line.split('~').collect();
        rows.push(vec![
            parts
                .first()
                .map(|s| Some((*s).to_string()))
                .unwrap_or(None),
            parts.get(1).map(|s| Some((*s).to_string())).unwrap_or(None),
        ]);
    }
    const COLS: [&str; 2] = ["代码", "名称"];
    Df::from_string_rows(&COLS, &rows)
}

/// 腾讯财经-港股-AH-股票历史行情（对应 akshare [`akshare.stock_zh_ah_daily`]）。
///
/// - `symbol`: 股票代码，如 `"02318"`
/// - `start_year`/`end_year`: 年份字符串（左闭右开，与 akshare 一致）
/// - `adjust`: `""` / `"qfq"` / `"hfq"`
///
/// # 返回列
/// `date, open, close, high, low, volume`（akshare 最终列契约）
pub fn stock_zh_ah_daily(
    symbol: &str,
    start_year: &str,
    end_year: &str,
    adjust: &str,
) -> Result<Df> {
    let http = HttpClient::default();
    let sy: i32 = start_year
        .parse()
        .map_err(|_| AkshareError::Param(format!("start_year 非数字: {start_year}")))?;
    let ey: i32 = end_year
        .parse()
        .map_err(|_| AkshareError::Param(format!("end_year 非数字: {end_year}")))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for year in sy..ey {
        let url = if adjust.is_empty() {
            "http://web.ifzq.gtimg.cn/appstock/app/kline/kline"
        } else {
            "https://web.ifzq.gtimg.cn/appstock/app/hkfqkline/get"
        };
        let params = json!({
            "_var": format!("kline_day{adjust}{year}"),
            "param": format!("hk{symbol},day,{year}-01-01,{}-12-31,640,{}", year + 1, adjust),
            "r": rand::random::<f64>().to_string(),
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let text = http.get_text(url, &params, Some("http://gu.qq.com/hk01033/gp"))?;
        // 响应 `var kline_dayxxx={...};` 取对象
        let start = text
            .find('{')
            .ok_or_else(|| AkshareError::Empty("腾讯 K 线响应缺少对象".into()))?;
        let end = text
            .rfind('}')
            .ok_or_else(|| AkshareError::Empty("腾讯 K 线响应缺少对象尾".into()))?;
        let obj: Value = serde_json::from_str(&text[start..=end])
            .map_err(|e| AkshareError::json(url, e.to_string()))?;
        // data.hk02318.qfqday / day / hfqday 数组，每行 "YYYY-MM-DD open close high low volume ..."
        let stock_key = format!("hk{symbol}");
        let data = obj.get("data").and_then(|d| d.get(&stock_key));
        let day = data
            .and_then(|d| d.get(format!("{adjust}day").as_str()))
            .or_else(|| data.and_then(|d| d.get("day")))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for line in day.iter().filter_map(Value::as_array) {
            let pick = |i: usize| {
                line.get(i)
                    .and_then(Value::as_str)
                    .map(|s| Some(s.to_string()))
                    .unwrap_or(None)
            };
            rows.push(vec![pick(0), pick(1), pick(2), pick(3), pick(4), pick(5)]);
        }
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
    const COLS: [&str; 6] = ["date", "open", "close", "high", "low", "volume"];
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 涨停股池（对应 akshare [`akshare.stock_zt_pool_em`]）。
///
/// `date`: 交易日 `YYYYMMDD`。数据为空（非交易日或当日无涨停）时返回
/// 带 16 列契约的空表（akshare 返回无列空表，本实现保持列契约一致）。
///
/// # 返回列
/// `序号, 代码, 名称, 涨跌幅, 最新价, 成交额, 流通市值, 总市值, 换手率,
/// 封板资金, 首次封板时间, 最后封板时间, 炸板次数, 涨停统计, 连板数, 所属行业`
pub fn stock_zt_pool_em(date: &str) -> Result<Df> {
    let http = HttpClient::default();
    let params = json!({
        "ut": UT_KLINE,
        "dpt": "wz.ztzt",
        "Pageindex": "0",
        "pagesize": "10000",
        "sort": "fbt:asc",
        "date": date,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(
        "https://push2ex.eastmoney.com/getTopicZTPool",
        &params,
        None,
    )?;
    let pool = value
        .get("data")
        .and_then(|d| d.get("pool"))
        .and_then(Value::as_array);
    match pool {
        None => Err(AkshareError::empty(format!("{date} 无涨停池数据"))),
        Some(pool) if pool.is_empty() => Df::from_string_rows(&ZT_POOL_SELECT, &[]),
        Some(pool) => finalize_zt_pool(pool),
    }
}

/// 涨停板行情系列共用拉取（对应 akshare `stock_zt_pool_*` 的 `push2ex` GET）。
///
/// 返回响应 `data.pool` 数组；`data` 为 `null` 或 `pool` 缺失时返回 `None`
/// （由调用方按 akshare 语义报错），`pool` 为空数组时返回 `Some(空)`。
fn fetch_zt_topic_pool(
    url: &str,
    date: &str,
    sort: &str,
    pagesize: &str,
) -> Result<Option<Vec<Value>>> {
    let http = HttpClient::default();
    let params = json!({
        "ut": UT_KLINE,
        "dpt": "wz.ztzt",
        "Pageindex": "0",
        "pagesize": pagesize,
        "sort": sort,
        "date": date,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(url, &params, None)?;
    match value
        .get("data")
        .and_then(|d| d.get("pool"))
        .and_then(Value::as_array)
    {
        None => Ok(None),
        Some(pool) => Ok(Some(pool.clone())),
    }
}

/// 昨日涨停股池（对应 akshare [`akshare.stock_zt_pool_previous_em`]）。
///
/// `date`: 交易日 `YYYYMMDD`。
///
/// # 返回列
/// `序号, 代码, 名称, 涨跌幅, 最新价, 涨停价, 成交额, 流通市值, 总市值, 换手率,
/// 涨速, 振幅, 昨日封板时间, 昨日连板数, 涨停统计, 所属行业`
pub fn stock_zt_pool_previous_em(date: &str) -> Result<Df> {
    let pool = fetch_zt_topic_pool(
        "https://push2ex.eastmoney.com/getYesterdayZTPool",
        date,
        "zs:desc",
        "5000",
    )?;
    match pool {
        None => Err(AkshareError::empty(format!("{date} 无昨日涨停池数据"))),
        Some(pool) if pool.is_empty() => Df::from_string_rows(&ZT_POOL_PREVIOUS_SELECT, &[]),
        Some(pool) => finalize_zt_pool_previous(&pool),
    }
}

/// 强势股池（对应 akshare [`akshare.stock_zt_pool_strong_em`]）。
///
/// `date`: 交易日 `YYYYMMDD`。
///
/// # 返回列
/// `序号, 代码, 名称, 涨跌幅, 最新价, 涨停价, 成交额, 流通市值, 总市值, 换手率,
/// 涨速, 是否新高, 量比, 涨停统计, 入选理由, 所属行业`
pub fn stock_zt_pool_strong_em(date: &str) -> Result<Df> {
    let pool = fetch_zt_topic_pool(
        "https://push2ex.eastmoney.com/getTopicQSPool",
        date,
        "zdp:desc",
        "5000",
    )?;
    match pool {
        None => Err(AkshareError::empty(format!("{date} 无强势股池数据"))),
        Some(pool) if pool.is_empty() => Df::from_string_rows(&ZT_POOL_STRONG_SELECT, &[]),
        Some(pool) => finalize_zt_pool_strong(&pool),
    }
}

/// 次新股池（对应 akshare [`akshare.stock_zt_pool_sub_new_em`]）。
///
/// `date`: 交易日 `YYYYMMDD`。
///
/// # 返回列
/// `序号, 代码, 名称, 涨跌幅, 最新价, 涨停价, 成交额, 流通市值, 总市值, 转手率,
/// 开板几日, 开板日期, 上市日期, 是否新高, 涨停统计, 所属行业`
pub fn stock_zt_pool_sub_new_em(date: &str) -> Result<Df> {
    let pool = fetch_zt_topic_pool(
        "https://push2ex.eastmoney.com/getTopicCXPooll",
        date,
        "ods:asc",
        "5000",
    )?;
    match pool {
        None => Err(AkshareError::empty(format!("{date} 无次新股池数据"))),
        Some(pool) if pool.is_empty() => Df::from_string_rows(&ZT_POOL_SUB_NEW_SELECT, &[]),
        Some(pool) => finalize_zt_pool_sub_new(&pool),
    }
}

/// 炸板股池（对应 akshare [`akshare.stock_zt_pool_zbgc_em`]）。
///
/// `date`: 交易日 `YYYYMMDD`（akshare 限定最近 30 个交易日，本实现交由接口返回空处理）。
///
/// # 返回列
/// `序号, 代码, 名称, 涨跌幅, 最新价, 涨停价, 成交额, 流通市值, 总市值, 换手率,
/// 涨速, 首次封板时间, 炸板次数, 涨停统计, 振幅, 所属行业`
pub fn stock_zt_pool_zbgc_em(date: &str) -> Result<Df> {
    let pool = fetch_zt_topic_pool(
        "https://push2ex.eastmoney.com/getTopicZBPool",
        date,
        "fbt:asc",
        "5000",
    )?;
    match pool {
        None => Err(AkshareError::empty(format!("{date} 无炸板股池数据"))),
        Some(pool) if pool.is_empty() => Df::from_string_rows(&ZT_POOL_ZBGC_SELECT, &[]),
        Some(pool) => finalize_zt_pool_zbgc(&pool),
    }
}

/// 跌停股池（对应 akshare [`akshare.stock_zt_pool_dtgc_em`]）。
///
/// `date`: 交易日 `YYYYMMDD`（akshare 限定最近 30 个交易日，本实现交由接口返回空处理）。
///
/// # 返回列
/// `序号, 代码, 名称, 涨跌幅, 最新价, 成交额, 流通市值, 总市值, 动态市盈率, 换手率,
/// 封单资金, 最后封板时间, 板上成交额, 连续跌停, 开板次数, 所属行业`
pub fn stock_zt_pool_dtgc_em(date: &str) -> Result<Df> {
    let pool = fetch_zt_topic_pool(
        "https://push2ex.eastmoney.com/getTopicDTPool",
        date,
        "fund:asc",
        "10000",
    )?;
    match pool {
        None => Err(AkshareError::empty(format!("{date} 无跌停股池数据"))),
        Some(pool) if pool.is_empty() => Df::from_string_rows(&ZT_POOL_DTGC_SELECT, &[]),
        Some(pool) => finalize_zt_pool_dtgc(&pool),
    }
}

/// 个股资金流（对应 akshare [`akshare.stock_individual_fund_flow`]）。
///
/// `market`: `"sh"`/`"sz"`/`"bj"`。
///
/// # 返回列
/// `日期, 收盘价, 涨跌幅, 主力净流入-净额, 主力净流入-净占比, 超大单净流入-净额,
/// 超大单净流入-净占比, 大单净流入-净额, 大单净流入-净占比, 中单净流入-净额,
/// 中单净流入-净占比, 小单净流入-净额, 小单净流入-净占比`
pub fn stock_individual_fund_flow(stock: &str, market: &str) -> Result<Df> {
    let m = match market {
        "sh" => "1",
        "sz" | "bj" => "0",
        _ => return Err(AkshareError::Param(format!("无效 market: {market}"))),
    };
    let secid = format!("{m}.{stock}");
    let http = HttpClient::default();
    let params = json!({
        "lmt": "0",
        "klt": "101",
        "secid": secid,
        "fields1": "f1,f2,f3,f7",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64,f65",
        "ut": "b2884a393a59ad64002292a3e90d46a5",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(
        "https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get",
        &params,
        None,
    )?;
    let klines = value
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array);
    match klines {
        None => Err(AkshareError::empty(format!("{stock} 无资金流数据"))),
        Some(k) => finalize_fflow(k),
    }
}

/// 资金流排名接口公共分页（不排序，保留接口返回顺序；对应 akshare 手动分页 concat）。
///
/// 返回 `(rows, )`：rows 为全部页的 `diff` 行。调用方负责重命名/编序。
fn fund_flow_rank_rows(http: &HttpClient, params: &Map<String, Value>) -> Result<Vec<Value>> {
    let urls = push2_urls("/api/qt/clist/get");
    http.fetch_paginated_diff_any(&urls, params, None)
}

/// 大盘资金流（对应 akshare [`akshare.stock_market_fund_flow`]）。
///
/// push2his `fflow/daykline/get`（secid=1.000001 + secid2=0.399001），15 字段。
///
/// # 返回列
/// `日期, 上证-收盘价, 上证-涨跌幅, 深证-收盘价, 深证-涨跌幅,
/// 主力净流入-净额, 主力净流入-净占比, 超大单净流入-净额, 超大单净流入-净占比,
/// 大单净流入-净额, 大单净流入-净占比, 中单净流入-净额, 中单净流入-净占比,
/// 小单净流入-净额, 小单净流入-净占比`
pub fn stock_market_fund_flow() -> Result<Df> {
    let http = HttpClient::default();
    let params = json!({
        "lmt": "0",
        "klt": "101",
        "secid": "1.000001",
        "secid2": "0.399001",
        "fields1": "f1,f2,f3,f7",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64,f65",
        "ut": "b2884a393a59ad64002292a3e90d46a5",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(
        "https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get",
        &params,
        None,
    )?;
    let klines = value
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // 15 字段：日期,主力净流入-净额,小单净流入-净额,中单净流入-净额,大单净流入-净额,
    // 超大单净流入-净额,主力净流入-净占比,小单净流入-净占比,中单净流入-净占比,大单净流入-净占比,
    // 超大单净流入-净占比,上证-收盘价,上证-涨跌幅,深证-收盘价,深证-涨跌幅
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for line in klines.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        rows.push(vec![
            pick(0),
            pick(11),
            pick(12),
            pick(13),
            pick(14),
            pick(1),
            pick(6),
            pick(5),
            pick(10),
            pick(4),
            pick(9),
            pick(3),
            pick(8),
            pick(2),
            pick(7),
        ]);
    }
    const COLS: [&str; 15] = [
        "日期",
        "上证-收盘价",
        "上证-涨跌幅",
        "深证-收盘价",
        "深证-涨跌幅",
        "主力净流入-净额",
        "主力净流入-净占比",
        "超大单净流入-净额",
        "超大单净流入-净占比",
        "大单净流入-净额",
        "大单净流入-净占比",
        "中单净流入-净额",
        "中单净流入-净占比",
        "小单净流入-净额",
        "小单净流入-净占比",
    ];
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 个股资金流排行（对应 akshare [`akshare.stock_individual_fund_flow_rank`]）。
///
/// - `indicator`: `"今日"` / `"3日"` / `"5日"` / `"10日"`
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, {n}日涨跌幅, {n}日主力净流入-净额, {n}日主力净流入-净占比,
/// {n}日超大单净流入-净额, {n}日超大单净流入-净占比, {n}日大单净流入-净额, {n}日大单净流入-净占比,
/// {n}日中单净流入-净额, {n}日中单净流入-净占比, {n}日小单净流入-净额, {n}日小单净流入-净占比`
pub fn stock_individual_fund_flow_rank(indicator: &str) -> Result<Df> {
    let (fid, fields, prefix): (&str, &str, &str) = match indicator {
        "今日" => (
            "f62",
            "f12,f14,f2,f3,f62,f184,f66,f69,f72,f75,f78,f81,f84,f87,f204,f205,f124",
            "今日",
        ),
        "3日" => (
            "f267",
            "f12,f14,f2,f127,f267,f268,f269,f270,f271,f272,f273,f274,f275,f276,f257,f258,f124",
            "3日",
        ),
        "5日" => (
            "f164",
            "f12,f14,f2,f109,f164,f165,f166,f167,f168,f169,f170,f171,f172,f173,f257,f258,f124",
            "5日",
        ),
        "10日" => (
            "f174",
            "f12,f14,f2,f160,f174,f175,f176,f177,f178,f179,f180,f181,f182,f183,f260,f261,f124",
            "10日",
        ),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 indicator: {other}，可选 今日/3日/5日/10日"
            )))
        }
    };
    let params = json!({
        "fid": fid,
        "po": "1",
        "pz": "100",
        "pn": "1",
        "np": "1",
        "fltt": "2",
        "invt": "2",
        "ut": "b2884a393a59ad64002292a3e90d46a5",
        "fs": "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2,m:1+t:2+f:!2,m:1+t:23+f:!2,m:0+t:7+f:!2,m:1+t:3+f:!2",
        "fields": fields,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let rows = fund_flow_rank_rows(&http, &params)?;
    fund_flow_rank_build(&rows, prefix)
}

/// 资金流排名输出列（名称前缀不同，字段抽取相同模式：f12/f14/f2 + 涨跌幅 + 9 个净流入列）。
fn fund_flow_rank_build(rows: &[Value], prefix: &str) -> Result<Df> {
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()), // 序号（akshare index=range(1,n+1)）
            f("f12"),
            f("f14"),
            f("f2"),
            f(if prefix == "今日" {
                "f3"
            } else if prefix == "3日" {
                "f127"
            } else if prefix == "5日" {
                "f109"
            } else {
                "f160"
            }),
            f(if prefix == "今日" {
                "f62"
            } else if prefix == "3日" {
                "f267"
            } else if prefix == "5日" {
                "f164"
            } else {
                "f174"
            }),
            f(if prefix == "今日" {
                "f184"
            } else if prefix == "3日" {
                "f268"
            } else if prefix == "5日" {
                "f165"
            } else {
                "f175"
            }),
            f(if prefix == "今日" {
                "f66"
            } else if prefix == "3日" {
                "f269"
            } else if prefix == "5日" {
                "f166"
            } else {
                "f176"
            }),
            f(if prefix == "今日" {
                "f69"
            } else if prefix == "3日" {
                "f270"
            } else if prefix == "5日" {
                "f167"
            } else {
                "f177"
            }),
            f(if prefix == "今日" {
                "f72"
            } else if prefix == "3日" {
                "f271"
            } else if prefix == "5日" {
                "f168"
            } else {
                "f178"
            }),
            f(if prefix == "今日" {
                "f75"
            } else if prefix == "3日" {
                "f272"
            } else if prefix == "5日" {
                "f169"
            } else {
                "f179"
            }),
            f(if prefix == "今日" {
                "f78"
            } else if prefix == "3日" {
                "f273"
            } else if prefix == "5日" {
                "f170"
            } else {
                "f180"
            }),
            f(if prefix == "今日" {
                "f81"
            } else if prefix == "3日" {
                "f274"
            } else if prefix == "5日" {
                "f171"
            } else {
                "f181"
            }),
            f(if prefix == "今日" {
                "f84"
            } else if prefix == "3日" {
                "f275"
            } else if prefix == "5日" {
                "f172"
            } else {
                "f182"
            }),
            f(if prefix == "今日" {
                "f87"
            } else if prefix == "3日" {
                "f276"
            } else if prefix == "5日" {
                "f173"
            } else {
                "f183"
            }),
        ]);
    }
    let cols: Vec<String> = vec![
        "序号".into(),
        "代码".into(),
        "名称".into(),
        "最新价".into(),
        format!("{prefix}涨跌幅"),
        format!("{prefix}主力净流入-净额"),
        format!("{prefix}主力净流入-净占比"),
        format!("{prefix}超大单净流入-净额"),
        format!("{prefix}超大单净流入-净占比"),
        format!("{prefix}大单净流入-净额"),
        format!("{prefix}大单净流入-净占比"),
        format!("{prefix}中单净流入-净额"),
        format!("{prefix}中单净流入-净占比"),
        format!("{prefix}小单净流入-净额"),
        format!("{prefix}小单净流入-净占比"),
    ];
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_numeric(&col_refs[3..])?;
    Ok(df)
}

/// 主力净流入排名（对应 akshare [`akshare.stock_main_fund_flow`]）。
///
/// - `symbol`: `"全部股票"` / `"沪深A股"` / `"沪市A股"` / `"科创板"` / `"深市A股"` /
///   `"创业板"` / `"沪市B股"` / `"深市B股"`
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 今日排行榜-主力净占比, 今日排行榜-今日排名, 今日排行榜-今日涨跌,
/// 5日排行榜-主力净占比, 5日排行榜-5日排名, 5日排行榜-5日涨跌,
/// 10日排行榜-主力净占比, 10日排行榜-10日排名, 10日排行榜-10日涨跌, 所属板块`
pub fn stock_main_fund_flow(symbol: &str) -> Result<Df> {
    let fs = match symbol {
        "全部股票" => "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2,m:1+t:2+f:!2,m:1+t:23+f:!2,m:0+t:7+f:!2,m:1+t:3+f:!2",
        "沪深A股" => "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2,m:1+t:2+f:!2,m:1+t:23+f:!2",
        "沪市A股" => "m:1+t:2+f:!2,m:1+t:23+f:!2",
        "科创板" => "m:1+t:23+f:!2",
        "深市A股" => "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2",
        "创业板" => "m:0+t:80+f:!2",
        "沪市B股" => "m:1+t:3+f:!2",
        "深市B股" => "m:0+t:7+f:!2",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全部股票/沪深A股/沪市A股/科创板/深市A股/创业板/沪市B股/深市B股"
            )))
        }
    };
    let params = json!({
        "fid": "f184",
        "po": "1",
        "pz": "100",
        "pn": "1",
        "np": "1",
        "fltt": "2",
        "invt": "2",
        "fields": "f2,f3,f12,f13,f14,f62,f184,f225,f165,f263,f109,f175,f264,f160,f100,f124,f265,f1",
        "ut": "b2884a393a59ad64002292a3e90d46a5",
        "fs": fs,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    // akshare 用 fetch_paginated_data（f3 数值降序 + 序号）
    let df = fetch_clist(&http, &push2_urls("/api/qt/clist/get"), &params)?;
    let df = df.select(&[
        "index", "f12", "f14", "f2", "f184", "f225", "f3", "f165", "f263", "f109", "f175", "f264",
        "f160", "f100",
    ])?;
    let mut df = df;
    df.rename_columns(&[
        "序号",
        "代码",
        "名称",
        "最新价",
        "今日排行榜-主力净占比",
        "今日排行榜-今日排名",
        "今日排行榜-今日涨跌",
        "5日排行榜-主力净占比",
        "5日排行榜-5日排名",
        "5日排行榜-5日涨跌",
        "10日排行榜-主力净占比",
        "10日排行榜-10日排名",
        "10日排行榜-10日涨跌",
        "所属板块",
    ])?;
    df.cast_numeric(&[
        "最新价",
        "今日排行榜-主力净占比",
        "今日排行榜-今日排名",
        "今日排行榜-今日涨跌",
        "5日排行榜-主力净占比",
        "5日排行榜-5日排名",
        "5日排行榜-5日涨跌",
        "10日排行榜-主力净占比",
        "10日排行榜-10日排名",
        "10日排行榜-10日涨跌",
    ])?;
    Ok(df)
}

/// 板块资金流排名（对应 akshare [`akshare.stock_sector_fund_flow_rank`]）。
///
/// - `indicator`: `"今日"` / `"5日"` / `"10日"`
/// - `sector_type`: `"行业资金流"` / `"概念资金流"` / `"地域资金流"`
///
/// # 返回列
/// `序号, 名称, {n}日涨跌幅, {n}日主力净流入-净额, {n}日主力净流入-净占比,
/// {n}日超大单净流入-净额, {n}日超大单净流入-净占比, {n}日大单净流入-净额, {n}日大单净流入-净占比,
/// {n}日中单净流入-净额, {n}日中单净流入-净占比, {n}日小单净流入-净额, {n}日小单净流入-净占比,
/// {n}日主力净流入最大股`
pub fn stock_sector_fund_flow_rank(indicator: &str, sector_type: &str) -> Result<Df> {
    let t = match sector_type {
        "行业资金流" => "2",
        "概念资金流" => "3",
        "地域资金流" => "1",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 sector_type: {other}，可选 行业资金流/概念资金流/地域资金流"
            )))
        }
    };
    let (fid0, stat, fields, prefix): (&str, &str, &str, &str) = match indicator {
        "今日" => (
            "f62",
            "1",
            "f12,f14,f2,f3,f62,f184,f66,f69,f72,f75,f78,f81,f84,f87,f204,f205,f124",
            "今日",
        ),
        "5日" => (
            "f164",
            "5",
            "f12,f14,f2,f109,f164,f165,f166,f167,f168,f169,f170,f171,f172,f173,f257,f258,f124",
            "5日",
        ),
        "10日" => (
            "f174",
            "10",
            "f12,f14,f2,f160,f174,f175,f176,f177,f178,f179,f180,f181,f182,f183,f260,f261,f124",
            "10日",
        ),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 indicator: {other}，可选 今日/5日/10日"
            )))
        }
    };
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "b2884a393a59ad64002292a3e90d46a5",
        "fltt": "2",
        "invt": "2",
        "fid0": fid0,
        "fs": format!("m:90 t:{t}"),
        "stat": stat,
        "fields": fields,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let rows = fund_flow_rank_rows(&http, &params)?;
    sector_fund_flow_rank_build(&rows, prefix)
}

/// 板块资金流排名输出（按 主力净流入-净额 数值降序 + 序号，对应 akshare sort_values）。
fn sector_fund_flow_rank_build(rows: &[Value], prefix: &str) -> Result<Df> {
    let main_net = |row: &Value| -> Option<String> {
        row.get(if prefix == "今日" {
            "f62"
        } else if prefix == "5日" {
            "f164"
        } else {
            "f174"
        })
        .and_then(json_value_to_string)
    };
    let mut out: Vec<(Option<String>, Vec<Option<String>>)> = Vec::with_capacity(rows.len());
    for row in rows {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        let main = main_net(row);
        out.push((
            main.clone(),
            vec![
                f("f14"),
                f(if prefix == "今日" {
                    "f3"
                } else if prefix == "5日" {
                    "f109"
                } else {
                    "f160"
                }),
                main,
                f(if prefix == "今日" {
                    "f184"
                } else if prefix == "5日" {
                    "f165"
                } else {
                    "f175"
                }),
                f(if prefix == "今日" {
                    "f66"
                } else if prefix == "5日" {
                    "f166"
                } else {
                    "f176"
                }),
                f(if prefix == "今日" {
                    "f69"
                } else if prefix == "5日" {
                    "f167"
                } else {
                    "f177"
                }),
                f(if prefix == "今日" {
                    "f72"
                } else if prefix == "5日" {
                    "f168"
                } else {
                    "f178"
                }),
                f(if prefix == "今日" {
                    "f75"
                } else if prefix == "5日" {
                    "f169"
                } else {
                    "f179"
                }),
                f(if prefix == "今日" {
                    "f78"
                } else if prefix == "5日" {
                    "f170"
                } else {
                    "f180"
                }),
                f(if prefix == "今日" {
                    "f81"
                } else if prefix == "5日" {
                    "f171"
                } else {
                    "f181"
                }),
                f(if prefix == "今日" {
                    "f84"
                } else if prefix == "5日" {
                    "f172"
                } else {
                    "f182"
                }),
                f(if prefix == "今日" {
                    "f87"
                } else if prefix == "5日" {
                    "f173"
                } else {
                    "f183"
                }),
                f(if prefix == "今日" {
                    "f204"
                } else if prefix == "5日" {
                    "f257"
                } else {
                    "f260"
                }),
            ],
        ));
    }
    // 按主力净流入-净额 数值降序（缺失置后），对应 akshare sort_values(ascending=False)
    let num = |v: &str| v.parse::<f64>().ok();
    out.sort_by(|a, b| {
        b.0.as_deref()
            .and_then(num)
            .partial_cmp(&a.0.as_deref().and_then(num))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut rows_out: Vec<Vec<Option<String>>> = Vec::with_capacity(out.len());
    for (i, (_, r)) in out.iter().enumerate() {
        let mut row = vec![Some((i + 1).to_string())];
        row.extend(r.iter().cloned());
        rows_out.push(row);
    }
    let cols: Vec<String> = vec![
        "序号".into(),
        "名称".into(),
        format!("{prefix}涨跌幅"),
        format!("{prefix}主力净流入-净额"),
        format!("{prefix}主力净流入-净占比"),
        format!("{prefix}超大单净流入-净额"),
        format!("{prefix}超大单净流入-净占比"),
        format!("{prefix}大单净流入-净额"),
        format!("{prefix}大单净流入-净占比"),
        format!("{prefix}中单净流入-净额"),
        format!("{prefix}中单净流入-净占比"),
        format!("{prefix}小单净流入-净额"),
        format!("{prefix}小单净流入-净占比"),
        format!("{prefix}主力净流入最大股"),
    ];
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &rows_out)?;
    df.cast_numeric(&col_refs[2..=12])?;
    Ok(df)
}

/// 板块代码映射（对应 akshare `_get_stock_sector_fund_flow_summary_code`：
/// clist `m:90 t:2` 行业 名称→代码；`m:90 t:3` 概念）。
fn sector_code_map(
    http: &HttpClient,
    t: &str,
) -> Result<std::collections::HashMap<String, String>> {
    let params = json!({
        "fid": "f62",
        "po": "1",
        "pz": "100",
        "pn": "1",
        "np": "1",
        "fltt": "2",
        "invt": "2",
        "ut": "8dec03ba335b81bf4ebdf7b29ec27d15",
        "fs": format!("m:90 t:{t}"),
        "fields": "f12,f14,f2,f3,f62,f184,f66,f69,f72,f75,f78,f81,f84,f87,f204,f205,f124,f1,f13",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let rows = fund_flow_rank_rows(http, &params)?;
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let name = row
            .get("f14")
            .and_then(json_value_to_string)
            .unwrap_or_default();
        let code = row
            .get("f12")
            .and_then(json_value_to_string)
            .unwrap_or_default();
        if !name.is_empty() && !code.is_empty() {
            map.insert(name, code);
        }
    }
    Ok(map)
}

/// 行业/概念历史资金流公共实现（push2his fflow，secid=90.{code}，11 列）。
fn sector_fflow_hist(symbol: &str, t: &str) -> Result<Df> {
    let http = HttpClient::default();
    let map = sector_code_map(&http, t)?;
    let code = map
        .get(symbol)
        .ok_or_else(|| AkshareError::Param(format!("未知板块名称: {symbol}（{t}）")))?;
    let params = json!({
        "lmt": "0",
        "klt": "101",
        "fields1": "f1,f2,f3,f7",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64,f65",
        "secid": format!("90.{code}"),
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(
        "https://push2his.eastmoney.com/api/qt/stock/fflow/daykline/get",
        &params,
        None,
    )?;
    let klines = value
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 15 字段：日期,主力净流入-净额,小单净流入-净额,中单净流入-净额,大单净流入-净额,
    // 超大单净流入-净额,主力净流入-净占比,小单净流入-净占比,中单净流入-净占比,大单净流入-净占比,
    // 超大单净流入-净占比,-,-,-,-
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for line in klines.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        rows.push(vec![
            pick(0),
            pick(1),
            pick(6),
            pick(5),
            pick(10),
            pick(4),
            pick(9),
            pick(3),
            pick(8),
            pick(2),
            pick(7),
        ]);
    }
    const COLS: [&str; 11] = [
        "日期",
        "主力净流入-净额",
        "主力净流入-净占比",
        "超大单净流入-净额",
        "超大单净流入-净占比",
        "大单净流入-净额",
        "大单净流入-净占比",
        "中单净流入-净额",
        "中单净流入-净占比",
        "小单净流入-净额",
        "小单净流入-净占比",
    ];
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

/// 行业历史资金流（对应 akshare [`akshare.stock_sector_fund_flow_hist`]）。
///
/// - `symbol`: 行业名称，如 `"汽车服务"`。
///
/// # 返回列
/// `日期, 主力净流入-净额, 主力净流入-净占比, 超大单净流入-净额, 超大单净流入-净占比,
/// 大单净流入-净额, 大单净流入-净占比, 中单净流入-净额, 中单净流入-净占比,
/// 小单净流入-净额, 小单净流入-净占比`
pub fn stock_sector_fund_flow_hist(symbol: &str) -> Result<Df> {
    sector_fflow_hist(symbol, "2")
}

/// 概念历史资金流（对应 akshare [`akshare.stock_concept_fund_flow_hist`]）。
///
/// - `symbol`: 概念名称，如 `"数据要素"`。
///
/// # 返回列
/// `日期, 主力净流入-净额, 主力净流入-净占比, 超大单净流入-净额, 超大单净流入-净占比,
/// 大单净流入-净额, 大单净流入-净占比, 中单净流入-净额, 中单净流入-净占比,
/// 小单净流入-净额, 小单净流入-净占比`
pub fn stock_concept_fund_flow_hist(symbol: &str) -> Result<Df> {
    sector_fflow_hist(symbol, "3")
}

/// 行业资金流-xx行业个股资金流（对应 akshare [`akshare.stock_sector_fund_flow_summary`]）。
///
/// - `symbol`: 行业名称，如 `"电源设备"`。
/// - `indicator`: `"今日"` / `"5日"` / `"10日"`
///
/// 接口返回 `data.diff` 为「序号→行」对象（akshare `pd.DataFrame(diff).T` 转置 +
/// `index.astype(int)+1` 序号），Rust 侧按对象键序展开。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, {n}日涨跌幅, {n}日主力净流入-净额, {n}日主力净流入-净占比,
/// {n}日超大单净流入-净额, {n}日超大单净流入-净占比, {n}日大单净流入-净额, {n}日大单净流入-净占比,
/// {n}日中单净流入-净额, {n}日中单净流入-净占比, {n}日小单净流入-净额, {n}日小单净流入-净占比`
pub fn stock_sector_fund_flow_summary(symbol: &str, indicator: &str) -> Result<Df> {
    let (fid, fields, prefix): (&str, &str, &str) = match indicator {
        "今日" => (
            "f62",
            "f12,f14,f2,f3,f62,f184,f66,f69,f72,f75,f78,f81,f84,f87,f204,f205,f124,f1,f13",
            "今日",
        ),
        "5日" => (
            "f164",
            "f12,f14,f2,f109,f164,f165,f166,f167,f168,f169,f170,f171,f172,f173,f257,f258,f124,f1,f13",
            "5日",
        ),
        "10日" => (
            "f174",
            "f12,f14,f2,f160,f174,f175,f176,f177,f178,f179,f180,f181,f182,f183,f260,f261,f124,f1,f13",
            "10日",
        ),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 indicator: {other}，可选 今日/5日/10日"
            )))
        }
    };
    let http = HttpClient::default();
    let map = sector_code_map(&http, "2")?;
    let code = map
        .get(symbol)
        .ok_or_else(|| AkshareError::Param(format!("未知行业名称: {symbol}")))?;
    let params = json!({
        "fid": fid,
        "po": "1",
        "pz": "5000",
        "pn": "1",
        "np": "2",
        "fltt": "2",
        "invt": "2",
        "fs": format!("b:{code}"),
        "fields": fields,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let value = http.get_json(&push2_urls("/api/qt/clist/get")[0], &params, None)?;
    // diff 为「序号→行」对象：按键序展开为行列表
    let diff = value
        .get("data")
        .and_then(|d| d.get("diff"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut rows_vec: Vec<Value> = Vec::new();
    match diff {
        Value::Array(arr) => rows_vec = arr,
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            rows_vec = entries.into_iter().map(|(_, v)| v).collect();
        }
        _ => {}
    }

    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows_vec.len());
    for (i, row) in rows_vec.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()), // 序号
            f("f12"),
            f("f14"),
            f("f2"),
            f(if prefix == "今日" {
                "f3"
            } else if prefix == "5日" {
                "f109"
            } else {
                "f160"
            }),
            f(if prefix == "今日" {
                "f62"
            } else if prefix == "5日" {
                "f164"
            } else {
                "f174"
            }),
            f(if prefix == "今日" {
                "f184"
            } else if prefix == "5日" {
                "f165"
            } else {
                "f175"
            }),
            f(if prefix == "今日" {
                "f66"
            } else if prefix == "5日" {
                "f166"
            } else {
                "f176"
            }),
            f(if prefix == "今日" {
                "f69"
            } else if prefix == "5日" {
                "f167"
            } else {
                "f177"
            }),
            f(if prefix == "今日" {
                "f72"
            } else if prefix == "5日" {
                "f168"
            } else {
                "f178"
            }),
            f(if prefix == "今日" {
                "f75"
            } else if prefix == "5日" {
                "f169"
            } else {
                "f179"
            }),
            f(if prefix == "今日" {
                "f78"
            } else if prefix == "5日" {
                "f170"
            } else {
                "f180"
            }),
            f(if prefix == "今日" {
                "f81"
            } else if prefix == "5日" {
                "f171"
            } else {
                "f181"
            }),
            f(if prefix == "今日" {
                "f84"
            } else if prefix == "5日" {
                "f172"
            } else {
                "f182"
            }),
            f(if prefix == "今日" {
                "f87"
            } else if prefix == "5日" {
                "f173"
            } else {
                "f183"
            }),
        ]);
    }
    let cols: Vec<String> = vec![
        "序号".into(),
        "代码".into(),
        "名称".into(),
        "最新价".into(),
        format!("{prefix}涨跌幅"),
        format!("{prefix}主力净流入-净额"),
        format!("{prefix}主力净流入-净占比"),
        format!("{prefix}超大单净流入-净额"),
        format!("{prefix}超大单净流入-净占比"),
        format!("{prefix}大单净流入-净额"),
        format!("{prefix}大单净流入-净占比"),
        format!("{prefix}中单净流入-净额"),
        format!("{prefix}中单净流入-净占比"),
        format!("{prefix}小单净流入-净额"),
        format!("{prefix}小单净流入-净占比"),
    ];
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_numeric(&col_refs[3..])?;
    Ok(df)
}

/// 沪深港通资金流向（对应 akshare [`akshare.stock_hsgt_fund_flow_summary_em`]）。
///
/// # 返回列
/// `交易日, 类型, 板块, 资金方向, 交易状态, 成交净买额, 资金净流入, 当日资金余额,
/// 上涨数, 持平数, 下跌数, 相关指数, 指数涨跌幅`（金额单位：亿元，对应 akshare ÷10000）
pub fn stock_hsgt_fund_flow_summary_em() -> Result<Df> {
    let mut extra = Map::new();
    extra.insert(
        "quoteColumns".into(),
        json!("status~07~BOARD_CODE,dayNetAmtIn~07~BOARD_CODE,dayAmtRemain~07~BOARD_CODE,dayAmtThreshold~07~BOARD_CODE,f104~07~BOARD_CODE,f105~07~BOARD_CODE,f106~07~BOARD_CODE,f3~03~INDEX_CODE~INDEX_f3,netBuyAmt~07~BOARD_CODE"),
    );
    extra.insert("quoteType".into(), json!("0"));
    extra.insert("sortTypes".into(), json!("1"));
    extra.insert("sortColumns".into(), json!("MUTUAL_TYPE"));
    let http = HttpClient::default();
    let rows = fetch_datacenter_pages(
        &http,
        "RPT_MUTUAL_QUOTA",
        "TRADE_DATE,MUTUAL_TYPE,BOARD_TYPE,MUTUAL_TYPE_NAME,FUNDS_DIRECTION,INDEX_CODE,INDEX_NAME,BOARD_CODE",
        &extra,
        "2000",
    )?;
    if rows.is_empty() {
        return Err(AkshareError::empty("无沪深港通资金流向数据"));
    }
    finalize_hsgt(&rows)
}

// ============ 11. 同行比较（东财 securities datacenter，RPT_PCF10_INDUSTRY_*） ============

/// A 股代码 → 东财 `SECUCODE`（`SZ000895` → `000895.SZ`）。
fn zh_secucode(symbol: &str) -> String {
    if symbol.len() >= 2 {
        format!("{}.{}", &symbol[2..], &symbol[0..2])
    } else {
        symbol.to_string()
    }
}

/// 东方财富-行情中心-同行比较-成长性比较（对应 akshare [`akshare.stock_zh_growth_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_GROWTH`（`datacenter.eastmoney.com/securities`，`source=HSF10`），
/// 按 `SECUCODE` 过滤个股；输出 21 列 `代码, 简称, 基本每股收益增长率-3年复合/-24A/-TTM/-25E/-26E/-27E,
/// 营业收入增长率-*(同上), 净利润增长率-*(同上), 基本每股收益增长率-3年复合排名`（无 `序号`，
/// 对应 akshare `rename`+`[[...]]` 选取）。各增长率/排名数值化。
pub fn stock_zh_growth_comparison_em(symbol: &str) -> Result<Df> {
    let code = zh_secucode(symbol);
    let filter = format!("(SECUCODE=\"{code}\")");
    let extra = report_extra("PAIMING", "1", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_GROWTH",
        "ALL",
        &extra,
        "0",
        "HSF10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 21] = [
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("MGSY_3Y", "基本每股收益增长率-3年复合"),
        ("MGSYTB", "基本每股收益增长率-24A"),
        ("MGSYTTM", "基本每股收益增长率-TTM"),
        ("MGSY_1E", "基本每股收益增长率-25E"),
        ("MGSY_2E", "基本每股收益增长率-26E"),
        ("MGSY_3E", "基本每股收益增长率-27E"),
        ("YYSR_3Y", "营业收入增长率-3年复合"),
        ("YYSRTB", "营业收入增长率-24A"),
        ("YYSRTTM", "营业收入增长率-TTM"),
        ("YYSR_1E", "营业收入增长率-25E"),
        ("YYSR_2E", "营业收入增长率-26E"),
        ("YYSR_3E", "营业收入增长率-27E"),
        ("JLR_3Y", "净利润增长率-3年复合"),
        ("JLRTB", "净利润增长率-24A"),
        ("JLRTTM", "净利润增长率-TTM"),
        ("JLR_1E", "净利润增长率-25E"),
        ("JLR_2E", "净利润增长率-26E"),
        ("JLR_3E", "净利润增长率-27E"),
        ("PAIMING", "基本每股收益增长率-3年复合排名"),
    ];
    const SELECT: [&str; 21] = [
        "代码",
        "简称",
        "基本每股收益增长率-3年复合",
        "基本每股收益增长率-24A",
        "基本每股收益增长率-TTM",
        "基本每股收益增长率-25E",
        "基本每股收益增长率-26E",
        "基本每股收益增长率-27E",
        "营业收入增长率-3年复合",
        "营业收入增长率-24A",
        "营业收入增长率-TTM",
        "营业收入增长率-25E",
        "营业收入增长率-26E",
        "营业收入增长率-27E",
        "净利润增长率-3年复合",
        "净利润增长率-24A",
        "净利润增长率-TTM",
        "净利润增长率-25E",
        "净利润增长率-26E",
        "净利润增长率-27E",
        "基本每股收益增长率-3年复合排名",
    ];
    const NUMERIC: [&str; 19] = [
        "基本每股收益增长率-3年复合",
        "基本每股收益增长率-24A",
        "基本每股收益增长率-TTM",
        "基本每股收益增长率-25E",
        "基本每股收益增长率-26E",
        "基本每股收益增长率-27E",
        "营业收入增长率-3年复合",
        "营业收入增长率-24A",
        "营业收入增长率-TTM",
        "营业收入增长率-25E",
        "营业收入增长率-26E",
        "营业收入增长率-27E",
        "净利润增长率-3年复合",
        "净利润增长率-24A",
        "净利润增长率-TTM",
        "净利润增长率-25E",
        "净利润增长率-26E",
        "净利润增长率-27E",
        "基本每股收益增长率-3年复合排名",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-行情中心-同行比较-杜邦分析比较（对应 akshare [`akshare.stock_zh_dupont_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_DBFX`（`source=HSF10`），按 `SECUCODE` 过滤；输出 19 列
/// `代码, 简称, ROE-3年平均/-22A/-23A/-24A, 净利率-*(同上), 总资产周转率-*(同上),
/// 权益乘数-*(同上), ROE-3年平均排名`（无 `序号`）。各比率/排名数值化。
pub fn stock_zh_dupont_comparison_em(symbol: &str) -> Result<Df> {
    let code = zh_secucode(symbol);
    let filter = format!("(SECUCODE=\"{code}\")");
    let extra = report_extra("PAIMING", "1", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_DBFX",
        "ALL",
        &extra,
        "0",
        "HSF10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 19] = [
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("ROE_AVG", "ROE-3年平均"),
        ("ROEPJ_L3", "ROE-22A"),
        ("ROEPJ_L2", "ROE-23A"),
        ("ROEPJ_L1", "ROE-24A"),
        ("XSJLL_AVG", "净利率-3年平均"),
        ("XSJLL_L3", "净利率-22A"),
        ("XSJLL_L2", "净利率-23A"),
        ("XSJLL_L1", "净利率-24A"),
        ("TOAZZL_AVG", "总资产周转率-3年平均"),
        ("TOAZZL_L3", "总资产周转率-22A"),
        ("TOAZZL_L2", "总资产周转率-23A"),
        ("TOAZZL_L1", "总资产周转率-24A"),
        ("QYCS_AVG", "权益乘数-3年平均"),
        ("QYCS_L3", "权益乘数-22A"),
        ("QYCS_L2", "权益乘数-23A"),
        ("QYCS_L1", "权益乘数-24A"),
        ("PAIMING", "ROE-3年平均排名"),
    ];
    const SELECT: [&str; 19] = [
        "代码",
        "简称",
        "ROE-3年平均",
        "ROE-22A",
        "ROE-23A",
        "ROE-24A",
        "净利率-3年平均",
        "净利率-22A",
        "净利率-23A",
        "净利率-24A",
        "总资产周转率-3年平均",
        "总资产周转率-22A",
        "总资产周转率-23A",
        "总资产周转率-24A",
        "权益乘数-3年平均",
        "权益乘数-22A",
        "权益乘数-23A",
        "权益乘数-24A",
        "ROE-3年平均排名",
    ];
    const NUMERIC: [&str; 17] = [
        "ROE-3年平均",
        "ROE-22A",
        "ROE-23A",
        "ROE-24A",
        "净利率-3年平均",
        "净利率-22A",
        "净利率-23A",
        "净利率-24A",
        "总资产周转率-3年平均",
        "总资产周转率-22A",
        "总资产周转率-23A",
        "总资产周转率-24A",
        "权益乘数-3年平均",
        "权益乘数-22A",
        "权益乘数-23A",
        "权益乘数-24A",
        "ROE-3年平均排名",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-行情中心-同行比较-公司规模（对应 akshare [`akshare.stock_zh_scale_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_MARKET`（`source=HSF10`），按 `SECUCODE`+`CORRE_SECUCODE` 过滤、
/// `TOTAL_CAP` 降序、`pageSize=5`；输出 10 列 `代码, 简称, 总市值, 总市值排名, 流通市值,
/// 流通市值排名, 营业收入, 营业收入排名, 净利润, 净利润排名`（无 `序号`）。各市值/收入/利润/排名数值化。
pub fn stock_zh_scale_comparison_em(symbol: &str) -> Result<Df> {
    let code = zh_secucode(symbol);
    let filter = format!("(SECUCODE=\"{code}\")(CORRE_SECUCODE=\"{code}\")");
    let extra = report_extra("TOTAL_CAP", "-1", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_MARKET",
        "SECUCODE,SECURITY_CODE,SECURITY_NAME_ABBR,ORG_CODE,CORRE_SECUCODE,\
CORRE_SECURITY_CODE,CORRE_SECURITY_NAME,CORRE_ORG_CODE,TOTAL_CAP,FREECAP,\
TOTAL_OPERATEINCOME,NETPROFIT,REPORT_TYPE,TOTAL_CAP_RANK,FREECAP_RANK,\
TOTAL_OPERATEINCOME_RANK,NETPROFIT_RANK",
        &extra,
        "5",
        "HSF10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 10] = [
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("TOTAL_CAP", "总市值"),
        ("TOTAL_CAP_RANK", "总市值排名"),
        ("FREECAP", "流通市值"),
        ("FREECAP_RANK", "流通市值排名"),
        ("TOTAL_OPERATEINCOME", "营业收入"),
        ("TOTAL_OPERATEINCOME_RANK", "营业收入排名"),
        ("NETPROFIT", "净利润"),
        ("NETPROFIT_RANK", "净利润排名"),
    ];
    const SELECT: [&str; 10] = [
        "代码",
        "简称",
        "总市值",
        "总市值排名",
        "流通市值",
        "流通市值排名",
        "营业收入",
        "营业收入排名",
        "净利润",
        "净利润排名",
    ];
    const NUMERIC: [&str; 8] = [
        "总市值",
        "总市值排名",
        "流通市值",
        "流通市值排名",
        "营业收入",
        "营业收入排名",
        "净利润",
        "净利润排名",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-行业对比-成长性对比（对应 akshare [`akshare.stock_hk_growth_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_HKGROWTH`（`datacenter.eastmoney.com/securities`，`source=F10`），
/// 按 `SECUCODE`+`CORRE_SECUCODE`（`{symbol}.HK`）过滤；输出 10 列 `代码, 简称,
/// 基本每股收益同比增长率(及排名), 营业收入同比增长率(及排名), 营业利润率同比增长率(及排名),
/// 总资产同比增长率(及排名)`（无 `序号`）。各比率/排名数值化。
pub fn stock_hk_growth_comparison_em(symbol: &str) -> Result<Df> {
    let code = format!("{symbol}.HK");
    let filter = format!("(SECUCODE=\"{code}\")(CORRE_SECUCODE=\"{code}\")");
    let extra = report_extra("", "", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_HKGROWTH",
        "SECUCODE,SECURITY_CODE,ORG_CODE,REPORT_DATE,TYPE_ID,TYPE_TYPE,\
TYPE_NAME,TYPE_NAME_EN,CORRE_SECURITY_CODE,CORRE_SECUCODE,CORRE_SECURITY_NAME,\
EPS_YOY,OPERATE_INCOME_YOY,OPERATE_PROFIT_YOY,TOTAL_ASSET_YOY,EPS_YOY_RANK,\
OPINCOME_YOY_RANK,OPROFIT_YOY_RANK,TOASSET_YOY_RANK",
        &extra,
        "0",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 10] = [
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("EPS_YOY", "基本每股收益同比增长率"),
        ("EPS_YOY_RANK", "基本每股收益同比增长率排名"),
        ("OPERATE_INCOME_YOY", "营业收入同比增长率"),
        ("OPINCOME_YOY_RANK", "营业收入同比增长率排名"),
        ("OPERATE_PROFIT_YOY", "营业利润率同比增长率"),
        ("OPROFIT_YOY_RANK", "营业利润率同比增长率排名"),
        // 注：akshare 原版 field_mapping 此处为 "基本每股收总资产同比增长率益同比增长率"
        // （基本每股收益同比增长率 + 总资产同比增长率 拼接的命名 bug），为保持列名完全对齐予以保留。
        ("TOTAL_ASSET_YOY", "基本每股收总资产同比增长率益同比增长率"),
        ("TOASSET_YOY_RANK", "总资产同比增长率排名"),
    ];
    const SELECT: [&str; 10] = [
        "代码",
        "简称",
        "基本每股收益同比增长率",
        "基本每股收益同比增长率排名",
        "营业收入同比增长率",
        "营业收入同比增长率排名",
        "营业利润率同比增长率",
        "营业利润率同比增长率排名",
        "基本每股收总资产同比增长率益同比增长率",
        "总资产同比增长率排名",
    ];
    const NUMERIC: [&str; 8] = [
        "基本每股收益同比增长率",
        "基本每股收益同比增长率排名",
        "营业收入同比增长率",
        "营业收入同比增长率排名",
        "营业利润率同比增长率",
        "营业利润率同比增长率排名",
        "基本每股收总资产同比增长率益同比增长率",
        "总资产同比增长率排名",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-行业对比-规模对比（对应 akshare [`akshare.stock_hk_scale_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_SCALE`（`source=F10`），按 `SECUCODE`+`CORRE_SECUCODE`（`{symbol}.HK`）
/// 过滤；输出 10 列 `代码, 简称, 总市值, 总市值排名, 流通市值, 流通市值排名, 营业总收入,
/// 营业总收入排名, 净利润, 净利润排名`（无 `序号`）。各市值/收入/利润/排名数值化。
pub fn stock_hk_scale_comparison_em(symbol: &str) -> Result<Df> {
    let code = format!("{symbol}.HK");
    let filter = format!("(SECUCODE=\"{code}\")(CORRE_SECUCODE=\"{code}\")");
    let extra = report_extra("", "", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_SCALE",
        "SECURITY_CODE,SECUCODE,TYPE_ID,TYPE_TYPE,TYPE_NAME,TYPE_NAME_EN,\
CORRE_SECURITY_CODE,CORRE_SECUCODE,CORRE_SECURITY_NAME,MAXSTDREPORTDATE,\
HKSDQMV,HKTOTAL_MARKET_CAP,OPERATE_INCOME,GROSS_PROFIT,HKSDQMV_RANK,\
HKTOTAL_CAP_RANK,OPERATE_INCOME_RANK,GROSS_PROFIT_RANK",
        &extra,
        "0",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 10] = [
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("HKSDQMV", "总市值"),
        ("HKSDQMV_RANK", "总市值排名"),
        ("HKTOTAL_MARKET_CAP", "流通市值"),
        ("HKTOTAL_CAP_RANK", "流通市值排名"),
        ("OPERATE_INCOME", "营业总收入"),
        ("OPERATE_INCOME_RANK", "营业总收入排名"),
        ("GROSS_PROFIT", "净利润"),
        ("GROSS_PROFIT_RANK", "净利润排名"),
    ];
    const SELECT: [&str; 10] = [
        "代码",
        "简称",
        "总市值",
        "总市值排名",
        "流通市值",
        "流通市值排名",
        "营业总收入",
        "营业总收入排名",
        "净利润",
        "净利润排名",
    ];
    const NUMERIC: [&str; 8] = [
        "总市值",
        "总市值排名",
        "流通市值",
        "流通市值排名",
        "营业总收入",
        "营业总收入排名",
        "净利润",
        "净利润排名",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-证券资料（对应 akshare [`akshare.stock_hk_security_profile_em`]）。
///
/// 报表 `RPT_HKF10_INFO_SECURITYINFO`（`datacenter.eastmoney.com/securities`，`source=F10`），
/// 按 `SECUCODE="{symbol}.HK"` 过滤；输出 14 列（无 `序号`）。`发行价/发行量(股)/每手股数/每股面值` 数值化。
pub fn stock_hk_security_profile_em(symbol: &str) -> Result<Df> {
    let code = format!("{symbol}.HK");
    let filter = format!(r#"(SECUCODE="{code}")"#);
    let extra = report_extra("", "", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_HKF10_INFO_SECURITYINFO",
        "SECUCODE,SECURITY_CODE,SECURITY_NAME_ABBR,SECURITY_TYPE,LISTING_DATE,ISIN_CODE,BOARD,\
TRADE_UNIT,TRADE_MARKET,GANGGUTONGBIAODISHEN,GANGGUTONGBIAODIHU,PAR_VALUE,\
ISSUE_PRICE,ISSUE_NUM,YEAR_SETTLE_DAY",
        &extra,
        "200",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 14] = [
        ("SECUCODE", "证券代码"),
        ("SECURITY_NAME_ABBR", "证券简称"),
        ("LISTING_DATE", "上市日期"),
        ("SECURITY_TYPE", "证券类型"),
        ("ISSUE_PRICE", "发行价"),
        ("ISSUE_NUM", "发行量(股)"),
        ("TRADE_UNIT", "每手股数"),
        ("PAR_VALUE", "每股面值"),
        ("TRADE_MARKET", "交易所"),
        ("BOARD", "板块"),
        ("YEAR_SETTLE_DAY", "年结日"),
        ("ISIN_CODE", "ISIN（国际证券识别编码）"),
        ("GANGGUTONGBIAODISHEN", "是否深港通标的"),
        ("GANGGUTONGBIAODIHU", "是否沪港通标的"),
    ];
    const SELECT: [&str; 14] = [
        "证券代码",
        "证券简称",
        "上市日期",
        "证券类型",
        "发行价",
        "发行量(股)",
        "每手股数",
        "每股面值",
        "交易所",
        "板块",
        "年结日",
        "ISIN（国际证券识别编码）",
        "是否沪港通标的",
        "是否深港通标的",
    ];
    const NUMERIC: [&str; 3] = ["发行价", "发行量(股)", "每手股数"];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-公司资料（对应 akshare [`akshare.stock_hk_company_profile_em`]）。
///
/// 报表 `RPT_HKF10_INFO_ORGPROFILE`（`source=F10`），按 `SECUCODE="{symbol}.HK"` 过滤；
/// 输出 17 列（无 `序号`）。仅 `员工人数` 数值化。
pub fn stock_hk_company_profile_em(symbol: &str) -> Result<Df> {
    let code = format!("{symbol}.HK");
    let filter = format!(r#"(SECUCODE="{code}")"#);
    let extra = report_extra("", "", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_HKF10_INFO_ORGPROFILE",
        "SECUCODE,SECURITY_CODE,ORG_NAME,ORG_EN_ABBR,BELONG_INDUSTRY,FOUND_DATE,CHAIRMAN,\
SECRETARY,ACCOUNT_FIRM,REG_ADDRESS,ADDRESS,YEAR_SETTLE_DAY,EMP_NUM,ORG_TEL,ORG_FAX,ORG_EMAIL,\
ORG_WEB,ORG_PROFILE,REG_PLACE",
        &extra,
        "200",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 17] = [
        ("ORG_NAME", "公司名称"),
        ("ORG_EN_ABBR", "英文名称"),
        ("REG_PLACE", "注册地"),
        ("REG_ADDRESS", "注册地址"),
        ("FOUND_DATE", "公司成立日期"),
        ("BELONG_INDUSTRY", "所属行业"),
        ("CHAIRMAN", "董事长"),
        ("SECRETARY", "公司秘书"),
        ("EMP_NUM", "员工人数"),
        ("ADDRESS", "办公地址"),
        ("ORG_WEB", "公司网址"),
        ("ORG_EMAIL", "E-MAIL"),
        ("YEAR_SETTLE_DAY", "年结日"),
        ("ORG_TEL", "联系电话"),
        ("ACCOUNT_FIRM", "核数师"),
        ("ORG_FAX", "传真"),
        ("ORG_PROFILE", "公司介绍"),
    ];
    const SELECT: [&str; 17] = [
        "公司名称",
        "英文名称",
        "注册地",
        "注册地址",
        "公司成立日期",
        "所属行业",
        "董事长",
        "公司秘书",
        "员工人数",
        "办公地址",
        "公司网址",
        "E-MAIL",
        "年结日",
        "联系电话",
        "核数师",
        "传真",
        "公司介绍",
    ];
    const NUMERIC: [&str; 1] = ["员工人数"];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-核心必读-最新指标（对应 akshare [`akshare.stock_hk_financial_indicator_em`]）。
///
/// 报表 `RPT_CUSTOM_HKF10_FN_MAININDICATORMAX`（`source=F10`），按 `SECUCODE="{symbol}.HK"` 过滤，
/// 按 `REPORT_DATE` 降序；输出 21 列（`股票代码` + 20 个数值指标，无 `序号`）。
pub fn stock_hk_financial_indicator_em(symbol: &str) -> Result<Df> {
    let code = format!("{symbol}.HK");
    let filter = format!(r#"(SECUCODE="{code}")"#);
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_CUSTOM_HKF10_FN_MAININDICATORMAX",
        "ORG_CODE,SECUCODE,SECURITY_CODE,SECURITY_NAME_ABBR,SECURITY_INNER_CODE,REPORT_DATE,BASIC_EPS,\
PER_NETCASH_OPERATE,BPS,BPS_NEDILUTED,COMMON_ACS,PER_SHARES,ISSUED_COMMON_SHARES,HK_COMMON_SHARES,\
TOTAL_MARKET_CAP,HKSK_MARKET_CAP,OPERATE_INCOME,OPERATE_INCOME_SQ,OPERATE_INCOME_QOQ,\
OPERATE_INCOME_QOQ_SQ,HOLDER_PROFIT,HOLDER_PROFIT_SQ,HOLDER_PROFIT_QOQ,HOLDER_PROFIT_QOQ_SQ,PE_TTM,\
PE_TTM_SQ,PB_TTM,PB_TTM_SQ,NET_PROFIT_RATIO,NET_PROFIT_RATIO_SQ,ROE_AVG,ROE_AVG_SQ,ROA,\
ROA_SQ,DIVIDEND_TTM,DIVIDEND_LFY,DIVI_RATIO,DIVIDEND_RATE,IS_CNY_CODE",
        &extra,
        "200",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 21] = [
        ("BASIC_EPS", "基本每股收益(元)"),
        ("BPS", "每股净资产(元)"),
        ("COMMON_ACS", "法定股本(股)"),
        ("PER_SHARES", "每手股"),
        ("DIVIDEND_TTM", "每股股息TTM(港元)"),
        ("DIVI_RATIO", "派息比率(%)"),
        ("ISSUED_COMMON_SHARES", "已发行股本(股)"),
        ("HK_COMMON_SHARES", "已发行股本-H股(股)"),
        ("PER_NETCASH_OPERATE", "每股经营现金流(元)"),
        ("DIVIDEND_RATE", "股息率TTM(%)"),
        ("TOTAL_MARKET_CAP", "总市值(港元)"),
        ("HKSK_MARKET_CAP", "港股市值(港元)"),
        ("OPERATE_INCOME", "营业总收入"),
        ("OPERATE_INCOME_QOQ", "营业总收入滚动环比增长(%)"),
        ("NET_PROFIT_RATIO", "销售净利率(%)"),
        ("HOLDER_PROFIT", "净利润"),
        ("HOLDER_PROFIT_QOQ", "净利润滚动环比增长(%)"),
        ("ROE_AVG", "股东权益回报率(%)"),
        ("PE_TTM", "市盈率"),
        ("PB_TTM", "市净率"),
        ("ROA", "总资产回报率(%)"),
    ];
    const SELECT: [&str; 21] = [
        "基本每股收益(元)",
        "每股净资产(元)",
        "法定股本(股)",
        "每手股",
        "每股股息TTM(港元)",
        "派息比率(%)",
        "已发行股本(股)",
        "已发行股本-H股(股)",
        "每股经营现金流(元)",
        "股息率TTM(%)",
        "总市值(港元)",
        "港股市值(港元)",
        "营业总收入",
        "营业总收入滚动环比增长(%)",
        "销售净利率(%)",
        "净利润",
        "净利润滚动环比增长(%)",
        "股东权益回报率(%)",
        "市盈率",
        "市净率",
        "总资产回报率(%)",
    ];
    const NUMERIC: [&str; 17] = [
        "基本每股收益(元)",
        "每股净资产(元)",
        "法定股本(股)",
        "已发行股本(股)",
        "已发行股本-H股(股)",
        "每股经营现金流(元)",
        "总市值(港元)",
        "港股市值(港元)",
        "营业总收入",
        "营业总收入滚动环比增长(%)",
        "销售净利率(%)",
        "净利润",
        "净利润滚动环比增长(%)",
        "股东权益回报率(%)",
        "市盈率",
        "市净率",
        "总资产回报率(%)",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-核心必读-分红派息（对应 akshare [`akshare.stock_hk_dividend_payout_em`]）。
///
/// 报表 `RPT_HKF10_MAIN_DIVBASIC`（`source=F10`），按 `SECURITY_CODE="{symbol}"`(无 `.HK`) 且
/// `IS_BFP="0"` 过滤，按 `NOTICE_DATE,EX_DIVIDEND_DATE` 降序；输出 7 列（无 `序号`）。
/// `最新公告日期/除净日/发放日` 截断为 `YYYY-MM-DD`。
pub fn stock_hk_dividend_payout_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")(IS_BFP="0")"#);
    let extra = report_extra(
        "NOTICE_DATE,EX_DIVIDEND_DATE",
        "-1,-1",
        Some(&filter),
        Some(""),
        None,
        None,
    );
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_HKF10_MAIN_DIVBASIC",
        "SECURITY_CODE,UPDATE_DATE,REPORT_TYPE,EX_DIVIDEND_DATE,DIVIDEND_DATE,\
TRANSFER_END_DATE,YEAR,PLAN_EXPLAIN,IS_BFP",
        &extra,
        "200",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 7] = [
        ("UPDATE_DATE", "最新公告日期"),
        ("YEAR", "财政年度"),
        ("PLAN_EXPLAIN", "分红方案"),
        ("REPORT_TYPE", "分配类型"),
        ("EX_DIVIDEND_DATE", "除净日"),
        ("TRANSFER_END_DATE", "截至过户日"),
        ("DIVIDEND_DATE", "发放日"),
    ];
    const SELECT: [&str; 7] = [
        "最新公告日期",
        "财政年度",
        "分红方案",
        "分配类型",
        "除净日",
        "截至过户日",
        "发放日",
    ];
    let mut df = finalize_report(&rows, &RENAME, &SELECT, &[], None)?;
    df.cast_date(&["最新公告日期", "除净日", "发放日"])?;
    Ok(df)
}

/// 复刻 akshare `stock_zh_valuation_comparison_em` 的行变换（对应其 `pd.concat([iloc[-1:], iloc[:-1]])`
/// + 首行排名串 + 行互换三段逻辑）：
/// 1. 将末行旋转到首行；
/// 2. 首行 `PAIMING`(排名) 改写为 `{原末行排名}/{TOTAL_COUNT}`（`TOTAL_COUNT` 取原始首行）；
/// 3. 交换第 1、2 行（`iloc[1]` ↔ `iloc[2]`），仅当行数 ≥ 3。
fn reorder_valuation_rows(rows: &[Value]) -> Vec<Value> {
    if rows.is_empty() {
        return Vec::new();
    }
    let total_count = rows
        .first()
        .and_then(|r| r.get("TOTAL_COUNT"))
        .and_then(json_value_to_string)
        .unwrap_or_default();
    let mut out: Vec<Value> = Vec::with_capacity(rows.len());
    if let Some(last) = rows.last() {
        out.push(last.clone());
    }
    out.extend(rows.iter().take(rows.len() - 1).cloned());
    if let Some(obj) = out.get_mut(0).and_then(|v| v.as_object_mut()) {
        let rank = obj
            .get("PAIMING")
            .and_then(json_value_to_string)
            .unwrap_or_default();
        obj.insert("PAIMING".into(), json!(format!("{rank}/{total_count}")));
    }
    if out.len() >= 3 {
        out.swap(1, 2);
    }
    out
}

/// 东方财富-行情中心-同行比较-估值比较（对应 akshare [`akshare.stock_zh_valuation_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_CVALUE`（`datacenter.eastmoney.com/securities`，`source=HSF10`），
/// 按 `SECUCODE="{zh_secucode(symbol)}"` 过滤、按 `PAIMING` 升序。输出 20 列 `排名, 代码, 简称,
/// PEG, 市盈率-TTM/25E/26E/27E, 市销率-24A/TTM/25E/26E/27E, 市净率-24A/MRQ,
/// 市现率1-24A/TTM, 市现率2-24A/TTM, EV/EBITDA-24A`。akshare 对非首行做旋转+排名串+行互换
/// （见 [`reorder_valuation_rows`]），本实现在原始 JSON 行级复刻该变换后再 `finalize_report`。
pub fn stock_zh_valuation_comparison_em(symbol: &str) -> Result<Df> {
    let code = zh_secucode(symbol);
    let filter = format!(r#"(SECUCODE="{code}")"#);
    let extra = report_extra("PAIMING", "1", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_CVALUE",
        "ALL",
        &extra,
        "",
        "HSF10",
        "PC",
    )?;
    let ordered = reorder_valuation_rows(&rows);
    const RENAME: [(&str, &str); 20] = [
        ("PAIMING", "排名"),
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("PB", "市净率-24A"),
        ("PB_MRQ", "市净率-MRQ"),
        ("PCE", "市现率1-24A"),
        ("PCE_TTM", "市现率1-TTM"),
        ("PCF", "市现率2-24A"),
        ("PCF_TTM", "市现率2-TTM"),
        ("PEG", "PEG"),
        ("PE_1Y", "市盈率-25E"),
        ("PE_2Y", "市盈率-26E"),
        ("PE_3Y", "市盈率-27E"),
        ("PE_TTM", "市盈率-TTM"),
        ("PS", "市销率-24A"),
        ("PS_1Y", "市销率-25E"),
        ("PS_2Y", "市销率-26E"),
        ("PS_3Y", "市销率-27E"),
        ("PS_TTM", "市销率-TTM"),
        ("QYBS", "EV/EBITDA-24A"),
    ];
    const SELECT: [&str; 20] = [
        "排名",
        "代码",
        "简称",
        "PEG",
        "市盈率-TTM",
        "市盈率-25E",
        "市盈率-26E",
        "市盈率-27E",
        "市销率-24A",
        "市销率-TTM",
        "市销率-25E",
        "市销率-26E",
        "市销率-27E",
        "市净率-24A",
        "市净率-MRQ",
        "市现率1-24A",
        "市现率1-TTM",
        "市现率2-24A",
        "市现率2-TTM",
        "EV/EBITDA-24A",
    ];
    const NUMERIC: [&str; 17] = [
        "PEG",
        "市盈率-TTM",
        "市盈率-25E",
        "市盈率-26E",
        "市盈率-27E",
        "市销率-24A",
        "市销率-TTM",
        "市销率-25E",
        "市销率-26E",
        "市销率-27E",
        "市净率-24A",
        "市净率-MRQ",
        "市现率1-24A",
        "市现率1-TTM",
        "市现率2-24A",
        "市现率2-TTM",
        "EV/EBITDA-24A",
    ];
    finalize_report(&ordered, &RENAME, &SELECT, &NUMERIC, None)
}

/// 东方财富-港股-行业对比-估值对比（对应 akshare [`akshare.stock_hk_valuation_comparison_em`]）。
///
/// 报表 `RPT_PCF10_INDUSTRY_HKCVALUE`（`source=F10`），按 `SECUCODE`+`CORRE_SECUCODE`（`{symbol}.HK`）
/// 过滤；输出 18 列 `代码, 简称, 市盈率-TTM/-LYR(及排名), 市净率-MRQ/-LYR(及排名),
/// 市销率-TTM/-LYR(及排名), 市现率-TTM/-LYR(及排名)`（无 `序号`、无行变换）。各指标与排名数值化。
pub fn stock_hk_valuation_comparison_em(symbol: &str) -> Result<Df> {
    let code = format!("{symbol}.HK");
    let filter = format!(r#"(SECUCODE="{code}")(CORRE_SECUCODE="{code}")"#);
    let extra = report_extra("", "", Some(&filter), Some(""), None, None);
    let rows = fetch_securities_pages(
        &HttpClient::default(),
        "RPT_PCF10_INDUSTRY_HKCVALUE",
        "SECUCODE,SECURITY_CODE,ORG_CODE,REPORT_DATE,TYPE_ID,TYPE_TYPE,TYPE_NAME,\
TYPE_NAME_EN,CORRE_SECURITY_CODE,CORRE_SECUCODE,CORRE_SECURITY_NAME,PE_TTM,PE_LYR,PB_MQR,\
PB_LYR,PS_TTM,PS_LYR,PCE_TTM,PCE_LYR,PE_TTM_RANK,PE_LYR_RANK,PB_MQR_RANK,PB_LYR_RANK,\
PS_TTM_RANK,PS_LYR_RANK,PCE_TTM_RANK,PCE_LYR_RANK",
        &extra,
        "",
        "F10",
        "PC",
    )?;
    const RENAME: [(&str, &str); 18] = [
        ("CORRE_SECURITY_CODE", "代码"),
        ("CORRE_SECURITY_NAME", "简称"),
        ("PE_TTM", "市盈率-TTM"),
        ("PE_TTM_RANK", "市盈率-TTM排名"),
        ("PE_LYR", "市盈率-LYR"),
        ("PE_LYR_RANK", "市盈率-LYR排名"),
        ("PB_MQR", "市净率-MRQ"),
        ("PB_MQR_RANK", "市净率-MRQ排名"),
        ("PB_LYR", "市净率-LYR"),
        ("PB_LYR_RANK", "市净率-LYR排名"),
        ("PS_TTM", "市销率-TTM"),
        ("PS_TTM_RANK", "市销率-TTM排名"),
        ("PS_LYR", "市销率-LYR"),
        ("PS_LYR_RANK", "市销率-LYR排名"),
        ("PCE_TTM", "市现率-TTM"),
        ("PCE_TTM_RANK", "市现率-TTM排名"),
        ("PCE_LYR", "市现率-LYR"),
        ("PCE_LYR_RANK", "市现率-LYR排名"),
    ];
    const SELECT: [&str; 18] = [
        "代码",
        "简称",
        "市盈率-TTM",
        "市盈率-TTM排名",
        "市盈率-LYR",
        "市盈率-LYR排名",
        "市净率-MRQ",
        "市净率-MRQ排名",
        "市净率-LYR",
        "市净率-LYR排名",
        "市销率-TTM",
        "市销率-TTM排名",
        "市销率-LYR",
        "市销率-LYR排名",
        "市现率-TTM",
        "市现率-TTM排名",
        "市现率-LYR",
        "市现率-LYR排名",
    ];
    const NUMERIC: [&str; 16] = [
        "市盈率-TTM",
        "市盈率-TTM排名",
        "市盈率-LYR",
        "市盈率-LYR排名",
        "市净率-MRQ",
        "市净率-MRQ排名",
        "市净率-LYR",
        "市净率-LYR排名",
        "市销率-TTM",
        "市销率-TTM排名",
        "市销率-LYR",
        "市销率-LYR排名",
        "市现率-TTM",
        "市现率-TTM排名",
        "市现率-LYR",
        "市现率-LYR排名",
    ];
    finalize_report(&rows, &RENAME, &SELECT, &NUMERIC, None)
}

// ============ 15. 东财数据中心：股市日历 / 高管持股 / 股票回购（datacenter-web RPT_*） ============

/// `RPT_ORGOP_ALL` 列清单（对应 akshare `stock_gsrl_gsdt_em` 的 `columns` 参数）。
const GSRL_COLUMNS: &str =
    "SECURITY_CODE,SECUCODE,SECURITY_NAME_ABBR,EVENT_TYPE,EVENT_CONTENT,TRADE_DATE";

/// 东财 JSON 键 → 中文列名（akshare `rename` 字典；`SECUCODE` 被重命名为 `-` 且未选中，故省略）。
const GSRL_RENAME: [(&str, &str); 5] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "简称"),
    ("EVENT_TYPE", "事件类型"),
    ("EVENT_CONTENT", "具体事项"),
    ("TRADE_DATE", "交易日"),
];

/// 选中列（`序号` 由 `index_name` 前置）。
const GSRL_SELECT: [&str; 5] = ["代码", "简称", "事件类型", "具体事项", "交易日"];

/// 日期列（akshare `pd.to_datetime` 截断为 `YYYY-MM-DD`）。
const GSRL_DATE: [&str; 1] = ["交易日"];

/// 东方财富-数据中心-股市日历-公司动态（对应 akshare [`akshare.stock_gsrl_gsdt_em`]）。
///
/// `date`：`YYYYMMDD` 格式交易日。报表 `RPT_ORGOP_ALL`，按 `TRADE_DATE` 过滤。
///
/// # 返回列
/// `序号, 代码, 简称, 事件类型, 具体事项, 交易日`
pub fn stock_gsrl_gsdt_em(date: &str) -> Result<Df> {
    let ymd = if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..])
    } else {
        return Err(AkshareError::Param(format!(
            "无效 date: {date}（应为 YYYYMMDD）"
        )));
    };
    let filter = format!("(TRADE_DATE='{ymd}')");
    let extra = report_extra("SECURITY_CODE", "1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_ORGOP_ALL", GSRL_COLUMNS, &extra, "5000")?;
    let mut df = finalize_report(&rows, &GSRL_RENAME, &GSRL_SELECT, &[], Some("序号"))?;
    df.cast_date(&GSRL_DATE)?;
    Ok(df)
}

/// `RPT_EXECUTIVE_HOLD_DETAILS` 列清单（`columns=ALL`；`DERIVE_SECURITY_CODE`/`ORG_CODE`/`GGEID`
/// 服务端返回但被 akshare 重命名为 `-` 且未选中，故省略）。
const HOLD_MGMT_RENAME: [(&str, &str); 16] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME", "名称"),
    ("CHANGE_DATE", "日期"),
    ("PERSON_NAME", "变动人"),
    ("CHANGE_SHARES", "变动股数"),
    ("AVERAGE_PRICE", "成交均价"),
    ("CHANGE_AMOUNT", "变动金额"),
    ("CHANGE_REASON", "变动原因"),
    ("CHANGE_RATIO", "变动比例"),
    ("CHANGE_AFTER_HOLDNUM", "变动后持股数"),
    ("HOLD_TYPE", "持股种类"),
    ("DSE_PERSON_NAME", "董监高人员姓名"),
    ("POSITION_NAME", "职务"),
    ("PERSON_DSE_RELATION", "变动人与董监高的关系"),
    ("BEGIN_HOLD_NUM", "开始时持有"),
    ("END_HOLD_NUM", "结束后持有"),
];
const HOLD_MGMT_SELECT: [&str; 16] = [
    "日期",
    "代码",
    "名称",
    "变动人",
    "变动股数",
    "成交均价",
    "变动金额",
    "变动原因",
    "变动比例",
    "变动后持股数",
    "持股种类",
    "董监高人员姓名",
    "职务",
    "变动人与董监高的关系",
    "开始时持有",
    "结束后持有",
];
const HOLD_MGMT_NUMERIC: [&str; 7] = [
    "变动股数",
    "成交均价",
    "变动金额",
    "变动比例",
    "变动后持股数",
    "开始时持有",
    "结束后持有",
];
const HOLD_MGMT_DATE: [&str; 1] = ["日期"];

/// 东方财富-数据中心-特色数据-高管持股-董监高及相关人员持股变动明细
/// （对应 akshare [`akshare.stock_hold_management_detail_em`]）。
///
/// 报表 `RPT_EXECUTIVE_HOLD_DETAILS`（`columns=ALL`），按 `CHANGE_DATE,SECURITY_CODE,PERSON_NAME`
/// 降序全量分页。akshare 未生成 `序号` 列，故 `index_name=None`。
///
/// # 返回列
/// `日期, 代码, 名称, 变动人, 变动股数, 成交均价, 变动金额, 变动原因, 变动比例, 变动后持股数,
/// 持股种类, 董监高人员姓名, 职务, 变动人与董监高的关系, 开始时持有, 结束后持有`
pub fn stock_hold_management_detail_em() -> Result<Df> {
    let extra = report_extra(
        "CHANGE_DATE,SECURITY_CODE,PERSON_NAME",
        "-1,1,1",
        Some(""),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_EXECUTIVE_HOLD_DETAILS", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &HOLD_MGMT_RENAME,
        &HOLD_MGMT_SELECT,
        &HOLD_MGMT_NUMERIC,
        None,
    )?;
    df.cast_date(&HOLD_MGMT_DATE)?;
    Ok(df)
}

/// 东方财富-数据中心-特色数据-高管持股-人员增减持股变动明细
/// （对应 akshare [`akshare.stock_hold_management_person_em`]）。
///
/// `symbol`：股票代码；`name`：高管名称。报表 `RPT_EXECUTIVE_HOLD_DETAILS`，按
/// `(SECURITY_CODE={symbol})(PERSON_NAME={name})` 过滤。
pub fn stock_hold_management_person_em(symbol: &str, name: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")(PERSON_NAME="{name}")"#);
    let extra = report_extra(
        "CHANGE_DATE,SECURITY_CODE,PERSON_NAME",
        "-1,1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_EXECUTIVE_HOLD_DETAILS", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &HOLD_MGMT_RENAME,
        &HOLD_MGMT_SELECT,
        &HOLD_MGMT_NUMERIC,
        None,
    )?;
    df.cast_date(&HOLD_MGMT_DATE)?;
    Ok(df)
}

/// 股票回购「实施进度」代码 → 中文标签（对应 akshare `process_map`，akshare 1.18.83）。
///
/// 服务端 `REPURPROGRESS` 可能为字符串（`"001"`）或整数（`1`），统一规整为零填充 3 位后查表。
fn repurchase_progress_label(code: &str) -> Option<&'static str> {
    match code {
        "001" => Some("董事会预案"),
        "002" => Some("股东大会通过"),
        "003" => Some("股东大会否决"),
        "004" => Some("实施中"),
        "005" => Some("停止实施"),
        "006" => Some("完成实施"),
        _ => None,
    }
}

/// `RPTA_WEB_GETHGLIST_NEW` 列清单（`columns=ALL`）。
const REPURCHASE_RENAME: [(&str, &str); 17] = [
    ("DIM_SCODE", "股票代码"),
    ("SECURITYSHORTNAME", "股票简称"),
    ("NEWPRICE", "最新价"),
    ("REPURPRICECAP", "计划回购价格区间"),
    ("REPURNUMLOWER", "计划回购数量区间-下限"),
    ("REPURNUMCAP", "计划回购数量区间-上限"),
    ("ZSZXX", "占公告前一日总股本比例-下限"),
    ("ZSZSX", "占公告前一日总股本比例-上限"),
    ("JEXX", "计划回购金额区间-下限"),
    ("JESX", "计划回购金额区间-上限"),
    ("DIM_TRADEDATE", "回购起始时间"),
    ("REPURPROGRESS", "实施进度"),
    ("REPURPRICELOWER1", "已回购股份价格区间-下限"),
    ("REPURPRICECAP1", "已回购股份价格区间-上限"),
    ("REPURNUM", "已回购股份数量"),
    ("REPURAMOUNT", "已回购金额"),
    ("UPDATEDATE", "最新公告日期"),
];
const REPURCHASE_SELECT: [&str; 17] = [
    "股票代码",
    "股票简称",
    "最新价",
    "计划回购价格区间",
    "计划回购数量区间-下限",
    "计划回购数量区间-上限",
    "占公告前一日总股本比例-下限",
    "占公告前一日总股本比例-上限",
    "计划回购金额区间-下限",
    "计划回购金额区间-上限",
    "回购起始时间",
    "实施进度",
    "已回购股份价格区间-下限",
    "已回购股份价格区间-上限",
    "已回购股份数量",
    "已回购金额",
    "最新公告日期",
];
const REPURCHASE_NUMERIC: [&str; 12] = [
    "最新价",
    "计划回购价格区间",
    "计划回购数量区间-下限",
    "计划回购数量区间-上限",
    "占公告前一日总股本比例-下限",
    "占公告前一日总股本比例-上限",
    "计划回购金额区间-下限",
    "计划回购金额区间-上限",
    "已回购股份价格区间-下限",
    "已回购股份价格区间-上限",
    "已回购股份数量",
    "已回购金额",
];
const REPURCHASE_DATE: [&str; 2] = ["回购起始时间", "最新公告日期"];

/// 东方财富-数据中心-股票回购-股票回购数据（对应 akshare [`akshare.stock_repurchase_em`]）。
///
/// 报表 `RPTA_WEB_GETHGLIST_NEW`（`columns=ALL`），按 `UPD,DIM_DATE,DIM_SCODE` 降序全量分页。
/// `实施进度` 由服务端代码经 [`repurchase_progress_label`] 映射为中文标签（对应 akshare
/// `process_map`）。`序号` 由 Rust 生成，`回购起始时间`/`最新公告日期` 截断为 `YYYY-MM-DD`，
/// 其余数值列数值化。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 最新价, 计划回购价格区间, 计划回购数量区间-下限, 计划回购数量区间-上限,
/// 占公告前一日总股本比例-下限, 占公告前一日总股本比例-上限, 计划回购金额区间-下限,
/// 计划回购金额区间-上限, 回购起始时间, 实施进度, 已回购股份价格区间-下限, 已回购股份价格区间-上限,
/// 已回购股份数量, 已回购金额, 最新公告日期`
pub fn stock_repurchase_em() -> Result<Df> {
    let extra = report_extra("UPD,DIM_DATE,DIM_SCODE", "-1,-1,-1", None, None, None, None);
    let mut rows = datacenter("RPTA_WEB_GETHGLIST_NEW", "ALL", &extra, "5000")?;
    for row in &mut rows {
        if let Some(v) = row.get_mut("REPURPROGRESS") {
            let code: Option<String> = match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(format!("{:03}", n.as_i64().unwrap_or(0))),
                _ => None,
            };
            if let Some(code) = code {
                if let Some(label) = repurchase_progress_label(&code) {
                    *v = Value::String(label.to_string());
                }
            }
        }
    }
    let mut df = finalize_report(
        &rows,
        &REPURCHASE_RENAME,
        &REPURCHASE_SELECT,
        &REPURCHASE_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&REPURCHASE_DATE)?;
    Ok(df)
}

// ============ 16. 东财数据中心：基金持仓明细（RPT_MAINDATA_MAIN_POSITIONDETAILS，datacenter-web） ============

/// `RPT_MAINDATA_MAIN_POSITIONDETAILS` 列清单（`columns=ALL`）。akshare 用「位置式列映射」
/// （`reset_index` 后按列序号赋中文名，序号列由 index 前置），等价于下方按 JSON 键的 rename：
/// 位置 0=序号(index)、2=SECURITY_CODE(股票代码)、4=SECURITY_NAME_ABBR(股票简称)、
/// 13=TOTAL_SHARES(持股数)、14=HOLD_MARKET_CAP(持股市值)、15=TOTAL_SHARES_RATIO(占总股本比例)、
/// 16=FREE_SHARES_RATIO(占流通股本比例)，其余键未选中（akshare 置为 `_`/`-`）。
const FUND_HOLD_DETAIL_RENAME: [(&str, &str); 6] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("TOTAL_SHARES", "持股数"),
    ("HOLD_MARKET_CAP", "持股市值"),
    ("TOTAL_SHARES_RATIO", "占总股本比例"),
    ("FREE_SHARES_RATIO", "占流通股本比例"),
];
const FUND_HOLD_DETAIL_SELECT: [&str; 6] = [
    "股票代码",
    "股票简称",
    "持股数",
    "持股市值",
    "占总股本比例",
    "占流通股本比例",
];
const FUND_HOLD_DETAIL_NUMERIC: [&str; 4] =
    ["持股数", "持股市值", "占总股本比例", "占流通股本比例"];

/// 东方财富-数据中心-主力数据-基金持仓-明细（对应 akshare [`akshare.stock_report_fund_hold_detail`]）。
///
/// `symbol`：基金代码；`date`：`YYYYMMDD` 财报发布日期。报表 `RPT_MAINDATA_MAIN_POSITIONDETAILS`
/// （`columns=ALL`），按 `(HOLDER_CODE={symbol})(REPORT_DATE='YYYY-MM-DD')` 过滤。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 持股数, 持股市值, 占总股本比例, 占流通股本比例`
pub fn stock_report_fund_hold_detail(symbol: &str, date: &str) -> Result<Df> {
    let ymd = if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..])
    } else {
        return Err(AkshareError::Param(format!(
            "无效 date: {date}（应为 YYYYMMDD）"
        )));
    };
    let filter = format!(r#"(HOLDER_CODE="{symbol}")(REPORT_DATE='{ymd}')"#);
    let extra = report_extra("SECURITY_CODE", "-1", Some(&filter), Some(""), None, None);
    let rows = datacenter("RPT_MAINDATA_MAIN_POSITIONDETAILS", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &FUND_HOLD_DETAIL_RENAME,
        &FUND_HOLD_DETAIL_SELECT,
        &FUND_HOLD_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ============ 17. 东财数据中心：基金持仓（dataapi host，位置式列映射 → 键 rename） ============

/// `stock_report_fund_hold` 的 `symbol` → 服务端 `type` 编码（对应 akshare `symbol_map`）。
fn fund_hold_type_code(symbol: &str) -> Result<&'static str> {
    match symbol {
        "基金持仓" => Ok("1"),
        "QFII持仓" => Ok("2"),
        "社保持仓" => Ok("3"),
        "券商持仓" => Ok("4"),
        "保险持仓" => Ok("5"),
        "信托持仓" => Ok("6"),
        _ => Err(AkshareError::Param(format!(
            "无效 symbol: {symbol}（应为 基金持仓/QFII持仓/社保持仓/券商持仓/保险持仓/信托持仓）"
        ))),
    }
}

/// 抓取 `data.eastmoney.com/dataapi/zlsj/list`（非标准 host，响应体为 `{data, pages}` 而非
/// `result.data`），按 `pages` 全量分页。
fn fetch_fund_hold_rows(date: &str, type_code: &str) -> Result<Vec<Value>> {
    let http = HttpClient::default();
    let mut all: Vec<Value> = Vec::new();
    let mut page: i64 = 1;
    loop {
        let mut params = Map::new();
        params.insert("date".into(), json!(date));
        params.insert("type".into(), json!(type_code));
        params.insert("zjc".into(), json!("0"));
        params.insert("sortField".into(), json!("HOULD_NUM"));
        params.insert("sortDirec".into(), json!("1"));
        params.insert("pageNum".into(), json!(page));
        params.insert("pageSize".into(), json!("500"));
        params.insert("p".into(), json!(page));
        params.insert("pageNo".into(), json!(page));
        let v = http.get_json(
            "https://data.eastmoney.com/dataapi/zlsj/list",
            &params,
            None,
        )?;
        let data = v
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if data.is_empty() {
            break;
        }
        let pages = v.get("pages").and_then(Value::as_i64).unwrap_or(1);
        all.extend(data);
        if page >= pages {
            break;
        }
        page += 1;
    }
    Ok(all)
}

/// `dataapi/zlsj/list` 列清单（位置式列映射等价按 JSON 键 rename；`序号` 由 index_name 前置）。
const FUND_HOLD_RENAME: [(&str, &str); 8] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("HOULD_NUM", "持有基金家数"),
    ("TOTAL_SHARES", "持股总数"),
    ("HOLD_VALUE", "持股市值"),
    ("HOLDCHA", "持股变化"),
    ("HOLDCHA_NUM", "持股变动数值"),
    ("HOLDCHA_RATIO", "持股变动比例"),
];
const FUND_HOLD_SELECT: [&str; 8] = [
    "股票代码",
    "股票简称",
    "持有基金家数",
    "持股总数",
    "持股市值",
    "持股变化",
    "持股变动数值",
    "持股变动比例",
];
const FUND_HOLD_NUMERIC: [&str; 5] = [
    "持有基金家数",
    "持股总数",
    "持股市值",
    "持股变动数值",
    "持股变动比例",
];

/// 东方财富-数据中心-主力数据-基金持仓（对应 akshare [`akshare.stock_report_fund_hold`]）。
///
/// `symbol`：`{基金持仓, QFII持仓, 社保持仓, 券商持仓, 保险持仓, 信托持仓}`；`date`：`YYYYMMDD`
/// 财报发布日期。走非标准 host `data.eastmoney.com/dataapi/zlsj/list`，按 `type` + `date` 过滤。
/// akshare 用「位置式列映射」，等价按 JSON 键 rename（序号由 index_name 前置）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 持有基金家数, 持股总数, 持股市值, 持股变化, 持股变动数值, 持股变动比例`
pub fn stock_report_fund_hold(symbol: &str, date: &str) -> Result<Df> {
    let type_code = fund_hold_type_code(symbol)?;
    let ymd = if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..])
    } else {
        return Err(AkshareError::Param(format!(
            "无效 date: {date}（应为 YYYYMMDD）"
        )));
    };
    let rows = fetch_fund_hold_rows(&ymd, type_code)?;
    let df = finalize_report(
        &rows,
        &FUND_HOLD_RENAME,
        &FUND_HOLD_SELECT,
        &FUND_HOLD_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

/// 科创板报告（对应 akshare [`akshare.stock_zh_kcb_report_em`]）。
///
/// `from_page`/`to_page`：起止页码（默认 `1`/`100`）。走
/// `np-anotice-stock.eastmoney.com/api/security/ann`（`ann_type=KCB`）。每行取
/// `codes[0]` 的 代码/名称 与 `columns[0]` 的公告类型，取 `art_code` 为 `公告代码`，
/// `公告日期` 归一 `YYYY-MM-DD`。
///
/// # 返回列
/// `代码, 名称, 公告标题, 公告类型, 公告日期, 公告代码`
pub fn stock_zh_kcb_report_em(from_page: &str, to_page: &str) -> Result<Df> {
    let from: i64 = from_page
        .parse()
        .map_err(|_| AkshareError::Param(format!("无效 from_page: {from_page}")))?;
    let mut to: i64 = to_page
        .parse()
        .map_err(|_| AkshareError::Param(format!("无效 to_page: {to_page}")))?;
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("sr".to_string(), json!("-1"));
    params.insert("page_size".to_string(), json!("100"));
    params.insert("ann_type".to_string(), json!("KCB"));
    params.insert("client_source".to_string(), json!("web"));
    params.insert("f_node".to_string(), json!("0"));
    params.insert("s_node".to_string(), json!("0"));

    params.insert("page_index".to_string(), json!(1));
    let first = http.get_json(NOTICE_KCB_URL, &params, None)?;
    let data = match first.get("data") {
        Some(d) => d,
        None => return build_kcb_df(&[]),
    };
    let total_hits = data.get("total_hits").and_then(Value::as_u64).unwrap_or(0);
    let page_size = data
        .get("page_size")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .max(1);
    let total_page = (total_hits / page_size) as i64;
    if to > total_page {
        to = total_page;
    }
    if to < from {
        to = from;
    }

    let mut items: Vec<Value> = Vec::new();
    if let Some(list) = data.get("list").and_then(Value::as_array) {
        items.extend(list.iter().cloned());
    }
    for page in (from.max(2))..=to {
        params.insert("page_index".to_string(), json!(page));
        match http.get_json(NOTICE_KCB_URL, &params, None) {
            Ok(v) => {
                if let Some(list) = v
                    .get("data")
                    .and_then(|d| d.get("list"))
                    .and_then(Value::as_array)
                {
                    items.extend(list.iter().cloned());
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    build_kcb_df(&items)
}

/// 科创板报告端点。
const NOTICE_KCB_URL: &str = "https://np-anotice-stock.eastmoney.com/api/security/ann";

/// 由已抓取的科创板公告列表数组构建 DataFrame（与网络解耦，便于离线测试）。
fn build_kcb_df(items: &[Value]) -> Result<Df> {
    let col_names: &[&str] = &[
        "代码",
        "名称",
        "公告标题",
        "公告类型",
        "公告日期",
        "公告代码",
    ];
    let mut data: Vec<Vec<Option<String>>> = Vec::with_capacity(items.len());
    for item in items {
        let code = item
            .get("codes")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("stock_code"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = item
            .get("codes")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("short_name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let notice_date = item
            .get("notice_date")
            .and_then(Value::as_str)
            .map(|s| {
                if s.len() >= 10 && (s.as_bytes()[4] == b'-' || s.as_bytes()[4] == b'/') {
                    s[0..10].to_string()
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_default();
        let column_name = item
            .get("columns")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("column_name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let art_code = item
            .get("art_code")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        data.push(vec![
            Some(code),
            Some(name),
            Some(title),
            Some(column_name),
            Some(notice_date),
            Some(art_code),
        ]);
    }
    let mut df = Df::from_string_rows(col_names, &data)?;
    df.cast_date(&["公告日期"])?;
    Ok(df)
}

#[cfg(test)]
mod kcb_report_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kcb_build_offline() {
        let raw = json!([{
            "art_code": "AN202608141827988685",
            "title": "长盈通:股票交易异常波动公告",
            "notice_date": "2026-08-15 00:00:00",
            "codes": [{"ann_type": "A,KCB,SHA", "stock_code": "688143", "short_name": "长盈通"}],
            "columns": [{"column_code": "001002004007", "column_name": "股票交易异常波动"}]
        }]);
        let df = build_kcb_df(raw.as_array().unwrap()).unwrap();
        assert_eq!(
            df.column_names(),
            vec![
                "代码",
                "名称",
                "公告标题",
                "公告类型",
                "公告日期",
                "公告代码"
            ]
        );
        assert_eq!(
            df.inner().column("代码").unwrap().str().unwrap().get(0),
            Some("688143")
        );
        assert_eq!(
            df.inner().column("公告代码").unwrap().str().unwrap().get(0),
            Some("AN202608141827988685")
        );
        assert_eq!(
            df.inner().column("公告日期").unwrap().str().unwrap().get(0),
            Some("2026-08-15")
        );
    }
}

// ============ 10. emweb F10 三大报表（按报告期/年度，原始字段键） ============
// 对应 akshare `stock_balance_sheet_by_report_em` / `by_yearly_em` 等。这一类走
// emweb F10 `NewFinanceAnalysis` 的 `zcfzb/lrb/xjllb` + `DateAjaxNew` 端点，akshare
// 对返回数据**不做中文 rename**（直接 `pd.DataFrame(data_json["data"])`），故本实现
// 列名保持 emweb 原始字段键（如 `REPORT_DATE`/`TOTAL_ASSETS`），与 akshare 列契约一致；
// 行 = 各报告期，宽表。

/// 把 emweb F10 三大报表的多期行（宽表、原始字段键）转成 [`Df`]。
///
/// 列名保持 emweb 原始键（与 akshare 一致）；首行键序决定列序；空表返回零行零列。
fn emweb_financial_report_df(rows: &[Value]) -> Result<Df> {
    let mut df = Df::from_json_rows_typed(rows)?;
    // 模拟 akshare：全空列 `pd.to_numeric(errors="coerce")` → Float64。
    // `from_json_rows_typed` 对全空列推断为 String，故需补齐这一步以保持 dtype 一致。
    let h = df.height();
    if h > 0 {
        let targets: Vec<String> = df
            .column_names()
            .into_iter()
            .filter(|name| {
                df.inner()
                    .column(name)
                    .map(|s| s.null_count() == h)
                    .unwrap_or(false)
            })
            .collect();
        let refs: Vec<&str> = targets.iter().map(String::as_str).collect();
        df.cast_numeric(&refs)?;
    }
    Ok(df)
}

/// 抓 emweb F10 个股页 `#hidctype` 隐藏域，得到 `companyType`。
fn emweb_f10_company_type(http: &HttpClient, symbol: &str) -> Result<String> {
    let url = "https://emweb.securities.eastmoney.com/PC_HSF10/NewFinanceAnalysis/Index";
    let mut params = Map::new();
    params.insert("type".into(), json!("web"));
    params.insert("code".into(), json!(symbol.to_lowercase()));
    let html = http.get_text(url, &params, None)?;
    let doc = Html::parse_document(&html);
    let sel = Selector::parse(r#"input[id="hidctype"]"#)
        .map_err(|e| AkshareError::Empty(format!("hidctype 选择器无效: {e}")))?;
    for el in doc.select(&sel) {
        if let Some(v) = el.value().attr("value") {
            return Ok(v.to_string());
        }
    }
    Err(AkshareError::Empty(
        "emweb 未返回 hidctype（公司类型）".into(),
    ))
}

/// emweb F10 三大报表（资产负债表/利润表/现金流量表）按报告期/年度/单季度的公共拉取流程。
///
/// 先取 `companyType`，再拉报告期列表（`{date_endpoint}`，`date_report_date_type`），
/// 每 5 个报告期一批调用 `{ajax_endpoint}`（`ajax_report_date_type` + `report_type`）取明细，
/// 拼接成多期宽表（原始字段键，与 akshare 一致）。
///
/// 注：按报告期/年度时报告期列表与明细共用同一 `reportDateType`；按单季度时列表用
/// `reportDateType=2` 而明细用 `reportDateType=0` + `reportType=2`（akshare 源码即如此）。
fn emweb_f10_financial_ex(
    symbol: &str,
    date_report_date_type: &str,
    date_endpoint: &str,
    ajax_endpoint: &str,
    ajax_report_date_type: &str,
    report_type: &str,
) -> Result<Df> {
    let http = HttpClient::default();
    let ctype = emweb_f10_company_type(&http, symbol)?;
    let code = symbol.to_uppercase();
    // 1) 报告期列表
    let durl = format!(
        "https://emweb.securities.eastmoney.com/PC_HSF10/NewFinanceAnalysis/{date_endpoint}"
    );
    let mut dparams = Map::new();
    dparams.insert("companyType".into(), json!(ctype.clone()));
    dparams.insert("reportDateType".into(), json!(date_report_date_type));
    dparams.insert("code".into(), json!(code.clone()));
    let dval = http.get_json(&durl, &dparams, None)?;
    let dates: Vec<String> = dval
        .get("data")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|o| {
                    o.get("REPORT_DATE")
                        .and_then(Value::as_str)
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    // 2) 每 5 个报告期一批拉明细
    let mut rows: Vec<Value> = Vec::new();
    for chunk in dates.chunks(5) {
        let aurl = format!(
            "https://emweb.securities.eastmoney.com/PC_HSF10/NewFinanceAnalysis/{ajax_endpoint}"
        );
        let mut aparams = Map::new();
        aparams.insert("companyType".into(), json!(ctype.clone()));
        aparams.insert("reportDateType".into(), json!(ajax_report_date_type));
        aparams.insert("reportType".into(), json!(report_type));
        aparams.insert("dates".into(), json!(chunk.join(",")));
        aparams.insert("code".into(), json!(code.clone()));
        let aval = http.get_json(&aurl, &aparams, None)?;
        match aval.get("data").and_then(Value::as_array) {
            Some(arr) if !arr.is_empty() => rows.extend(arr.iter().cloned()),
            _ => break,
        }
    }
    emweb_financial_report_df(&rows)
}

/// 三大报表按报告期/年度（对应 akshare 各 `by_report_em` / `by_yearly_em`）。
///
/// 报告期列表与明细共用同一 `reportDateType`（`0`=报告期、`1`=年度），`reportType=1`。
fn emweb_f10_financial(
    symbol: &str,
    report_date_type: &str,
    date_endpoint: &str,
    ajax_endpoint: &str,
) -> Result<Df> {
    emweb_f10_financial_ex(
        symbol,
        report_date_type,
        date_endpoint,
        ajax_endpoint,
        report_date_type,
        "1",
    )
}

/// 个股资产负债表-按报告期（对应 akshare [`akshare.stock_balance_sheet_by_report_em`]）。
///
/// 走 emweb F10 `NewFinanceAnalysis/zcfzbDateAjaxNew` + `zcfzbAjaxNew`，列名保持 emweb
/// 原始字段键（如 `REPORT_DATE`/`TOTAL_ASSETS` 等），行 = 各报告期，与 akshare 一致。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600519"`，内部转大写）
pub fn stock_balance_sheet_by_report_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial(symbol, "0", "zcfzbDateAjaxNew", "zcfzbAjaxNew")
}

/// 个股资产负债表-按年度（对应 akshare [`akshare.stock_balance_sheet_by_yearly_em`]）。
///
/// 与 [`stock_balance_sheet_by_report_em`] 仅 `reportDateType` 不同（`1`=年度）。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600036"`，内部转大写）
pub fn stock_balance_sheet_by_yearly_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial(symbol, "1", "zcfzbDateAjaxNew", "zcfzbAjaxNew")
}

/// 个股利润表-按报告期（对应 akshare [`akshare.stock_profit_sheet_by_report_em`]）。
///
/// 走 emweb F10 `NewFinanceAnalysis` 流程（`lrbDateAjaxNew`/`lrbAjaxNew`，
/// `reportDateType=0`），与资产负债表函数仅端点前缀不同；akshare **不重命名列**，
/// 返回原始字段键（如 `REPORT_DATE`/`OPERATE_INCOME` 等），行 = 各报告期，与 akshare 一致。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600519"`，内部转大写）
pub fn stock_profit_sheet_by_report_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial(symbol, "0", "lrbDateAjaxNew", "lrbAjaxNew")
}

/// 个股利润表-按年度（对应 akshare [`akshare.stock_profit_sheet_by_yearly_em`]）。
///
/// 与 [`stock_profit_sheet_by_report_em`] 仅 `reportDateType` 不同（`1`=年度）。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600519"`，内部转大写）
pub fn stock_profit_sheet_by_yearly_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial(symbol, "1", "lrbDateAjaxNew", "lrbAjaxNew")
}

/// 个股现金流量表-按报告期（对应 akshare [`akshare.stock_cash_flow_sheet_by_report_em`]）。
///
/// 走 emweb F10 `NewFinanceAnalysis` 流程（`xjllbDateAjaxNew`/`xjllbAjaxNew`，
/// `reportDateType=0`）；akshare **不重命名列**，返回原始字段键（如 `REPORT_DATE`/`CASH_FLOW_NET_INCREASE` 等）。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600519"`，内部转大写）
pub fn stock_cash_flow_sheet_by_report_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial(symbol, "0", "xjllbDateAjaxNew", "xjllbAjaxNew")
}

/// 个股现金流量表-按年度（对应 akshare [`akshare.stock_cash_flow_sheet_by_yearly_em`]）。
///
/// 与 [`stock_cash_flow_sheet_by_report_em`] 仅 `reportDateType` 不同（`1`=年度）。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600036"`，内部转大写）
pub fn stock_cash_flow_sheet_by_yearly_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial(symbol, "1", "xjllbDateAjaxNew", "xjllbAjaxNew")
}

/// 个股利润表-按单季度（对应 akshare [`akshare.stock_profit_sheet_by_quarterly_em`]）。
///
/// 走 emweb F10 `NewFinanceAnalysis` 流程（`lrbDateAjaxNew`/`lrbAjaxNew`）。与
/// [`stock_profit_sheet_by_report_em`] 的差异（沿用 akshare 源码）：报告期列表用
/// `reportDateType=2`，但明细用 `reportDateType=0` + `reportType=2`。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600519"`，内部转大写）
pub fn stock_profit_sheet_by_quarterly_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial_ex(symbol, "2", "lrbDateAjaxNew", "lrbAjaxNew", "0", "2")
}

/// 个股现金流量表-按单季度（对应 akshare [`akshare.stock_cash_flow_sheet_by_quarterly_em`]）。
///
/// 走 emweb F10 `NewFinanceAnalysis` 流程（`xjllbDateAjaxNew`/`xjllbAjaxNew`）。与
/// [`stock_cash_flow_sheet_by_report_em`] 的差异（沿用 akshare 源码）：报告期列表用
/// `reportDateType=2`，但明细用 `reportDateType=0` + `reportType=2`。
///
/// - `symbol`：带市场标识的股票代码（如 `"SH600519"`，内部转大写）
pub fn stock_cash_flow_sheet_by_quarterly_em(symbol: &str) -> Result<Df> {
    emweb_f10_financial_ex(symbol, "2", "xjllbDateAjaxNew", "xjllbAjaxNew", "0", "2")
}

/// emweb F10 三大报表（已退市个股）按报告期的公共拉取流程（对应 akshare `*_delisted_em`）。
///
/// 与在市个股走 emweb F10 `NewFinanceAnalysis` 不同，已退市个股走
/// `datacenter.eastmoney.com/securities/api/data/get`（`type`/`sty` + `filter`）。先取报告期列表
/// （`RPT_F10_FINANCE_GINCOME`），再按 `REPORT_DATE in (...)` 拉取指定报表（资产负债表/利润表/
/// 现金流量表）。akshare 不重命名列，返回原始字段键；`sr=-1`/`st=REPORT_DATE` 已由接口按报告期
/// 降序返回，与 akshare `sort_values(by=["REPORT_DATE"], ascending=False)` 一致。
fn emweb_f10_delisted_report(symbol: &str, report_type: &str, sty: &str) -> Result<Df> {
    if symbol.len() < 2 {
        return Err(AkshareError::Empty(format!(
            "已退市个股代码格式异常: {symbol}"
        )));
    }
    let secucode = format!("{}.{}", &symbol[2..], &symbol[..2]); // "SZ000013" -> "000013.SZ"
                                                                 // 1) 报告期列表
    let mut list_extra = Map::new();
    list_extra.insert("filter".into(), json!(format!("(SECUCODE=\"{secucode}\")")));
    list_extra.insert("sr".into(), json!("-1"));
    list_extra.insert("st".into(), json!("REPORT_DATE"));
    let list = crate::sources::eastmoney::fetch_securities_data_get(
        &HttpClient::default(),
        "RPT_F10_FINANCE_GINCOME",
        "SECUCODE,SECURITY_CODE,REPORT_DATE,REPORT_TYPE,REPORT_DATE_NAME",
        &list_extra,
        "200",
        "HSF10",
        "PC",
    )?;
    let dates: Vec<String> = list
        .iter()
        .filter_map(|o| {
            o.get("REPORT_DATE")
                .and_then(Value::as_str)
                .map(|s| format!("'{}'", crate::sources::eastmoney::date_only(s)))
        })
        .collect();
    if dates.is_empty() {
        return Err(AkshareError::Empty(
            "已退市个股未取到报告期列表（可能已无财务数据）".into(),
        ));
    }
    // 2) 拉取指定报表
    let mut extra = Map::new();
    extra.insert(
        "filter".into(),
        json!(format!(
            "(SECUCODE=\"{secucode}\")(REPORT_DATE in ({}))",
            dates.join(",")
        )),
    );
    extra.insert("sr".into(), json!("-1"));
    extra.insert("st".into(), json!("REPORT_DATE"));
    let rows = crate::sources::eastmoney::fetch_securities_data_get(
        &HttpClient::default(),
        report_type,
        sty,
        &extra,
        "200",
        "HSF10",
        "PC",
    )?;
    Df::from_json_rows_typed(&rows)
}

/// 已退市个股资产负债表-按报告期（对应 akshare [`akshare.stock_balance_sheet_by_report_delisted_em`]）。
///
/// 走 `datacenter.eastmoney.com/securities/api/data/get`（`RPT_F10_FINANCE_GBALANCE`），
/// 与在市个股走 emweb F10 `NewFinanceAnalysis` 不同；返回原始字段键，不重命名。
///
/// - `symbol`：带市场标识的**已退市**股票代码（如 `"SZ000013"`，内部转 `000013.SZ`）
pub fn stock_balance_sheet_by_report_delisted_em(symbol: &str) -> Result<Df> {
    emweb_f10_delisted_report(symbol, "RPT_F10_FINANCE_GBALANCE", "F10_FINANCE_GBALANCE")
}

/// 已退市个股利润表-按报告期（对应 akshare [`akshare.stock_profit_sheet_by_report_delisted_em`]）。
///
/// 走 `datacenter.eastmoney.com/securities/api/data/get`（`RPT_F10_FINANCE_GINCOME`），返回原始字段键。
///
/// - `symbol`：带市场标识的**已退市**股票代码（如 `"SZ000013"`，内部转 `000013.SZ`）
pub fn stock_profit_sheet_by_report_delisted_em(symbol: &str) -> Result<Df> {
    emweb_f10_delisted_report(symbol, "RPT_F10_FINANCE_GINCOME", "APP_F10_GINCOME")
}

/// 已退市个股现金流量表-按报告期（对应 akshare [`akshare.stock_cash_flow_sheet_by_report_delisted_em`]）。
///
/// 走 `datacenter.eastmoney.com/securities/api/data/get`（`RPT_F10_FINANCE_GCASHFLOW`），返回原始字段键。
///
/// - `symbol`：带市场标识的**已退市**股票代码（如 `"SZ000013"`，内部转 `000013.SZ`）
pub fn stock_cash_flow_sheet_by_report_delisted_em(symbol: &str) -> Result<Df> {
    emweb_f10_delisted_report(symbol, "RPT_F10_FINANCE_GCASHFLOW", "APP_F10_GCASHFLOW")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::eastmoney::finalize_clist;
    use serde_json::json;

    /// 验证估值对比的行变换：旋转末行到首 + 首行排名串 `{末行排名}/{TOTAL_COUNT}` + 交换 1/2 行。
    #[test]
    fn reorder_valuation_rows_offline() {
        let rows = json!([
            {"PAIMING": 1, "TOTAL_COUNT": 8, "CORRE_SECURITY_CODE": "A"},
            {"PAIMING": 2, "TOTAL_COUNT": 8, "CORRE_SECURITY_CODE": "B"},
            {"PAIMING": 3, "TOTAL_COUNT": 8, "CORRE_SECURITY_CODE": "C"},
        ]);
        let ordered = reorder_valuation_rows(rows.as_array().unwrap());
        assert_eq!(ordered.len(), 3);
        // 首行 = 原末行 C，排名改写为 "3/8"
        assert_eq!(ordered[0]["PAIMING"], json!("3/8"));
        assert_eq!(ordered[0]["CORRE_SECURITY_CODE"], json!("C"));
        // 交换后第 1 行为原第 1 行 B、第 2 行为原第 0 行 A
        assert_eq!(ordered[1]["CORRE_SECURITY_CODE"], json!("B"));
        assert_eq!(ordered[2]["CORRE_SECURITY_CODE"], json!("A"));
        assert_eq!(ordered[1]["PAIMING"], json!(2));
        assert_eq!(ordered[2]["PAIMING"], json!(1));
    }

    /// 离线验证 spot 全管线：原始 clist 行 → 排序/序号 → 列重命名/选择 → 数值转换。
    /// 字段集与 akshare spot_em 的 fields 参数一致。
    #[test]
    fn spot_pipeline_offline() {
        let rows = json!([
            {
                "f2": "10.5", "f3": "9.9", "f4": "0.9", "f5": "100000", "f6": "1050000",
                "f7": "3.2", "f8": "0.5", "f9": "8.1", "f10": "1.2", "f11": "0.1",
                "f12": "000001", "f14": "平安银行", "f15": "10.8", "f16": "10.2",
                "f17": "10.3", "f18": "9.6", "f20": "200000000000", "f21": "180000000000",
                "f22": "5.0", "f23": "12.0", "f24": "0.05", "f25": "0.9"
            },
            {
                "f2": "7.8", "f3": "10.0", "f4": "0.7", "f5": "80000", "f6": "624000",
                "f7": "2.1", "f8": "0.3", "f9": "6.5", "f10": "0.9", "f11": "-0.2",
                "f12": "600000", "f14": "浦发银行", "f15": "7.9", "f16": "7.6",
                "f17": "7.7", "f18": "7.1", "f20": "150000000000", "f21": "120000000000",
                "f22": "-3.0", "f23": "8.0", "f24": "-0.1", "f25": "0.7"
            }
        ]);
        let rows: Vec<Value> = rows.as_array().cloned().unwrap();
        let df = finalize_clist(rows).unwrap();
        let df = spot_common_transform(df).unwrap();

        // 列名与顺序逐字对齐 akshare spot_em 的最终 select
        let expected = [
            "序号",
            "代码",
            "名称",
            "最新价",
            "涨跌幅",
            "涨跌额",
            "成交量",
            "成交额",
            "振幅",
            "最高",
            "最低",
            "今开",
            "昨收",
            "量比",
            "换手率",
            "市盈率-动态",
            "市净率",
            "总市值",
            "流通市值",
            "涨速",
            "5分钟涨跌",
            "60日涨跌幅",
            "年初至今涨跌幅",
        ];
        assert_eq!(df.column_names(), expected);

        // 按涨跌幅降序：浦发银行(10.0) 应在 平安银行(9.9) 之前
        let names = df.inner().column("名称").unwrap().str().unwrap();
        assert_eq!(names.get(0), Some("浦发银行"));
        assert_eq!(names.get(1), Some("平安银行"));

        // 序号 int64、涨跌幅 f64
        let idx = df.inner().column("序号").unwrap().i64().unwrap();
        assert_eq!(idx.get(0), Some(1));
        let pct = df.inner().column("涨跌幅").unwrap().f64().unwrap();
        assert_eq!(pct.get(0), Some(10.0));
    }

    #[test]
    fn financial_report_raw_df_offline() {
        // emweb F10 三大报表返回宽表（原始字段键），与 akshare 不做中文 rename 一致；
        // Df::from_json_rows 以首行键序建列。
        let rows = json!([
            {"REPORT_DATE":"2024-03-31","SECUCODE":"600519.SH","TOTAL_ASSETS":"123.0","EQUITY_BALANCE":"50.0"},
            {"REPORT_DATE":"2023-12-31","SECUCODE":"600519.SH","TOTAL_ASSETS":"120.0","EQUITY_BALANCE":"48.0"}
        ]);
        let df = emweb_financial_report_df(rows.as_array().unwrap()).unwrap();
        assert_eq!(df.height(), 2);
        let cols = df.column_names();
        assert_eq!(cols[0], "REPORT_DATE");
        assert!(cols.iter().any(|c| *c == "SECUCODE"));
        assert!(cols.iter().any(|c| *c == "TOTAL_ASSETS"));
        assert!(cols.iter().any(|c| *c == "EQUITY_BALANCE"));
    }

    #[test]
    fn financial_report_typed_df_offline() {
        // 验证 Df::from_json_rows_typed 的列类型推断与 akshare pd.DataFrame(records) 对齐：
        // 数值 JSON → Float64/Int64；全空列 → Float64（akshare pd.to_numeric(errors='coerce')）。
        let rows = json!([
            {"REPORT_DATE":"2024-03-31","OPERATE_INCOME":123.5,"TOTAL_ASSETS":120,"EMPTY_COL":null},
            {"REPORT_DATE":"2023-12-31","OPERATE_INCOME":100.0,"TOTAL_ASSETS":90,"EMPTY_COL":null}
        ]);
        let df = emweb_financial_report_df(rows.as_array().unwrap()).unwrap();
        assert_eq!(df.height(), 2);

        let op_income = df.inner().column("OPERATE_INCOME").unwrap().dtype();
        assert!(op_income.is_float(), "OPERATE_INCOME 应推断为 float64");
        let total_assets = df.inner().column("TOTAL_ASSETS").unwrap().dtype();
        assert!(
            total_assets.is_integer(),
            "TOTAL_ASSETS 应推断为 int64（无空值整数列）"
        );
        let empty = df.inner().column("EMPTY_COL").unwrap().dtype();
        assert!(
            empty.is_float(),
            "全空列应模仿 akshare pd.to_numeric(errors='coerce') 置为 float64"
        );
        let rd = df.inner().column("REPORT_DATE").unwrap().dtype();
        assert!(rd.is_string(), "日期列应保持 str");
    }
}

/// 东方财富-行情中心-沪深个股-风险警示板（对应 akshare [`akshare.stock_zh_a_st_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率`
pub fn stock_zh_a_st_em() -> Result<Df> {
    let urls = crate::sources::eastmoney::push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2", "invt": "2", "fid": "f3",
        "fs": "m:0 f:4,m:1 f:4",
        "fields": "f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let http = HttpClient::default();
    let df = crate::sources::eastmoney::fetch_clist(
        &http,
        &urls,
        params.as_object().expect("静态参数"),
    )?;
    const RENAME: [(&str, &str); 17] = [
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
    const SELECT: [&str; 17] = [
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
    ];
    const NUMERIC: [&str; 14] = [
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
    ];
    crate::sources::eastmoney::finalize_spot(df, &RENAME, &SELECT, &NUMERIC)
}

/// 东方财富-行情中心-沪深个股-新股（对应 akshare [`akshare.stock_zh_a_new_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率`
pub fn stock_zh_a_new_em() -> Result<Df> {
    let urls = crate::sources::eastmoney::push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2", "invt": "2", "fid": "f26",
        "fs": "m:0 f:8,m:1 f:8",
        "fields": "f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let http = HttpClient::default();
    let df = crate::sources::eastmoney::fetch_clist(
        &http,
        &urls,
        params.as_object().expect("静态参数"),
    )?;
    const RENAME: [(&str, &str); 17] = [
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
    const SELECT: [&str; 17] = [
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
    ];
    const NUMERIC: [&str; 14] = [
        "最新价",
        "涨跌幅",
        "涨跌额",
        "成交量",
        "成交额",
        "振幅",
        "最高",
        "最低",
        "今开",
        "昨收",
        "量比",
        "换手率",
        "市盈率-动态",
        "市净率",
    ];
    crate::sources::eastmoney::finalize_spot(df, &RENAME, &SELECT, &NUMERIC)
}

/// 东方财富-港股-实时行情（对应 akshare [`akshare.stock_hk_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额, 换手率, 市盈率-动态, 市净率, 振幅, 量比`
pub fn stock_hk_spot_em() -> Result<Df> {
    let urls = crate::sources::eastmoney::push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2", "invt": "2", "fid": "f12",
        "fs": "m:128 t:3,m:128 t:4,m:128 t:1,m:128 t:2",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let http = HttpClient::default();
    let df = crate::sources::eastmoney::fetch_clist(
        &http,
        &urls,
        params.as_object().expect("静态参数"),
    )?;
    const RENAME: [(&str, &str); 17] = [
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
    const SELECT: [&str; 12] = [
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
        "成交量",
        "成交额",
    ];
    const NUMERIC: [&str; 9] = [
        "最新价",
        "涨跌额",
        "涨跌幅",
        "今开",
        "最高",
        "最低",
        "昨收",
        "成交量",
        "成交额",
    ];
    crate::sources::eastmoney::finalize_spot(df, &RENAME, &SELECT, &NUMERIC)
}

#[cfg(test)]
mod tests_e1 {
    use super::*;

    /// 锁定股票回购「实施进度」代码 → 中文标签映射（对应 akshare `process_map`）。
    #[test]
    fn repurchase_progress_label_offline() {
        assert_eq!(repurchase_progress_label("001"), Some("董事会预案"));
        assert_eq!(repurchase_progress_label("002"), Some("股东大会通过"));
        assert_eq!(repurchase_progress_label("003"), Some("股东大会否决"));
        assert_eq!(repurchase_progress_label("004"), Some("实施中"));
        assert_eq!(repurchase_progress_label("005"), Some("停止实施"));
        assert_eq!(repurchase_progress_label("006"), Some("完成实施"));
        assert_eq!(repurchase_progress_label("999"), None);
    }

    /// 锁定基金持仓 `symbol` → 服务端 `type` 编码映射（对应 akshare `symbol_map`）。
    #[test]
    fn fund_hold_type_code_offline() {
        assert_eq!(fund_hold_type_code("基金持仓").unwrap(), "1");
        assert_eq!(fund_hold_type_code("QFII持仓").unwrap(), "2");
        assert_eq!(fund_hold_type_code("社保持仓").unwrap(), "3");
        assert_eq!(fund_hold_type_code("券商持仓").unwrap(), "4");
        assert_eq!(fund_hold_type_code("保险持仓").unwrap(), "5");
        assert_eq!(fund_hold_type_code("信托持仓").unwrap(), "6");
        assert!(fund_hold_type_code("非法").is_err());
    }

    /// 键名映射（finalize_spot）列契约：与 akshare 输出一致。
    /// 模拟 fetch_clist 输出行（含 index 序号 + f2..f25 键），验证 rename+select。
    #[test]
    fn st_em_offline_contract() {
        let rows = json!([
            {"f2": 4.5, "f3": 9.9, "f4": 0.41, "f5": 100000, "f6": 4500000.0,
             "f7": 11.0, "f8": 3.2, "f9": 30.0, "f10": 1.5, "f12": "000001", "f13": 1,
             "f14": "平安银行", "f15": 4.9, "f16": 4.4, "f17": 4.6, "f18": 4.09,
             "f20": 1e12, "f21": 9e11, "f23": 1.0, "f24": 1.0, "f25": 0.9, "f22": 1.0,
             "f11": 1.0, "f62": 1.0, "f128": 1.0, "f136": 1.0, "f115": 1.0, "f152": 1.0}
        ]);
        // 与真实链路一致：fetch_clist 内部先 finalize_clist（排序 + Int64 序号列）
        let df =
            crate::sources::eastmoney::finalize_clist(rows.as_array().unwrap().to_vec()).unwrap();
        const RENAME: [(&str, &str); 17] = [
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
        const SELECT: [&str; 17] = [
            "序号",
            "代码",
            "名称",
            "最新价",
            "涨跌幅",
            "涨跌额",
            "成交量",
            "成交额",
            "振幅",
            "最高",
            "最低",
            "今开",
            "昨收",
            "量比",
            "换手率",
            "市盈率-动态",
            "市净率",
        ];
        const NUMERIC: [&str; 14] = [
            "最新价",
            "涨跌幅",
            "涨跌额",
            "成交量",
            "成交额",
            "振幅",
            "最高",
            "最低",
            "今开",
            "昨收",
            "量比",
            "换手率",
            "市盈率-动态",
            "市净率",
        ];
        let df = crate::sources::eastmoney::finalize_spot(df, &RENAME, &SELECT, &NUMERIC).unwrap();
        assert_eq!(df.column_names(), SELECT);
        assert_eq!(df.height(), 1);
        let code = df.inner().column("代码").unwrap().str().unwrap();
        assert_eq!(code.get(0), Some("000001"));
        let name = df.inner().column("名称").unwrap().str().unwrap();
        assert_eq!(name.get(0), Some("平安银行"));
        let px = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(4.5));
        let chg = df.inner().column("涨跌幅").unwrap().f64().unwrap();
        assert_eq!(chg.get(0), Some(9.9));
        let seq = df.inner().column("序号").unwrap().i64().unwrap();
        assert_eq!(seq.get(0), Some(1));
        let pb = df.inner().column("市净率").unwrap().f64().unwrap();
        assert_eq!(pb.get(0), Some(0.9));
    }

    /// 空数据：返回空表但列契约完整。
    #[test]
    fn st_em_offline_empty() {
        let rows = json!([]);
        let df = crate::core::df::Df::from_json_rows(rows.as_array().unwrap()).unwrap();
        const SELECT: [&str; 17] = [
            "序号",
            "代码",
            "名称",
            "最新价",
            "涨跌幅",
            "涨跌额",
            "成交量",
            "成交额",
            "振幅",
            "最高",
            "最低",
            "今开",
            "昨收",
            "量比",
            "换手率",
            "市盈率-动态",
            "市净率",
        ];
        let df = crate::sources::eastmoney::finalize_spot(df, &[], &SELECT, &[]).unwrap();
        assert_eq!(df.height(), 0);
        assert_eq!(df.column_names(), SELECT);
    }
}
