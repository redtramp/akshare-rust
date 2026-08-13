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

    df = df.sort_by("f3", false, true)?;

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
///
/// 约定：时间列必须位于每行首位（`line[0]`），且 `src_cols`/`out_cols` 中的时间列名为
/// `"时间"`（与 akshare 各分钟级接口一致）。数据缺失（空行）时返回带目标列名的空表。
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
        // 先归一化（补秒）再与带秒的 start/end 比较：避免原始 16 字符行
        // （如 "2024-01-02 09:30"）作为前缀比较小于 start（"2024-01-02 09:30:00"）
        // 而被误过滤（pandas 标签切片会包含该行）。
        let tn = normalize_dt(t);
        if tn.as_str() < start || tn.as_str() > end {
            continue;
        }
        let mut row: Vec<Option<String>> = Vec::with_capacity(out_cols.len());
        for oc in out_cols {
            if *oc == "时间" {
                row.push(Some(tn.clone()));
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
        // 空表也按最终列名构造，保证调用方拿到的列契约一致
        return Df::from_string_rows(select, &[]);
    }
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, (*to).into());
    }
    let mut df = df.select(select)?;
    df.cast_numeric(numeric)?;
    Ok(df)
}

/// 东财 datacenter `RPT_*` 报表的通用收尾：按「json 字段键 → 中文列名」映射表，从每行抽取 `select` 顺序的中文列，并对 `numeric` 列做数值化。
///
/// 对应 akshare 的 `big_df.columns = [...]`（键→中文重命名）+ `df[cols]`（选择）+ `pd.to_numeric(errors="coerce")`。与 [`finalize_spot`] 不同，本函数按**字段键**而非 f-字段码重命名（如 `SECURITY_CODE`），因此对响应列序不敏感，映射更稳健。
///
/// `index_name`：若 `Some(name)`，则在最前列生成一列名为 `name` 的 1-based 序号（对应 akshare `df.reset_index()` + `index+1`，服务端响应本身不含 `index` 字段）。
pub(crate) fn finalize_report(
    rows: &[Value],
    rename: &[(&str, &str)],
    select: &[&str],
    numeric: &[&str],
    index_name: Option<&str>,
) -> Result<Df> {
    let full_select: Vec<&str> = match index_name {
        Some(n) => {
            let mut v = Vec::with_capacity(select.len() + 1);
            v.push(n);
            v.extend(select.iter().copied());
            v
        }
        None => select.to_vec(),
    };
    let mut full_numeric: Vec<&str> = numeric.to_vec();
    if let Some(n) = index_name {
        full_numeric.insert(0, n);
    }
    if rows.is_empty() {
        // 空表也按最终列名构造，保证调用方拿到的列契约一致
        return Df::from_string_rows(&full_select, &[]);
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let Some(obj) = row.as_object() else {
            return Err(AkshareError::Empty("报表行不是 JSON 对象".into()));
        };
        let mut r: Vec<Option<String>> = Vec::with_capacity(full_select.len());
        if index_name.is_some() {
            r.push(Some((i + 1).to_string()));
        }
        for col in select {
            let key = rename.iter().find(|(_, c)| c == col).map(|(k, _)| *k);
            let val = key.and_then(|k| obj.get(k)).and_then(json_value_to_string);
            r.push(val);
        }
        out.push(r);
    }
    let mut df = Df::from_string_rows(&full_select, &out)?;
    df.cast_numeric(&full_numeric)?;
    Ok(df)
}

/// 板块名称/概念列表公共列契约（行业/概念一致）。
pub(crate) const BOARD_NAME_SELECT: [&str; 12] = [
    "排名",
    "板块名称",
    "板块代码",
    "最新价",
    "涨跌额",
    "涨跌幅",
    "总市值",
    "换手率",
    "上涨家数",
    "下跌家数",
    "领涨股票",
    "领涨股票-涨跌幅",
];

/// 板块名称/概念列表的数值列（除 排名/板块名称/板块代码/领涨股票 外全部数值化）。
pub(crate) const BOARD_NAME_NUMERIC: [&str; 8] = [
    "最新价",
    "涨跌额",
    "涨跌幅",
    "总市值",
    "换手率",
    "上涨家数",
    "下跌家数",
    "领涨股票-涨跌幅",
];

/// 板块成份（cons）公共列契约（行业/概念一致）。
pub(crate) const BOARD_CONS_SELECT: [&str; 16] = [
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
    "换手率",
    "市盈率-动态",
    "市净率",
];

/// 板块成份的数值列（除 序号/代码/名称 外全部数值化）。
pub(crate) const BOARD_CONS_NUMERIC: [&str; 13] = [
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
    "换手率",
    "市盈率-动态",
    "市净率",
];

/// 板块历史行情列（由 K 线源列重排得到）。
pub(crate) const BOARD_HIST_SELECT: [&str; 11] = [
    "日期",
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

/// 涨停股池列契约（对应 akshare `stock_zt_pool_em` 的最终 select）。
pub(crate) const ZT_POOL_SELECT: [&str; 16] = [
    "序号",
    "代码",
    "名称",
    "涨跌幅",
    "最新价",
    "成交额",
    "流通市值",
    "总市值",
    "换手率",
    "封板资金",
    "首次封板时间",
    "最后封板时间",
    "炸板次数",
    "涨停统计",
    "连板数",
    "所属行业",
];

/// 个股资金流列契约（对应 akshare `stock_individual_fund_flow` 的最终 select）。
pub(crate) const FFLOW_SELECT: [&str; 13] = [
    "日期",
    "收盘价",
    "涨跌幅",
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

/// 沪深港通资金流向列契约（对应 akshare `stock_hsgt_fund_flow_summary_em` 的最终 select）。
pub(crate) const HSGT_SELECT: [&str; 13] = [
    "交易日",
    "类型",
    "板块",
    "资金方向",
    "交易状态",
    "成交净买额",
    "资金净流入",
    "当日资金余额",
    "上涨数",
    "持平数",
    "下跌数",
    "相关指数",
    "指数涨跌幅",
];

/// 板块名称/概念列表公共加工：按字段名重命名 + 选择 + 数值化。
///
/// 说明：akshare 源实现按“位置”重命名，但其列名表数量与请求字段数并不一致
/// （上游缺陷，如概念板块 28 列名 vs 24 字段），本实现按东财字段标准语义
/// （`f2`=最新价、`f12`=板块代码、`f14`=板块名称、`f104`/`f105`=上涨/下跌家数、
/// `f128`=领涨股票）重命名，保证数值落在正确列上；最终列契约与 akshare 一致。
pub(crate) fn finalize_board_name(df: Df, rename: &[(&str, &str)]) -> Result<Df> {
    if df.height() == 0 {
        return Df::from_string_rows(&BOARD_NAME_SELECT, &[]);
    }
    let mut df = df;
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, (*to).into());
    }
    let mut df = df.select(&BOARD_NAME_SELECT)?;
    df.cast_numeric(&BOARD_NAME_NUMERIC)?;
    Ok(df)
}

/// 板块成份（cons）公共加工（行业/概念同构）。
///
/// 与 [`finalize_board_name`] 同理：按东财字段标准语义映射
/// （`f2`=最新价…`f12`=代码、`f14`=名称、`f15`=最高…`f25`=市净率）。
pub(crate) fn finalize_board_cons(df: Df) -> Result<Df> {
    if df.height() == 0 {
        return Df::from_string_rows(&BOARD_CONS_SELECT, &[]);
    }
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
        ("f12", "代码"),
        ("f14", "名称"),
        ("f15", "最高"),
        ("f16", "最低"),
        ("f17", "今开"),
        ("f18", "昨收"),
        ("f25", "市净率"),
    ];
    let mut df = df;
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, (*to).into());
    }
    let mut df = df.select(&BOARD_CONS_SELECT)?;
    df.cast_numeric(&BOARD_CONS_NUMERIC)?;
    Ok(df)
}

/// 抓取板块名称列表并返回 `(板块名称, 板块代码)` 对（用于名称→代码解析）。
pub(crate) fn board_name_pairs(
    http: &HttpClient,
    fs: &str,
    fields: &str,
    rename: &[(&str, &str)],
) -> Result<Vec<(String, String)>> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1", "ut": UT_CLIST,
        "fltt": "2", "invt": "2", "fid": "f3", "fs": fs, "fields": fields,
    });
    let params = params.as_object().cloned().unwrap_or_default();
    let df = finalize_board_name(fetch_clist(http, &urls, &params)?, rename)?;
    let (Some(names), Some(codes)) = (
        df.inner()
            .column("板块名称")
            .ok()
            .and_then(|c| c.str().ok()),
        df.inner()
            .column("板块代码")
            .ok()
            .and_then(|c| c.str().ok()),
    ) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(df.height());
    for (n, c) in names.iter().zip(codes.iter()) {
        if let (Some(n), Some(c)) = (n, c) {
            out.push((n.to_string(), c.to_string()));
        }
    }
    Ok(out)
}

/// K 线接口（带附加参数，如板块 K 线的 `smplmt`/`lmt`）。
pub fn fetch_kline_ext(
    http: &HttpClient,
    secid: &str,
    klt: &str,
    fqt: &str,
    beg: &str,
    end: &str,
    extra: &[(&str, &str)],
) -> Result<Vec<Vec<String>>> {
    let mut params = json!({
        "fields1": "f1,f2,f3,f4,f5,f6",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f116",
        "ut": UT_KLINE,
        "klt": klt,
        "fqt": fqt,
        "secid": secid,
        "beg": beg,
        "end": end,
    });
    if let Some(obj) = params.as_object_mut() {
        for (k, v) in extra {
            obj.insert((*k).into(), Value::String((*v).into()));
        }
    }
    kline_lines(http, params)
}

/// 解析 datacenter 响应文本为 JSON。
///
/// 个别东财接口（如 `RPT_STOCK_PARTICIPATION`）在携带 `callback` 参数时返回
/// JSONP（`callback(...);` 包裹），无法直接 `serde_json::from_str`。本函数在严格解析
/// 失败后尝试剥离 JSONP 外层包裹再解析；对普通 JSON 响应（绝大多数 `RPT_*`）无任何影响。
fn parse_datacenter_response(text: &str, url: &str) -> Result<Value> {
    // 先尝试严格解析（普通 JSON 响应，绝大多数 RPT_*）。
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Ok(v);
    }
    let s = text.trim();
    // 个别接口（如 RPT_STOCK_PARTICIPATION）在携带 callback 时返回 JSONP
    //（`callback(...);` 包裹）。剥离首个 `(` 之前的前缀与最外层括号，取内部 JSON。
    // 对 `jQuery123({...});` / `cb({...})` 等形态均适用，且不影响普通 JSON。
    if let Some(open) = s.find('(') {
        if let Some(close) = s.rfind(')') {
            if close > open {
                let inner = &s[open + 1..close];
                if let Ok(v) = serde_json::from_str::<Value>(inner) {
                    return Ok(v);
                }
            }
        }
    }
    serde_json::from_str::<Value>(text).map_err(|e| AkshareError::json(url, e.to_string()))
}

/// 东财 data-api 通用分页抓取（datacenter-web / data.eastmoney.com/dataapi /
/// datacenter.eastmoney.com/special 共用）：按 `result.pages` 循环翻页；页数据为空时提前终止。
///
/// - `url`：请求地址（datacenter-web 常规为 `https://datacenter-web.eastmoney.com/api/data/v1/get`；
///   分析师排名用 `https://data.eastmoney.com/dataapi/invest/list`；分析师详情用
///   `https://datacenter.eastmoney.com/special/api/data/v1/get`）。
/// - `page_size` 为 `"0"` 时表示单页请求（不附加 `pageSize`/`pageNumber`，对应个别接口
///   默认返回全量，如分析师历史指数 `RPT_RESEARCHER_DETAILS`）。
/// - `source`/`client` 透传（datacenter-web 常规为 `WEB`；北交所申购为 `NEEQSELECT`）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn fetch_eastmoney_pages(
    http: &HttpClient,
    url: &str,
    report_name: &str,
    columns: &str,
    extra: &Map<String, Value>,
    page_size: &str,
    source: &str,
    client: &str,
) -> Result<Vec<Value>> {
    let mut all: Vec<Value> = Vec::new();
    let mut page: i64 = 1;
    let paginate = page_size != "0";
    loop {
        let mut params = extra.clone();
        params.insert("reportName".into(), Value::String(report_name.into()));
        params.insert("columns".into(), Value::String(columns.into()));
        if paginate {
            params.insert("pageSize".into(), Value::String(page_size.into()));
            params.insert("pageNumber".into(), Value::from(page));
        }
        params.insert("source".into(), Value::String(source.into()));
        params.insert("client".into(), Value::String(client.into()));
        let text = http.get_text(url, &params, None)?;
        let value = parse_datacenter_response(&text, url)?;
        let data = value
            .get("result")
            .and_then(|r| r.get("data"))
            .and_then(Value::as_array);
        let Some(data) = data else {
            break;
        };
        if data.is_empty() {
            break;
        }
        let pages = value
            .get("result")
            .and_then(|r| r.get("pages"))
            .and_then(Value::as_i64)
            .unwrap_or(1);
        all.extend(data.iter().cloned());
        if !paginate || page >= pages {
            break;
        }
        page += 1;
    }
    Ok(all)
}

/// `datacenter.eastmoney.com/securities` 分页数据（F10/同行比较/HK-US F10 系列）。A 股同行比较
/// 用 `source=HSF10`、港股同行比较用 `source=F10`（均 `client=PC`），与 `datacenter-web` 的
/// `WEB/WEB` 不同。`source`/`client` 由调用方透传。其余翻页/JSONP 剥壳逻辑与
/// [`fetch_datacenter_pages`] 完全一致（共用 [`fetch_eastmoney_pages`]）。
pub(crate) fn fetch_securities_pages(
    http: &HttpClient,
    report_name: &str,
    columns: &str,
    extra: &Map<String, Value>,
    page_size: &str,
    source: &str,
    client: &str,
) -> Result<Vec<Value>> {
    fetch_eastmoney_pages(
        http,
        "https://datacenter.eastmoney.com/securities/api/data/v1/get",
        report_name,
        columns,
        extra,
        page_size,
        source,
        client,
    )
}

/// datacenter-web 分页数据（见 [`fetch_eastmoney_pages`]，固定 `source=WEB`/`client=WEB`）。
pub(crate) fn fetch_datacenter_pages(
    http: &HttpClient,
    report_name: &str,
    columns: &str,
    extra: &Map<String, Value>,
    page_size: &str,
) -> Result<Vec<Value>> {
    fetch_eastmoney_pages(
        http,
        "https://datacenter-web.eastmoney.com/api/data/v1/get",
        report_name,
        columns,
        extra,
        page_size,
        "WEB",
        "WEB",
    )
}

/// 日期截断：`"2026-08-05 00:00:00"` → `"2026-08-05"`（对应 akshare `dt.date`）。
pub(crate) fn date_only(s: &str) -> &str {
    if s.len() >= 10 {
        &s[..10]
    } else {
        s
    }
}

/// 前导补零到 6 位（对应 akshare `str.zfill(6)`，如封板时间 `92500` → `"092500"`）。
pub(crate) fn zfill6(s: &str) -> String {
    format!("{s:0>6}")
}

/// 数值除以缩放因子并转字符串（对应 akshare `df[col] / N`）。
pub(crate) fn num_div(v: &Value, div: f64) -> Option<String> {
    v.as_f64().map(|f| (f / div).to_string())
}

/// 字符串数值除以缩放因子并转字符串。
pub(crate) fn num_div_str(s: &str, div: f64) -> Option<String> {
    s.parse::<f64>().ok().map(|f| (f / div).to_string())
}

/// 涨停统计 `{days, ct}` → `"days/ct"`（对应 akshare 拼接）。
fn zttj_str(v: &Value) -> Option<String> {
    match (
        v.get("days").and_then(Value::as_i64),
        v.get("ct").and_then(Value::as_i64),
    ) {
        (Some(d), Some(c)) => Some(format!("{d}/{c}")),
        _ => None,
    }
}

/// 在 0 列插入 1 起始的 int64 序号列（对应 akshare `reset_index + 1`）。
pub(crate) fn insert_index_col(df: &mut Df, name: &str) -> Result<()> {
    let idx: Vec<Option<i64>> = (1..=df.height()).map(|i| Some(i as i64)).collect();
    let chunked = Int64Chunked::from_iter_options(name.into(), idx.iter().copied());
    df.inner_mut()
        .insert_column(0, chunked.into_series().into())?;
    Ok(())
}

/// 涨停股池加工（对应 akshare `stock_zt_pool_em`：最新价 ÷1000、封板时间补零、
/// 涨停统计 `days/ct`，16 列含 int64 序号）。
pub(crate) fn finalize_zt_pool(pool: &[Value]) -> Result<Df> {
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(pool.len());
    for item in pool {
        let f = |k: &str| item.get(k).and_then(json_value_to_string);
        rows.push(vec![
            f("c"),
            f("n"),
            f("zdp"),
            item.get("p").and_then(|v| num_div(v, 1000.0)),
            f("amount"),
            f("ltsz"),
            f("tshare"),
            f("hs"),
            f("fund"),
            f("fbt").map(|s| zfill6(&s)),
            f("lbt").map(|s| zfill6(&s)),
            f("zbc"),
            item.get("zttj").and_then(zttj_str),
            f("lbc"),
            f("hybk"),
        ]);
    }
    let mut df = Df::from_string_rows(&ZT_POOL_SELECT[1..], &rows)?;
    df.cast_numeric(&[
        "涨跌幅",
        "最新价",
        "成交额",
        "流通市值",
        "总市值",
        "换手率",
        "封板资金",
        "炸板次数",
        "连板数",
    ])?;
    insert_index_col(&mut df, "序号")?;
    Ok(df)
}

/// 个股资金流加工（15 字段 klines 行 → 13 列，日期截断，数值化）。
pub(crate) fn finalize_fflow(klines: &[Value]) -> Result<Df> {
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for line in klines {
        let Some(s) = line.as_str() else {
            continue;
        };
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 13 {
            continue;
        }
        rows.push(vec![
            Some(date_only(parts[0]).to_string()),
            Some(parts[11].to_string()),
            Some(parts[12].to_string()),
            Some(parts[1].to_string()),
            Some(parts[6].to_string()),
            Some(parts[5].to_string()),
            Some(parts[10].to_string()),
            Some(parts[4].to_string()),
            Some(parts[9].to_string()),
            Some(parts[3].to_string()),
            Some(parts[8].to_string()),
            Some(parts[2].to_string()),
            Some(parts[7].to_string()),
        ]);
    }
    let mut df = Df::from_string_rows(&FFLOW_SELECT, &rows)?;
    df.cast_numeric(&[
        "收盘价",
        "涨跌幅",
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
    ])?;
    Ok(df)
}

/// 沪深港通资金流向加工（13 列；成交净买额/资金净流入/当日资金余额 ÷10000）。
pub(crate) fn finalize_hsgt(rows: &[Value]) -> Result<Df> {
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for item in rows {
        let f = |k: &str| item.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("TRADE_DATE").map(|s| date_only(&s).to_string()),
            f("BOARD_TYPE"),
            f("MUTUAL_TYPE_NAME"),
            f("FUNDS_DIRECTION"),
            f("status"),
            f("netBuyAmt").and_then(|s| num_div_str(&s, 10000.0)),
            f("dayNetAmtIn").and_then(|s| num_div_str(&s, 10000.0)),
            f("dayAmtRemain").and_then(|s| num_div_str(&s, 10000.0)),
            f("f104"),
            f("f106"),
            f("f105"),
            f("INDEX_NAME"),
            f("INDEX_f3"),
        ]);
    }
    let mut df = Df::from_string_rows(&HSGT_SELECT, &out)?;
    df.cast_numeric(&[
        "成交净买额",
        "资金净流入",
        "当日资金余额",
        "交易状态",
        "上涨数",
        "持平数",
        "下跌数",
        "指数涨跌幅",
    ])?;
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

#[cfg(test)]
mod tests_b3 {
    use super::*;
    use serde_json::json;

    /// 涨停股池：最新价 ÷1000、封板时间补零、涨停统计 days/ct、序号 int64。
    #[test]
    fn zt_pool_offline() {
        let pool = json!([
            {
                "c": "002792", "n": "通宇通讯", "p": 37190, "zdp": 9.997,
                "amount": 307960640, "ltsz": 12563692485.58, "tshare": 19481462030.08,
                "hs": 2.45, "lbc": 2, "fbt": 92500, "lbt": 92500, "fund": 305090470,
                "zbc": 0, "hybk": "通信设备",
                "zttj": {"days": 5, "ct": 4},
            }
        ]);
        let df = finalize_zt_pool(pool.as_array().unwrap()).unwrap();
        assert_eq!(df.column_names(), ZT_POOL_SELECT);
        assert_eq!(df.height(), 1);
        let idx = df.inner().column("序号").unwrap().i64().unwrap();
        assert_eq!(idx.get(0), Some(1));
        let px = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(37.19));
        let t = df.inner().column("首次封板时间").unwrap().str().unwrap();
        assert_eq!(t.get(0), Some("092500"));
        let zttj = df.inner().column("涨停统计").unwrap().str().unwrap();
        assert_eq!(zttj.get(0), Some("5/4"));
    }

    /// 个股资金流：真实响应行 → 13 列契约，日期截断。
    #[test]
    fn fflow_offline() {
        let klines = json!([
            "2026-02-06,-462514.0,6527678.0,-6065163.0,-10508481.0,10045967.0,-0.50,7.01,-6.51,-11.28,10.78,4.85,-0.61,0.00,0.00"
        ]);
        let df = finalize_fflow(klines.as_array().unwrap()).unwrap();
        assert_eq!(df.column_names(), FFLOW_SELECT);
        let date = df.inner().column("日期").unwrap().str().unwrap();
        assert_eq!(date.get(0), Some("2026-02-06"));
        let close = df.inner().column("收盘价").unwrap().f64().unwrap();
        assert_eq!(close.get(0), Some(4.85));
        let main = df.inner().column("主力净流入-净额").unwrap().f64().unwrap();
        assert_eq!(main.get(0), Some(-462514.0));
        let small = df
            .inner()
            .column("小单净流入-净占比")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(small.get(0), Some(7.01));
    }

    /// 沪深港通：÷10000 缩放 + 13 列契约。
    #[test]
    fn hsgt_offline() {
        let rows = json!([{
            "TRADE_DATE": "2026-08-07 00:00:00", "BOARD_TYPE": "1", "MUTUAL_TYPE_NAME": "沪股通",
            "FUNDS_DIRECTION": "北向", "status": "1", "netBuyAmt": 123456789.0,
            "dayNetAmtIn": 1000000.0, "dayAmtRemain": 50000000.0, "f104": 800,
            "f106": 60, "f105": 300, "INDEX_NAME": "上证指数", "INDEX_f3": 0.5,
        }]);
        let df = finalize_hsgt(rows.as_array().unwrap()).unwrap();
        assert_eq!(df.column_names(), HSGT_SELECT);
        let net = df.inner().column("成交净买额").unwrap().f64().unwrap();
        assert_eq!(net.get(0), Some(12345.6789));
        let day = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(day.get(0), Some("2026-08-07"));
        // 交易状态 由 finalize_hsgt 数值化（akshare 为 int64，与数值类归一一致）
        let st = df.inner().column("交易状态").unwrap().f64().unwrap();
        assert_eq!(st.get(0), Some(1.0));
    }

    /// 板块名称/概念列表公共加工：字段语义映射 + 12 列契约。
    #[test]
    fn board_name_offline() {
        let rows = json!([
            {"index": 1, "f2": 11500.39, "f3": -0.15, "f12": "BK1627", "f14": "综合Ⅲ",
             "f104": 7, "f105": 10, "f128": "XX股份", "f20": 1e12, "f8": 1.2,
             "f4": -17.2, "f141": 0.3}
        ]);
        let df = Df::from_json_rows(rows.as_array().unwrap()).unwrap();
        let rename = [
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
        let df = finalize_board_name(df, &rename).unwrap();
        assert_eq!(df.column_names(), BOARD_NAME_SELECT);
        let name = df.inner().column("板块名称").unwrap().str().unwrap();
        assert_eq!(name.get(0), Some("综合Ⅲ"));
        let code = df.inner().column("板块代码").unwrap().str().unwrap();
        assert_eq!(code.get(0), Some("BK1627"));
        let px = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(11500.39));
    }

    /// 板块成份公共加工：16 列契约 + 数值化。
    #[test]
    fn board_cons_offline() {
        let rows = json!([
            {"index": 1, "f2": 10.5, "f3": 3.2, "f4": 0.3, "f5": 100000, "f6": 1050000.0,
             "f7": 2.1, "f8": 0.5, "f9": 8.1, "f12": "000001", "f14": "平安银行",
             "f15": 10.8, "f16": 10.2, "f17": 10.3, "f18": 9.6, "f25": 0.9}
        ]);
        let df = Df::from_json_rows(rows.as_array().unwrap()).unwrap();
        let df = finalize_board_cons(df).unwrap();
        assert_eq!(df.column_names(), BOARD_CONS_SELECT);
        let code = df.inner().column("代码").unwrap().str().unwrap();
        assert_eq!(code.get(0), Some("000001"));
        let pb = df.inner().column("市净率").unwrap().f64().unwrap();
        assert_eq!(pb.get(0), Some(0.9));
    }
}
