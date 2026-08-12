//! HTML 表格解析（对应 pandas `read_html` 的核心语义）。
//!
//! 仅覆盖本工程所需的最简形态：把 HTML 文本中的 `<table>` 解析为
//! `table -> row -> cell` 的二维字符串数组。调用方据此重建 DataFrame
//! （通常跳过首行表头，按列索引重建列名，与 akshare 的 `read_html()[i]`
//! 之后 `columns=[...]` 重命名一致）。
//!
//! 不处理 `rowspan`/`colspan` 跨格（当前 use case 的表格均为简单二维表）。

use crate::core::error::{AkshareError, Result};
use scraper::{Html, Selector};

/// 折叠单元格内空白（对应 pandas read_html 的空白处理）。
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// 解析 HTML 文本中所有 `<table>`。
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
        let html = "<table><tr><td>a</td></tr></table><div></div><table><tr><td>b</td></tr></table>";
        let tables = read_html_tables(html).unwrap();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[1][0][0], "b");
    }
}
