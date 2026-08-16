//! 同花顺数据源（`data.10jqka.com.cn` 数据中心 + `fund.10jqka.com.cn` 理财）。
//!
//! 对应 akshare 的 `stock_feature/ths.js` 加密 + `pd.read_html` / BeautifulSoup 表格解析：
//! - 数据中心页面要求 `Cookie: v={token}`（由 `ths.js::v()` 生成，60 字符 token，
//!   经 rquickjs 执行，与 py_mini_racer 输出逐字符一致）
//! - 排名类页面为 HTML 表格（`table.m-table.J-ajax-table`），thead 为表头、tbody 为数据，
//!   总页数在 `<span class="page_info">1/24</span>` 中
//!
//! 本模块抽取自 `fund/mod.rs` 内联的 ths 逻辑，独立成源后同时服务
//! `stock_feature`（`stock_rank_*_ths`）与 `stock_fundamental`（`stock_finance_ths` 等）。

use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::core::js_engine::ths_get_v;
use scraper::{Html, Selector};
use serde_json::{Map, Value};

/// 同花顺数据中心页面 UA（与 akshare `stock_technology_ths.py` 一致）。
const THS_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/89.0.4389.90 Safari/537.36";

/// 带 v token Cookie 的 GET，返回按字符集解码后的 HTML 文本。
///
/// 对应 akshare `requests.get(url, headers={"Cookie": f"v={v_code}"})`；
/// 每次调用都重新生成 token（与 akshare 每页 `js_code.call("v")` 一致）。
pub fn fetch_ths(url: &str) -> Result<String> {
    let v = ths_get_v()?;
    let cookie = format!("v={v}");
    let http = HttpClient::default();
    http.get_text_with_headers(
        url,
        &Map::new(),
        &[("User-Agent", THS_UA), ("Cookie", &cookie)],
        None,
    )
}

/// 抓取同花顺理财 JSONP 页面（`fund.10jqka.com.cn`，无需 v token），返回剥壳后的 JSON。
///
/// 对应 akshare `fund_etf_ths.py::fund_etf_category_ths` 的 `r.text[2:-1]` 剥壳；
/// 响应形如 `g({...})`，去掉首 `g(` 与尾 `)` 后按 JSON 解析。
pub fn fetch_ths_jsonp(url: &str) -> Result<Value> {
    let http = HttpClient::default();
    let text = http.get_text(url, &Map::new(), None)?;
    let json_text = text
        .trim()
        .strip_prefix("g(")
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| AkshareError::Empty("ths jsonp 响应格式异常".into()))?;
    serde_json::from_str(json_text).map_err(|e| AkshareError::json(url, e.to_string()))
}

/// 从 `<span class="page_info">1/24</span>` 提取总页数；找不到返回 1。
pub fn total_pages(html: &str) -> u32 {
    let Ok(sel) = Selector::parse("span.page_info") else {
        return 1;
    };
    let doc = Html::parse_document(html);
    if let Some(span) = doc.select(&sel).next() {
        let text = span.text().collect::<String>();
        if let Some(p) = text.split('/').nth(1) {
            if let Ok(n) = p.trim().parse() {
                return n;
            }
        }
    }
    1
}

/// 解析页面数据表格（`table.m-table.J-ajax-table`），返回每行的 td 文本（trim）。
///
/// 表头在 thead（th），数据在 tbody（td）——与 akshare 的 `pd.read_html(header=0)[0]`
/// 及 `BeautifulSoup(tbody tr td)` 两条解析路径等价；找不到表格或没有数据行时报 `Empty`。
pub fn parse_ths_table(html: &str) -> Result<Vec<Vec<String>>> {
    let table_sel = Selector::parse("table.m-table.J-ajax-table")
        .map_err(|e| AkshareError::js(format!("解析表格选择器失败: {e}")))?;
    let tr_sel =
        Selector::parse("tr").map_err(|e| AkshareError::js(format!("解析行选择器失败: {e}")))?;
    let td_sel = Selector::parse("td")
        .map_err(|e| AkshareError::js(format!("解析单元格选择器失败: {e}")))?;

    let doc = Html::parse_document(html);
    let table = doc
        .select(&table_sel)
        .next()
        .ok_or_else(|| AkshareError::Empty("同花顺页面缺少 m-table J-ajax-table 数据表".into()))?;
    let mut out: Vec<Vec<String>> = Vec::new();
    for tr in table.select(&tr_sel) {
        let cells: Vec<String> = tr
            .select(&td_sel)
            .map(|td| td.text().collect::<String>().trim().to_string())
            .collect();
        if !cells.is_empty() {
            out.push(cells);
        }
    }
    Ok(out)
}

/// 抓取排名页全部分页并合并数据行。
///
/// `url_for_page(page)` 构造第 page 页 URL；首页同时用于探测总页数
/// （`page_info` span），随后逐页抓取合并。每页携带全新 v token。
pub fn fetch_ths_rank(url_for_page: &dyn Fn(u32) -> String) -> Result<Vec<Vec<String>>> {
    let first = fetch_ths(&url_for_page(1))?;
    let total = total_pages(&first).max(1);
    let mut rows = parse_ths_table(&first)?;
    for page in 2..=total {
        let text = fetch_ths(&url_for_page(page))?;
        rows.extend(parse_ths_table(&text)?);
    }
    Ok(rows)
}

/// 抓取概念时间表全部分页并合并 `(名称, 代码)` 对（对应 akshare
/// `__stock_board_concept_summary_ths`）。
///
/// 每页提取含 `detail` 的 `<a href=".../code/{code}/">` 链接；
/// 页码取自 `page_info` span，找不到时仅取首页。
pub fn fetch_ths_summary(url_for_page: &dyn Fn(u32) -> String) -> Result<Vec<(String, String)>> {
    let first = fetch_ths(&url_for_page(1))?;
    let total = total_pages(&first).max(1);
    let mut out = extract_detail_links(&first);
    for page in 2..=total {
        let text = fetch_ths(&url_for_page(page))?;
        out.extend(extract_detail_links(&text));
    }
    Ok(out)
}

/// 提取页面中所有含 `detail` 的 `<a>` 链接为 `(文本, 代码)`（代码取 href 倒数第 2 段）。
fn extract_detail_links(html: &str) -> Vec<(String, String)> {
    let a_sel = Selector::parse("a").expect("静态选择器");
    let doc = Html::parse_document(html);
    let mut out = Vec::new();
    for a in doc.select(&a_sel) {
        let Some(href) = a.value().attr("href") else {
            continue;
        };
        if !href.contains("detail") {
            continue;
        }
        let name = a.text().collect::<String>().trim().to_string();
        let code = href.rsplit('/').nth(1).unwrap_or("").to_string();
        if !name.is_empty() && !code.is_empty() {
            out.push((name, code));
        }
    }
    out
}

/// 解析同花顺板块名册页的 `div.cate_inner` 链接为 `(名称, 代码)` 列表
/// （对应 akshare `_get_stock_board_industry_name_ths` / `_get_stock_board_concept_name_ths`）。
pub fn parse_cate_inner(html: &str) -> Result<Vec<(String, String)>> {
    let div_sel = Selector::parse("div.cate_inner")
        .map_err(|e| AkshareError::js(format!("解析 cate_inner 选择器失败: {e}")))?;
    let a_sel =
        Selector::parse("a").map_err(|e| AkshareError::js(format!("解析 a 选择器失败: {e}")))?;
    let doc = Html::parse_document(html);
    let Some(div) = doc.select(&div_sel).next() else {
        return Err(AkshareError::Empty(
            "同花顺板块页缺少 div.cate_inner".into(),
        ));
    };
    let mut out = Vec::new();
    for a in div.select(&a_sel) {
        let Some(href) = a.value().attr("href") else {
            continue;
        };
        let name = a.text().collect::<String>().trim().to_string();
        let code = href.rsplit('/').nth(1).unwrap_or("").to_string();
        if !name.is_empty() && !code.is_empty() {
            out.push((name, code));
        }
    }
    Ok(out)
}

/// 解析板块简介页的 `div.board-infos`（dt=项目、dd=值，值内换行折叠为 `/`），
/// 返回 `(项目, 值)` 列表（对应 akshare `stock_board_*_info_ths` 的 dt/dd 提取）。
pub fn parse_board_infos(html: &str) -> Result<Vec<(String, String)>> {
    let div_sel = Selector::parse("div.board-infos")
        .map_err(|e| AkshareError::js(format!("解析 board-infos 选择器失败: {e}")))?;
    let dt_sel =
        Selector::parse("dt").map_err(|e| AkshareError::js(format!("解析 dt 选择器失败: {e}")))?;
    let dd_sel =
        Selector::parse("dd").map_err(|e| AkshareError::js(format!("解析 dd 选择器失败: {e}")))?;
    let doc = Html::parse_document(html);
    let Some(div) = doc.select(&div_sel).next() else {
        return Err(AkshareError::Empty(
            "同花顺板块简介页缺少 div.board-infos".into(),
        ));
    };
    let names: Vec<String> = div
        .select(&dt_sel)
        .map(|dt| dt.text().collect::<String>().trim().to_string())
        .collect();
    let values: Vec<String> = div
        .select(&dd_sel)
        .map(|dd| dd.text().collect::<String>().trim().replace('\n', "/"))
        .collect();
    if names.len() != values.len() {
        return Err(AkshareError::Empty("板块简介 dt/dd 数量不一致".into()));
    }
    Ok(names.into_iter().zip(values).collect())
}

/// 解析含 `<thead>`/`<tbody>` 的数据表格（对应 akshare 的
/// `thead th + tbody tr td` 提取，用于新股申购/分红/公司大事等页）。
///
/// 返回 `(表头, 数据行)`；表头取 thead 内首个 tr 的 th 文本，
/// 数据行取 tbody 内每行 tr 的 th+td 单元格文本（新股页首列为日期 th）。
pub fn parse_ths_theaded_table(html: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    parse_ths_theaded_table_sel(html, "table#maintable, table.m_table", 0)
}

/// [`parse_ths_theaded_table`] 的可选选择器版本：同一页面存在多张
/// 同类表格时（如公司大事页两张 `data_table_1`），按选择器命中顺序
/// 用 `nth`（0 起始）选取目标表。
pub fn parse_ths_theaded_table_sel(
    html: &str,
    selector: &str,
    nth: usize,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let table_sel = Selector::parse(selector)
        .map_err(|e| AkshareError::js(format!("解析表格选择器失败: {e}")))?;
    let thead_sel = Selector::parse("thead")
        .map_err(|e| AkshareError::js(format!("解析 thead 选择器失败: {e}")))?;
    let tbody_sel = Selector::parse("tbody")
        .map_err(|e| AkshareError::js(format!("解析 tbody 选择器失败: {e}")))?;
    let th_sel =
        Selector::parse("th").map_err(|e| AkshareError::js(format!("解析 th 选择器失败: {e}")))?;
    let tr_sel =
        Selector::parse("tr").map_err(|e| AkshareError::js(format!("解析 tr 选择器失败: {e}")))?;
    let td_sel =
        Selector::parse("td").map_err(|e| AkshareError::js(format!("解析 td 选择器失败: {e}")))?;

    let doc = Html::parse_document(html);
    let table = doc
        .select(&table_sel)
        .nth(nth)
        .ok_or_else(|| AkshareError::Empty(format!("同花顺页面缺少数据表格 ({selector})")))?;

    // 表头：thead 内首个 tr 的 th（get_text 折叠空白 → 拼接）
    let mut headers: Vec<String> = Vec::new();
    if let Some(thead) = table.select(&thead_sel).next() {
        if let Some(tr) = thead.select(&tr_sel).next() {
            headers = tr.select(&th_sel).map(|th| collapse_text(&th)).collect();
        }
    }

    // 数据行：tbody 内每行 tr 的 th+td 单元格（新股页首列为日期 th）
    let mut rows: Vec<Vec<String>> = Vec::new();
    if let Some(tbody) = table.select(&tbody_sel).next() {
        for tr in tbody.select(&tr_sel) {
            let mut cells: Vec<String> = Vec::new();
            for cell in tr.select(&th_sel).chain(tr.select(&td_sel)) {
                cells.push(collapse_text(&cell));
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
    }
    Ok((headers, rows))
}

/// 折叠元素文本：逐文本节点 trim 后拼接（等价于 bs4 `get_text(strip=True)`）。
fn collapse_text(el: &scraper::ElementRef<'_>) -> String {
    el.text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML: &str = r#"
<html><body>
<span class="page_info">1/24</span>
<table class="m-table J-ajax-table">
<thead><tr><th>序号</th><th>股票代码</th><th>股票简称</th></tr></thead>
<tbody>
<tr class="even"><td class="first">1</td><td><a href="http://x/000009/">000009</a></td><td>中国宝安</td></tr>
<tr class="odd"><td>2</td><td>600000</td><td>浦发银行</td></tr>
</tbody>
</table>
</body></html>"#;

    #[test]
    fn total_pages_parses() {
        assert_eq!(total_pages(SAMPLE_HTML), 24);
        assert_eq!(total_pages("<html></html>"), 1);
    }

    #[test]
    fn parse_table_rows_ok() {
        let rows = parse_ths_table(SAMPLE_HTML).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], ["1", "000009", "中国宝安"].map(str::to_string));
        assert_eq!(rows[1], vec!["2", "600000", "浦发银行"]);
    }

    #[test]
    fn parse_table_missing_is_err() {
        assert!(parse_ths_table("<html><body>无表格</body></html>").is_err());
    }

    #[test]
    fn parse_table_empty_tbody_ok() {
        // 排名无成员时 tbody 为空：应返回空行而非报错（akshare 上游此时会
        // 因 `pd.DataFrame([])` 设置列名报 Length mismatch，Rust 版保持空表列契约）。
        let html = r#"<html><body>
        <table class="m-table J-ajax-table">
        <thead><tr><th>序号</th><th>股票代码</th></tr></thead>
        <tbody></tbody>
        </table>
        </body></html>"#;
        let rows = parse_ths_table(html).unwrap();
        assert!(rows.is_empty());
    }
}
