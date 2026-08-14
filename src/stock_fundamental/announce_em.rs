//! 东财公告大全 / 主营构成（emweb F10 + np-anotice-stock）。
//!
//! 对应 akshare `stock_fundamental/stock_zygc.py`、`stock_fundamental/stock_notice.py`：
//! - [`stock_zygc_em`]：东财个股 F10「主营构成」，`emweb.securities.eastmoney.com/PC_HSF10/
//!   BusinessAnalysis/PageAjax` 返回的 `zygcfx` 数组，按 akshare 硬编码字典重命名为中文列，
//!   `分类类型` 枚举映射（`1→按行业分类` 等），收入/成本/利润/毛利率数值化，`报告日期` 归一化。
//! - [`stock_notice_report`] / [`stock_individual_notice_report`]：东财「公告大全」
//!   `np-anotice-stock.eastmoney.com/api/security/ann`。每条公告的 `codes`（可能含多证券，
//!   按 `ann_type` 以 `A` 开头者优先）与 `columns[0]` 嵌套数组分别提供 代码/名称 与 公告类型；
//!   按报告类型映射 `f_node`，按日期或个股区间分页抓取；`网址` 由 代码+art_code 拼接。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use serde_json::{json, Map, Value};

/// 东财公告大全端点（沪深京 A 股 + 科创板）。
const NOTICE_URL: &str = "https://np-anotice-stock.eastmoney.com/api/security/ann";
/// 东财个股 F10 主营构成端点。
const ZYGC_URL: &str = "https://emweb.securities.eastmoney.com/PC_HSF10/BusinessAnalysis/PageAjax";

/// 公告类型 → `f_node` 映射（对应 akshare `report_map`）。
const REPORT_MAP: &[(&str, &str)] = &[
    ("全部", "0"),
    ("财务报告", "1"),
    ("融资公告", "2"),
    ("风险提示", "3"),
    ("信息变更", "4"),
    ("重大事项", "5"),
    ("资产重组", "6"),
    ("持股变动", "7"),
];

/// 取公告类型对应的 `f_node`（未知类型回退 `"0"`）。
fn notice_node(symbol: &str) -> &str {
    REPORT_MAP
        .iter()
        .find(|(name, _)| *name == symbol)
        .map(|(_, node)| *node)
        .unwrap_or("0")
}

/// `YYYYMMDD` → `YYYY-MM-DD`（对应 akshare `"-".join([date[:4], date[4:6], date[6:]]`）。
fn fmt_ymd8(date: &str) -> String {
    if date.len() == 8 && date.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])
    } else {
        date.to_string()
    }
}

/// 截断日期时间字符串的前 10 位（对应 akshare `pd.to_datetime(...).dt.date`）。
fn date_day(raw: &str) -> String {
    let s = raw.trim();
    if s.len() >= 10 && (s.as_bytes()[4] == b'-' || s.as_bytes()[4] == b'/') {
        s[0..10].to_string()
    } else {
        s.to_string()
    }
}

/// 主营构成 `分类类型` 枚举映射（`1→按行业分类` 等），未知值原样保留。
fn map_mainop_type(v: &str) -> String {
    match v {
        "1" => "按行业分类",
        "2" => "按产品分类",
        "3" => "按地区分类",
        other => other,
    }
    .to_string()
}

/// 安全取字符串字段。
fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// 东财个股 F10 主营构成。
///
/// `symbol`：带市场标识的股票代码，如 `"SH688041"`。返回 11 列
/// `股票代码, 报告日期, 分类类型, 主营构成, 主营收入, 收入比例, 主营成本, 成本比例,
/// 主营利润, 利润比例, 毛利率`；`报告日期` 归一 `YYYY-MM-DD`，收入/成本/利润/毛利率数值化，
/// `分类类型` 枚举映射。
pub fn stock_zygc_em(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("code".to_string(), json!(symbol));
    let data = http.get_json(ZYGC_URL, &params, None)?;
    let rows = data.get("zygcfx").and_then(Value::as_array);
    let rows = match rows {
        Some(r) => r,
        None => return build_zygc_df(&[]),
    };
    build_zygc_df(rows)
}

/// 由已抓取的 `zygcfx` 数组构建主营构成 DataFrame（与网络解耦，便于离线测试）。
fn build_zygc_df(rows: &[Value]) -> Result<Df> {
    let col_names: &[&str] = &[
        "股票代码",
        "报告日期",
        "分类类型",
        "主营构成",
        "主营收入",
        "收入比例",
        "主营成本",
        "成本比例",
        "主营利润",
        "利润比例",
        "毛利率",
    ];
    let mut data: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in rows {
        data.push(vec![
            Some(str_of(r, "SECURITY_CODE")),
            Some(date_day(&str_of(r, "REPORT_DATE"))),
            Some(map_mainop_type(&str_of(r, "MAINOP_TYPE"))),
            Some(str_of(r, "ITEM_NAME")),
            Some(str_of(r, "MAIN_BUSINESS_INCOME")),
            Some(str_of(r, "MBI_RATIO")),
            Some(str_of(r, "MAIN_BUSINESS_COST")),
            Some(str_of(r, "MBC_RATIO")),
            Some(str_of(r, "MAIN_BUSINESS_RPOFIT")),
            Some(str_of(r, "MBR_RATIO")),
            Some(str_of(r, "GROSS_RPOFIT_RATIO")),
        ]);
    }
    let mut df = Df::from_string_rows(col_names, &data)?;
    df.cast_date(&["报告日期"])?;
    df.cast_numeric(&[
        "主营收入",
        "收入比例",
        "主营成本",
        "成本比例",
        "主营利润",
        "利润比例",
        "毛利率",
    ])?;
    Ok(df)
}

/// 分页抓取公告列表（沪深京 A 股或科创板）。
///
/// `ann_type`：`"A"`（沪深京）/ `"KCB"`（科创板）；`f_node` 为报告类型节点；
/// `stock_list`/`begin_time`/`end_time` 可选（个股或日期区间过滤）。
fn fetch_announcements(
    ann_type: &str,
    f_node: &str,
    stock_list: Option<&str>,
    begin_time: Option<&str>,
    end_time: Option<&str>,
) -> Result<Vec<Value>> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("sr".to_string(), json!("-1"));
    params.insert("page_size".to_string(), json!("100"));
    params.insert("ann_type".to_string(), json!(ann_type));
    params.insert("client_source".to_string(), json!("web"));
    params.insert("f_node".to_string(), json!(f_node));
    params.insert("s_node".to_string(), json!("0"));
    if let Some(s) = stock_list {
        params.insert("stock_list".to_string(), json!(s));
    }
    if let Some(b) = begin_time {
        params.insert("begin_time".to_string(), json!(b));
    }
    if let Some(e) = end_time {
        params.insert("end_time".to_string(), json!(e));
    }

    params.insert("page_index".to_string(), json!(1));
    let first = http.get_json(NOTICE_URL, &params, None)?;
    let data = match first.get("data") {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    let total_hits = data.get("total_hits").and_then(Value::as_u64).unwrap_or(0);
    let page_size = data.get("page_size").and_then(Value::as_u64).unwrap_or(100).max(1);
    let total_pages = total_hits.div_ceil(page_size);

    let mut acc: Vec<Value> = Vec::with_capacity(total_hits as usize);
    if let Some(list) = data.get("list").and_then(Value::as_array) {
        acc.extend(list.iter().cloned());
    }
    for page in 2..=total_pages {
        params.insert("page_index".to_string(), json!(page));
        match http.get_json(NOTICE_URL, &params, None) {
            Ok(v) => {
                if let Some(list) = v.get("data").and_then(|d| d.get("list")).and_then(Value::as_array)
                {
                    acc.extend(list.iter().cloned());
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    Ok(acc)
}

/// 由已抓取的公告列表数组构建 DataFrame。
///
/// `with_url=true`：输出 `网址` 列（沪深京 A 股公告，由 代码+art_code 拼接）；
/// `with_url=false`：输出 `公告代码` 列（科创板报告，直接取 art_code）。
fn build_notice_df(items: &[Value], with_url: bool) -> Result<Df> {
    let col_names: &[&str] = if with_url {
        &["代码", "名称", "公告标题", "公告类型", "公告日期", "网址"]
    } else {
        &["代码", "名称", "公告标题", "公告类型", "公告日期", "公告代码"]
    };
    let mut data: Vec<Vec<Option<String>>> = Vec::with_capacity(items.len());
    for item in items {
        let codes = item.get("codes").and_then(Value::as_array);
        let chosen = match codes {
            Some(c) if c.len() == 1 => &c[0],
            Some(c) => c
                .iter()
                .find(|x| {
                    x.get("ann_type")
                        .and_then(Value::as_str)
                        .map(|s| s.starts_with('A'))
                        .unwrap_or(false)
                })
                .unwrap_or(&c[0]),
            None => &Value::Null,
        };
        let stock_code = str_of(chosen, "stock_code");
        let short_name = str_of(chosen, "short_name");
        let title = str_of(item, "title");
        let notice_date = date_day(&str_of(item, "notice_date"));
        let column_name = item
            .get("columns")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .and_then(|c| c.get("column_name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let art_code = str_of(item, "art_code");

        let last = if with_url {
            Some(format!(
                "https://data.eastmoney.com/notices/detail/{stock_code}/{art_code}.html"
            ))
        } else {
            Some(art_code.clone())
        };
        data.push(vec![
            Some(stock_code),
            Some(short_name),
            Some(title),
            Some(column_name),
            Some(notice_date),
            last,
        ]);
    }
    let mut df = Df::from_string_rows(col_names, &data)?;
    df.cast_date(&["公告日期"])?;
    Ok(df)
}

/// 东财公告大全（按日期，全市场或指定报告类型）。
///
/// `symbol`：报告类型（`全部`/`财务报告`/`融资公告`/`风险提示`/`信息变更`/`重大事项`/
/// `资产重组`/`持股变动`）；`date`：`YYYYMMDD`。返回 6 列
/// `代码, 名称, 公告标题, 公告类型, 公告日期, 网址`。
pub fn stock_notice_report(symbol: &str, date: &str) -> Result<Df> {
    let begin = fmt_ymd8(date);
    let items = fetch_announcements("A", notice_node(symbol), None, Some(&begin), Some(&begin))?;
    build_notice_df(&items, true)
}

/// 东财公告大全（个股，按报告类型 + 日期区间）。
///
/// `security`：股票代码；`symbol`：报告类型（同 [`stock_notice_report`]）；
/// `begin_date`/`end_date`：起止日期（`YYYYMMDD` 或 `YYYY-MM-DD`）。返回 6 列
/// `代码, 名称, 公告标题, 公告类型, 公告日期, 网址`。
pub fn stock_individual_notice_report(
    security: &str,
    symbol: &str,
    begin_date: &str,
    end_date: &str,
) -> Result<Df> {
    let items = fetch_announcements(
        "A",
        notice_node(symbol),
        Some(security),
        Some(begin_date),
        Some(end_date),
    )?;
    build_notice_df(&items, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn zygc_build_offline() {
        let raw = json!([
            {
                "SECURITY_CODE": "688041",
                "REPORT_DATE": "2026-06-30 00:00:00",
                "MAINOP_TYPE": "2",
                "ITEM_NAME": "高端处理器",
                "MAIN_BUSINESS_INCOME": "9096769000",
                "MBI_RATIO": "0.999753",
                "MAIN_BUSINESS_COST": "4074821000",
                "MBC_RATIO": "0.999824",
                "MAIN_BUSINESS_RPOFIT": "5021948000",
                "MBR_RATIO": "0.999694",
                "GROSS_RPOFIT_RATIO": "0.552058"
            },
            {
                "SECURITY_CODE": "688041",
                "REPORT_DATE": "2026-06-30",
                "MAINOP_TYPE": "3",
                "ITEM_NAME": "技术服务",
                "MAIN_BUSINESS_INCOME": "1981132",
                "MBI_RATIO": "0.000218",
                "MAIN_BUSINESS_COST": "717132",
                "MBC_RATIO": "0.000176",
                "MAIN_BUSINESS_RPOFIT": "1264000",
                "MBR_RATIO": "0.000252",
                "GROSS_RPOFIT_RATIO": "0.638019"
            }
        ]);
        let rows = raw.as_array().unwrap();
        let df = build_zygc_df(rows).unwrap();
        assert_eq!(
            df.column_names(),
            vec![
                "股票代码", "报告日期", "分类类型", "主营构成", "主营收入", "收入比例",
                "主营成本", "成本比例", "主营利润", "利润比例", "毛利率"
            ]
        );
        // 分类类型枚举映射
        let typ = df.inner().column("分类类型").unwrap().str().unwrap();
        assert_eq!(typ.get(0), Some("按产品分类"));
        assert_eq!(typ.get(1), Some("按地区分类"));
        // 日期归一
        let d = df.inner().column("报告日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-06-30"));
        // 数值化
        let inc = df.inner().column("主营收入").unwrap();
        assert!(inc.f64().is_ok());
        assert_eq!(inc.f64().unwrap().get(0), Some(9.096_769e9));
    }

    #[test]
    fn notice_build_offline_multi_code() {
        // codes 含可转债 + A 股，应按 ann_type 以 A 开头者优先取 601963
        let raw = json!([
            {
                "art_code": "AN202608141827988737",
                "title": "重庆银行:关于...的公告",
                "notice_date": "2026-08-15 00:00:00",
                "codes": [
                    {"ann_type": "KZZ,Bond", "stock_code": "113056", "short_name": "重银转债"},
                    {"ann_type": "A,SHA", "stock_code": "601963", "short_name": "重庆银行"}
                ],
                "columns": [{"column_code": "001", "column_name": "借贷"}]
            }
        ]);
        let items = raw.as_array().unwrap();
        let df = build_notice_df(items, true).unwrap();
        assert_eq!(
            df.column_names(),
            vec!["代码", "名称", "公告标题", "公告类型", "公告日期", "网址"]
        );
        let code = df.inner().column("代码").unwrap().str().unwrap();
        assert_eq!(code.get(0), Some("601963"));
        let name = df.inner().column("名称").unwrap().str().unwrap();
        assert_eq!(name.get(0), Some("重庆银行"));
        let url = df.inner().column("网址").unwrap().str().unwrap();
        assert_eq!(
            url.get(0),
            Some("https://data.eastmoney.com/notices/detail/601963/AN202608141827988737.html")
        );
        let d = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-08-15"));
    }

    #[test]
    fn notice_build_offline_kcb() {
        let raw = json!([
            {
                "art_code": "AN202608141827988685",
                "title": "长盈通:股票交易异常波动公告",
                "notice_date": "2026-08-15 00:00:00",
                "codes": [{"ann_type": "A,KCB,SHA", "stock_code": "688143", "short_name": "长盈通"}],
                "columns": [{"column_code": "001002004007", "column_name": "股票交易异常波动"}]
            }
        ]);
        let items = raw.as_array().unwrap();
        let df = build_notice_df(items, false).unwrap();
        assert_eq!(
            df.column_names(),
            vec!["代码", "名称", "公告标题", "公告类型", "公告日期", "公告代码"]
        );
        let gc = df.inner().column("公告代码").unwrap().str().unwrap();
        assert_eq!(gc.get(0), Some("AN202608141827988685"));
    }

    #[test]
    fn notice_node_map() {
        assert_eq!(notice_node("财务报告"), "1");
        assert_eq!(notice_node("资产重组"), "6");
        assert_eq!(notice_node("未知类型"), "0");
    }

    #[test]
    fn fmt_ymd8_ok() {
        assert_eq!(fmt_ymd8("20220511"), "2022-05-11");
        assert_eq!(fmt_ymd8("2025-01-01"), "2025-01-01");
    }
}
