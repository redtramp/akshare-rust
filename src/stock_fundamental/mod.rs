//! 基本面数据（对应 akshare `stock_fundamental/` 目录）。
//!
//! 已实现：
//! - 限售股解禁（`stock_restricted_release_*_em`）：东财 `datacenter-web` 的 `RPT_*` 报表
//!   （`RPT_LIFTDAY_STA` / `RPT_LIFT_STAGE` / `RPT_LIFT_GD`），复用 `stock_feature` 的
//!   `datacenter` / `report_extra` / `fmt_ymd` 与 `sources::eastmoney::finalize_report`
//!   工具，列名与 akshare 逐字对齐。数值列（数量/市值类）服务端以「股」为单位返回，
//!   与 akshare 一致地统一除以 10000 转为「万股/万元」；日期列截断为 `YYYY-MM-DD`。
//! - 同花顺财务指标（`stock_financial_*_ths`，对应 akshare `stock_finance_ths.py`）：
//!   旧系列（`stock_financial_abstract/debt/benefit/cash_ths`）解析
//!   `basic.10jqka.com.cn` 的 HTML `<p id="main">` 内嵌 JSON / `flashData` 双重 JSON，
//!   按 akshare 的 `title→df_index` + 转置 + `reset_index(报告期)` 变换输出；
//!   新系列（`*_new_ths`）走 `basicapi/finance/index/v1/app_data/` 报表，
//!   按 `index_list` 展平为宽表（report_date/report_name/report_period/quarter_name/metric_name
//!   + 动态指标字段）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::finalize_report;
use crate::stock_feature::{datacenter, fmt_ymd, report_extra};
use scraper::{Html, Selector};
use serde_json::{Map, Value};

// ============ 1. stock_restricted_release_summary_em ============

const SUMMARY_RENAME: [(&str, &str); 7] = [
    ("FREE_DATE", "解禁时间"),
    ("LIFT_ORG_NUM", "当日解禁股票家数"),
    ("LIFT_NUM", "解禁数量"),
    ("MARKET_CAP", "实际解禁数量"),
    ("INDEX_PRICE", "实际解禁市值"),
    ("CHANGE_RATE", "沪深300指数"),
    ("PLAN_LIFT_NUM", "沪深300指数涨跌幅"),
];
const SUMMARY_SELECT: [&str; 7] = [
    "解禁时间",
    "当日解禁股票家数",
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "沪深300指数",
    "沪深300指数涨跌幅",
];
const SUMMARY_NUMERIC: [&str; 6] = [
    "当日解禁股票家数",
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "沪深300指数",
    "沪深300指数涨跌幅",
];
const SUMMARY_DATE: [&str; 1] = ["解禁时间"];

/// 限售股解禁汇总（对应 akshare [`akshare.stock_restricted_release_summary_em`]）。
///
/// `symbol`：板块（默认 `"全部股票"`，可选 沪市A股/科创板/深市A股/创业板/京市A股）；
/// `start_date` / `end_date`：区间 `YYYYMMDD`（默认 `"20221101"` / `"20221209"`）。
/// 报表 `RPT_LIFTDAY_STA`，按解禁日升序。
///
/// # 返回列
/// `序号, 解禁时间, 当日解禁股票家数, 解禁数量, 实际解禁数量, 实际解禁市值,
/// 沪深300指数, 沪深300指数涨跌幅`
pub fn stock_restricted_release_summary_em(
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    const SYMBOL_MAP: &[(&str, &str)] = &[
        ("全部股票", "000300"),
        ("沪市A股", "000001"),
        ("科创板", "000688"),
        ("深市A股", "399001"),
        ("创业板", "399001"),
        ("京市A股", "999999"),
    ];
    let code = SYMBOL_MAP
        .iter()
        .find(|(k, _)| *k == symbol)
        .map(|(_, v)| *v)
        .ok_or_else(|| AkshareError::Param(format!("未知板块 symbol: {symbol}")))?;
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!(r#"(INDEX_CODE="{code}")(FREE_DATE>='{sd}')(FREE_DATE<='{ed}')"#);
    let extra = report_extra("FREE_DATE", "1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_LIFTDAY_STA", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &SUMMARY_RENAME,
        &SUMMARY_SELECT,
        &SUMMARY_NUMERIC,
        Some("序号"),
    )?;
    df.scale("解禁数量", 10000.0)?
        .scale("实际解禁数量", 10000.0)?
        .scale("实际解禁市值", 10000.0)?
        .cast_date(&SUMMARY_DATE)?;
    Ok(df)
}

// ============ 2. stock_restricted_release_detail_em ============

const DETAIL_RENAME: [(&str, &str); 11] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("FREE_DATE", "解禁时间"),
    ("CURRENT_FREE_SHARES", "实际解禁数量"),
    ("ABLE_FREE_SHARES", "解禁数量"),
    ("LIFT_MARKET_CAP", "实际解禁市值"),
    ("FREE_RATIO", "占解禁前流通市值比例"),
    ("NEW", "解禁前一交易日收盘价"),
    ("B20_ADJCHRATE", "解禁前20日涨跌幅"),
    ("A20_ADJCHRATE", "解禁后20日涨跌幅"),
    ("FREE_SHARES_TYPE", "限售股类型"),
];
const DETAIL_SELECT: [&str; 11] = [
    "股票代码",
    "股票简称",
    "解禁时间",
    "限售股类型",
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "占解禁前流通市值比例",
    "解禁前一交易日收盘价",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
];
const DETAIL_NUMERIC: [&str; 7] = [
    "解禁数量",
    "实际解禁数量",
    "实际解禁市值",
    "占解禁前流通市值比例",
    "解禁前一交易日收盘价",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
];
const DETAIL_DATE: [&str; 1] = ["解禁时间"];
const DETAIL_COLUMNS: &str = "SECURITY_CODE,SECURITY_NAME_ABBR,FREE_DATE,CURRENT_FREE_SHARES,ABLE_FREE_SHARES,LIFT_MARKET_CAP,FREE_RATIO,NEW,B20_ADJCHRATE,A20_ADJCHRATE,FREE_SHARES_TYPE,TOTAL_RATIO,NON_FREE_SHARES,BATCH_HOLDER_NUM";

/// 限售股解禁详情（对应 akshare [`akshare.stock_restricted_release_detail_em`]）。
///
/// `start_date` / `end_date`：区间 `YYYYMMDD`（默认 `"20221202"` / `"20241202"`）。
/// 报表 `RPT_LIFT_STAGE`，按解禁日、实际解禁数量降序。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 解禁时间, 限售股类型, 解禁数量, 实际解禁数量,
/// 实际解禁市值, 占解禁前流通市值比例, 解禁前一交易日收盘价, 解禁前20日涨跌幅,
/// 解禁后20日涨跌幅`
pub fn stock_restricted_release_detail_em(start_date: &str, end_date: &str) -> Result<Df> {
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!(r#"(FREE_DATE>='{sd}')(FREE_DATE<='{ed}')"#);
    let extra = report_extra(
        "FREE_DATE,CURRENT_FREE_SHARES",
        "1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_LIFT_STAGE", DETAIL_COLUMNS, &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &DETAIL_RENAME,
        &DETAIL_SELECT,
        &DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.scale("解禁数量", 10000.0)?
        .scale("实际解禁数量", 10000.0)?
        .scale("实际解禁市值", 10000.0)?
        .cast_date(&DETAIL_DATE)?;
    Ok(df)
}

// ============ 3. stock_restricted_release_queue_em ============

const QUEUE_RENAME: [(&str, &str); 12] = [
    ("FREE_DATE", "解禁时间"),
    ("CURRENT_FREE_SHARES", "实际解禁数量"),
    ("ABLE_FREE_SHARES", "解禁数量"),
    ("LIFT_MARKET_CAP", "实际解禁数量市值"),
    ("FREE_RATIO", "占流通市值比例"),
    ("NEW", "解禁前一交易日收盘价"),
    ("B20_ADJCHRATE", "解禁前20日涨跌幅"),
    ("A20_ADJCHRATE", "解禁后20日涨跌幅"),
    ("FREE_SHARES_TYPE", "限售股类型"),
    ("TOTAL_RATIO", "占总市值比例"),
    ("NON_FREE_SHARES", "未解禁数量"),
    ("BATCH_HOLDER_NUM", "解禁股东数"),
];
const QUEUE_SELECT: [&str; 12] = [
    "解禁时间",
    "解禁股东数",
    "解禁数量",
    "实际解禁数量",
    "未解禁数量",
    "实际解禁数量市值",
    "占总市值比例",
    "占流通市值比例",
    "解禁前一交易日收盘价",
    "限售股类型",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
];
const QUEUE_NUMERIC: [&str; 10] = [
    "解禁数量",
    "实际解禁数量",
    "未解禁数量",
    "实际解禁数量市值",
    "占总市值比例",
    "占流通市值比例",
    "解禁前一交易日收盘价",
    "解禁前20日涨跌幅",
    "解禁后20日涨跌幅",
    "解禁股东数",
];
const QUEUE_DATE: [&str; 1] = ["解禁时间"];

/// 个股限售股解禁批次（对应 akshare [`akshare.stock_restricted_release_queue_em`]）。
///
/// `symbol`：股票代码（默认 `"600000"`）。同一张 `RPT_LIFT_STAGE` 报表，按解禁日降序。
///
/// # 返回列
/// `序号, 解禁时间, 解禁股东数, 解禁数量, 实际解禁数量, 未解禁数量, 实际解禁数量市值,
/// 占总市值比例, 占流通市值比例, 解禁前一交易日收盘价, 限售股类型, 解禁前20日涨跌幅,
/// 解禁后20日涨跌幅`
pub fn stock_restricted_release_queue_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let extra = report_extra("FREE_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_LIFT_STAGE", DETAIL_COLUMNS, &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &QUEUE_RENAME,
        &QUEUE_SELECT,
        &QUEUE_NUMERIC,
        Some("序号"),
    )?;
    df.scale("解禁数量", 10000.0)?
        .scale("实际解禁数量", 10000.0)?
        .scale("未解禁数量", 10000.0)?
        .scale("实际解禁数量市值", 10000.0)?
        .cast_date(&QUEUE_DATE)?;
    Ok(df)
}

// ============ 4. stock_restricted_release_stockholder_em ============

const STOCKHOLDER_RENAME: [(&str, &str); 8] = [
    ("LIMITED_HOLDER_NAME", "股东名称"),
    ("ADD_LISTING_SHARES", "解禁数量"),
    ("ACTUAL_LISTED_SHARES", "实际解禁数量"),
    ("ADD_LISTING_CAP", "解禁市值"),
    ("LOCK_MONTH", "锁定期"),
    ("RESIDUAL_LIMITED_SHARES", "剩余未解禁数量"),
    ("FREE_SHARES_TYPE", "限售股类型"),
    ("PLAN_FEATURE", "进度"),
];
const STOCKHOLDER_SELECT: [&str; 8] = [
    "股东名称",
    "解禁数量",
    "实际解禁数量",
    "解禁市值",
    "锁定期",
    "剩余未解禁数量",
    "限售股类型",
    "进度",
];
const STOCKHOLDER_NUMERIC: [&str; 5] = [
    "解禁数量",
    "实际解禁数量",
    "解禁市值",
    "锁定期",
    "剩余未解禁数量",
];

/// 限售股解禁股东明细（对应 akshare [`akshare.stock_restricted_release_stockholder_em`]）。
///
/// `symbol`：股票代码（默认 `"600000"`）；`date`：解禁日 `YYYYMMDD`（默认 `"20200904"`）。
/// 报表 `RPT_LIFT_GD`，按解禁数量降序。
///
/// # 返回列
/// `序号, 股东名称, 解禁数量, 实际解禁数量, 解禁市值, 锁定期, 剩余未解禁数量,
/// 限售股类型, 进度`
pub fn stock_restricted_release_stockholder_em(symbol: &str, date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(r#"(SECURITY_CODE="{symbol}")(FREE_DATE='{d}')"#);
    let extra = report_extra("ADD_LISTING_SHARES", "-1", Some(&filter), None, None, None);
    let rows = datacenter(
        "RPT_LIFT_GD",
        "LIMITED_HOLDER_NAME,ADD_LISTING_SHARES,ACTUAL_LISTED_SHARES,ADD_LISTING_CAP,LOCK_MONTH,RESIDUAL_LIMITED_SHARES,FREE_SHARES_TYPE,PLAN_FEATURE",
        &extra,
        "500",
    )?;
    let df = finalize_report(
        &rows,
        &STOCKHOLDER_RENAME,
        &STOCKHOLDER_SELECT,
        &STOCKHOLDER_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ============ 5. 同花顺财务指标（旧系列 + 新系列，8 个） ============

/// 同花顺基础页 UA（与 akshare `stock_finance_ths.py` 的 `cons.headers` 一致）。
const THS_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/89.0.4389.90 Safari/537.36";

/// 股票所属市场代码（对应 akshare `__get_market_code`）。
/// 深市（000/001/002/003/300）→ 33；沪市（600/601/603/605/688）→ 17；
/// 北交所（920）→ 151；无法识别 → 0。
/// 代码不足 6 位报 `Param`（对应 akshare `raise "请输入正确的股票代码"`）。
fn market_code(symbol: &str) -> Result<i64> {
    if symbol.trim().len() < 6 {
        return Err(AkshareError::Param("请输入正确的股票代码".into()));
    }
    for p in ["000", "001", "002", "003", "300"] {
        if symbol.starts_with(p) {
            return Ok(33);
        }
    }
    for p in ["600", "601", "603", "605", "688"] {
        if symbol.starts_with(p) {
            return Ok(17);
        }
    }
    if symbol.starts_with("920") {
        return Ok(151);
    }
    Ok(0)
}

/// 单元格转字符串，对齐 pandas `str()` 语义：布尔大写（`True`/`False`）、
/// 数值走 `to_string`、空值为 `None`。
fn pandas_cell(v: Option<&Value>) -> Option<String> {
    match v {
        None => None,
        Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(if *b { "True".into() } else { "False".into() }),
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => Some(other.to_string()),
    }
}

/// 旧系列核心解析（`abstract`/`debt`/`benefit`/`cash` 共用）。
///
/// 对应 akshare 的 `title→df_index`（list 取首元素）+ 按 indicator 选
/// `report`/`simple`/`year` 数组 + 转置 + `reset_index` 重命名 `报告期` 变换；
/// 转置后行 = 报告期、列 = 指标名。`do_sort=true`（abstract）按 `报告期`
/// 字符串升序（ISO 日期字符串序 = 时间序）。
fn parse_old_finance(json: &Value, indicator: &str, do_sort: bool) -> Result<Df> {
    let title = json.get("title").and_then(Value::as_array).ok_or_else(|| {
        AkshareError::Empty("同花顺财务旧系列缺 title".into())
    })?;
    let df_index: Vec<String> = title
        .iter()
        .map(|item| match item {
            Value::Array(a) => a
                .first()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            other => other.as_str().unwrap_or_default().to_string(),
        })
        .collect();
    let key = match indicator {
        "按单季度" => "simple",
        "按年度" => "year",
        _ => "report",
    };
    let arr = json
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty(format!("同花顺财务旧系列缺 {key} 数据")))?;
    // arr[0] = 报告期（列头），arr[1..] = 各指标的逐期值
    let dates: Vec<Option<String>> = arr
        .first()
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|v| pandas_cell(Some(v))).collect())
        .unwrap_or_default();
    if dates.is_empty() {
        return Df::from_string_rows(&["报告期"], &[]);
    }
    let mut col_names: Vec<&str> = Vec::with_capacity(df_index.len());
    col_names.push("报告期");
    for item in &df_index[1..] {
        col_names.push(item);
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(dates.len());
    // 指标列是否所有期都是 JSON 数字（对应 pandas 推断：全数字列 → int64/float64，
    // 任一单元格为字符串（含数字字符串，如 每股净资产 新旧报告期混用）→ object）
    let mut col_all_number: Vec<bool> = vec![true; arr.len().saturating_sub(1)];
    for (i, date) in dates.iter().enumerate() {
        let mut row = vec![date.clone()];
        for (m, mrow) in arr.iter().skip(1).enumerate() {
            let cell = mrow.as_array().and_then(|c| c.get(i));
            if !matches!(cell, Some(Value::Number(_))) {
                col_all_number[m] = false;
            }
            row.push(cell.and_then(|v| pandas_cell(Some(v))));
        }
        rows.push(row);
    }
    let mut df = Df::from_string_rows(&col_names, &rows)?;
    let numeric_cols: Vec<&str> = col_names[1..]
        .iter()
        .zip(col_all_number.iter())
        .filter(|(_, all_num)| **all_num)
        .map(|(c, _)| *c)
        .collect();
    if !numeric_cols.is_empty() {
        df.cast_numeric(&numeric_cols)?;
    }
    if do_sort {
        df = df.sort_by("报告期", true, false)?;
    }
    Ok(df)
}

/// 同花顺-财务指标-主要指标（对应 akshare [`akshare.stock_financial_abstract_ths`]）。
///
/// `symbol`：股票代码（默认 `"000063"`）；`indicator`：`按报告期` / `按单季度` / `按年度`。
/// 数据源 `https://basic.10jqka.com.cn/new/{symbol}/finance.html`（`<p id="main">` 内嵌 JSON）。
///
/// # 返回列
/// `报告期, <各财务指标列>`（`报告期` 升序）
pub fn stock_financial_abstract_ths(symbol: &str, indicator: &str) -> Result<Df> {
    let url = format!("https://basic.10jqka.com.cn/new/{symbol}/finance.html");
    let http = HttpClient::default();
    let text = http.get_text_with_headers(&url, &Map::new(), &[("User-Agent", THS_UA)], None)?;
    // 提取 `<p id="main">` 内嵌 JSON（对应 akshare `soup.find("p", {"id": "main"}).string`）
    let p_sel = Selector::parse("p#main")
        .map_err(|e| AkshareError::Empty(format!("解析选择器失败: {e}")))?;
    let doc = Html::parse_document(&text);
    let json_text = doc
        .select(&p_sel)
        .next()
        .map(|p| p.text().collect::<String>())
        .ok_or_else(|| AkshareError::Empty("同花顺财务页缺 p#main 数据".into()))?;
    let json: Value =
        serde_json::from_str(&json_text).map_err(|e| AkshareError::json(&url, e.to_string()))?;
    parse_old_finance(&json, indicator, true)
}

/// 同花顺-财务指标-资产负债表（对应 akshare [`akshare.stock_financial_debt_ths`]）。
///
/// `symbol`：股票代码；`indicator`：`按报告期` / `按年度`。数据源
/// `https://basic.10jqka.com.cn/api/stock/finance/{symbol}_debt.json`（`flashData` 双重 JSON）。
///
/// # 返回列
/// `报告期, <各财务指标列>`
pub fn stock_financial_debt_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_old_finance(symbol, "debt", indicator)
}

/// 同花顺-财务指标-利润表（对应 akshare [`akshare.stock_financial_benefit_ths`]）。
///
/// `symbol`：股票代码；`indicator`：`按报告期` / `按单季度` / `按年度`。数据源
/// `https://basic.10jqka.com.cn/api/stock/finance/{symbol}_benefit.json`。
///
/// # 返回列
/// `报告期, <各财务指标列>`
pub fn stock_financial_benefit_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_old_finance(symbol, "benefit", indicator)
}

/// 同花顺-财务指标-现金流量表（对应 akshare [`akshare.stock_financial_cash_ths`]）。
///
/// `symbol`：股票代码；`indicator`：`按报告期` / `按单季度` / `按年度`。数据源
/// `https://basic.10jqka.com.cn/api/stock/finance/{symbol}_cash.json`。
///
/// # 返回列
/// `报告期, <各财务指标列>`
pub fn stock_financial_cash_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_old_finance(symbol, "cash", indicator)
}

/// 旧系列 API 型函数公共实现（`{symbol}_{kind}.json` → `flashData` 双重 JSON 解析）。
fn fetch_old_finance(symbol: &str, kind: &str, indicator: &str) -> Result<Df> {
    let url = format!("https://basic.10jqka.com.cn/api/stock/finance/{symbol}_{kind}.json");
    let http = HttpClient::default();
    let outer = http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", THS_UA)], None)?;
    let flash = outer
        .get("flashData")
        .and_then(Value::as_str)
        .ok_or_else(|| AkshareError::Empty("同花顺财务接口缺 flashData".into()))?;
    // flashData 是 JSON 字符串，需二次解析（对应 akshare `json.loads(json.loads(r.text)["flashData"])`）
    let inner: Value =
        serde_json::from_str(flash).map_err(|e| AkshareError::json(&url, e.to_string()))?;
    parse_old_finance(&inner, indicator, false)
}

/// 新系列公共解析（`*_new_ths` 共用）：`app_data` 报表展平。
///
/// 对应 akshare：遍历 `data.data` 每个报告期的 `index_list`，逐指标展平为
/// `report_date/report_name/report_period/quarter_name/metric_name` + 动态字段
/// （`value`/`single`/`yoy`/`mom`/`single_yoy`）的宽表；列序 = 各记录键首现顺序。
/// 新版 pandas 对全字符串列推断 StringDtype（非 object），akshare 的数值化逻辑
/// 被跳过，故输出保持字符串列（实测 000063 各列 dtype 均为 str）。
fn parse_new_finance(json: &Value) -> Result<Df> {
    let Some(reports) = json
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(Value::as_array)
    else {
        return Df::from_string_rows(&[], &[]);
    };
    if reports.is_empty() {
        return Df::from_string_rows(&[], &[]);
    }
    let mut col_order: Vec<String> = Vec::new();
    let mut records: Vec<Vec<(String, Option<String>)>> = Vec::new();
    for rep in reports {
        let base: [(&str, Option<String>); 4] = [
            ("report_date", rep.get("date").and_then(cell)),
            ("report_name", rep.get("report_name").and_then(cell)),
            ("report_period", rep.get("report").and_then(cell)),
            ("quarter_name", rep.get("quarter_name").and_then(cell)),
        ];
        let index_list = rep
            .get("index_list")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (metric_name, metric_values) in index_list {
            let mut rec: Vec<(String, Option<String>)> = base
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            rec.push(("metric_name".into(), Some(metric_name.clone())));
            match metric_values {
                Value::Object(fields) => {
                    for (k, v) in fields {
                        rec.push((k.clone(), cell(&v)));
                    }
                }
                other => rec.push(("value".into(), cell(&other))),
            }
            for (k, _) in &rec {
                if !col_order.contains(k) {
                    col_order.push(k.clone());
                }
            }
            records.push(rec);
        }
    }
    let col_refs: Vec<&str> = col_order.iter().map(String::as_str).collect();
    let rows: Vec<Vec<Option<String>>> = records
        .iter()
        .map(|rec| {
            col_order
                .iter()
                .map(|k| {
                    rec.iter()
                        .find(|(rk, _)| rk == k)
                        .and_then(|(_, v)| v.clone())
                })
                .collect()
        })
        .collect();
    Df::from_string_rows(&col_refs, &rows)
}

/// 新系列请求 + 解析（`id` 区分 重要指标/资产负债表/利润表/现金流量表）。
fn fetch_new_finance(symbol: &str, indicator: &str, id: &str, periods: &[(&str, &str)]) -> Result<Df> {
    let period = periods
        .iter()
        .find(|(k, _)| *k == indicator)
        .map(|(_, v)| *v)
        .unwrap_or(periods.last().map(|(_, v)| *v).unwrap_or("4"));
    let url = "https://basic.10jqka.com.cn/basicapi/finance/index/v1/app_data/";
    let mut params = Map::new();
    params.insert("code".into(), Value::String(symbol.into()));
    params.insert("id".into(), Value::String(id.into()));
    params.insert("market".into(), Value::from(market_code(symbol)?));
    params.insert("type".into(), Value::String("stock".into()));
    params.insert("page".into(), Value::String("1".into()));
    params.insert("size".into(), Value::String("50".into()));
    params.insert("period".into(), Value::String(period.into()));
    let http = HttpClient::default();
    let json = http.get_json_with_headers(url, &params, &[("User-Agent", THS_UA)], None)?;
    parse_new_finance(&json)
}

/// 同花顺-财务指标-重要指标（新，对应 akshare [`akshare.stock_financial_abstract_new_ths`]）。
///
/// `symbol`：股票代码；`indicator`：`按报告期` / `一季度` / `二季度` / `三季度` /
/// `四季度` / `按年度`。数据源 `basicapi/finance/index/v1/app_data/`（`id=client_stock_importance`）。
///
/// # 返回列
/// `report_date, report_name, report_period, quarter_name, metric_name, value, single, yoy, mom, single_yoy`
pub fn stock_financial_abstract_new_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_new_finance(
        symbol,
        indicator,
        "client_stock_importance",
        &[
            ("按报告期", "0"),
            ("一季度", "1"),
            ("二季度", "2"),
            ("三季度", "3"),
            ("四季度", "4"),
            ("按年度", "4"),
        ],
    )
}

/// 同花顺-财务指标-资产负债表（新，对应 akshare [`akshare.stock_financial_debt_new_ths`]）。
///
/// `symbol`：股票代码；`indicator`：`按报告期` / `按年度`。数据源同 [`stock_financial_abstract_new_ths`]
/// （`id=client_stock_debt`）。
///
/// # 返回列
/// `report_date, report_name, report_period, quarter_name, metric_name, value, single, yoy, mom, single_yoy`
pub fn stock_financial_debt_new_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_new_finance(
        symbol,
        indicator,
        "client_stock_debt",
        &[("按报告期", "0"), ("按年度", "4")],
    )
}

/// 同花顺-财务指标-利润表（新，对应 akshare [`akshare.stock_financial_benefit_new_ths`]）。
///
/// `symbol`：股票代码；`indicator`：同 [`stock_financial_abstract_new_ths`]
/// （`id=client_stock_benefit`）。
///
/// # 返回列
/// `report_date, report_name, report_period, quarter_name, metric_name, value, single, yoy, mom, single_yoy`
pub fn stock_financial_benefit_new_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_new_finance(
        symbol,
        indicator,
        "client_stock_benefit",
        &[
            ("按报告期", "0"),
            ("一季度", "1"),
            ("二季度", "2"),
            ("三季度", "3"),
            ("四季度", "4"),
            ("按年度", "4"),
        ],
    )
}

/// 同花顺-财务指标-现金流量表（新，对应 akshare [`akshare.stock_financial_cash_new_ths`]）。
///
/// `symbol`：股票代码；`indicator`：同 [`stock_financial_abstract_new_ths`]
/// （`id=client_stock_cash`）。
///
/// # 返回列
/// `report_date, report_name, report_period, quarter_name, metric_name, value, single, yoy, mom, single_yoy`
pub fn stock_financial_cash_new_ths(symbol: &str, indicator: &str) -> Result<Df> {
    fetch_new_finance(
        symbol,
        indicator,
        "client_stock_cash",
        &[
            ("按报告期", "0"),
            ("一季度", "1"),
            ("二季度", "2"),
            ("三季度", "3"),
            ("四季度", "4"),
            ("按年度", "4"),
        ],
    )
}

/// JSON 值 → Option<String>（数值走 `to_string`，与 pandas 逐单元格 str 一致）。
fn cell(v: &Value) -> Option<String> {
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

    /// 抽取列名（与 parity export_parity 同口径），用于断言列契约顺序。
    fn col_names(df: &Df) -> Vec<String> {
        df.export_parity(0)["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn summary_offline_contract() {
        let rows = vec![json!({
            "FREE_DATE": "2022-11-01 00:00:00",
            "LIFT_ORG_NUM": 3,
            "LIFT_NUM": 123456789,
            "MARKET_CAP": 234567890,
            "INDEX_PRICE": 3500.12,
            "CHANGE_RATE": 1.23,
            "PLAN_LIFT_NUM": -0.45,
        })];
        let mut df = finalize_report(
            &rows,
            &SUMMARY_RENAME,
            &SUMMARY_SELECT,
            &SUMMARY_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.scale("解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量", 10000.0).unwrap();
        df.scale("实际解禁市值", 10000.0).unwrap();
        df.cast_date(&SUMMARY_DATE).unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "解禁时间",
                "当日解禁股票家数",
                "解禁数量",
                "实际解禁数量",
                "实际解禁市值",
                "沪深300指数",
                "沪深300指数涨跌幅",
            ]
        );
        // 序号 1 起始
        let idx = df.inner().column("序号").unwrap().f64().unwrap().get(0);
        assert_eq!(idx, Some(1.0));
        // 日期截断为 YYYY-MM-DD
        let d = df.inner().column("解禁时间").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2022-11-01"));
        // 数量列 ÷10000
        let n = df
            .inner()
            .column("解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(n, 12345.6789));
        let m = df
            .inner()
            .column("实际解禁市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(m, 0.350012));
        // 指数类列不缩放
        let i = df
            .inner()
            .column("沪深300指数")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(i, 1.23));
    }

    #[test]
    fn detail_offline_contract() {
        let rows = vec![json!({
            "SECURITY_CODE": "600000",
            "SECURITY_NAME_ABBR": "浦发银行",
            "FREE_DATE": "2022-12-02 00:00:00",
            "CURRENT_FREE_SHARES": 100000000,
            "ABLE_FREE_SHARES": 200000000,
            "LIFT_MARKET_CAP": 300000000,
            "FREE_RATIO": 12.5,
            "NEW": 7.5,
            "B20_ADJCHRATE": 3.2,
            "A20_ADJCHRATE": -2.1,
            "FREE_SHARES_TYPE": "首发原股东限售股份",
        })];
        let mut df = finalize_report(
            &rows,
            &DETAIL_RENAME,
            &DETAIL_SELECT,
            &DETAIL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.scale("解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量", 10000.0).unwrap();
        df.scale("实际解禁市值", 10000.0).unwrap();
        df.cast_date(&DETAIL_DATE).unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "股票代码",
                "股票简称",
                "解禁时间",
                "限售股类型",
                "解禁数量",
                "实际解禁数量",
                "实际解禁市值",
                "占解禁前流通市值比例",
                "解禁前一交易日收盘价",
                "解禁前20日涨跌幅",
                "解禁后20日涨跌幅",
            ]
        );
        let d = df.inner().column("解禁时间").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2022-12-02"));
        let qty = df
            .inner()
            .column("解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(qty, 20000.0));
        let actual = df
            .inner()
            .column("实际解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(actual, 10000.0));
        let cap = df
            .inner()
            .column("实际解禁市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(cap, 30000.0));
    }

    #[test]
    fn queue_offline_contract() {
        let rows = vec![json!({
            "FREE_DATE": "2022-12-02 00:00:00",
            "CURRENT_FREE_SHARES": 100000000,
            "ABLE_FREE_SHARES": 200000000,
            "LIFT_MARKET_CAP": 300000000,
            "FREE_RATIO": 12.5,
            "NEW": 7.5,
            "B20_ADJCHRATE": 3.2,
            "A20_ADJCHRATE": -2.1,
            "FREE_SHARES_TYPE": "首发原股东限售股份",
            "TOTAL_RATIO": 5.5,
            "NON_FREE_SHARES": 400000000,
            "BATCH_HOLDER_NUM": 8,
        })];
        let mut df = finalize_report(
            &rows,
            &QUEUE_RENAME,
            &QUEUE_SELECT,
            &QUEUE_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.scale("解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量", 10000.0).unwrap();
        df.scale("未解禁数量", 10000.0).unwrap();
        df.scale("实际解禁数量市值", 10000.0).unwrap();
        df.cast_date(&QUEUE_DATE).unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "解禁时间",
                "解禁股东数",
                "解禁数量",
                "实际解禁数量",
                "未解禁数量",
                "实际解禁数量市值",
                "占总市值比例",
                "占流通市值比例",
                "解禁前一交易日收盘价",
                "限售股类型",
                "解禁前20日涨跌幅",
                "解禁后20日涨跌幅",
            ]
        );
        let d = df.inner().column("解禁时间").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2022-12-02"));
        let holders = df
            .inner()
            .column("解禁股东数")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(holders, 8.0));
        let total = df
            .inner()
            .column("实际解禁数量市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(total, 30000.0));
        let residual = df
            .inner()
            .column("未解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(residual, 40000.0));
    }

    #[test]
    fn market_code_mapping() {
        assert_eq!(market_code("000063").unwrap(), 33);
        assert_eq!(market_code("300750").unwrap(), 33);
        assert_eq!(market_code("600519").unwrap(), 17);
        assert_eq!(market_code("688981").unwrap(), 17);
        assert_eq!(market_code("920001").unwrap(), 151);
        assert_eq!(market_code("123456").unwrap(), 0);
        // 代码不足 6 位报 Param（对应 akshare raise "请输入正确的股票代码"）
        assert!(matches!(
            market_code("123"),
            Err(AkshareError::Param(_))
        ));
    }

    #[test]
    fn old_finance_transpose_and_sort() {
        // 模拟 abstract 的 `<p id="main">` JSON：title 首元素为纯字符串，其余为数组
        let json = serde_json::json!({
            "title": ["科目\\时间", ["净利润", "元", 0, false, true], ["净利润同比增长率", "", 0, false, true]],
            "report": [
                ["2026-03-31", "2025-12-31", "2025-09-30"],
                ["13.10亿", "56.18亿", "53.22亿"],
                [false, false, true]
            ],
            "year": [["2025-12-31"], ["56.18亿"], [false]]
        });
        let df = parse_old_finance(&json, "按报告期", true).unwrap();
        assert_eq!(df.column_names(), vec!["报告期", "净利润", "净利润同比增长率"]);
        // do_sort=true：报告期升序（ISO 字符串序 = 时间序）
        let d = df.inner().column("报告期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2025-09-30"));
        assert_eq!(d.get(2), Some("2026-03-31"));
        // 转置随排序联动：2025-09-30（原数组末尾）排到第 0 行
        let profit = df.inner().column("净利润").unwrap().str().unwrap();
        assert_eq!(profit.get(0), Some("53.22亿"));
        assert_eq!(profit.get(2), Some("13.10亿"));
        // 布尔 → 大写（对应 pandas str(True/False)）
        let yoy = df.inner().column("净利润同比增长率").unwrap().str().unwrap();
        assert_eq!(yoy.get(0), Some("True"));
        assert_eq!(yoy.get(2), Some("False"));
        // 含单位/布尔列保持 str
        assert_eq!(
            df.inner().column("净利润").unwrap().dtype().to_string(),
            "str"
        );
    }

    #[test]
    fn old_finance_numeric_column_cast() {
        let json = serde_json::json!({
            "title": ["科目", "每股收益", "现金"],
            "report": [
                ["2026-03-31", "2025-12-31"],
                [1.23, 2.34],
                ["1000", "2000"]
            ]
        });
        let df = parse_old_finance(&json, "按报告期", false).unwrap();
        assert_eq!(df.column_names(), vec!["报告期", "每股收益", "现金"]);
        // 全 JSON 数字列 → float64（对应 pandas 推断）
        assert_eq!(
            df.inner().column("每股收益").unwrap().dtype().to_string(),
            "f64"
        );
        let eps = df.inner().column("每股收益").unwrap().f64().unwrap();
        assert_eq!(eps.get(0), Some(1.23));
        // JSON 字符串列（即使内容为数字）→ 保持 str（对应 pandas object）
        assert_eq!(
            df.inner().column("现金").unwrap().dtype().to_string(),
            "str"
        );
        let cash = df.inner().column("现金").unwrap().str().unwrap();
        assert_eq!(cash.get(0), Some("1000"));
    }

    #[test]
    fn new_finance_flatten_and_column_order() {
        let json = serde_json::json!({
            "data": {"data": [
                {
                    "date": "2026-03-31",
                    "report_name": "2026一季报",
                    "report": "2026-1",
                    "quarter_name": "2026一季度",
                    "index_list": {
                        "index_per_operating_cash_flow_net": {"value": "-0.4136", "yoy": "-2.07"},
                        "profit_total": "1502233000.0000"
                    }
                },
                {
                    "date": "2025-12-31",
                    "report_name": "2025年报",
                    "report": "2025-4",
                    "quarter_name": "2025四季度",
                    "index_list": {
                        "index_per_operating_cash_flow_net": {"value": "0.5", "yoy": null}
                    }
                }
            ]}
        });
        let df = parse_new_finance(&json).unwrap();
        assert_eq!(
            df.column_names(),
            vec![
                "report_date",
                "report_name",
                "report_period",
                "quarter_name",
                "metric_name",
                "value",
                "yoy"
            ]
        );
        assert_eq!(df.height(), 3);
        let metric = df.inner().column("metric_name").unwrap().str().unwrap();
        assert_eq!(metric.get(0), Some("index_per_operating_cash_flow_net"));
        assert_eq!(metric.get(1), Some("profit_total"));
        // 标量指标 → value 列
        let v = df.inner().column("value").unwrap().str().unwrap();
        assert_eq!(v.get(1), Some("1502233000.0000"));
        assert_eq!(v.get(2), Some("0.5"));
        // 缺失字段（profit_total 无 yoy）→ 空
        let yoy = df.inner().column("yoy").unwrap().str().unwrap();
        assert!(yoy.get(1).is_none());
    }

    #[test]
    fn new_finance_empty_reports() {
        assert_eq!(
            parse_new_finance(&serde_json::json!({"data": {"data": []}}))
                .unwrap()
                .height(),
            0
        );
        assert_eq!(
            parse_new_finance(&serde_json::json!({"data": {}}))
                .unwrap()
                .height(),
            0
        );
    }

    #[test]
    fn stockholder_offline_contract() {
        let rows = vec![json!({
            "LIMITED_HOLDER_NAME": "张三",
            "ADD_LISTING_SHARES": 100000,
            "ACTUAL_LISTED_SHARES": 90000,
            "ADD_LISTING_CAP": 200000,
            "LOCK_MONTH": 12,
            "RESIDUAL_LIMITED_SHARES": 50000,
            "FREE_SHARES_TYPE": "首发原股东限售股份",
            "PLAN_FEATURE": "已实施",
        })];
        let df = finalize_report(
            &rows,
            &STOCKHOLDER_RENAME,
            &STOCKHOLDER_SELECT,
            &STOCKHOLDER_NUMERIC,
            Some("序号"),
        )
        .unwrap();

        assert_eq!(
            col_names(&df),
            vec![
                "序号",
                "股东名称",
                "解禁数量",
                "实际解禁数量",
                "解禁市值",
                "锁定期",
                "剩余未解禁数量",
                "限售股类型",
                "进度",
            ]
        );
        // 无日期列、无缩放：数值保持原值
        let qty = df
            .inner()
            .column("解禁数量")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(qty, 100000.0));
        let cap = df
            .inner()
            .column("解禁市值")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(cap, 200000.0));
        let lock = df
            .inner()
            .column("锁定期")
            .unwrap()
            .f64()
            .unwrap()
            .get(0)
            .unwrap();
        assert!(approx(lock, 12.0));
    }
}
