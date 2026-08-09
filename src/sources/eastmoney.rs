//! 东方财富（eastmoney）数据源。
//!
//! akshare 最大单一数据源（源码中 1000+ 处引用）。本模块提供统一入口：
//! - `fetch_clist`：分页行情列表（对应 akshare `fetch_paginated_data`，
//!   自动翻页 + 按 f3 涨跌幅降序 + 生成序号）
//! - `fetch_kline`：K 线接口（`stock/kline/get`，对应 `stock_zh_a_hist` 等）
//!
//! 说明：`fetch_clist` 在 akshare 中按 `f3` 字段降序排序并重置序号
//! （对应 `sort_values + reset_index + 1`），此处保持一致以便差分对齐。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use polars::prelude::{Int64Chunked, IntoSeries, NewChunkedArray};
use serde_json::{json, Map, Value};

/// 行情列表接口公共参数中的 `ut` 值。
pub const UT_CLIST: &str = "bd1d9ddb04089700cf9c27f6f7426281";
/// K 线接口公共参数中的 `ut` 值。
pub const UT_KLINE: &str = "7eea3edcaed734bea9cbfc24409ed989";

/// push2 行情节点列表：东财多节点部署，单节点可能被限流或故障，
/// 依次尝试直到成功（生产环境的节点容灾做法，akshare 亦曾切换
/// `82.push2`/`90.push2` 等节点解决此类问题）。
pub const PUSH2_HOSTS: &[&str] = &[
    "push2.eastmoney.com",
    "90.push2.eastmoney.com",
    "82.push2.eastmoney.com",
    "7.push2.eastmoney.com",
    "28.push2.eastmoney.com",
    "16.push2.eastmoney.com",
    "48.push2.eastmoney.com",
];

/// 生成 push2 多节点 URL 列表（如 `path = "/api/qt/clist/get"`）。
pub fn push2_urls(path: &str) -> Vec<String> {
    PUSH2_HOSTS
        .iter()
        .map(|host| format!("https://{host}{path}"))
        .collect()
}

/// 分页抓取 clist 行情列表并合并、排序、编序（对应 akshare `fetch_paginated_data`）。
///
/// `urls` 为候选节点 URL 列表（见 [`push2_urls`]），首页自动故障转移。
/// 返回的 `Df` 首列 `index` 为 1 起始的序号，后续列按响应字段顺序。
pub fn fetch_clist(
    http: &HttpClient,
    urls: &[String],
    base_params: &Map<String, Value>,
) -> Result<Df> {
    let rows = http.fetch_paginated_diff_any(urls, base_params, None)?;
    finalize_clist(rows)
}

/// 对 clist 原始行做最终加工：按 f3（涨跌幅）**数值**降序排序、生成 int64 序号列。
///
/// 对应 akshare `sort_values(by="f3", ascending=False) + reset_index(drop=True) + 1`。
/// 提取为纯函数便于离线单测（不依赖网络）。
pub(crate) fn finalize_clist(rows: Vec<Value>) -> Result<Df> {
    let mut df = Df::from_json_rows(&rows)?;
    if df.height() == 0 {
        return Ok(df);
    }

    df = df.sort_by("f3", false)?;

    let idx: Vec<Option<i64>> = (1..=df.height()).map(|i| Some(i as i64)).collect();
    df.inner_mut().insert_column(0, {
        let chunked = Int64Chunked::from_iter_options("index".into(), idx.iter().copied());
        chunked.into_series().into()
    })?;

    Ok(df)
}

/// 单只标的 K 线（`push2his/api/qt/stock/kline/get`）。
///
/// - `secid`：`{market}.{symbol}`，如 `0.000001`、`1.600000`
/// - `klt`：周期编码（101=日, 102=周, 103=月, 1/5/15/30/60=分钟）
/// - `fqt`：复权（0=不复权, 1=前复权, 2=后复权）
///
/// 返回 11 列字符串（日期,开盘,收盘,最高,最低,成交量,成交额,振幅,涨跌幅,涨跌额,换手率），
/// 由调用方决定是否追加股票代码列（与 akshare 各函数保持一致）。
pub fn fetch_kline(
    http: &HttpClient,
    secid: &str,
    klt: &str,
    fqt: &str,
    beg: &str,
    end: &str,
) -> Result<Vec<Vec<String>>> {
    let params = json!({
        "fields1": "f1,f2,f3,f4,f5,f6",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f116",
        "ut": UT_KLINE,
        "klt": klt,
        "fqt": fqt,
        "secid": secid,
        "beg": beg,
        "end": end,
    });
    kline_lines(http, params)
}

/// 分钟级 K 线（对应 akshare `stock_zh_a_hist_min_em` 的 klt 分支：
/// `beg=0, end=20500000`，11 字段、无 f116）。
pub fn fetch_kline_min(
    http: &HttpClient,
    secid: &str,
    klt: &str,
    fqt: &str,
) -> Result<Vec<Vec<String>>> {
    let params = json!({
        "fields1": "f1,f2,f3,f4,f5,f6",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61",
        "ut": UT_KLINE,
        "klt": klt,
        "fqt": fqt,
        "secid": secid,
        "beg": "0",
        "end": "20500000",
    });
    kline_lines(http, params)
}

/// 分时线（trends2/get，对应 akshare `stock_zh_a_hist_min_em` period=1 分支）。
///
/// `ndays` 拉取天数（如 `"5"`）；`iscr` 是否含盘前数据（`"0"`/`"1"`）。
/// 每行 8 字段：时间,开盘,收盘,最高,最低,成交量,成交额,均价（iscr=1 时末列为最新价）。
pub fn fetch_trends(
    http: &HttpClient,
    secid: &str,
    ndays: &str,
    iscr: &str,
) -> Result<Vec<Vec<String>>> {
    let url = "https://push2his.eastmoney.com/api/qt/stock/trends2/get";
    let params = json!({
        "fields1": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,f13",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58",
        "ut": UT_KLINE,
        "ndays": ndays,
        "iscr": iscr,
        "secid": secid,
    });
    let params = params.as_object().cloned().unwrap_or_default();

    let value = http.get_json(url, &params, None)?;
    let trends = value
        .get("data")
        .and_then(|d| d.get("trends"))
        .and_then(Value::as_array);

    let Some(trends) = trends else {
        return Ok(Vec::new());
    };

    Ok(trends
        .iter()
        .filter_map(Value::as_str)
        .map(|line| line.split(',').map(|s| s.to_string()).collect())
        .collect())
}

/// K 线原始行提取（公共底层，由 [`fetch_kline`]/[`fetch_kline_min`] 复用）。
fn kline_lines(http: &HttpClient, params: Value) -> Result<Vec<Vec<String>>> {
    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let params = params.as_object().cloned().unwrap_or_default();

    let value = http.get_json(url, &params, None)?;
    let klines = value
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array);

    let Some(klines) = klines else {
        return Ok(Vec::new());
    };

    Ok(klines
        .iter()
        .filter_map(Value::as_str)
        .map(|line| line.split(',').map(|s| s.to_string()).collect())
        .collect())
}

/// 判断 A 股代码所属市场（对应 akshare `stock_zh_a_hist` 的 `market_code`）。
pub fn a_share_market_code(symbol: &str) -> &'static str {
    if symbol.starts_with('6') {
        "1" // 沪市
    } else {
        "0" // 深市/京市
    }
}

/// 判断 ETF 代码所属市场（对应 akshare `get_market_id`）。
pub fn etf_market_id(symbol: &str) -> &'static str {
    if symbol.starts_with('5') || symbol.starts_with('6') {
        "1"
    } else {
        "0"
    }
}

/// 解析 K 线一行：按 akshare 列序返回字段。
/// 返回 11 个字段：日期,开盘,收盘,最高,最低,成交量,成交额,振幅,涨跌幅,涨跌额,换手率。
fn parse_kline_row(line: &[String]) -> Vec<Option<String>> {
    // 服务端可能返回 11 或 12 个字段（含 f116 股票代码），取前 11 个
    line.iter().take(11).map(|s| Some(s.clone())).collect()
}

/// 由 K 线原始行构建带指定列名的 Df（通用 helper）。
///
/// `extra_col`：追加列（如股票代码），插入到第 2 列位置（日期之后），
/// 对应 akshare `stock_zh_a_hist` 的列序 [日期, 股票代码, 开盘, ...]。
pub fn kline_to_df(
    col_names: &[&str],
    klines: &[Vec<String>],
    extra_col: Option<(&str, Vec<String>)>,
) -> Result<Df> {
    if klines.is_empty() {
        return Df::from_string_rows(col_names, &[]);
    }
    let mut rows: Vec<Vec<Option<String>>> = klines.iter().map(|k| parse_kline_row(k)).collect();
    if let Some((_, values)) = &extra_col {
        for (r, v) in rows.iter_mut().zip(values.iter()) {
            r.insert(1, Some(v.clone()));
        }
    }
    let mut df = Df::from_string_rows(col_names, &rows)?;
    // 数值列从 index 1 开始（若含股票代码则跳过它，保持字符串）
    let start = if extra_col.is_some() { 2 } else { 1 };
    let _ = df.cast_numeric(&col_names[start..]);
    Ok(df)
}

/// 通用 K 线列名（不带股票代码，对应 fund_etf_hist_em 等）。
pub const KLINE_COLS: [&str; 11] = [
    "日期",
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

/// 带股票代码的 K 线列名（对应 stock_zh_a_hist）。
pub const KLINE_COLS_WITH_SYMBOL: [&str; 12] = [
    "日期",
    "股票代码",
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

/// 简单校验响应是否含数据（空响应返回 Empty 错误）。
pub fn require_kline_data(klines: &[Vec<String>], symbol: &str) -> Result<()> {
    if klines.is_empty() {
        Err(AkshareError::empty(format!("{symbol} 无 K 线数据")))
    } else {
        Ok(())
    }
}

/// 时间戳归一化：分钟级时间缺秒时补 `":00"`（对齐 pandas
/// `to_datetime(...).astype(str)` 的 `"2024-01-02 09:35:00"` 格式）。
pub fn normalize_dt(s: &str) -> String {
    if s.len() == 16 && s.as_bytes().get(10) == Some(&b' ') {
        format!("{s}:00")
    } else {
        s.to_string()
    }
}

/// 分钟级 K 线/分时通用处理（对应 akshare datetime 切片 + 列选择 + 数值化）。
///
/// - 按时间字符串区间过滤（`start <= 时间 <= end`；时间格式固定宽度，
///   字符串比较与 pandas 标签切片语义等价）
/// - 时间列归一化补秒
/// - 按 `out_cols` 目标列序建表（从 `src_cols` 中取对应源列），`numeric_out` 转 f64
pub fn min_kline_to_df(
    lines: &[Vec<String>],
    start: &str,
    end: &str,
    src_cols: &[&str],
    out_cols: &[&str],
    numeric_out: &[&str],
) -> Result<Df> {
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for line in lines {
        let Some(t) = line.first() else {
            continue;
        };
        if t.as_str() < start || t.as_str() > end {
            continue;
        }
        let mut row: Vec<Option<String>> = Vec::with_capacity(out_cols.len());
        for oc in out_cols {
            if *oc == "时间" {
                row.push(Some(normalize_dt(t)));
                continue;
            }
            let src_idx = src_cols.iter().position(|c| c == oc);
            row.push(src_idx.and_then(|i| line.get(i)).cloned());
        }
        rows.push(row);
    }
    let mut df = Df::from_string_rows(out_cols, &rows)?;
    let _ = df.cast_numeric(numeric_out);
    Ok(df)
}

/// spot 类公共列处理：重命名 + 选择 + 数值转换。
///
/// 对应 akshare `df.rename(columns=...) + df[cols] + to_numeric(errors="coerce")`。
/// 行序（f3 降序 + 序号）已在 [`finalize_clist`] 完成。
pub(crate) fn finalize_spot(
    mut df: Df,
    rename: &[(&str, &str)],
    select: &[&str],
    numeric: &[&str],
) -> Result<Df> {
    if df.height() == 0 {
        return Ok(df);
    }
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, (*to).into());
    }
    let mut df = df.select(select)?;
    df.cast_numeric(numeric)?;
    Ok(df)
}

/// JSON 值转字符串（null → None）。
pub(crate) fn json_value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 排序必须是数值序而非字典序（"10.0" > "9.9"，字典序则相反），
    /// 且涨跌幅缺失（"-"）的行排末尾（对应 akshare sort_values 的 NaN 处理）。
    #[test]
    fn finalize_clist_sorts_numerically_desc_with_nulls_last() {
        let rows = json!([
            {"f3": "10.0", "f12": "000010"},
            {"f3": "9.9", "f12": "000001"},
            {"f3": "-", "f12": "000003"},
            {"f3": "1.5", "f12": "000002"},
        ]);
        let rows: Vec<Value> = rows.as_array().cloned().unwrap();
        let df = finalize_clist(rows).unwrap();

        let codes = df.inner().column("f12").unwrap().str().unwrap();
        let got: Vec<&str> = codes.iter().map(|v| v.unwrap_or("")).collect();
        assert_eq!(got, vec!["000010", "000001", "000002", "000003"]);

        // 序号列为 int64 且 1 起始（对应 akshare reset_index + 1 的 dtype）
        let idx = df.inner().column("index").unwrap().i64().unwrap();
        assert_eq!(idx.get(0), Some(1));
        assert_eq!(idx.get(3), Some(4));
    }

    #[test]
    fn finalize_clist_empty_rows() {
        let df = finalize_clist(Vec::new()).unwrap();
        assert_eq!(df.height(), 0);
    }

    /// 分钟线通用处理：时间区间过滤、缺秒补 ":00"、按目标列序重排。
    #[test]
    fn min_kline_to_df_filters_reorders_normalizes() {
        let lines = vec![
            vec![
                "2024-01-02 09:35".into(),
                "10.0".into(),
                "10.1".into(),
                "10.2".into(),
                "9.9".into(),
                "1000".into(),
                "10100".into(),
                "3.0".into(),
                "1.0".into(),
                "0.1".into(),
                "0.5".into(),
            ],
            vec![
                "2024-01-02 10:00".into(),
                "10.2".into(),
                "10.3".into(),
                "10.4".into(),
                "10.0".into(),
                "800".into(),
                "8200".into(),
                "3.9".into(),
                "1.98".into(),
                "0.2".into(),
                "0.4".into(),
            ],
            vec![
                "2024-01-03 09:30".into(),
                "10.4".into(),
                "10.5".into(),
                "10.6".into(),
                "10.1".into(),
                "900".into(),
                "9400".into(),
                "4.7".into(),
                "1.94".into(),
                "0.2".into(),
                "0.6".into(),
            ],
        ];
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
        let df = min_kline_to_df(
            &lines,
            "2024-01-02 00:00:00",
            "2024-01-02 23:59:59",
            &src,
            &out,
            &out[1..],
        )
        .unwrap();
        assert_eq!(df.height(), 2);
        // 时间归一化补秒
        let t = df.inner().column("时间").unwrap().str().unwrap();
        assert_eq!(t.get(0), Some("2024-01-02 09:35:00"));
        assert_eq!(t.get(1), Some("2024-01-02 10:00:00"));
        // 重排正确：涨跌幅列应取源第 8 字段(1.0/1.98)而非源第 5 字段(成交量)
        let pct = df.inner().column("涨跌幅").unwrap().f64().unwrap();
        assert_eq!(pct.get(0), Some(1.0));
        assert_eq!(pct.get(1), Some(1.98));
        let vol = df.inner().column("成交量").unwrap().f64().unwrap();
        assert_eq!(vol.get(0), Some(1000.0));
    }
}
