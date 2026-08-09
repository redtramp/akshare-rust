//! 股票数据接口。
//!
//! 首批实现（对应 akshare `stock_feature/stock_hist_em.py`）：
//! - [`stock_zh_a_hist`]：A 股历史行情（日/周/月，支持复权）
//! - [`stock_zh_a_spot_em`]：A 股实时行情（分页全量）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    a_share_market_code, fetch_clist, fetch_kline, kline_to_df, push2_urls, require_kline_data,
    KLINE_COLS, KLINE_COLS_WITH_SYMBOL,
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

/// 实时行情公共列处理：重命名 + 选择 + 数值转换。
fn spot_common_transform(mut df: Df) -> Result<Df> {
    if df.height() == 0 {
        return Ok(df);
    }
    let rename_map = [
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
    let mut cols_to_keep: Vec<&str> = Vec::with_capacity(rename_map.len());
    for (from, to) in &rename_map {
        let _ = df.inner_mut().rename(from, (*to).into());
        cols_to_keep.push(to);
    }
    let mut df = df.select(&cols_to_keep)?;
    let numeric_cols: Vec<&str> = [
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
    ]
    .to_vec();
    df.cast_numeric(&numeric_cols)?;
    Ok(df)
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

        // 列名与顺序对齐 akshare spot_em
        let expected = [
            "序号",
            "最新价",
            "涨跌幅",
            "涨跌额",
            "成交量",
            "成交额",
            "振幅",
            "换手率",
            "市盈率-动态",
            "量比",
            "5分钟涨跌",
            "代码",
            "名称",
            "最高",
            "最低",
            "今开",
            "昨收",
            "总市值",
            "流通市值",
            "60日涨跌幅",
            "年初至今涨跌幅",
            "涨速",
            "市净率",
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
