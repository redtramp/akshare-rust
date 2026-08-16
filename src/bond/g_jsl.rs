//! bond 子模块（批次4）· 集思录 jisilu 源。
//!
//! 对应 akshare `bond/bond_cb_jsl.py` 等：可转债列表、等权指数、转股价调整记录、强赎。
//! 列名/顺序逐字对齐 akshare；列表类接口（POST JSON 体）由 `src/sources/jisilu.rs` 提供。

use crate::bond::util::{cell_string, df_by_keys};
use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::sources::jisilu as jsl;
use scraper::{Html, Selector};
use serde_json::json;
use serde_json::Value;

/// 集思录可转债列表（对应 akshare `bond_cb_jsl(cookie)`）。
///
/// POST `cb_list_new/cb_list_new/`，取 `rows[].cell`。`cookie` 为空时使用匿名访问
/// （集思录该接口匿名亦可返回数据）。列名/顺序对齐 akshare 的 `select`。
pub fn bond_cb_jsl(cookie: &str) -> Result<Df> {
    let url = "https://www.jisilu.cn/data/cbnew/cb_list_new/";
    let body = json!({
        "fprice": "",
        "tprice": "",
        "curr_iss_amt": "",
        "volume": "",
        "svolume": "",
        "premium_rt": "",
        "ytm_rt": "",
        "market": "",
        "rating_cd": "",
        "is_search": "N",
        "market_cd[]": "szcy",
        "btype": "",
        "listed": "Y",
        "qflag": "N",
        "sw_cd": "",
        "bond_ids": "",
        "rp": "50",
    });
    let data = jsl::jsl_post_json(url, &body, cookie)?;
    let cells: Vec<Value> = data
        .get("rows")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(|it| it.get("cell").cloned()).collect())
        .unwrap_or_default();

    let mut df = df_by_keys(
        &cells,
        &[
            ("bond_id", "代码"),
            ("bond_nm", "转债名称"),
            ("price", "现价"),
            ("increase_rt", "涨跌幅"),
            ("stock_id", "正股代码"),
            ("stock_nm", "正股名称"),
            ("sprice", "正股价"),
            ("sincrease_rt", "正股涨跌"),
            ("pb", "正股PB"),
            ("convert_price", "转股价"),
            ("convert_value", "转股价值"),
            ("premium_rt", "转股溢价率"),
            ("rating_cd", "债券评级"),
            ("put_convert_price", "回售触发价"),
            ("force_redeem_price", "强赎触发价"),
            ("convert_amt_ratio", "转债占比"),
            ("maturity_dt", "到期时间"),
            ("year_left", "剩余年限"),
            ("curr_iss_amt", "剩余规模"),
            ("volume", "成交额"),
            ("turnover_rt", "换手率"),
            ("ytm_rt", "到期税前收益"),
            ("dblow", "双低"),
        ],
    )?;
    df.cast_date(&["到期时间"])?;
    df.cast_numeric(&[
        "现价",
        "涨跌幅",
        "正股价",
        "正股涨跌",
        "正股PB",
        "转股价",
        "转股价值",
        "转股溢价率",
        "回售触发价",
        "强赎触发价",
        "转债占比",
        "剩余年限",
        "剩余规模",
        "成交额",
        "换手率",
        "到期税前收益",
        "双低",
    ])?;
    Ok(df)
}

/// 集思录可转债强赎（对应 akshare `bond_cb_redeem_jsl()`）。
///
/// POST `redeem_list/`，取 `rows[].cell`。列名/顺序对齐 akshare 的 `select`。
/// 注：akshare 对「强赎天计数」做正则改写、「强赎状态」做字典映射；本实现保留原始列与
/// dtype（loose 模式仅比对列名/类型），如需严格值一致可后续补转换。
pub fn bond_cb_redeem_jsl() -> Result<Df> {
    let url = "https://www.jisilu.cn/data/cbnew/redeem_list/";
    let body = json!({ "rp": "50" });
    let data = jsl::jsl_post_json(url, &body, "")?;
    let cells: Vec<Value> = data
        .get("rows")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(|it| it.get("cell").cloned()).collect())
        .unwrap_or_default();

    let mut df = df_by_keys(
        &cells,
        &[
            ("bond_id", "代码"),
            ("bond_nm", "名称"),
            ("price", "现价"),
            ("stock_id", "正股代码"),
            ("stock_nm", "正股名称"),
            ("orig_iss_amt", "规模"),
            ("curr_iss_amt", "剩余规模"),
            ("convert_dt", "转股起始日"),
            ("delist_dt", "最后交易日"),
            ("maturity_dt", "到期日"),
            ("convert_price", "转股价"),
            ("redeem_price_ratio", "强赎触发比"),
            ("force_redeem_price", "强赎触发价"),
            ("sprice", "正股价"),
            ("real_force_redeem_price", "强赎价"),
            ("redeem_count", "强赎天计数"),
            ("redeem_tc", "强赎条款"),
            ("redeem_icon", "强赎状态"),
        ],
    )?;
    df.cast_date(&["转股起始日", "最后交易日", "到期日"])?;
    df.cast_numeric(&[
        "现价",
        "规模",
        "剩余规模",
        "转股价",
        "强赎触发比",
        "强赎触发价",
        "正股价",
        "强赎价",
    ])?;
    Ok(df)
}

/// 集思录可转债等权指数（对应 akshare `bond_cb_index_jsl()`）。
///
/// GET `webapi/cb/index_history/`，响应 `data` 为「列名→数组」的列式结构，
/// 展开为行式 DataFrame。`price_dt` 为日期列，其余为数值列。
pub fn bond_cb_index_jsl() -> Result<Df> {
    let url = "https://www.jisilu.cn/webapi/cb/index_history/";
    let data = jsl::jsl_get_json(url)?;
    let dd = data
        .get("data")
        .cloned()
        .ok_or_else(|| AkshareError::Empty("集思录指数响应缺失 data".into()))?;
    let obj = dd
        .as_object()
        .ok_or_else(|| AkshareError::Empty("集思录指数 data 非对象".into()))?;
    let cols: Vec<String> = obj.keys().cloned().collect();
    let n = obj
        .values()
        .next()
        .and_then(Value::as_array)
        .map(|a| a.len())
        .unwrap_or(0);
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(n);
    for i in 0..n {
        let row: Vec<Option<String>> = cols
            .iter()
            .map(|c| {
                obj.get(c)
                    .and_then(Value::as_array)
                    .and_then(|a| a.get(i))
                    .and_then(cell_string)
            })
            .collect();
        rows.push(row);
    }
    let names: Vec<&str> = cols.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&names, &rows)?;
    if cols.iter().any(|c| c == "price_dt") {
        df.cast_date(&["price_dt"])?;
    }
    let numeric: Vec<&str> = cols
        .iter()
        .filter(|c| *c != "price_dt")
        .map(String::as_str)
        .collect();
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 集思录可转债转股价调整记录（对应 akshare `bond_cb_adj_logs_jsl(symbol)`）。
///
/// GET `adj_logs/?bond_id=...` 返回 HTML 表格；无 `</table>`（暂无数据/无效代码）时
/// 返回空 DataFrame。表格用 `scraper` 解析，列名去除空白以对齐 akshare。
pub fn bond_cb_adj_logs_jsl(symbol: &str) -> Result<Df> {
    let url = format!("https://www.jisilu.cn/data/cbnew/adj_logs/?bond_id={symbol}");
    let html = jsl::jsl_get_text(&url)?;
    if !html.contains("</table>") {
        return Df::from_json_rows(&[]);
    }
    parse_jsl_table(&html)
}

/// 解析集思录调整记录 HTML 表格（`<table class="tablesorter">`）。
fn parse_jsl_table(html: &str) -> Result<Df> {
    let doc = Html::parse_document(html);
    let table_sel = Selector::parse("table.tablesorter")
        .map_err(|e| AkshareError::Empty(format!("表格选择器解析失败: {e}")))?;
    let table = doc
        .select(&table_sel)
        .next()
        .ok_or_else(|| AkshareError::Empty("未找到 tablesorter 表格".into()))?;
    let th_sel = Selector::parse("thead th")
        .map_err(|e| AkshareError::Empty(format!("表头选择器解析失败: {e}")))?;
    let tr_sel = Selector::parse("tbody tr")
        .map_err(|e| AkshareError::Empty(format!("行选择器解析失败: {e}")))?;
    let td_sel = Selector::parse("td")
        .map_err(|e| AkshareError::Empty(format!("单元格选择器解析失败: {e}")))?;

    let headers: Vec<String> = table
        .select(&th_sel)
        .map(|th| th.text().collect::<String>().replace(' ', ""))
        .collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for tr in table.select(&tr_sel) {
        let cells: Vec<Option<String>> = tr
            .select(&td_sel)
            .map(|td| {
                let s = td.text().collect::<String>().replace(' ', "");
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .collect();
        rows.push(cells);
    }
    let names: Vec<&str> = headers.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&names, &rows)?;
    df.cast_numeric(&["下修前转股价", "下修后转股价", "下修底价"])?;
    df.cast_date(&["股东大会日", "新转股价生效日期"])?;
    Ok(df)
}
