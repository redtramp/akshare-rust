//! 碳排放交易数据源（energy 分类）。
//!
//! 对应 akshare `energy/energy_carbon.py` 全部 6 个函数：
//! - 广州碳排放权交易中心行情（`energy_carbon_gz`）
//! - 湖北碳排放权交易中心现货每日概况（`energy_carbon_hb`）
//! - 碳交易网行情信息（`energy_carbon_domestic`，`k.tanjiaoyi.com:8080`，8 个交易试点）
//! - 北京碳排放权交易平台公开行情（`energy_carbon_bj`，分页 HTML）
//! - 深圳碳排放交易所国内碳情（`energy_carbon_sz`）/ 国际碳情（`energy_carbon_eu`，分页 HTML）
//!
//! 注：`energy_carbon_sz` / `energy_carbon_eu` 上游 `cerx.cn` 在本机不可达（连接超时），
//! 与 akshare 同样失败；已按 akshare 原逻辑实现，待环境恢复后可对账。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::html::read_html_tables;
use crate::core::http::HttpClient;
use scraper::{Html, Selector};
use serde_json::{Map, Value};

const GZ_URL: &str = "http://ets.cnemission.com/carbon/portalIndex/markethistory";
const HB_URL: &str = "https://www.hbets.cn/";
const DOMESTIC_URL: &str = "http://k.tanjiaoyi.com:8080/KDataController/getHouseDatasInAverage.do";
const BJ_URL: &str = "https://www.bjets.com.cn/article/jyxx/";

const GZ_HEADERS: &[(&str, &str)] = &[(
    "user-agent",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
)];

/// 广州碳排放权交易中心-行情信息（对应 akshare [`energy_carbon_gz`]）。
///
/// # 返回列
/// `日期, 品种, 开盘价, 收盘价, 最高价, 最低价, 涨跌, 涨跌幅, 成交数量, 成交金额`
pub fn energy_carbon_gz() -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("Top".into(), Value::String("1".into()));
    params.insert("beginTime".into(), Value::String("2010-01-01".into()));
    params.insert("endTime".into(), Value::String("2030-09-12".into()));

    let text = http.get_text_with_headers(GZ_URL, &params, GZ_HEADERS, None)?;
    let tables = read_html_tables(&text)?;
    let table = tables
        .get(1)
        .ok_or_else(|| AkshareError::Empty("广州碳市场未解析到行情表".into()))?;
    if table.len() < 2 {
        return Err(AkshareError::Empty("广州碳市场行情表为空".into()));
    }
    // 跳过首行表头（pd.read_html(header=0) 语义）
    let data_rows: Vec<Vec<Option<String>>> = table[1..]
        .iter()
        .map(|r| r.iter().map(|c| Some(c.clone())).collect())
        .collect();

    let names = [
        "日期",
        "品种",
        "开盘价",
        "收盘价",
        "最高价",
        "最低价",
        "涨跌",
        "涨跌幅",
        "成交数量",
        "成交金额",
    ];
    let mut df = Df::from_string_rows(&names, &data_rows)?;
    df.cast_date(&["日期"])?;
    // 涨跌幅带百分号，先剥离再数值化
    df.strip_suffix(&["涨跌幅"], "%")?;
    df.cast_numeric(&[
        "开盘价",
        "收盘价",
        "最高价",
        "最低价",
        "涨跌",
        "涨跌幅",
        "成交数量",
        "成交金额",
    ])?;
    // akshare 按日期升序
    let sorted = df.sort_by("日期", true, false)?;
    Ok(sorted)
}

/// 湖北碳排放权交易中心-现货交易数据-配额-每日概况（对应 akshare [`energy_carbon_hb`]）。
///
/// # 返回列
/// `日期, 成交价, 成交量, 最新, 涨跌`
pub fn energy_carbon_hb() -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text_with_headers(HB_URL, &Map::new(), GZ_HEADERS, None)?;

    // 提取内联脚本中 `cjj = '[...]'` 的 JSON 数组（与 akshare 切片逻辑一致）
    let start = text
        .find("cjj = '[")
        .ok_or_else(|| AkshareError::Empty("湖北碳市场未找到 cjj 数组".into()))?
        + 7;
    let end = text
        .rfind("cjj =")
        .ok_or_else(|| AkshareError::Empty("湖北碳市场未找到 cjj 结束".into()))?
        - 31;
    if end <= start {
        return Err(AkshareError::Empty("湖北碳市场 cjj 数组切片异常".into()));
    }
    let sub = &text[start..end];
    let arr: Vec<Value> =
        serde_json::from_str(sub).map_err(|e| AkshareError::json(HB_URL, e.to_string()))?;

    let rows: Vec<Vec<Option<String>>> = arr
        .iter()
        .map(|v| {
            let get = |k: &str| -> Option<String> {
                v.get(k).and_then(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    Value::Null => None,
                    other => Some(other.to_string()),
                })
            };
            vec![get("riqi"), get("cjj"), get("cjl"), get("zx"), get("zd")]
        })
        .collect();

    let mut df = Df::from_string_rows(&["日期", "成交价", "成交量", "最新", "涨跌"], &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["成交价", "成交量", "最新", "涨跌"])?;
    Ok(df)
}

/// JSON 标量 → 字符串（Null → None）。
fn carbon_cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// 按 `symbol` 从碳交易网响应对象提取行（对应 akshare 的位置列重命名）。
///
/// 原始字段顺序为 `deal, HOUSEID, DEALNUM, HOUSENAME, DEALAMOUNT, INDATE, houseCnt`，
/// akshare 将其位置重命名为 `成交价, _, 成交量, 地点, 成交额, 日期, _`。
fn carbon_domestic_rows(value: &Value, symbol: &str) -> Vec<Vec<Option<String>>> {
    value
        .get(symbol)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    let get = |k: &str| v.get(k).and_then(carbon_cell);
                    vec![
                        get("INDATE"),
                        get("deal"),
                        get("DEALNUM"),
                        get("DEALAMOUNT"),
                        get("HOUSENAME"),
                    ]
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 碳交易网-行情信息（对应 akshare [`energy_carbon_domestic`]）。
///
/// - `symbol`: `湖北` / `上海` / `北京` / `重庆` / `广东` / `天津` / `深圳` / `福建`
///
/// # 返回列
/// `日期, 成交价, 成交量, 成交额, 地点`
pub fn energy_carbon_domestic(symbol: &str) -> Result<Df> {
    const SYMBOLS: [&str; 8] = [
        "湖北", "上海", "北京", "重庆", "广东", "天津", "深圳", "福建",
    ];
    if !SYMBOLS.contains(&symbol) {
        return Err(AkshareError::Param(format!(
            "无效 symbol: {symbol}，可选 湖北/上海/北京/重庆/广东/天津/深圳/福建"
        )));
    }
    let mut params = Map::new();
    params.insert(
        "lcnK".into(),
        Value::String("53f75bfcefff58e4046ccfa42171636c".into()),
    );
    params.insert("brand".into(), Value::String("TAN".into()));
    let http = HttpClient::default();
    let text = http.get_text(DOMESTIC_URL, &params, None)?;
    // 响应形如 `null({...})`：取首个 "(" 与最后一个 ")" 之间（akshare 切片语义）
    let start = text
        .find('(')
        .ok_or_else(|| AkshareError::Empty("碳交易网响应缺少 (".into()))?
        + 1;
    let end = text
        .rfind(')')
        .ok_or_else(|| AkshareError::Empty("碳交易网响应缺少 )".into()))?;
    if end <= start {
        return Err(AkshareError::Empty("碳交易网响应切片异常".into()));
    }
    let value: Value = serde_json::from_str(&text[start..end])
        .map_err(|e| AkshareError::json(DOMESTIC_URL, e.to_string()))?;
    let rows = carbon_domestic_rows(&value, symbol);
    let mut df = Df::from_string_rows(&["日期", "成交价", "成交量", "成交额", "地点"], &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["成交价", "成交量", "成交额"])?;
    Ok(df)
}

/// 拆分北京碳市场「成交额」单元，如 `2,548,260.51(BEA)` → (`2548260.51`, `BEA`)。
fn split_amount_unit(cell: &str) -> (String, Option<String>) {
    let cell = cell.trim();
    match cell.char_indices().find(|(_, c)| *c == '(' || *c == '（') {
        Some((i, ch)) => {
            let amount = cell[..i].trim().replace(',', "");
            let unit = cell[i + ch.len_utf8()..]
                .split([')', '）'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            (amount, if unit.is_empty() { None } else { Some(unit) })
        }
        None => (cell.replace(',', ""), None),
    }
}

/// 从北京碳市场首页首张表的 `<script>` 解析总页数（对应 akshare `split("=")[-1]`）。
fn bj_total_page(html: &str) -> Result<u32> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("table script")
        .map_err(|e| AkshareError::Empty(format!("分页选择器解析失败: {e}")))?;
    let script = doc
        .select(&sel)
        .next()
        .map(|n| n.text().collect::<String>())
        .ok_or_else(|| AkshareError::Empty("北京碳市场未找到分页脚本".into()))?;
    let token = script
        .split('=')
        .next_back()
        .unwrap_or("")
        .trim()
        .trim_end_matches(';')
        .trim_matches('"');
    token
        .parse::<u32>()
        .map_err(|_| AkshareError::Empty(format!("北京碳市场总页数解析失败: {token}")))
}

/// 北京市碳排放权电子交易平台-公开交易行情（对应 akshare [`energy_carbon_bj`]）。
///
/// 首页无分页参数，其余页为 `?{i}`；共约 150 页。
///
/// # 返回列
/// `日期, 成交量, 成交均价, 成交额, 成交单位`
pub fn energy_carbon_bj() -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text_with_headers(BJ_URL, &Map::new(), GZ_HEADERS, None)?;
    let total_page = bj_total_page(&text)?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for page in 1..=total_page {
        let suffix = if page == 1 {
            String::new()
        } else {
            page.to_string()
        };
        let url = format!("{BJ_URL}?{suffix}");
        let page_text = http.get_text_with_headers(&url, &Map::new(), GZ_HEADERS, None)?;
        let tables = read_html_tables(&page_text)?;
        let table = match tables.first() {
            Some(t) => t,
            None => continue,
        };
        for r in table.iter().skip(1) {
            if r.len() < 4 {
                continue;
            }
            let (amount, unit) = split_amount_unit(&r[3]);
            rows.push(vec![
                Some(r[0].trim().to_string()),
                Some(r[1].trim().to_string()),
                Some(r[2].trim().to_string()),
                Some(amount),
                unit,
            ]);
        }
    }
    let mut df =
        Df::from_string_rows(&["日期", "成交量", "成交均价", "成交额", "成交单位"], &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["成交量", "成交均价", "成交额"])?;
    df.sort_by("日期", true, false)
}

/// 从深圳碳交易所页面 `div.pagebar` 的最后一个 `<option>` 解析总页数。
fn cerx_page_num(html: &str) -> Result<u32> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("div.pagebar option")
        .map_err(|e| AkshareError::Empty(format!("分页选择器解析失败: {e}")))?;
    let last = doc
        .select(&sel)
        .next_back()
        .map(|n| n.text().collect::<String>())
        .ok_or_else(|| AkshareError::Empty("碳交易所未找到分页".into()))?;
    last.trim()
        .parse::<u32>()
        .map_err(|_| AkshareError::Empty(format!("碳交易所总页数解析失败: {last}")))
}

/// 深圳碳交易所分页表抓取公共实现（国内/国际碳情同构）。
fn cerx_market(first_url: &str, page_pattern: &str) -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text_with_headers(first_url, &Map::new(), GZ_HEADERS, None)?;
    let page_num = cerx_page_num(&text)?;
    let tables = read_html_tables(&text)?;
    let header: Vec<String> = tables
        .first()
        .and_then(|t| t.first())
        .cloned()
        .unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    if let Some(t) = tables.first() {
        for r in t.iter().skip(1) {
            rows.push(r.iter().map(|c| Some(c.clone())).collect());
        }
    }
    for page in 2..=page_num {
        let url = page_pattern.replace("{page}", &page.to_string());
        let page_text = http.get_text_with_headers(&url, &Map::new(), GZ_HEADERS, None)?;
        if let Ok(page_tables) = read_html_tables(&page_text) {
            if let Some(t) = page_tables.first() {
                for r in t.iter().skip(1) {
                    rows.push(r.iter().map(|c| Some(c.clone())).collect());
                }
            }
        }
    }
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &rows)?;
    df.cast_date(&["交易日期"])?;
    df.cast_numeric(&[
        "开盘价",
        "最高价",
        "最低价",
        "成交均价",
        "收盘价",
        "成交量",
        "成交额",
    ])?;
    df.sort_by("交易日期", true, false)
}

/// 深圳碳排放交易所-国内碳情（对应 akshare [`energy_carbon_sz`]）。
///
/// # 返回列
/// `交易日期, 开盘价, 最高价, 最低价, 成交均价, 收盘价, 成交量, 成交额`
pub fn energy_carbon_sz() -> Result<Df> {
    cerx_market(
        "http://www.cerx.cn/dailynewsCN/index.htm",
        "http://www.cerx.cn/dailynewsCN/index_{page}.htm",
    )
}

/// 深圳碳排放交易所-国际碳情（对应 akshare [`energy_carbon_eu`]）。
///
/// # 返回列
/// `交易日期, 开盘价, 最高价, 最低价, 成交均价, 收盘价, 成交量, 成交额`
pub fn energy_carbon_eu() -> Result<Df> {
    cerx_market(
        "http://www.cerx.cn/dailynewsOuter/index.htm",
        "http://www.cerx.cn/dailynewsOuter/index_{page}.htm",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_amount_unit_strips_commas_and_unit() {
        assert_eq!(
            split_amount_unit("2,548,260.51(BEA)"),
            ("2548260.51".to_string(), Some("BEA".to_string()))
        );
        assert_eq!(
            split_amount_unit("163,053.00（PCER）"),
            ("163053.00".to_string(), Some("PCER".to_string()))
        );
        assert_eq!(split_amount_unit("1234"), ("1234".to_string(), None));
    }

    #[test]
    fn bj_total_page_reads_last_assignment() {
        let html = "<table><tr><th>日期</th></tr>\n<script>\n currentpage = \"1\";\n totalpage = \"150\";\n</script></table>";
        assert_eq!(bj_total_page(html).unwrap(), 150);
    }

    #[test]
    fn cerx_page_num_reads_last_option() {
        let html =
            "<div class='pagebar'><option>1</option><option>2</option><option>37</option></div>";
        assert_eq!(cerx_page_num(html).unwrap(), 37);
    }

    #[test]
    fn carbon_domestic_rows_maps_positional_fields() {
        let value: Value = serde_json::from_str(
            r#"{"湖北":[{"deal":21,"HOUSEID":"x","DEALNUM":510020,"HOUSENAME":"湖北","DEALAMOUNT":1.07104E7,"INDATE":"2014-04-02","houseCnt":1}]}"#,
        )
        .unwrap();
        let rows = carbon_domestic_rows(&value, "湖北");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].as_deref(), Some("2014-04-02"));
        assert_eq!(rows[0][1].as_deref(), Some("21"));
        assert_eq!(rows[0][2].as_deref(), Some("510020"));
        assert_eq!(rows[0][4].as_deref(), Some("湖北"));
    }

    #[test]
    fn gz_strips_percent_before_numeric() {
        // 涨跌幅 "0.47%" → 剥离后数值化
        let rows = vec![vec![
            Some("20260811".into()),
            Some("GDEA".into()),
            Some("38.15".into()),
            Some("38.33".into()),
            Some("38.5".into()),
            Some("37.95".into()),
            Some("0.18".into()),
            Some("0.47%".into()),
            Some("68620".into()),
            Some("2629623.29".into()),
        ]];
        let mut df = Df::from_string_rows(
            &[
                "日期",
                "品种",
                "开盘价",
                "收盘价",
                "最高价",
                "最低价",
                "涨跌",
                "涨跌幅",
                "成交数量",
                "成交金额",
            ],
            &rows,
        )
        .unwrap();
        df.cast_date(&["日期"]).unwrap();
        df.strip_suffix(&["涨跌幅"], "%").unwrap();
        df.cast_numeric(&["涨跌幅"]).unwrap();
        assert_eq!(df.column_names()[7], "涨跌幅");
    }
}
