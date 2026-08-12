//! HTML 表格解析（对应 akshare 的 `pd.read_html`）。
//!
//! 用 `scraper` 解析 HTML，返回所有 `<table>` 对应的 `Df`（顺序与文档一致）。
//! 列名优先取 `<thead> th>`；若无 `thead` 则取首行 `<th>/<td>` 作表头，数据从下一行起。
//! 单元值初始为字符串，空单元格记为 `None`（对齐 akshare 对缺失值的处理）；
//! 调用方按需 `cast_numeric` / `cast_date`。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use scraper::{ElementRef, Html, Selector};

/// 把单元格文本收成单行（去首尾空白，内部空白折叠为单空格）。
fn cell_text(el: ElementRef<'_>) -> Option<String> {
    let s: String = el.text().collect::<Vec<_>>().join("").split_whitespace().collect();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 解析单个 `<table>` 元素为 `Df`。
fn parse_one_table(table: &ElementRef<'_>) -> Result<Df> {
    let thead_sel = Selector::parse("thead")
        .map_err(|e| AkshareError::Empty(format!("thead 选择器解析失败: {e}")))?;
    let th_sel = Selector::parse("th")
        .map_err(|e| AkshareError::Empty(format!("th 选择器解析失败: {e}")))?;
    let tr_sel = Selector::parse("tr")
        .map_err(|e| AkshareError::Empty(format!("tr 选择器解析失败: {e}")))?;
    let td_sel = Selector::parse("td")
        .map_err(|e| AkshareError::Empty(format!("td 选择器解析失败: {e}")))?;

    // 1) 表头：优先 thead > th；否则用首行 tr 的 th/td。
    let headers: Vec<String> = if table.select(&thead_sel).next().is_some() {
        table.select(&th_sel).filter_map(cell_text).collect()
    } else {
        match table.select(&tr_sel).next() {
            Some(tr) => {
                let mut hs: Vec<String> = tr.select(&th_sel).filter_map(cell_text).collect();
                if hs.is_empty() {
                    hs = tr.select(&td_sel).filter_map(cell_text).collect();
                }
                hs
            }
            None => Vec::new(),
        }
    };

    // 2) 数据行：有 thead 时取全部 tr；否则跳过头行取剩余 tr。
    let data_rows: Vec<ElementRef<'_>> = if table.select(&thead_sel).next().is_some() {
        table.select(&tr_sel).collect()
    } else {
        table.select(&tr_sel).skip(1).collect()
    };

    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for tr in data_rows {
        // thead 已被单独取过，跳过它以免重复计入数据
        if tr.select(&th_sel).next().is_some() && table.select(&thead_sel).next().is_some() {
            continue;
        }
        let cells: Vec<Option<String>> = tr.select(&td_sel).map(cell_text).collect();
        if cells.iter().all(|c| c.is_none()) {
            continue;
        }
        rows.push(cells);
    }

    let names: Vec<&str> = headers.iter().map(String::as_str).collect();
    Df::from_string_rows(&names, &rows)
}

/// 解析 HTML 中所有 `<table>`，逐个返回 `Df`（顺序与文档一致）。
///
/// 对应 akshare `pd.read_html(html)` 返回的列表；调用方按索引取目标表
/// （多数债券场景取 `[0]` 或 `[10]`）。空 HTML / 无表格时报 `Empty`。
pub fn read_html(html: &str) -> Result<Vec<Df>> {
    let doc = Html::parse_document(html);
    let table_sel = Selector::parse("table")
        .map_err(|e| AkshareError::Empty(format!("table 选择器解析失败: {e}")))?;
    let mut out: Vec<Df> = Vec::new();
    for table in doc.select(&table_sel) {
        out.push(parse_one_table(&table)?);
    }
    if out.is_empty() {
        return Err(AkshareError::Empty("HTML 中未找到任何 <table>".into()));
    }
    Ok(out)
}
