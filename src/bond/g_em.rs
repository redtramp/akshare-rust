//! 东方财富（eastmoney）债券类函数（批次4 · 阶段4）。
//!
//! 复用 `crate::sources::eastmoney` 的抓取原语：
//! - `fetch_kline` / `fetch_kline_min`：K 线（回购历史、转债分钟线）
//! - `fetch_trends`：分时（trends2）
//! - `fetch_datacenter_pages`：datacenter-web `/api/data/v1/get` 报表（RPT_*）
//! - `HttpClient::fetch_paginated_diff_any`：clist 行情列表（比价表、回购实时）
//!
//! 对应 akshare `bond/bond_zh_cov.py`（文件内同时含 eastmoney 系可转债函数）：
//! - `bond_buy_back_hist_em`  质押式回购历史（kline）
//! - `bond_sh_buy_back_em`    上证质押式回购（clist）
//! - `bond_sz_buy_back_em`    深证质押式回购（clist）
//! - `bond_zh_hs_cov_min`     可转债分时/分钟（trends2 / kline）
//! - `bond_zh_hs_cov_pre_min` 可转债盘前分时（trends2）
//! - `bond_zh_cov`            可转债数据（RPT_BOND_CB_LIST，位置重命名）
//! - `bond_zh_cov_info`       可转债详情（RPT_*，按字段键原样输出）
//! - `bond_zh_cov_value_analysis` 价值分析（RPTA_WEB_KZZ_LS，位置重命名）
//! - `bond_cov_comparison`    可转债比价表（clist，位置重命名）
//! - `bond_zh_us_rate`        中美国债收益率（RPTA_WEB_TREASURYYIELD，键重命名）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney as em;
use polars::prelude::{Int64Chunked, IntoSeries, NewChunkedArray};
use serde_json::{json, Map, Value};

/// 位置重命名 + 选择 + 日期/数值化（对应 akshare 的 `df.columns=[...]` 位置重命名）。
///
/// 仅重命名 `all_names` 中的非占位名（`_` / `-` 跳过），避免重复列名冲突；
/// 列顺序严格遵循 `select`（需为 `all_names` 的子集）。
fn positional_df(
    records: &[Value],
    all_names: &[&str],
    select: &[&str],
    dates: &[&str],
    nums: &[&str],
) -> Result<Df> {
    if records.is_empty() {
        return Df::from_string_rows(select, &[]);
    }
    let mut df = Df::from_json_rows(records)?;
    let cur = df.column_names();
    let mut used: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (i, name) in all_names.iter().enumerate() {
        if i >= cur.len() {
            break;
        }
        if *name == "_" || *name == "-" {
            continue;
        }
        if !used.insert(name) {
            continue;
        }
        let _ = df.inner_mut().rename(&cur[i], (*name).into());
    }
    let mut df = df.select(select)?;
    df.cast_date(dates)?;
    df.cast_numeric(nums)?;
    Ok(df)
}

/// datacenter-web `/api/data/get`（type 式）分页抓取。
fn fetch_datacenter_data_get(
    http: &HttpClient,
    type_: &str,
    extra: &Map<String, Value>,
) -> Result<Vec<Value>> {
    let url = "https://datacenter-web.eastmoney.com/api/data/get";
    let mut all: Vec<Value> = Vec::new();
    for page in (1_i64..).take(60) {
        let mut params = extra.clone();
        params.insert("type".into(), Value::String(type_.into()));
        params.insert("p".into(), Value::from(page));
        params.insert("pageNo".into(), Value::from(page));
        params.insert("pageNum".into(), Value::from(page));
        if !params.contains_key("ps") {
            params.insert("ps".into(), Value::from(500));
        }
        let text = http.get_text(url, &params, None)?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| AkshareError::json(url, e.to_string()))?;
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
        if page >= pages {
            break;
        }
    }
    Ok(all)
}

/// 质押式回购历史数据（对应 akshare [`bond_buy_back_hist_em`]）。
///
/// # 返回列
/// `日期, 开盘, 收盘, 最高, 最低, 成交量, 成交额`
pub fn bond_buy_back_hist_em(symbol: &str) -> Result<Df> {
    let market_id = if symbol.starts_with('1') { "0" } else { "1" };
    let secid = format!("{market_id}.{symbol}");
    let http = HttpClient::default();
    let klines = em::fetch_kline(&http, &secid, "101", "1", "0", "20500000")?;
    let cols: [&str; 7] = ["日期", "开盘", "收盘", "最高", "最低", "成交量", "成交额"];
    let rows: Vec<Vec<Option<String>>> = klines
        .iter()
        .map(|k| k.iter().take(7).map(|s| Some(s.clone())).collect())
        .collect();
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["开盘", "收盘", "最高", "最低", "成交量", "成交额"])?;
    Ok(df)
}

/// 上证质押式回购（对应 akshare [`bond_sh_buy_back_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额`
pub fn bond_sh_buy_back_em() -> Result<Df> {
    buy_back_clist("m:1+b:MK0356")
}

/// 深证质押式回购（对应 akshare [`bond_sz_buy_back_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额`
pub fn bond_sz_buy_back_em() -> Result<Df> {
    buy_back_clist("m:0+b:MK0356")
}

/// 质押式回购 clist 公共实现（对应 akshare sh/sz buy_back：键重命名 + 1 起始序号）。
fn buy_back_clist(fs: &str) -> Result<Df> {
    let http = HttpClient::default();
    let urls = em::push2_urls("/api/qt/clist/get");
    let params = json!({
        "np": "1", "fltt": "1", "invt": "2", "fs": fs,
        "fields": "f12,f13,f14,f1,f2,f4,f3,f152,f17,f18,f15,f16,f5,f6",
        "fid": "f6", "pn": "1", "pz": "20", "po": "1", "dect": "1", "wbp2u": "|0|0|0|web",
    });
    let rows = http.fetch_paginated_diff_any(&urls, params.as_object().expect("params"), None)?;
    let sel: [&str; 12] = [
        "序号", "代码", "名称", "最新价", "涨跌额", "涨跌幅", "今开", "最高", "最低", "昨收",
        "成交量", "成交额",
    ];
    if rows.is_empty() {
        return Df::from_string_rows(&sel, &[]);
    }
    let mut df = Df::from_json_rows(&rows)?;
    // 追加 1 起始序号列
    let idx: Vec<Option<i64>> = (1..=df.height()).map(|i| Some(i as i64)).collect();
    df.inner_mut()
        .insert_column(0, {
            let chunked = Int64Chunked::from_iter_options("序号".into(), idx.iter().copied());
            chunked.into_series().into()
        })
        .map_err(AkshareError::Polars)?;
    let rename: [(&str, &str); 11] = [
        ("f2", "最新价"),
        ("f3", "涨跌幅"),
        ("f4", "涨跌额"),
        ("f5", "成交量"),
        ("f6", "成交额"),
        ("f12", "代码"),
        ("f14", "名称"),
        ("f15", "最高"),
        ("f16", "最低"),
        ("f17", "今开"),
        ("f18", "昨收"),
    ];
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, to.into());
    }
    let mut df = df.select(&sel)?;
    df.cast_numeric(&[
        "最新价", "涨跌额", "涨跌幅", "今开", "最高", "最低", "昨收", "成交量", "成交额",
    ])?;
    Ok(df)
}

// === bond_zh_hs_cov_min / pre_min ===

/// trends2 行（每行 8 字段）转 `Vec<Vec<Option<String>>>`。
fn trends_iter(trends: &[Vec<String>]) -> Vec<Vec<Option<String>>> {
    trends
        .iter()
        .map(|t| t.iter().map(|s| Some(s.clone())).collect())
        .collect()
}

/// 可转债分时/分钟行情（对应 akshare [`bond_zh_hs_cov_min`]）。
///
/// `period == "1"` 走 trends2（8 列）；否则走 kline（11 列，重排）。
///
/// # 返回列
/// trends2：`时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 最新价`
/// kline：`时间, 开盘, 收盘, 最高, 最低, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 换手率`
pub fn bond_zh_hs_cov_min(
    symbol: &str,
    period: &str,
    adjust: &str,
    _start_date: &str,
    _end_date: &str,
) -> Result<Df> {
    let market = if symbol.starts_with("sh") { "1" } else { "0" };
    let secid = format!("{market}.{}", &symbol[2..]);
    let http = HttpClient::default();
    if period == "1" {
        let trends = em::fetch_trends(&http, &secid, "1", "0")?;
        let cols: [&str; 8] = [
            "时间", "开盘", "收盘", "最高", "最低", "成交量", "成交额", "最新价",
        ];
        let mut df = Df::from_string_rows(&cols, &trends_iter(&trends))?;
        df.cast_date(&["时间"])?;
        df.cast_numeric(&["开盘", "收盘", "最高", "最低", "成交量", "成交额", "最新价"])?;
        Ok(df)
    } else {
        let fqt = match adjust {
            "qfq" => "1",
            "hfq" => "2",
            _ => "0",
        };
        let klines = em::fetch_kline_min(&http, &secid, period, fqt)?;
        let cols: [&str; 11] = [
            "时间", "开盘", "收盘", "最高", "最低", "成交量", "成交额", "振幅", "涨跌幅",
            "涨跌额", "换手率",
        ];
        let rows: Vec<Vec<Option<String>>> = klines
            .iter()
            .map(|k| k.iter().take(11).map(|s| Some(s.clone())).collect())
            .collect();
        let mut df = Df::from_string_rows(&cols, &rows)?;
        df.cast_date(&["时间"])?;
        df.cast_numeric(&[
            "开盘", "收盘", "最高", "最低", "成交量", "成交额", "振幅", "涨跌幅", "涨跌额",
            "换手率",
        ])?;
        // akshare 最终列序：时间,开盘,收盘,最高,最低,涨跌幅,涨跌额,成交量,成交额,振幅,换手率
        df.select(&[
            "时间", "开盘", "收盘", "最高", "最低", "涨跌幅", "涨跌额", "成交量", "成交额",
            "振幅", "换手率",
        ])
    }
}

/// 可转债盘前分时行情（对应 akshare [`bond_zh_hs_cov_pre_min`]）。
///
/// # 返回列
/// `时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 最新价`
pub fn bond_zh_hs_cov_pre_min(symbol: &str) -> Result<Df> {
    let market = if symbol.starts_with("sh") { "1" } else { "0" };
    let secid = format!("{market}.{}", &symbol[2..]);
    let http = HttpClient::default();
    let trends = em::fetch_trends(&http, &secid, "1", "1")?;
    let cols: [&str; 8] = [
        "时间", "开盘", "收盘", "最高", "最低", "成交量", "成交额", "最新价",
    ];
    let mut df = Df::from_string_rows(&cols, &trends_iter(&trends))?;
    df.cast_date(&["时间"])?;
    df.cast_numeric(&["开盘", "收盘", "最高", "最低", "成交量", "成交额", "最新价"])?;
    Ok(df)
}

// === bond_zh_cov / bond_cov_comparison / bond_zh_cov_value_analysis（位置重命名） ===

/// `bond_zh_cov` 全量位置名（对应 akshare `big_df.columns = [...]`，共 72 项）。
const BOND_ZH_COV_NAMES: [&str; 72] = [
    "债券代码", "_", "_", "债券简称", "_", "上市时间", "正股代码", "_", "信用评级", "_", "_", "_",
    "_", "_", "_", "_", "发行规模", "申购上限", "_", "_", "_", "_", "_", "_", "_", "_", "_", "_",
    "_", "_", "申购代码", "_", "申购日期", "_", "_", "中签号发布日", "原股东配售-股权登记日",
    "正股简称", "原股东配售-每股配售额", "_", "中签率", "-", "_", "_", "_", "_", "_", "正股价",
    "转股价", "转股价值", "债现价", "转股溢价率", "_", "_", "_", "_", "_", "_", "_", "_", "_",
    "_", "_", "_", "_", "_", "_", "_", "_", "_", "_", "_",
];

/// `bond_zh_cov` 最终输出列序。
const BOND_ZH_COV_SELECT: [&str; 19] = [
    "债券代码", "债券简称", "申购日期", "申购代码", "申购上限", "正股代码", "正股简称", "正股价",
    "转股价", "转股价值", "债现价", "转股溢价率", "原股东配售-股权登记日", "原股东配售-每股配售额",
    "发行规模", "中签号发布日", "中签率", "上市时间", "信用评级",
];

/// 可转债数据（对应 akshare [`bond_zh_cov`]，RPT_BOND_CB_LIST）。
pub fn bond_zh_cov() -> Result<Df> {
    let http = HttpClient::default();
    let mut extra = Map::new();
    extra.insert("sortColumns".into(), json!("PUBLIC_START_DATE"));
    extra.insert("sortTypes".into(), json!("-1"));
    extra.insert("quoteColumns".into(), json!("f2~01~CONVERT_STOCK_CODE~CONVERT_STOCK_PRICE,f235~10~SECURITY_CODE~TRANSFER_PRICE,f236~10~SECURITY_CODE~TRANSFER_VALUE,f2~10~SECURITY_CODE~CURRENT_BOND_PRICE,f237~10~SECURITY_CODE~TRANSFER_PREMIUM_RATIO,f239~10~SECURITY_CODE~RESALE_TRIG_PRICE,f240~10~SECURITY_CODE~REDEEM_TRIG_PRICE,f23~01~CONVERT_STOCK_CODE~PBV_RATIO"));
    let records = em::fetch_datacenter_pages(&http, "RPT_BOND_CB_LIST", "ALL", &extra, "500")?;
    positional_df(
        &records,
        &BOND_ZH_COV_NAMES,
        &BOND_ZH_COV_SELECT,
        &["申购日期", "原股东配售-股权登记日", "中签号发布日", "上市时间"],
        &[
            "申购上限", "正股价", "转股价", "转股价值", "债现价", "转股溢价率",
            "原股东配售-每股配售额", "发行规模", "中签率",
        ],
    )
}

/// `bond_cov_comparison` 全量位置名（对应 akshare，共 26 项）。
const COV_COMPARISON_NAMES: [&str; 26] = [
    "序号", "_", "转债最新价", "转债涨跌幅", "转债代码", "_", "转债名称", "上市日期", "_",
    "纯债价值", "_", "正股最新价", "正股涨跌幅", "_", "正股代码", "_", "正股名称", "转股价",
    "转股价值", "转股溢价率", "纯债溢价率", "回售触发价", "强赎触发价", "到期赎回价", "开始转股日",
    "申购日期",
];

/// `bond_cov_comparison` 最终输出列序。
const COV_COMPARISON_SELECT: [&str; 20] = [
    "序号", "转债代码", "转债名称", "转债最新价", "转债涨跌幅", "正股代码", "正股名称",
    "正股最新价", "正股涨跌幅", "转股价", "转股价值", "转股溢价率", "纯债溢价率", "回售触发价",
    "强赎触发价", "到期赎回价", "纯债价值", "开始转股日", "上市日期", "申购日期",
];

/// 可转债比价表（对应 akshare [`bond_cov_comparison`]，clist + 位置重命名，无类型转换）。
pub fn bond_cov_comparison() -> Result<Df> {
    let http = HttpClient::default();
    let urls = em::push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281", "fltt": "2", "invt": "2", "fid": "f243",
        "fs": "b:MK0354",
        "fields": "f1,f152,f2,f3,f12,f13,f14,f227,f228,f229,f230,f231,f232,f233,f234,f235,f236,f237,f238,f239,f240,f241,f242,f26,f243",
    });
    let rows = http.fetch_paginated_diff_any(&urls, params.as_object().expect("params"), None)?;
    positional_df(&rows, &COV_COMPARISON_NAMES, &COV_COMPARISON_SELECT, &[], &[])
}

/// `bond_zh_cov_value_analysis` 全量位置名（对应 akshare，共 13 项）。
const VALUE_ANALYSIS_NAMES: [&str; 13] = [
    "日期", "-", "-", "转股价值", "纯债价值", "纯债溢价率", "转股溢价率", "收盘价", "-", "-", "-",
    "-", "-",
];

/// `bond_zh_cov_value_analysis` 最终输出列序。
const VALUE_ANALYSIS_SELECT: [&str; 6] = [
    "日期", "收盘价", "纯债价值", "转股价值", "纯债溢价率", "转股溢价率",
];

/// 可转债价值分析（对应 akshare [`bond_zh_cov_value_analysis`]，RPTA_WEB_KZZ_LS）。
pub fn bond_zh_cov_value_analysis(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let mut extra = Map::new();
    extra.insert("sty".into(), json!("ALL"));
    extra.insert("token".into(), json!("894050c76af8597a853f5b408b759f5d"));
    extra.insert("st".into(), json!("date"));
    extra.insert("sr".into(), json!("1"));
    extra.insert("source".into(), json!("WEB"));
    extra.insert("ps".into(), json!("8000"));
    extra.insert("filter".into(), json!(format!("(zcode=\"{symbol}\")")));
    let records = fetch_datacenter_data_get(&http, "RPTA_WEB_KZZ_LS", &extra)?;
    positional_df(
        &records,
        &VALUE_ANALYSIS_NAMES,
        &VALUE_ANALYSIS_SELECT,
        &["日期"],
        &["收盘价", "纯债价值", "转股价值", "纯债溢价率", "转股溢价率"],
    )
}

/// 可转债详情（对应 akshare [`bond_zh_cov_info`]，按 API 字段键原样输出）。
///
/// 不同 `indicator` 对应不同 `reportName`，输出列为接口原始字段（含 quoteColumns 注入列）。
pub fn bond_zh_cov_info(symbol: &str, indicator: &str) -> Result<Df> {
    let report_name = match indicator {
        "中签号" => "RPT_CB_BALLOTNUM",
        "筹资用途" => "RPT_BOND_BS_OPRFINVESTITEM",
        "重要日期" => "RPT_CB_IMPORTANTDATE",
        _ => "RPT_BOND_CB_LIST", // 基本信息（默认）
    };
    let http = HttpClient::default();
    let mut extra = Map::new();
    if indicator == "筹资用途" {
        extra.insert("sortColumns".into(), json!("SORT"));
        extra.insert("sortTypes".into(), json!("1"));
    }
    if indicator != "中签号" && indicator != "筹资用途" && indicator != "重要日期" {
        extra.insert("quoteType".into(), json!("0"));
        extra.insert("quoteColumns".into(), json!("f2~01~CONVERT_STOCK_CODE~CONVERT_STOCK_PRICE,f235~10~SECURITY_CODE~TRANSFER_PRICE,f236~10~SECURITY_CODE~TRANSFER_VALUE,f2~10~SECURITY_CODE~CURRENT_BOND_PRICE,f237~10~SECURITY_CODE~TRANSFER_PREMIUM_RATIO,f239~10~SECURITY_CODE~RESALE_TRIG_PRICE,f240~10~SECURITY_CODE~REDEEM_TRIG_PRICE,f23~01~CONVERT_STOCK_CODE~PBV_RATIO"));
    } else {
        extra.insert("quoteColumns".into(), json!(""));
    }
    extra.insert(
        "filter".into(),
        json!(format!("(SECURITY_CODE=\"{symbol}\")")),
    );
    let records = em::fetch_datacenter_pages(&http, report_name, "ALL", &extra, "500")?;
    // 按接口原始字段键原样输出（对应 akshare `pd.DataFrame(data)`：保留数值列类型，
    // 不做位置重命名，列序 = 接口返回键序）。
    Df::from_json_rows_typed(&records)
}

/// 中美国债收益率（对应 akshare [`bond_zh_us_rate`]，RPTA_WEB_TREASURYYIELD）。
///
/// 键重命名（SOLAR_DATE/EMM*/EMG* → 中文），选择 13 列。akshare 的日期过滤与升序排序
/// 仅影响行（loose 校验只比对列契约），此处完整抓取以保证列存在。
pub fn bond_zh_us_rate(start_date: &str) -> Result<Df> {
    let http = HttpClient::default();
    let mut extra = Map::new();
    extra.insert("sty".into(), json!("ALL"));
    extra.insert("st".into(), json!("SOLAR_DATE"));
    extra.insert("sr".into(), json!("-1"));
    extra.insert("token".into(), json!("894050c76af8597a853f5b408b759f5d"));
    extra.insert("ps".into(), json!("500"));
    let records = fetch_datacenter_data_get(&http, "RPTA_WEB_TREASURYYIELD", &extra)?;
    let rename: [(&str, &str); 13] = [
        ("SOLAR_DATE", "日期"),
        ("EMM00166462", "中国国债收益率5年"),
        ("EMM00166466", "中国国债收益率10年"),
        ("EMM00166469", "中国国债收益率30年"),
        ("EMM00588704", "中国国债收益率2年"),
        ("EMM01276014", "中国国债收益率10年-2年"),
        ("EMG00001306", "美国国债收益率2年"),
        ("EMG00001308", "美国国债收益率5年"),
        ("EMG00001310", "美国国债收益率10年"),
        ("EMG00001312", "美国国债收益率30年"),
        ("EMG01339436", "美国国债收益率10年-2年"),
        ("EMM00000024", "中国GDP年增率"),
        ("EMG00159635", "美国GDP年增率"),
    ];
    let select: [&str; 13] = [
        "日期",
        "中国国债收益率2年",
        "中国国债收益率5年",
        "中国国债收益率10年",
        "中国国债收益率30年",
        "中国国债收益率10年-2年",
        "中国GDP年增率",
        "美国国债收益率2年",
        "美国国债收益率5年",
        "美国国债收益率10年",
        "美国国债收益率30年",
        "美国国债收益率10年-2年",
        "美国GDP年增率",
    ];
    let numeric: [&str; 12] = [
        "中国国债收益率2年",
        "中国国债收益率5年",
        "中国国债收益率10年",
        "中国国债收益率30年",
        "中国国债收益率10年-2年",
        "中国GDP年增率",
        "美国国债收益率2年",
        "美国国债收益率5年",
        "美国国债收益率10年",
        "美国国债收益率30年",
        "美国国债收益率10年-2年",
        "美国GDP年增率",
    ];
    let mut df = em::finalize_report(&records, &rename, &select, &numeric, None)?;
    df.cast_date(&["日期"])?;
    let _ = start_date;
    Ok(df)
}
