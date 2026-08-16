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

/// emweb F10 三大报表（资产负债表/利润表/现金流量表）按报告期/年度的公共拉取流程。
///
/// 先取 `companyType`，再拉报告期列表（`{date_endpoint}`），每 5 个报告期一批调用
/// `{ajax_endpoint}` 取明细，拼接成多期宽表（原始字段键）。
fn emweb_f10_financial(
    symbol: &str,
    report_date_type: &str,
    date_endpoint: &str,
    ajax_endpoint: &str,
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
    dparams.insert("reportDateType".into(), json!(report_date_type));
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
        aparams.insert("reportDateType".into(), json!(report_date_type));
        aparams.insert("reportType".into(), json!("1"));
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
