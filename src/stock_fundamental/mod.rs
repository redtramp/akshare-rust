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

// ============ 6. 同花顺-盈利预测/公司大事（4 个） ============

/// 同花顺-盈利预测（对应 akshare [`akshare.stock_profit_forecast_ths`]）。
///
/// `symbol`：股票代码；`indicator`：
/// `预测年报每股收益 / 预测年报净利润 / 业绩预测详表-机构 / 业绩预测详表-详细指标预测`。
/// 页面为 GBK 编码的 `worth.html`；当页面含「本年度暂无机构做出业绩预测」时
/// 前两个 indicator 返回空表，后两个改读前两张表（与 akshare 分支一致）。
///
/// # 返回列
/// - 前两 indicator：`年度, 预测机构数, 最小值, 均值, 最大值, 行业平均数`
/// - 机构详表：`机构名称, 研究员, 预测年报每股收益{年}预测 ×N, 预测年报净利润{年}预测 ×N, 报告日期`
/// - 详细指标：`预测指标, {年}-实际值 ×M, 预测{年}-平均 ×K`
pub fn stock_profit_forecast_ths(symbol: &str, indicator: &str) -> Result<Df> {
    let url = format!("https://basic.10jqka.com.cn/new/{symbol}/worth.html");
    let html = crate::sources::ths::fetch_ths(&url)?;
    let no_forecast = html.contains("本年度暂无机构做出业绩预测");
    match indicator {
        "预测年报每股收益" => {
            if no_forecast {
                return Df::from_string_rows(
                    &["年度", "预测机构数", "最小值", "均值", "最大值", "行业平均数"],
                    &[],
                );
            }
            pf_eps_net(&html, 0)
        }
        "预测年报净利润" => {
            if no_forecast {
                return Df::from_string_rows(
                    &["年度", "预测机构数", "最小值", "均值", "最大值", "行业平均数"],
                    &[],
                );
            }
            pf_eps_net(&html, 1)
        }
        "业绩预测详表-机构" => {
            // 有预测读第 2 张表，无预测读第 0 张（akshare 分支）
            pf_org(&html, if no_forecast { 0 } else { 2 })
        }
        "业绩预测详表-详细指标预测" => {
            let idx = if no_forecast { 1 } else { 3 };
            pf_detail_idx(&html, idx)
        }
        _ => Err(AkshareError::Param(format!(
            "未知 indicator: {indicator}（可选：预测年报每股收益/预测年报净利润/业绩预测详表-机构/业绩预测详表-详细指标预测）"
        ))),
    }
}

/// 每股收益/净利润年度汇总表（第 idx 张表）：`年度, 预测机构数, 最小值, 均值, 最大值, 行业平均数`。
fn pf_eps_net(html: &str, idx: usize) -> Result<Df> {
    let (headers, rows) = parse_table_nth(html, idx)?;
    let cols: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
    let string_rows: Vec<Vec<Option<String>>> = rows
        .iter()
        .map(|r| (0..cols.len()).map(|i| r.get(i).cloned()).collect())
        .collect();
    let mut df = Df::from_string_rows(&cols, &string_rows)?;
    // 年度保持字符串（akshare astype(str)），其余数值化
    let numeric: Vec<&str> = cols
        .iter()
        .copied()
        .filter(|c| *c != "年度")
        .collect();
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 机构详表：两级表头展开为 9 列 + 前缀拼接（对应 akshare MultiIndex `item[1]` 处理）。
///
/// 页面表头结构：
/// ```text
/// 机构名称(rowspan2) | 研究员(rowspan2) | 预测年报每股收益（元）(colspan3) | 预测年报净利润（元）(colspan3) | 报告日期(rowspan2)
///                   |                 | 2026预测 2027预测 2028预测 | 2026预测 2027预测 2028预测 |
/// ```
/// pandas 展开为 9 列 MultiIndex，akshare 取第 2 层再对每股收益/净利润列加前缀。
pub(crate) fn pf_org(html: &str, idx: usize) -> Result<Df> {
    // 解析 thead 两行单元格（含 colspan/rowspan）
    let cells = parse_thead_rows(html, idx)?;
    let col_count = cells.iter().map(|r| r.iter().map(|c| c.1).sum::<usize>()).max().unwrap_or(0);
    if col_count == 0 {
        return Err(AkshareError::Empty("机构详表无表头".into()));
    }
    // level0：第一行按 colspan 展开
    let mut level0: Vec<&str> = Vec::with_capacity(col_count);
    for (text, span) in &cells[0] {
        for _ in 0..*span {
            level0.push(text);
        }
    }
    // level1：第二行补位；rowspan=2 的列（机构名称/研究员/报告日期）沿用 level0
    let mut level1: Vec<&str> = level0.clone();
    if let Some(row1) = cells.get(1) {
        let mut pos = 0;
        for (text, span) in row1 {
            for _ in 0..*span {
                if pos < level1.len() {
                    level1[pos] = text;
                }
                pos += 1;
            }
        }
    }
    // 列名：前 2 列与末列取 level1，中间按 level0 分组加前缀
    let mut cols: Vec<String> = Vec::with_capacity(col_count);
    for (i, l0) in level0.iter().enumerate() {
        if i < 2 || i + 1 == col_count {
            cols.push(level1[i].to_string());
        } else if l0.contains("每股收益") {
            cols.push(format!("预测年报每股收益{}", level1[i]));
        } else {
            cols.push(format!("预测年报净利润{}", level1[i]));
        }
    }
    let cols_ref: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
    let string_rows: Vec<Vec<Option<String>>> = parse_table_rows(html, idx, col_count)?
        .iter()
        .map(|r| (0..col_count).map(|i| r.get(i).cloned()).collect())
        .collect();
    let mut df = Df::from_string_rows(&cols_ref, &string_rows)?;
    df.cast_date(&["报告日期"])?;
    // 每股收益三列数值化（akshare 中为 float64；净利润带「亿」单位保持字符串）
    let eps_cols: Vec<&str> = cols_ref
        .iter()
        .copied()
        .filter(|c| c.starts_with("预测年报每股收益"))
        .collect();
    if !eps_cols.is_empty() {
        df.cast_numeric(&eps_cols)?;
    }
    Ok(df)
}

/// 解析第 `idx` 张表 thead 的单元格：`(文本, 展开列数)`，colspan 缺失按 1。
fn parse_thead_rows(html: &str, idx: usize) -> Result<Vec<Vec<(String, usize)>>> {
    let table_sel = Selector::parse("table")
        .map_err(|e| AkshareError::js(format!("解析 table 选择器失败: {e}")))?;
    let tr_sel = Selector::parse("tr")
        .map_err(|e| AkshareError::js(format!("解析 tr 选择器失败: {e}")))?;
    let cell_sel = Selector::parse("th, td")
        .map_err(|e| AkshareError::js(format!("解析 th/td 选择器失败: {e}")))?;
    let doc = Html::parse_document(html);
    let table = doc
        .select(&table_sel)
        .nth(idx)
        .ok_or_else(|| AkshareError::Empty(format!("第 {idx} 张表不存在")))?;
    let mut out = Vec::new();
    for tr in table.select(&tr_sel) {
        let mut row = Vec::new();
        for cell in tr.select(&cell_sel) {
            let span = cell
                .value()
                .attr("colspan")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);
            row.push((collapse_text(&cell), span));
        }
        if !row.is_empty() {
            out.push(row);
        }
    }
    Ok(out)
}

/// 解析第 `idx` 张表 tbody 数据行（每行取前 `width` 个 td）。
fn parse_table_rows(html: &str, idx: usize, width: usize) -> Result<Vec<Vec<String>>> {
    let table_sel = Selector::parse("table")
        .map_err(|e| AkshareError::js(format!("解析 table 选择器失败: {e}")))?;
    let tbody_sel = Selector::parse("tbody")
        .map_err(|e| AkshareError::js(format!("解析 tbody 选择器失败: {e}")))?;
    let tr_sel = Selector::parse("tr")
        .map_err(|e| AkshareError::js(format!("解析 tr 选择器失败: {e}")))?;
    let td_sel = Selector::parse("td")
        .map_err(|e| AkshareError::js(format!("解析 td 选择器失败: {e}")))?;
    let doc = Html::parse_document(html);
    let table = doc
        .select(&table_sel)
        .nth(idx)
        .ok_or_else(|| AkshareError::Empty(format!("第 {idx} 张表不存在")))?;
    let mut out = Vec::new();
    if let Some(tbody) = table.select(&tbody_sel).next() {
        for tr in tbody.select(&tr_sel) {
            let cells: Vec<String> = tr
                .select(&td_sel)
                .map(|td| collapse_text(&td))
                .collect();
            if !cells.is_empty() {
                out.push(cells);
            }
        }
    }
    let _ = width;
    Ok(out)
}

/// 详细指标预测表：列名 `（实际值）/（平均）` → `-实际值/-平均`（对应 akshare replace）。
fn pf_detail_idx(html: &str, idx: usize) -> Result<Df> {
    let (headers, rows) = parse_table_nth(html, idx)?;
    let headers: Vec<String> = headers
        .iter()
        .map(|h| h.replace('（', "-").replace('）', ""))
        .collect();
    let cols: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
    let string_rows: Vec<Vec<Option<String>>> = rows
        .iter()
        .map(|r| (0..cols.len()).map(|i| r.get(i).cloned()).collect())
        .collect();
    Df::from_string_rows(&cols, &string_rows)
}

/// 取第 `idx` 张 `<table>` 的 thead/tbody 内容。
pub(crate) fn parse_table_nth(html: &str, idx: usize) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let table_sel = Selector::parse("table")
        .map_err(|e| AkshareError::js(format!("解析 table 选择器失败: {e}")))?;
    let doc = Html::parse_document(html);
    let table = doc
        .select(&table_sel)
        .nth(idx)
        .ok_or_else(|| AkshareError::Empty(format!("第 {idx} 张表不存在")))?;
    let thead_sel = Selector::parse("thead")
        .map_err(|e| AkshareError::js(format!("解析 thead 选择器失败: {e}")))?;
    let tbody_sel = Selector::parse("tbody")
        .map_err(|e| AkshareError::js(format!("解析 tbody 选择器失败: {e}")))?;
    let tr_sel = Selector::parse("tr")
        .map_err(|e| AkshareError::js(format!("解析 tr 选择器失败: {e}")))?;
    let th_sel = Selector::parse("th")
        .map_err(|e| AkshareError::js(format!("解析 th 选择器失败: {e}")))?;
    let td_sel = Selector::parse("td")
        .map_err(|e| AkshareError::js(format!("解析 td 选择器失败: {e}")))?;

    // 表头：所有 thead tr 的 th 文本（机构详表有两行，pandas 展开 MultiIndex）
    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    if let Some(thead) = table.select(&thead_sel).next() {
        for tr in thead.select(&tr_sel) {
            let ths: Vec<String> = tr
                .select(&th_sel)
                .map(|th| collapse_text(&th))
                .collect();
            if !ths.is_empty() {
                headers.extend(ths);
            }
        }
    }
    if let Some(tbody) = table.select(&tbody_sel).next() {
        for tr in tbody.select(&tr_sel) {
            let cells: Vec<String> = tr
                .select(&td_sel)
                .map(|td| collapse_text(&td))
                .collect();
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
    }
    Ok((headers, rows))
}

/// 折叠元素文本：逐文本节点 trim 后拼接（等价于 bs4 `get_text(strip=True)`）。
pub(crate) fn collapse_text(el: &scraper::ElementRef<'_>) -> String {
    el.text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<String>()
}

/// 同花顺-公司大事-高管持股变动（对应 akshare [`akshare.stock_management_change_ths`]）。
///
/// `symbol`：股票代码；页面 `event.html`（GB2312）。表格行内单元格含隐藏
/// 节点，akshare 以「thead 文本 → 表头、tbody 文本 → 数据」整体切分重建，
/// 此处等价：表头取 thead th 文本，数据行取 tbody 每行 th+td 文本（去空格）。
///
/// # 返回列
/// `变动日期, 变动人, 与公司高管关系, 变动数量, 交易均价, 剩余股数, 股份变动途径`
pub fn stock_management_change_ths(symbol: &str) -> Result<Df> {
    let url = format!("https://basic.10jqka.com.cn/new/{symbol}/event.html");
    let html = crate::sources::ths::fetch_ths(&url)?;
    let (headers, rows) = crate::sources::ths::parse_ths_theaded_table_sel(
        &html,
        "table[class=\"data_table_1 m_table m_hl\"]",
        0,
    )?;
    let rename = [("变动数量（股）", "变动数量"), ("交易均价（元）", "交易均价"), ("剩余股数（股）", "剩余股数")];
    let cols: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
    let string_rows: Vec<Vec<Option<String>>> = rows
        .iter()
        .map(|r| {
            (0..cols.len())
                .map(|i| r.get(i).map(|s| s.replace([' ', '\t'], "")))
                .collect()
        })
        .collect();
    let mut df = Df::from_string_rows(&cols, &string_rows)?;
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, (*to).into());
    }
    // 变动日期 2026.04.20 → 2026-04-20（对应 akshare to_datetime + dt.date）
    df.cast_date(&["变动日期"])?;
    df = df.sort_by("变动日期", true, false)?;
    Ok(df)
}

/// 同花顺-公司大事-股东持股变动（对应 akshare [`akshare.stock_shareholder_change_ths`]）。
///
/// `symbol`：股票代码；页面 `event.html`。
///
/// # 返回列
/// `公告日期, 变动股东, 变动数量, 交易均价, 剩余股份总数, 变动期间, 变动途径`
pub fn stock_shareholder_change_ths(symbol: &str) -> Result<Df> {
    let url = format!("https://basic.10jqka.com.cn/new/{symbol}/event.html");
    let html = crate::sources::ths::fetch_ths(&url)?;
    let (headers, rows) = crate::sources::ths::parse_ths_theaded_table_sel(
        &html,
        "table[class=\"m_table data_table_1 m_hl\"]",
        0,
    )?;
    let rename = [
        ("变动数量(股)", "变动数量"),
        ("交易均价(元)", "交易均价"),
        ("剩余股份总数(股)", "剩余股份总数"),
    ];
    let cols: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
    let string_rows: Vec<Vec<Option<String>>> = rows
        .iter()
        .map(|r| {
            (0..cols.len())
                .map(|i| r.get(i).map(|s| s.replace([' ', '\t'], "")))
                .collect()
        })
        .collect();
    let mut df = Df::from_string_rows(&cols, &string_rows)?;
    for (from, to) in rename {
        let _ = df.inner_mut().rename(from, (*to).into());
    }
    df.cast_date(&["公告日期"])?;
    df = df.sort_by("公告日期", true, false)?;
    Ok(df)
}

// === BATCH3 STOCK_FUNDAMENTAL REMAINING (ths/sina/em) ===
//
// 本区域实现 akshare `stock_fundamental` 分类下、除已落地 15 个函数外的 10 个公开函数：
// - 乐咕股息率：`stock_a_gxl_lg`（复用 `src/legu/mod.rs` 两步流）
// - 东财 datacenter 大宗交易系列（6 个）：`stock_dzjy_hygtj` / `stock_dzjy_hyyybtj` /
//   `stock_dzjy_mrmx` / `stock_dzjy_mrtj` / `stock_dzjy_sctj` / `stock_dzjy_yybph`
//   复用 `stock_feature` 的 `datacenter` / `report_extra` / `fmt_ymd` 与
//   `sources::eastmoney::finalize_report`（键→中文按位置重命名，序号 1-based 前置，
//   日期列截断 `YYYY-MM-DD`；列名/列序与 akshare `reset_index + columns=[...]` 逐字一致）
// - 雪球个股公司简介（3 个）：`stock_individual_basic_info_xq` / `_hk_xq` / `_us_xq`
//   个股接口需登录态（`xq_a_token`），按 PLAN §D2 返回 `AuthRequired`（不伪造数据）

// ============ 7. 乐咕-股息率-A 股（复用 legu 两步流） ============

/// 乐咕乐股-股息率-A 股股息率（对应 akshare [`akshare.stock_a_gxl_lg`]）。
///
/// 复用 [`crate::legu::stock_a_gxl_lg`] 的两步流（`get_token_lg` md5 日期 token +
/// 页面 `_csrf` → 会话 cookie + `X-CSRF-Token` 头请求 API）。
///
/// `symbol`：`上证A股` / `深证A股` / `创业板` / `科创板`（默认 `上证A股`）。
///
/// # 返回列
/// `日期, 股息率`
pub fn stock_a_gxl_lg(symbol: &str) -> Result<Df> {
    crate::legu::stock_a_gxl_lg(symbol)
}

// ============ 8. 东财 datacenter 大宗交易系列（6 个） ============

const HYGTJ_RENAME: [(&str, &str); 15] = [
    ("SECURITY_CODE", "证券代码"),
    ("SECURITY_NAME_ABBR", "证券简称"),
    ("CLOSE_PRICE", "最新价"),
    ("CHANGE_RATE", "涨跌幅"),
    ("TRADE_DATE", "最近上榜日"),
    ("DEAL_AMT", "总成交额"),
    ("PREMIUM_RATIO", "折溢率"),
    ("SUM_TURNOVERRATE", "成交总额/流通市值"),
    ("DEAL_NUM", "上榜次数-总计"),
    ("PREMIUM_TIMES", "上榜次数-溢价"),
    ("DISCOUNT_TIMES", "上榜次数-折价"),
    ("D1_AVG_ADJCHRATE", "上榜日后平均涨跌幅-1日"),
    ("D5_AVG_ADJCHRATE", "上榜日后平均涨跌幅-5日"),
    ("D10_AVG_ADJCHRATE", "上榜日后平均涨跌幅-10日"),
    ("D20_AVG_ADJCHRATE", "上榜日后平均涨跌幅-20日"),
];
const HYGTJ_SELECT: [&str; 15] = [
    "证券代码",
    "证券简称",
    "最新价",
    "涨跌幅",
    "最近上榜日",
    "上榜次数-总计",
    "上榜次数-溢价",
    "上榜次数-折价",
    "总成交额",
    "折溢率",
    "成交总额/流通市值",
    "上榜日后平均涨跌幅-1日",
    "上榜日后平均涨跌幅-5日",
    "上榜日后平均涨跌幅-10日",
    "上榜日后平均涨跌幅-20日",
];
const HYGTJ_NUMERIC: [&str; 12] = [
    "最新价",
    "涨跌幅",
    "上榜次数-总计",
    "上榜次数-溢价",
    "上榜次数-折价",
    "总成交额",
    "折溢率",
    "成交总额/流通市值",
    "上榜日后平均涨跌幅-1日",
    "上榜日后平均涨跌幅-5日",
    "上榜日后平均涨跌幅-10日",
    "上榜日后平均涨跌幅-20日",
];
const HYGTJ_DATE: [&str; 1] = ["最近上榜日"];

/// 东方财富-数据中心-大宗交易-活跃A股统计（对应 akshare [`akshare.stock_dzjy_hygtj`]）。
///
/// `symbol`：时间区间 `近一月` / `近三月` / `近六月` / `近一年`（默认 `近三月`）。
/// 报表 `RPT_BLOCKTRADE_ACSTA`，按 `DATE_TYPE_CODE` 过滤。
///
/// # 返回列
/// `序号, 证券代码, 证券简称, 最新价, 涨跌幅, 最近上榜日, 上榜次数-总计, 上榜次数-溢价,
/// 上榜次数-折价, 总成交额, 折溢率, 成交总额/流通市值, 上榜日后平均涨跌幅-1日/5日/10日/20日`
pub fn stock_dzjy_hygtj(symbol: &str) -> Result<Df> {
    let period = match symbol {
        "近一月" => "1",
        "近三月" => "3",
        "近六月" => "6",
        "近一年" => "12",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 近一月/近三月/近六月/近一年）"
            )))
        }
    };
    let filter = format!("(DATE_TYPE_CODE={period})");
    let extra = report_extra(
        "DEAL_NUM,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter(
        "RPT_BLOCKTRADE_ACSTA",
        "SECURITY_CODE,SECUCODE,SECURITY_NAME_ABBR,CLOSE_PRICE,CHANGE_RATE,TRADE_DATE,DEAL_AMT,PREMIUM_RATIO,SUM_TURNOVERRATE,DEAL_NUM,PREMIUM_TIMES,DISCOUNT_TIMES,D1_AVG_ADJCHRATE,D5_AVG_ADJCHRATE,D10_AVG_ADJCHRATE,D20_AVG_ADJCHRATE,DATE_TYPE_CODE",
        &extra,
        "5000",
    )?;
    let mut df = finalize_report(
        &rows,
        &HYGTJ_RENAME,
        &HYGTJ_SELECT,
        &HYGTJ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&HYGTJ_DATE)?;
    Ok(df)
}

const HYYYBTJ_RENAME: [(&str, &str); 8] = [
    ("OPERATEDEPT_NAME", "营业部名称"),
    ("ONLIST_DATE", "最近上榜日"),
    ("STOCK_DETAILS", "买入的股票"),
    ("BUYER_NUM", "次数总计-买入"),
    ("SELLER_NUM", "次数总计-卖出"),
    ("TOTAL_BUYAMT", "成交金额统计-买入"),
    ("TOTAL_SELLAMT", "成交金额统计-卖出"),
    ("TOTAL_NETAMT", "成交金额统计-净买入额"),
];
const HYYYBTJ_SELECT: [&str; 8] = [
    "最近上榜日",
    "营业部名称",
    "次数总计-买入",
    "次数总计-卖出",
    "成交金额统计-买入",
    "成交金额统计-卖出",
    "成交金额统计-净买入额",
    "买入的股票",
];
const HYYYBTJ_NUMERIC: [&str; 5] = [
    "次数总计-买入",
    "次数总计-卖出",
    "成交金额统计-买入",
    "成交金额统计-卖出",
    "成交金额统计-净买入额",
];
const HYYYBTJ_DATE: [&str; 1] = ["最近上榜日"];

/// 东方财富-数据中心-大宗交易-活跃营业部统计（对应 akshare [`akshare.stock_dzjy_hyyybtj`]）。
///
/// `symbol`：`当前交易日` / `近3日` / `近5日` / `近10日` / `近30日`（默认 `近3日`）。
/// 报表 `RPT_BLOCKTRADE_OPERATEDEPTSTATISTICS`，按 `N_DATE` 过滤。
///
/// # 返回列
/// `序号, 最近上榜日, 营业部名称, 次数总计-买入, 次数总计-卖出, 成交金额统计-买入,
/// 成交金额统计-卖出, 成交金额统计-净买入额, 买入的股票`
pub fn stock_dzjy_hyyybtj(symbol: &str) -> Result<Df> {
    let n = match symbol {
        "当前交易日" => "1",
        "近3日" => "3",
        "近5日" => "5",
        "近10日" => "10",
        "近30日" => "30",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 当前交易日/近3日/近5日/近10日/近30日）"
            )))
        }
    };
    let filter = format!("(N_DATE=-{n})");
    let extra = report_extra(
        "BUYER_NUM,TOTAL_BUYAMT",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter(
        "RPT_BLOCKTRADE_OPERATEDEPTSTATISTICS",
        "OPERATEDEPT_CODE,OPERATEDEPT_NAME,ONLIST_DATE,STOCK_DETAILS,BUYER_NUM,SELLER_NUM,TOTAL_BUYAMT,TOTAL_SELLAMT,TOTAL_NETAMT,N_DATE",
        &extra,
        "5000",
    )?;
    let mut df = finalize_report(
        &rows,
        &HYYYBTJ_RENAME,
        &HYYYBTJ_SELECT,
        &HYYYBTJ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&HYYYBTJ_DATE)?;
    Ok(df)
}

const MRMX_A_RENAME: [(&str, &str); 12] = [
    ("TRADE_DATE", "交易日期"),
    ("SECURITY_CODE", "证券代码"),
    ("SECURITY_NAME_ABBR", "证券简称"),
    ("CHANGE_RATE", "涨跌幅"),
    ("CLOSE_PRICE", "收盘价"),
    ("DEAL_PRICE", "成交价"),
    ("PREMIUM_RATIO", "折溢率"),
    ("DEAL_VOLUME", "成交量"),
    ("DEAL_AMT", "成交额"),
    ("TURNOVER_RATE", "成交额/流通市值"),
    ("BUYER_NAME", "买方营业部"),
    ("SELLER_NAME", "卖方营业部"),
];
const MRMX_A_SELECT: [&str; 12] = [
    "交易日期",
    "证券代码",
    "证券简称",
    "涨跌幅",
    "收盘价",
    "成交价",
    "折溢率",
    "成交量",
    "成交额",
    "成交额/流通市值",
    "买方营业部",
    "卖方营业部",
];
const MRMX_A_NUMERIC: [&str; 7] = [
    "涨跌幅",
    "收盘价",
    "成交价",
    "折溢率",
    "成交量",
    "成交额",
    "成交额/流通市值",
];
const MRMX_A_DATE: [&str; 1] = ["交易日期"];

const MRMX_B_RENAME: [(&str, &str); 8] = [
    ("TRADE_DATE", "交易日期"),
    ("SECURITY_CODE", "证券代码"),
    ("SECURITY_NAME_ABBR", "证券简称"),
    ("DEAL_PRICE", "成交价"),
    ("DEAL_VOLUME", "成交量"),
    ("DEAL_AMT", "成交额"),
    ("BUYER_NAME", "买方营业部"),
    ("SELLER_NAME", "卖方营业部"),
];
const MRMX_B_SELECT: [&str; 8] = [
    "交易日期",
    "证券代码",
    "证券简称",
    "成交价",
    "成交量",
    "成交额",
    "买方营业部",
    "卖方营业部",
];
const MRMX_B_NUMERIC: [&str; 3] = ["成交价", "成交量", "成交额"];
const MRMX_B_DATE: [&str; 1] = ["交易日期"];

/// 东方财富-数据中心-大宗交易-每日明细（对应 akshare [`akshare.stock_dzjy_mrmx`]）。
///
/// `symbol`：`A股` / `B股` / `基金` / `债券`（默认 `基金`）；`start_date` / `end_date`：
/// 区间 `YYYYMMDD`。报表 `RPT_DATA_BLOCKTRADE`，按 `SECURITY_TYPE_WEB` + 交易日区间过滤。
///
/// A 股输出 13 列（含 `涨跌幅/收盘价`，`SECUCODE` 弃用）；B股/基金/债券输出 9 列
/// （仅 `成交价/成交量/成交额/买方营业部/卖方营业部`）。
///
/// # 返回列（A股）
/// `序号, 交易日期, 证券代码, 证券简称, 涨跌幅, 收盘价, 成交价, 折溢率, 成交量,
/// 成交额, 成交额/流通市值, 买方营业部, 卖方营业部`
pub fn stock_dzjy_mrmx(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let st = match symbol {
        "A股" => "1",
        "B股" => "2",
        "基金" => "3",
        "债券" => "4",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 A股/B股/基金/债券）"
            )))
        }
    };
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!("(SECURITY_TYPE_WEB={st})(TRADE_DATE>='{sd}')(TRADE_DATE<='{ed}')");
    let extra = report_extra("SECURITY_CODE", "1", Some(&filter), None, None, None);
    let rows = datacenter(
        "RPT_DATA_BLOCKTRADE",
        "TRADE_DATE,SECURITY_CODE,SECUCODE,SECURITY_NAME_ABBR,CHANGE_RATE,CLOSE_PRICE,DEAL_PRICE,PREMIUM_RATIO,DEAL_VOLUME,DEAL_AMT,TURNOVER_RATE,BUYER_NAME,SELLER_NAME,CHANGE_RATE_1DAYS,CHANGE_RATE_5DAYS,CHANGE_RATE_10DAYS,CHANGE_RATE_20DAYS,BUYER_CODE,SELLER_CODE",
        &extra,
        "5000",
    )?;
    if symbol == "A股" {
        let mut df = finalize_report(
            &rows,
            &MRMX_A_RENAME,
            &MRMX_A_SELECT,
            &MRMX_A_NUMERIC,
            Some("序号"),
        )?;
        df.cast_date(&MRMX_A_DATE)?;
        Ok(df)
    } else {
        let mut df = finalize_report(
            &rows,
            &MRMX_B_RENAME,
            &MRMX_B_SELECT,
            &MRMX_B_NUMERIC,
            Some("序号"),
        )?;
        df.cast_date(&MRMX_B_DATE)?;
        Ok(df)
    }
}

const MRTJ_RENAME: [(&str, &str); 11] = [
    ("TRADE_DATE", "交易日期"),
    ("SECURITY_CODE", "证券代码"),
    ("SECURITY_NAME_ABBR", "证券简称"),
    ("CHANGE_RATE", "涨跌幅"),
    ("CLOSE_PRICE", "收盘价"),
    ("AVERAGE_PRICE", "成交价"),
    ("PREMIUM_RATIO", "折溢率"),
    ("DEAL_NUM", "成交笔数"),
    ("VOLUME", "成交总量"),
    ("DEAL_AMT", "成交总额"),
    ("TURNOVERRATE", "成交总额/流通市值"),
];
const MRTJ_SELECT: [&str; 11] = [
    "交易日期",
    "证券代码",
    "证券简称",
    "涨跌幅",
    "收盘价",
    "成交价",
    "折溢率",
    "成交笔数",
    "成交总量",
    "成交总额",
    "成交总额/流通市值",
];
const MRTJ_NUMERIC: [&str; 8] = [
    "涨跌幅",
    "收盘价",
    "成交价",
    "折溢率",
    "成交笔数",
    "成交总量",
    "成交总额",
    "成交总额/流通市值",
];
const MRTJ_DATE: [&str; 1] = ["交易日期"];

/// 东方财富-数据中心-大宗交易-每日统计（对应 akshare [`akshare.stock_dzjy_mrtj`]）。
///
/// `start_date` / `end_date`：区间 `YYYYMMDD`（默认 `20220105` / `20220105`）。
/// 报表 `RPT_BLOCKTRADE_STA`，按交易日区间过滤。
///
/// # 返回列
/// `序号, 交易日期, 证券代码, 证券简称, 涨跌幅, 收盘价, 成交价, 折溢率, 成交笔数,
/// 成交总量, 成交总额, 成交总额/流通市值`
pub fn stock_dzjy_mrtj(start_date: &str, end_date: &str) -> Result<Df> {
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!("(TRADE_DATE>='{sd}')(TRADE_DATE<='{ed}')");
    let extra = report_extra("TURNOVERRATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter(
        "RPT_BLOCKTRADE_STA",
        "TRADE_DATE,SECURITY_CODE,SECUCODE,SECURITY_NAME_ABBR,CHANGE_RATE,CLOSE_PRICE,AVERAGE_PRICE,PREMIUM_RATIO,DEAL_NUM,VOLUME,DEAL_AMT,TURNOVERRATE,D1_CLOSE_ADJCHRATE,D5_CLOSE_ADJCHRATE,D10_CLOSE_ADJCHRATE,D20_CLOSE_ADJCHRATE",
        &extra,
        "5000",
    )?;
    let mut df = finalize_report(
        &rows,
        &MRTJ_RENAME,
        &MRTJ_SELECT,
        &MRTJ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&MRTJ_DATE)?;
    Ok(df)
}

const SCTJ_RENAME: [(&str, &str); 8] = [
    ("TRADE_DATE", "交易日期"),
    ("SZ_INDEX", "上证指数"),
    ("SZ_CHANGE_RATE", "上证指数涨跌幅"),
    ("BLOCKTRADE_DEAL_AMT", "大宗交易成交总额"),
    ("PREMIUM_DEAL_AMT", "溢价成交总额"),
    ("PREMIUM_RATIO", "溢价成交总额占比"),
    ("DISCOUNT_DEAL_AMT", "折价成交总额"),
    ("DISCOUNT_RATIO", "折价成交总额占比"),
];
const SCTJ_SELECT: [&str; 8] = [
    "交易日期",
    "上证指数",
    "上证指数涨跌幅",
    "大宗交易成交总额",
    "溢价成交总额",
    "溢价成交总额占比",
    "折价成交总额",
    "折价成交总额占比",
];
const SCTJ_NUMERIC: [&str; 7] = [
    "上证指数",
    "上证指数涨跌幅",
    "大宗交易成交总额",
    "溢价成交总额",
    "溢价成交总额占比",
    "折价成交总额",
    "折价成交总额占比",
];
const SCTJ_DATE: [&str; 1] = ["交易日期"];

/// 东方财富-数据中心-大宗交易-市场统计（对应 akshare [`akshare.stock_dzjy_sctj`]）。
///
/// 无参数。报表 `PRT_BLOCKTRADE_MARKET_STA`，按 `TRADE_DATE` 降序。
///
/// # 返回列
/// `序号, 交易日期, 上证指数, 上证指数涨跌幅, 大宗交易成交总额, 溢价成交总额,
/// 溢价成交总额占比, 折价成交总额, 折价成交总额占比`
pub fn stock_dzjy_sctj() -> Result<Df> {
    let extra = report_extra("TRADE_DATE", "-1", None, None, None, None);
    let rows = datacenter(
        "PRT_BLOCKTRADE_MARKET_STA",
        "TRADE_DATE,SZ_INDEX,SZ_CHANGE_RATE,BLOCKTRADE_DEAL_AMT,PREMIUM_DEAL_AMT,PREMIUM_RATIO,DISCOUNT_DEAL_AMT,DISCOUNT_RATIO",
        &extra,
        "500",
    )?;
    let mut df = finalize_report(
        &rows,
        &SCTJ_RENAME,
        &SCTJ_SELECT,
        &SCTJ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&SCTJ_DATE)?;
    Ok(df)
}

const YYBPH_RENAME: [(&str, &str); 13] = [
    ("OPERATEDEPT_NAME", "营业部名称"),
    ("D1_BUYER_NUM", "上榜后1天-买入次数"),
    ("D1_AVERAGE_INCREASE", "上榜后1天-平均涨幅"),
    ("D1_RISE_PROBABILITY", "上榜后1天-上涨概率"),
    ("D5_BUYER_NUM", "上榜后5天-买入次数"),
    ("D5_AVERAGE_INCREASE", "上榜后5天-平均涨幅"),
    ("D5_RISE_PROBABILITY", "上榜后5天-上涨概率"),
    ("D10_BUYER_NUM", "上榜后10天-买入次数"),
    ("D10_AVERAGE_INCREASE", "上榜后10天-平均涨幅"),
    ("D10_RISE_PROBABILITY", "上榜后10天-上涨概率"),
    ("D20_BUYER_NUM", "上榜后20天-买入次数"),
    ("D20_AVERAGE_INCREASE", "上榜后20天-平均涨幅"),
    ("D20_RISE_PROBABILITY", "上榜后20天-上涨概率"),
];
const YYBPH_SELECT: [&str; 13] = [
    "营业部名称",
    "上榜后1天-买入次数",
    "上榜后1天-平均涨幅",
    "上榜后1天-上涨概率",
    "上榜后5天-买入次数",
    "上榜后5天-平均涨幅",
    "上榜后5天-上涨概率",
    "上榜后10天-买入次数",
    "上榜后10天-平均涨幅",
    "上榜后10天-上涨概率",
    "上榜后20天-买入次数",
    "上榜后20天-平均涨幅",
    "上榜后20天-上涨概率",
];
const YYBPH_NUMERIC: [&str; 12] = [
    "上榜后1天-买入次数",
    "上榜后1天-平均涨幅",
    "上榜后1天-上涨概率",
    "上榜后5天-买入次数",
    "上榜后5天-平均涨幅",
    "上榜后5天-上涨概率",
    "上榜后10天-买入次数",
    "上榜后10天-平均涨幅",
    "上榜后10天-上涨概率",
    "上榜后20天-买入次数",
    "上榜后20天-平均涨幅",
    "上榜后20天-上涨概率",
];

/// 东方财富-数据中心-大宗交易-营业部排行（对应 akshare [`akshare.stock_dzjy_yybph`]）。
///
/// `symbol`：`近一月` / `近三月` / `近六月` / `近一年`（默认 `近三月`）。
/// 报表 `RPT_BLOCKTRADE_OPERATEDEPT_RANK`，按 `N_DATE` 过滤。
///
/// # 返回列
/// `序号, 营业部名称, 上榜后1天-买入次数, 上榜后1天-平均涨幅, 上榜后1天-上涨概率,
/// 上榜后5天/10天/20天-买入次数/平均涨幅/上涨概率`
pub fn stock_dzjy_yybph(symbol: &str) -> Result<Df> {
    let n = match symbol {
        "近一月" => "30",
        "近三月" => "90",
        "近六月" => "180",
        "近一年" => "360",
        _ => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 近一月/近三月/近六月/近一年）"
            )))
        }
    };
    let filter = format!("(N_DATE=-{n})");
    let extra = report_extra(
        "D5_BUYER_NUM,D1_AVERAGE_INCREASE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter(
        "RPT_BLOCKTRADE_OPERATEDEPT_RANK",
        "OPERATEDEPT_CODE,OPERATEDEPT_NAME,D1_BUYER_NUM,D1_AVERAGE_INCREASE,D1_RISE_PROBABILITY,D5_BUYER_NUM,D5_AVERAGE_INCREASE,D5_RISE_PROBABILITY,D10_BUYER_NUM,D10_AVERAGE_INCREASE,D10_RISE_PROBABILITY,D20_BUYER_NUM,D20_AVERAGE_INCREASE,D20_RISE_PROBABILITY,N_DATE,RELATED_ORG_CODE",
        &extra,
        "5000",
    )?;
    let df = finalize_report(
        &rows,
        &YYBPH_RENAME,
        &YYBPH_SELECT,
        &YYBPH_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ============ 9. 雪球-个股公司简介（3 个，需登录态） ============

/// 雪球-个股-公司概况-公司简介（A 股，对应 akshare [`akshare.stock_individual_basic_info_xq`]）。
///
/// `symbol`：A 股代码（如 `SH601127`）。雪球个股接口需有效登录态（`xq_a_token`）；
/// 无登录态时上游返回 `error_code: 400016`，按 PLAN §D2 返回
/// [`AkshareError::AuthRequired`]（带诊断），不伪造数据。
///
/// # 返回列（登录态可用时）
/// `item, value`
pub fn stock_individual_basic_info_xq(symbol: &str) -> Result<Df> {
    xq_company(
        "https://stock.xueqiu.com/v5/stock/f10/cn/company.json",
        symbol,
    )
}

/// 雪球-个股-公司概况-公司简介（港股，对应 akshare [`akshare.stock_individual_basic_info_hk_xq`]）。
///
/// `symbol`：港股代码（如 `02097`）。其余同上（需登录态，无则 `AuthRequired`）。
///
/// # 返回列（登录态可用时）
/// `item, value`
pub fn stock_individual_basic_info_hk_xq(symbol: &str) -> Result<Df> {
    xq_company(
        "https://stock.xueqiu.com/v5/stock/f10/hk/company.json",
        symbol,
    )
}

/// 雪球-个股-公司概况-公司简介（美股，对应 akshare [`akshare.stock_individual_basic_info_us_xq`]）。
///
/// `symbol`：美股代码（如 `NVDA`）。其余同上（需登录态，无则 `AuthRequired`）。
///
/// # 返回列（登录态可用时）
/// `item, value`
pub fn stock_individual_basic_info_us_xq(symbol: &str) -> Result<Df> {
    xq_company(
        "https://stock.xueqiu.com/v5/stock/f10/us/company.json",
        symbol,
    )
}

/// 雪球公司简介公共抓取（无登录态 → `AuthRequired`，见 PLAN §D2）。
///
/// 先访问 `xueqiu.com/` 建立会话 cookie（响应可能为 WAF 页，仅需写 cookie），
/// 再请求 `company.json`。`get_json_allow_status` 即使 HTTP 非 2xx 也返回响应体，
/// 其内 `detect_block_or_auth` 命中 `400016` 即返回 `AuthRequired`；若响应含 `data`
/// （登录态有效），则按 akshare `pd.DataFrame(data).reset_index()` → `item/value`
/// 两列构建（每对 `key=value` 一行）。
fn xq_company(url: &str, symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    // 1) 建立会话 cookie（首页可能为 WAF 页，仅写 cookie，跳过内容检测）
    let _ = http.get_text_allow_blocked("https://xueqiu.com/", &Map::new(), None);
    let mut params = Map::new();
    params.insert("symbol".into(), Value::String(symbol.into()));
    // 2) 个股接口：allow_status 以便读取 400016 业务错误
    let json = http.get_json_allow_status(url, &params, Some("https://xueqiu.com/"))?;
    let Some(data) = json.get("data") else {
        let code = json.get("error_code").and_then(Value::as_u64).unwrap_or(0);
        let desc = json
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("");
        return Err(AkshareError::AuthRequired(format!(
            "雪球个股接口需登录态(xq_a_token)；上游返回 {code}: {desc} (url: {url}, symbol: {symbol})"
        )));
    };
    build_xq_df(data)
}

/// 由雪球公司简介 `data` 对象构建 `item, value` 两列表（离线可测，对应 akshare
/// `pd.DataFrame(data).reset_index()` + 重命名 `item/value`）。
fn build_xq_df(data: &Value) -> Result<Df> {
    let obj = data
        .as_object()
        .ok_or_else(|| AkshareError::Empty("雪球公司数据非对象".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(obj.len());
    for (k, v) in obj {
        rows.push(vec![Some(k.clone()), cell(v)]);
    }
    Df::from_string_rows(&["item", "value"], &rows)
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
        assert!(matches!(market_code("123"), Err(AkshareError::Param(_))));
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
        assert_eq!(
            df.column_names(),
            vec!["报告期", "净利润", "净利润同比增长率"]
        );
        // do_sort=true：报告期升序（ISO 字符串序 = 时间序）
        let d = df.inner().column("报告期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2025-09-30"));
        assert_eq!(d.get(2), Some("2026-03-31"));
        // 转置随排序联动：2025-09-30（原数组末尾）排到第 0 行
        let profit = df.inner().column("净利润").unwrap().str().unwrap();
        assert_eq!(profit.get(0), Some("53.22亿"));
        assert_eq!(profit.get(2), Some("13.10亿"));
        // 布尔 → 大写（对应 pandas str(True/False)）
        let yoy = df
            .inner()
            .column("净利润同比增长率")
            .unwrap()
            .str()
            .unwrap();
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

    #[test]
    fn dzjy_hygtj_offline_contract() {
        let rows = vec![json!({
            "SECURITY_CODE": "600000", "SECUCODE": "600000.SH", "SECURITY_NAME_ABBR": "浦发银行",
            "CLOSE_PRICE": 7.5, "CHANGE_RATE": 1.2, "TRADE_DATE": "2026-01-15 00:00:00",
            "DEAL_AMT": 123456789, "PREMIUM_RATIO": -2.3, "SUM_TURNOVERRATE": 5.5,
            "DEAL_NUM": 10, "PREMIUM_TIMES": 3, "DISCOUNT_TIMES": 7,
            "D1_AVG_ADJCHRATE": 0.5, "D5_AVG_ADJCHRATE": 1.1, "D10_AVG_ADJCHRATE": 2.2,
            "D20_AVG_ADJCHRATE": 3.3, "DATE_TYPE_CODE": 3,
        })];
        let mut df = finalize_report(
            &rows,
            &HYGTJ_RENAME,
            &HYGTJ_SELECT,
            &HYGTJ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&HYGTJ_DATE).unwrap();
        // 16 列契约（含 序号 前置，SECUCODE/DATE_TYPE_CODE 弃用列已丢弃）
        assert_eq!(df.column_names().len(), 16);
        assert!(df.column_names().iter().any(|c| c == "证券代码"));
        assert!(!df.column_names().iter().any(|c| c == "SECUCODE"));
        // 序号 1 起始
        assert_eq!(
            df.inner().column("序号").unwrap().f64().unwrap().get(0),
            Some(1.0)
        );
        // 日期截断为 YYYY-MM-DD
        assert_eq!(
            df.inner()
                .column("最近上榜日")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("2026-01-15")
        );
    }

    #[test]
    fn dzjy_mrmx_offline_contract() {
        let rows = vec![json!({
            "TRADE_DATE": "2024-01-02 00:00:00", "SECURITY_CODE": "600519", "SECUCODE": "600519.SH",
            "SECURITY_NAME_ABBR": "贵州茅台", "CHANGE_RATE": -0.5, "CLOSE_PRICE": 1685.0,
            "DEAL_PRICE": 1670.0, "PREMIUM_RATIO": -0.9, "DEAL_VOLUME": 12000,
            "DEAL_AMT": 20040000, "TURNOVER_RATE": 0.01, "BUYER_NAME": "营业部A", "SELLER_NAME": "营业部B",
        })];
        // A 股分支：13 列
        let mut df_a = finalize_report(
            &rows,
            &MRMX_A_RENAME,
            &MRMX_A_SELECT,
            &MRMX_A_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df_a.cast_date(&MRMX_A_DATE).unwrap();
        assert_eq!(df_a.column_names().len(), 13);
        assert!(df_a.column_names().iter().any(|c| c == "收盘价"));
        assert!(!df_a.column_names().iter().any(|c| c == "SECUCODE"));
        assert_eq!(
            df_a.inner()
                .column("交易日期")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("2024-01-02")
        );
        // 数值列已数值化
        assert_eq!(
            df_a.inner().column("成交价").unwrap().f64().unwrap().get(0),
            Some(1670.0)
        );
        // B股/基金/债券分支：9 列（无 涨跌幅/收盘价）
        let mut df_b = finalize_report(
            &rows,
            &MRMX_B_RENAME,
            &MRMX_B_SELECT,
            &MRMX_B_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df_b.cast_date(&MRMX_B_DATE).unwrap();
        assert_eq!(df_b.column_names().len(), 9);
        assert!(!df_b.column_names().iter().any(|c| c == "收盘价"));
        assert!(df_b.column_names().iter().any(|c| c == "成交价"));
    }

    #[test]
    fn dzjy_sctj_offline_contract() {
        let rows = vec![json!({
            "TRADE_DATE": "2026-03-10 00:00:00", "SZ_INDEX": 3300.5, "SZ_CHANGE_RATE": 0.8,
            "BLOCKTRADE_DEAL_AMT": 123456789, "PREMIUM_DEAL_AMT": 50000000,
            "PREMIUM_RATIO": 40.5, "DISCOUNT_DEAL_AMT": 70000000, "DISCOUNT_RATIO": 56.7,
        })];
        let mut df = finalize_report(
            &rows,
            &SCTJ_RENAME,
            &SCTJ_SELECT,
            &SCTJ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&SCTJ_DATE).unwrap();
        assert_eq!(df.column_names().len(), 9);
        // 第一数据列是 交易日期（序号前置），第二是 上证指数
        assert_eq!(
            df.inner().column("上证指数").unwrap().f64().unwrap().get(0),
            Some(3300.5)
        );
        assert_eq!(
            df.inner().column("交易日期").unwrap().str().unwrap().get(0),
            Some("2026-03-10")
        );
    }

    #[test]
    fn dzjy_yybph_offline_contract() {
        let rows = vec![json!({
            "OPERATEDEPT_CODE": "10188715", "OPERATEDEPT_NAME": "华泰证券营业部",
            "D1_BUYER_NUM": 5, "D1_AVERAGE_INCREASE": 1.2, "D1_RISE_PROBABILITY": 60.0,
            "D5_BUYER_NUM": 12, "D5_AVERAGE_INCREASE": 2.3, "D5_RISE_PROBABILITY": 55.0,
            "D10_BUYER_NUM": 20, "D10_AVERAGE_INCREASE": 3.1, "D10_RISE_PROBABILITY": 50.0,
            "D20_BUYER_NUM": 30, "D20_AVERAGE_INCREASE": 4.0, "D20_RISE_PROBABILITY": 48.0,
            "N_DATE": -90, "RELATED_ORG_CODE": "X",
        })];
        let df = finalize_report(
            &rows,
            &YYBPH_RENAME,
            &YYBPH_SELECT,
            &YYBPH_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        // 14 列契约（OPERATEDEPT_CODE/N_DATE/RELATED_ORG_CODE 弃用列已丢弃）
        assert_eq!(df.column_names().len(), 14);
        assert!(df.column_names().iter().any(|c| c == "营业部名称"));
        assert!(!df.column_names().iter().any(|c| c == "OPERATEDEPT_CODE"));
        assert_eq!(
            df.inner()
                .column("上榜后1天-买入次数")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(5.0)
        );
    }

    #[test]
    fn xq_company_build_offline() {
        // 登录态有效时 data 为对象 → item/value 两列，每对 key=value 一行
        let data = json!({
            "公司名称": "赛力斯", "英文名称": "SERES", "成立日期": "2007-05-11",
        });
        let df = build_xq_df(&data).unwrap();
        assert_eq!(df.column_names(), vec!["item", "value"]);
        assert_eq!(df.height(), 3);
        assert_eq!(
            df.inner().column("item").unwrap().str().unwrap().get(0),
            Some("公司名称")
        );
        assert_eq!(
            df.inner().column("value").unwrap().str().unwrap().get(0),
            Some("赛力斯")
        );
        // 非对象 → Empty 错误
        assert!(build_xq_df(&json!([1, 2, 3])).is_err());
    }
}
