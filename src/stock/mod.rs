//! 股票数据接口。
//!
//! 首批实现（对应 akshare `stock_feature/stock_hist_em.py`）：
//! - [`stock_zh_a_hist`]：A 股历史行情（日/周/月，支持复权）
//! - [`stock_zh_a_spot_em`]：A 股实时行情（分页全量）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    a_share_market_code, fetch_clist, fetch_kline, fetch_kline_min, fetch_trends,
    json_value_to_string, kline_to_df, min_kline_to_df, push2_urls, require_kline_data, KLINE_COLS,
    KLINE_COLS_WITH_SYMBOL,
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
