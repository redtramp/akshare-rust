//! HTML 表格解析（对应 pandas `read_html` 的核心语义 / akshare 的 `pd.read_html`）。
//!
//! 提供两种粒度：
//! - [`read_html_tables`]：返回 `Vec<Vec<Vec<String>>>`（最简二维字符串数组），
//!   供 `currency_boc` / `carbon` 等直接按索引取单元格。
//! - [`read_html`]：返回 `Vec<Df>`（顺序与文档一致），供债券等需要列式 DataFrame 的场景。
//!
//! 仅覆盖本工程所需的最简形态：简单二维表，不处理 `rowspan`/`colspan` 跨格。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use scraper::{ElementRef, Html, Selector};

/// 折叠单元格内空白（对应 pandas read_html 的空白处理）。
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// 把单元格文本收成单行（去首尾空白，内部空白折叠为单空格）。
fn cell_text(el: ElementRef<'_>) -> Option<String> {
    let s: String = el
        .text()
        .collect::<Vec<_>>()
        .join("")
        .split_whitespace()
        .collect();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 解析 HTML 文本中所有 `<table>`（返回二维字符串数组）。
///
/// 返回顺序 = 文档中 `<table>` 出现顺序；每个 table 为行数组，
/// 每行是单元格文本数组（`<th>` 与 `<td>` 按文档顺序收集，已折叠空白）。
pub fn read_html_tables(html: &str) -> Result<Vec<Vec<Vec<String>>>> {
    let doc = Html::parse_document(html);
    let table_sel = Selector::parse("table").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let tr_sel = Selector::parse("tr").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let cell_sel = Selector::parse("th,td").map_err(|e| AkshareError::Empty(e.to_string()))?;

    let mut tables: Vec<Vec<Vec<String>>> = Vec::new();
    for table in doc.select(&table_sel) {
        let mut rows: Vec<Vec<String>> = Vec::new();
        for tr in table.select(&tr_sel) {
            let mut cells: Vec<String> = Vec::new();
            for c in tr.select(&cell_sel) {
                cells.push(clean(&c.text().collect::<Vec<_>>().join("")));
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
        if !rows.is_empty() {
            tables.push(rows);
        }
    }
    Ok(tables)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_table() {
        let html = r#"
        <html><body>
        <table>
          <tr><th>日期</th><th>价格</th></tr>
          <tr><td>2024-01-01</td><td>10.5</td></tr>
          <tr><td>2024-01-02</td><td>11.0</td></tr>
        </table>
        </body></html>"#;
        let tables = read_html_tables(html).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].len(), 3);
        assert_eq!(tables[0][0], vec!["日期", "价格"]);
        assert_eq!(tables[0][1], vec!["2024-01-01", "10.5"]);
        assert_eq!(tables[0][2], vec!["2024-01-02", "11.0"]);
    }

    #[test]
    fn collapses_whitespace() {
        let html = "<table><tr><td>  hello   world  </td></tr></table>";
        let tables = read_html_tables(html).unwrap();
        assert_eq!(tables[0][0][0], "hello world");
    }

    #[test]
    fn multiple_tables() {
        let html =
            "<table><tr><td>a</td></tr></table><div></div><table><tr><td>b</td></tr></table>";
        let tables = read_html_tables(html).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[1][0][0], "b");
    }
}
