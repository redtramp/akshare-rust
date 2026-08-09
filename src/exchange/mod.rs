//! 交易所数据源（上交所/深交所）。
//!
//! 首批实现（对应 akshare `stock_feature/stock_margin_sse.py` 与
//! `stock_margin_szse.py` 的 JSON 接口）：
//! - [`stock_margin_sse`]：上交所融资融券汇总
//! - [`stock_margin_detail_sse`]：上交所融资融券明细
//! - [`stock_margin_szse`]：深交所融资融券汇总
//!
//! 说明：`stock_margin_detail_szse` 走 xlsx 下载（需 Excel 解析库），暂缓。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use serde_json::{json, Value};

const SSE_REFERER: &str = "https://www.sse.com.cn/";
const SZSE_REFERER: &str = "https://www.szse.cn/disclosure/margin/object/index.html";

/// 上交所融资融券汇总（对应 akshare [`akshare.stock_margin_sse`]）。
///
/// `start_date`/`end_date`: `YYYYMMDD`。
///
/// # 返回列
/// `信用交易日期, 融资余额, 融资买入额, 融券余量, 融券余量金额, 融券卖出量, 融资融券余额`
pub fn stock_margin_sse(start_date: &str, end_date: &str) -> Result<Df> {
    let url = "https://query.sse.com.cn/marketdata/tradedata/queryMargin.do";
    let params = json!({
        "isPagination": "true",
        "beginDate": start_date,
        "endDate": end_date,
        "tabType": "",
        "stockCode": "",
        "pageHelp.pageSize": "5000",
        "pageHelp.pageNo": "1",
        "pageHelp.beginPage": "1",
        "pageHelp.cacheSize": "1",
        "pageHelp.endPage": "5",
    });
    let http = HttpClient::default();
    let data = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SSE_REFERER),
    )?;
    let rows = data.get("result").and_then(Value::as_array).cloned();
    let rows = rows.unwrap_or_default();
    // 位置列名映射（akshare 的 13 列名表，"_" 为占位；响应键序 = 位置序）
    const SRC_ORDER: [&str; 13] = [
        "_",
        "信用交易日期",
        "_",
        "融券卖出量",
        "融券余量",
        "融券余量金额",
        "_",
        "_",
        "融资买入额",
        "融资融券余额",
        "融资余额",
        "_",
        "_",
    ];
    const OUT_ORDER: [&str; 7] = [
        "信用交易日期",
        "融资余额",
        "融资买入额",
        "融券余量",
        "融券余量金额",
        "融券卖出量",
        "融资融券余额",
    ];
    margin_sse_df(&rows, &SRC_ORDER, &OUT_ORDER)
}

/// 上交所融资融券明细（对应 akshare [`akshare.stock_margin_detail_sse`]）。
///
/// `date`: `YYYYMMDD`。
///
/// # 返回列
/// `信用交易日期, 标的证券代码, 标的证券简称, 融资余额, 融资买入额, 融资偿还额,
/// 融券余量, 融券卖出量, 融券偿还量`
pub fn stock_margin_detail_sse(date: &str) -> Result<Df> {
    let url = "https://query.sse.com.cn/marketdata/tradedata/queryMargin.do";
    let params = json!({
        "isPagination": "true",
        "tabType": "mxtype",
        "detailsDate": date,
        "stockCode": "",
        "beginDate": "",
        "endDate": "",
        "pageHelp.pageSize": "5000",
        "pageHelp.pageCount": "50",
        "pageHelp.pageNo": "1",
        "pageHelp.beginPage": "1",
        "pageHelp.cacheSize": "1",
        "pageHelp.endPage": "21",
    });
    let http = HttpClient::default();
    let data = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SSE_REFERER),
    )?;
    let rows = data.get("result").and_then(Value::as_array).cloned();
    let rows = rows.unwrap_or_default();
    // 位置列名映射（akshare 的 13 列名表，"_" 为占位）
    const SRC_ORDER: [&str; 13] = [
        "_",
        "信用交易日期",
        "融券偿还量",
        "融券卖出量",
        "融券余量",
        "_",
        "_",
        "融资偿还额",
        "融资买入额",
        "_",
        "融资余额",
        "标的证券简称",
        "标的证券代码",
    ];
    const OUT_ORDER: [&str; 9] = [
        "信用交易日期",
        "标的证券代码",
        "标的证券简称",
        "融资余额",
        "融资买入额",
        "融资偿还额",
        "融券余量",
        "融券卖出量",
        "融券偿还量",
    ];
    margin_sse_df(&rows, &SRC_ORDER, &OUT_ORDER)
}

/// SSE 融资融券公共变换：按响应键序取列 → 位置映射到目标列序。
///
/// 注意：akshare 用 `pd.DataFrame(result)`（列序=响应键序）+ 位置列名表覆盖，
/// 因此目标列 = 响应第 i 键对应 OUT 表第 i 个名字。此处按位置取即可。
fn margin_sse_df(rows: &[Value], names: &[&str], out_order: &[&str]) -> Result<Df> {
    if rows.is_empty() {
        return Df::from_string_rows(out_order, &[]);
    }
    let mut out_rows: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in rows {
        let obj = r
            .as_object()
            .ok_or_else(|| crate::core::error::AkshareError::Empty("result 元素非对象".into()))?;
        // 位置映射：第 i 个响应值 → names[i]
        let values: Vec<Option<String>> = obj
            .values()
            .take(names.len())
            .map(|v| match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect();
        let mut row: Vec<Option<String>> = Vec::with_capacity(names.len());
        for i in 0..names.len() {
            row.push(values.get(i).cloned().flatten());
        }
        out_rows.push(row);
    }
    // 用唯一占位列名构建（polars 不允许重复列名），再映射到目标名
    let placeholders: Vec<String> = (0..names.len()).map(|i| format!("c{i}")).collect();
    let ph_refs: Vec<&str> = placeholders.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&ph_refs, &out_rows)?;
    // 位置重命名：c{i} → names[i]
    let renamed: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            if *n == "_" {
                format!("c{i}")
            } else {
                n.to_string()
            }
        })
        .collect();
    let renamed_refs: Vec<&str> = renamed.iter().map(String::as_str).collect();
    df.rename_columns(&renamed_refs)?;
    // 只保留目标列
    df = df.select(out_order)?;
    // 数值列
    let numeric: Vec<&str> = out_order
        .iter()
        .copied()
        .filter(|c| *c != "信用交易日期" && *c != "标的证券代码" && *c != "标的证券简称")
        .collect();
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 深交所融资融券汇总（对应 akshare [`akshare.stock_margin_szse`]）。
///
/// `date`: `YYYYMMDD`。
///
/// # 返回列
/// `融资买入额, 融资余额, 融券卖出量, 融券余量, 融券余额, 融资融券余额`
pub fn stock_margin_szse(date: &str) -> Result<Df> {
    let url = "https://www.szse.cn/api/report/ShowReport/data";
    let d = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
    let params = json!({
        "SHOWTYPE": "JSON",
        "CATALOGID": "1837_xxpl",
        "txtDate": d,
        "tab1PAGENO": "1",
        "random": "0.7425245522795993",
    });
    let http = HttpClient::default();
    let data = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SZSE_REFERER),
    )?;
    let arr = data.as_array().cloned().unwrap_or_default();
    let rows = arr
        .first()
        .and_then(|v| v.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if rows.is_empty() {
        return Df::from_string_rows(
            &[
                "融资买入额",
                "融资余额",
                "融券卖出量",
                "融券余量",
                "融券余额",
                "融资融券余额",
            ],
            &[],
        );
    }
    let df = Df::from_json_rows(&rows)?;
    // 重命名（akshare 按位置映射 6 列）
    const OUT: [&str; 6] = [
        "融资买入额",
        "融资余额",
        "融券卖出量",
        "融券余量",
        "融券余额",
        "融资融券余额",
    ];
    let mut out = df;
    let cur = out.column_names();
    let take: Vec<&str> = cur.iter().take(6).map(String::as_str).collect();
    out = out.select(&take)?;
    out.rename_columns(&OUT)?;
    // 千分位逗号移除 + 数值化
    out.strip_commas(&OUT)?;
    out.cast_numeric(&OUT)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_margin_position_mapping() {
        let rows = vec![serde_json::json!({
            "ROWNUM_": null,
            "opDate": "20240809",
            "rqchl": null,
            "rqmcl": 53384990,
            "rqyl": 260469,
            "rqylje": 1234567,
            "rzche": null,
            "rzjmr": null,
            "rzmre": 999888777,
            "rzrqye": 555555,
            "rzye": 111222,
            "scode": null,
            "sname": null,
        })];
        let out = margin_sse_df(
            &rows,
            &[
                "_",
                "信用交易日期",
                "_",
                "融券卖出量",
                "融券余量",
                "融券余量金额",
                "_",
                "_",
                "融资买入额",
                "融资融券余额",
                "融资余额",
                "_",
                "_",
            ],
            &[
                "信用交易日期",
                "融资余额",
                "融资买入额",
                "融券余量",
                "融券余量金额",
                "融券卖出量",
                "融资融券余额",
            ],
        )
        .unwrap();
        assert_eq!(
            out.column_names(),
            vec![
                "信用交易日期",
                "融资余额",
                "融资买入额",
                "融券余量",
                "融券余量金额",
                "融券卖出量",
                "融资融券余额",
            ]
        );
        assert_eq!(out.height(), 1);
    }

    #[test]
    fn sse_margin_empty() {
        let out = margin_sse_df(&[], &["_", "信用交易日期"], &["信用交易日期"]).unwrap();
        assert_eq!(out.height(), 0);
        assert_eq!(out.column_names(), vec!["信用交易日期"]);
    }
}
