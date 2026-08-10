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
    fetch_kline_ext, fetch_kline_min, fetch_trends, finalize_board_cons, finalize_board_name,
    finalize_fflow, finalize_hsgt, finalize_zt_pool, json_value_to_string, kline_to_df,
    min_kline_to_df, push2_urls, require_kline_data, BOARD_HIST_SELECT, KLINE_COLS,
    KLINE_COLS_WITH_SYMBOL, UT_CLIST, UT_KLINE, ZT_POOL_SELECT,
};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::eastmoney::finalize_clist;
    use serde_json::json;

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
