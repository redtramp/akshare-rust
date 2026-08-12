//! 人民币汇率数据源（批次 5 长尾 · currency）。
//!
//! 对应 akshare `currency/currency_china_bank_sina.py` 与 `currency/currency_safe.py`：
//! - 新浪财经-中行人民币牌价历史（`currency_boc_sina`）
//! - 国家外汇管理局人民币汇率中间价（`currency_boc_safe`）
//!
//! 注：`currency_boc_safe` 的「历史 Excel 部分」依赖 `pd.read_excel`，而 Rust 侧
//! 无 xlsx 解析器，故仅实现「近期在线查询部分」（`RMBQuery.do` POST），返回与 akshare
//! 完全一致的 26 列（日期 + 25 币种），仅行数覆盖近期区间。其余 currency 函数
//! （currencyscoop / investing.com 系）需 API key 或已被封，不在本批次。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::html::read_html_tables;
use crate::core::http::HttpClient;
use scraper::{Html, Selector};
use serde_json::{Map, Value};
use std::collections::HashMap;

const SINA_URL: &str = "http://biz.finance.sina.com.cn/forex/forex.php";
const SAFE_QUERY_URL: &str = "https://www.safe.gov.cn/AppStructured/hlw/RMBQuery.do";

/// 中行牌价 6 列（与 akshare `columns=[...]` 一致）。
const SINA_COLS: &[&str] = &[
    "日期",
    "中行汇买价",
    "中行钞买价",
    "中行钞卖价/汇卖价",
    "央行中间价",
    "中行折算价",
];

/// SAFE 中间价 26 列（日期 + 25 币种，与 akshare `currency_boc_safe` 输出顺序一致）。
const SAFE_COLS: &[&str] = &[
    "日期", "美元", "欧元", "日元", "港元", "英镑", "澳元", "新西兰元", "新加坡元", "瑞士法郎",
    "加元", "澳门元", "林吉特", "卢布", "兰特", "韩元", "迪拉姆", "里亚尔", "福林", "兹罗提",
    "丹麦克朗", "瑞典克朗", "挪威克朗", "里拉", "比索", "泰铢",
];

/// `YYYYMMDD` → `YYYY-MM-DD`。
fn date_fmt(d: &str) -> Result<String> {
    if d.len() != 8 || !d.chars().all(|c| c.is_ascii_digit()) {
        return Err(AkshareError::Empty(format!("日期需为 YYYYMMDD，收到: {d}")));
    }
    Ok(format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]))
}

/// 新浪外汇 symbol → 代码映射（对应 akshare `_currency_boc_sina_map`）。
fn fetch_sina_map(start_date: &str, end_date: &str) -> Result<HashMap<String, String>> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("startdate".into(), Value::String(date_fmt(start_date)?));
    params.insert("enddate".into(), Value::String(date_fmt(end_date)?));
    params.insert("money_code".into(), Value::String("EUR".into()));
    params.insert("type".into(), Value::String("0".into()));
    let text = http.get_text_with_headers(SINA_URL, &params, &[], None)?;
    let doc = Html::parse_document(&text);
    let sel = Selector::parse(r#"#money_code option"#)
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    let mut map = HashMap::new();
    for opt in doc.select(&sel) {
        let name = opt.text().collect::<Vec<_>>().join("").trim().to_string();
        let code = opt.value().attr("value").unwrap_or("").to_string();
        if !name.is_empty() && !code.is_empty() {
            map.insert(name, code);
        }
    }
    if map.is_empty() {
        return Err(AkshareError::Empty("新浪外汇代码映射为空".into()));
    }
    Ok(map)
}

/// 解析新浪牌价分页总数（`a.page` 中的最大数字；无分页则 1）。
fn parse_sina_page_count(text: &str) -> usize {
    let doc = Html::parse_document(text);
    let sel = Selector::parse("a.page").ok();
    let mut max = 1usize;
    if let Some(sel) = sel {
        for a in doc.select(&sel) {
            let t = a.text().collect::<Vec<_>>().join("").trim().to_string();
            if let Ok(n) = t.parse::<usize>() {
                max = max.max(n);
            }
        }
    }
    max
}

/// 新浪财经-中行人民币牌价历史数据查询（对应 akshare [`currency_boc_sina`]）。
///
/// # 参数
/// - `symbol`：币种中文名（如 `"美元"`）
/// - `start_date` / `end_date`：`YYYYMMDD`
///
/// # 返回列
/// `日期, 中行汇买价, 中行钞买价, 中行钞卖价/汇卖价, 央行中间价, 中行折算价`
pub fn currency_boc_sina(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let map = fetch_sina_map(start_date, end_date)?;
    let code = map
        .get(symbol)
        .ok_or_else(|| AkshareError::Empty(format!("未知外汇 symbol: {symbol}")))?
        .clone();

    let http = HttpClient::default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();

    // 第一页：取分页总数并收集行
    let (total, page1_rows) = {
        let mut params = Map::new();
        params.insert("money_code".into(), Value::String(code.clone()));
        params.insert("type".into(), Value::String("0".into()));
        params.insert("startdate".into(), Value::String(date_fmt(start_date)?));
        params.insert("enddate".into(), Value::String(date_fmt(end_date)?));
        params.insert("page".into(), Value::String("1".into()));
        params.insert("call_type".into(), Value::String("ajax".into()));
        let text = http.get_text_with_headers(SINA_URL, &params, &[], None)?;
        let total = parse_sina_page_count(&text);
        let rows = extract_sina_rows(&text)?;
        (total, rows)
    };
    rows.extend(page1_rows);

    // 后续页
    for p in 2..=total {
        let mut params = Map::new();
        params.insert("money_code".into(), Value::String(code.clone()));
        params.insert("type".into(), Value::String("0".into()));
        params.insert("startdate".into(), Value::String(date_fmt(start_date)?));
        params.insert("enddate".into(), Value::String(date_fmt(end_date)?));
        params.insert("page".into(), Value::String(p.to_string()));
        params.insert("call_type".into(), Value::String("ajax".into()));
        let text = http.get_text_with_headers(SINA_URL, &params, &[], None)?;
        rows.extend(extract_sina_rows(&text)?);
    }

    let mut df = Df::from_string_rows(SINA_COLS, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&[
        "中行汇买价",
        "中行钞买价",
        "中行钞卖价/汇卖价",
        "央行中间价",
        "中行折算价",
    ])?;
    let sorted = df.sort_by("日期", true, false)?;
    Ok(sorted)
}

/// 从新浪牌价页面抽取数据行。
///
/// akshare 使用 `pd.read_html(..., header=0)`，首行作为表头被跳过；故此处跳过
/// 首行（表头），仅保留数据行，避免每页多统计一行（总行数对齐 python）。
fn extract_sina_rows(text: &str) -> Result<Vec<Vec<Option<String>>>> {
    let tables = read_html_tables(text)?;
    let mut out = Vec::new();
    if let Some(table) = tables.first() {
        for r in table.iter().skip(1) {
            if r.len() >= SINA_COLS.len() {
                out.push(r.iter().map(|c| Some(c.clone())).collect());
            }
        }
    }
    Ok(out)
}

/// 国家外汇管理局-人民币汇率中间价（对应 akshare [`currency_boc_safe`]）。
///
/// 仅包含「近期在线查询部分」（历史 Excel 因无 xlsx 解析器省略），
/// 返回与 akshare 完全一致的 26 列（日期 + 25 币种）。
///
/// # 返回列
/// `日期, 美元, 欧元, 日元, 港元, 英镑, 澳元, 新西兰元, 新加坡元, 瑞士法郎, 加元, 澳门元, 林吉特, 卢布, 兰特, 韩元, 迪拉姆, 里亚尔, 福林, 兹罗提, 丹麦克朗, 瑞典克朗, 挪威克朗, 里拉, 比索, 泰铢`
pub fn currency_boc_safe() -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("startDate".into(), Value::String("2010-01-01".into()));
    params.insert("endDate".into(), Value::String("2030-12-31".into()));
    params.insert("queryYN".into(), Value::String("true".into()));
    let text = http.post_form_text(SAFE_QUERY_URL, &params, &[])?;
    let tables = read_html_tables(&text)?;
    let table = tables
        .last()
        .ok_or_else(|| AkshareError::Empty("SAFE 中间价未解析到表".into()))?;
    // 末表首行通常为表头；跳过首行（pd.read_html 取最后一表，含表头）
    let data_rows: Vec<Vec<Option<String>>> = table
        .iter()
        .skip(1)
        .map(|r| r.iter().map(|c| Some(c.clone())).collect())
        .collect();

    let mut df = Df::from_string_rows(SAFE_COLS, &data_rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&SAFE_COLS[1..])?;
    let sorted = df.sort_by("日期", true, false)?;
    Ok(sorted)
}
