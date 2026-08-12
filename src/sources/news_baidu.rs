//! 百度股市通财经日历数据源（批次 5 长尾 · news）。
//!
//! 对应 akshare `news/news_baidu.py`：
//! 经济数据 / 停复牌 / 分红派息 / 财报披露 四类日历，均来自
//! `https://finance.pae.baidu.com/sapi/v1/financecalendar`。
//!
//! 实测：该接口在带 `accept: application/vnd.finance-web.v1+json` 等常规
//! 请求头下可直接返回 JSON（akshare 的 cookie 流程在本环境可省），故 Rust 侧
//! 直接 GET + 解析 `Result.calendarInfo`，按日期聚合 `list` 并分页。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};
use std::collections::HashSet;

const BAIDU_CALENDAR_URL: &str = "https://finance.pae.baidu.com/sapi/v1/financecalendar";

/// 百度财经日历标准请求头（对应 akshare `_baidu_finance_calendar` 的 headers）。
const BAIDU_HEADERS: &[(&str, &str)] = &[
    (
        "accept",
        "application/vnd.finance-web.v1+json",
    ),
    (
        "user-agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/142.0.0.0 Safari/537.36",
    ),
    ("origin", "https://finance.baidu.com"),
    ("referer", "https://finance.baidu.com/"),
];

/// 取某日某类别的日历条目（已完成分页聚合）。
///
/// 对应 akshare `_baidu_finance_calendar`：先取首页，从 `calendarInfo` 中匹配
/// 目标日期得到 `total`，按每页 100 条计算页数后逐页聚合 `list`。
fn fetch_baidu_calendar(date: &str, cate: &str) -> Result<Vec<Value>> {
    if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
        return Err(AkshareError::Empty(format!(
            "百度日历日期需为 YYYYMMDD，收到: {date}"
        )));
    }
    let formatted = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
    let http = HttpClient::default();

    let mut params = Map::new();
    params.insert("start_date".into(), Value::String(formatted.clone()));
    params.insert("end_date".into(), Value::String(formatted.clone()));
    params.insert("pn".into(), Value::String("0".into()));
    params.insert("rn".into(), Value::String("100".into()));
    params.insert("cate".into(), Value::String(cate.into()));
    params.insert("finClientType".into(), Value::String("pc".into()));

    let first = http.get_json_with_headers(BAIDU_CALENDAR_URL, &params, BAIDU_HEADERS, None)?;

    let cal = first
        .get("Result")
        .and_then(|r| r.get("calendarInfo"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut total: i64 = 0;
    for it in &cal {
        if it.get("date").and_then(Value::as_str) == Some(formatted.as_str()) {
            total += it.get("total").and_then(Value::as_i64).unwrap_or(0);
        }
    }
    let pages = if total > 0 {
        ((total + 99) / 100) as usize
    } else {
        1
    };

    let mut items: Vec<Value> = Vec::new();
    for page in 0..pages {
        let data = if page == 0 {
            first.clone()
        } else {
            let mut p = params.clone();
            p.insert("pn".into(), Value::String(page.to_string()));
            http.get_json_with_headers(BAIDU_CALENDAR_URL, &p, BAIDU_HEADERS, None)?
        };
        let cal = data
            .get("Result")
            .and_then(|r| r.get("calendarInfo"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for it in &cal {
            if it.get("date").and_then(Value::as_str) == Some(formatted.as_str()) {
                if let Some(list) = it.get("list").and_then(Value::as_array) {
                    for x in list {
                        items.push(x.clone());
                    }
                }
            }
        }
    }
    Ok(items)
}

/// 通用：按 akshare 的「rename → 保底列 → 选列顺序 → 数值/日期化」流程构建 DataFrame。
///
/// - `rename`：JSON 键 → 中文列名
/// - `required`：必须存在的列（缺失时填 `fill`）
/// - `order`：最终列顺序（仅保留出现的列）
/// - `numeric` / `dates`：需数值化 / 日期化的列
/// - `fill`：保底列的填充值（`"-"` 或 `None`）
/// - `fallbacks`：某列缺失时回退到另一个 JSON 键（如 市值 ← 总市值）
#[allow(clippy::too_many_arguments)]
fn build_calendar_df(
    items: &[Value],
    rename: &[(&str, &str)],
    required: &[&str],
    order: &[&str],
    numeric: &[&str],
    dates: &[&str],
    fill: Option<&str>,
    fallbacks: &[(&str, &str)],
) -> Result<Df> {
    // 出现的列：rename 中任一 JSON 键在条目里存在，或属于 required。
    // key_present：列对应的 JSON 键（或 fallback 源键）在「任一」条目里出现。
    //   akshare 仅当整列缺失时才用 `fill`（'-'）补整列；列已出现但个别单元格为空
    //   时保持 NaN（不补 '-'）。
    let mut present: HashSet<&str> = HashSet::new();
    let mut key_present: HashSet<&str> = HashSet::new();
    for item in items {
        if let Some(obj) = item.as_object() {
            for (jk, ck) in rename {
                if obj.contains_key(*jk) {
                    present.insert(*ck);
                    key_present.insert(*ck);
                }
            }
            // fallbacks 的源键存在也会使目标列出现
            for (col, src) in fallbacks {
                if obj.contains_key(*src) {
                    present.insert(*col);
                    key_present.insert(*col);
                }
            }
        }
    }
    for r in required {
        present.insert(*r);
    }
    let cols: Vec<&str> = order
        .iter()
        .copied()
        .filter(|c| present.contains(*c))
        .collect();

    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(items.len());
    for item in items {
        let obj = item.as_object();
        let row: Vec<Option<String>> = cols
            .iter()
            .map(|c| {
                let jk = rename.iter().find(|(_, ck)| *ck == *c).map(|(jk, _)| *jk);
                let mut val = match jk {
                    Some(jk) => match obj.and_then(|o| o.get(jk)) {
                        Some(Value::String(s)) => Some(s.clone()),
                        Some(Value::Null) => None,
                        Some(other) => Some(other.to_string()),
                        None => None,
                    },
                    None => None,
                };
                // fallback：当前列值为 None 时尝试源键
                if val.is_none() {
                    if let Some((_, src)) = fallbacks.iter().find(|(col, _)| *col == *c) {
                        val = obj
                            .and_then(|o| o.get(*src))
                            .and_then(|v| match v {
                                Value::String(s) => Some(s.clone()),
                                Value::Null => None,
                                other => Some(other.to_string()),
                            });
                    }
                }
                // 整列缺失（required 且 key 从未出现）才用 fill 补整列
                if val.is_none() && required.contains(c) && !key_present.contains(c) {
                    val = fill.map(str::to_string);
                }
                val
            })
            .collect();
        rows.push(row);
    }

    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(numeric)?;
    df.cast_date(dates)?;
    Ok(df)
}

/// 百度股市通-经济数据（对应 akshare [`news_economic_baidu`]）。
///
/// # 返回列
/// `日期, 时间, 地区, 事件, 公布, 预期, 前值, 重要性`
pub fn news_economic_baidu(date: &str) -> Result<Df> {
    let items = fetch_baidu_calendar(date, "economic_data")?;
    build_calendar_df(
        &items,
        &[
            ("date", "日期"),
            ("time", "时间"),
            ("title", "事件"),
            ("star", "重要性"),
            ("formerVal", "前值"),
            ("pubVal", "公布"),
            ("region", "地区"),
            ("indicateVal", "预期"),
            ("country", "国家"),
            ("timePeriod", "统计周期"),
        ],
        &["公布", "预期", "前值", "重要性"],
        &[
            "日期", "时间", "国家", "地区", "事件", "统计周期", "公布", "预期", "前值", "重要性",
        ],
        &["公布", "预期", "前值", "重要性"],
        &["日期"],
        None,
        &[],
    )
}

/// 百度股市通-交易提醒-停复牌（对应 akshare [`news_trade_notify_suspend_baidu`]）。
///
/// # 返回列
/// `股票代码, 股票简称, 交易所代码, 停牌时间, 复牌时间, 停牌事项说明, 市值, 公告日期, 公告时间, 证券类型, 市场类型, 是否跳过`
pub fn news_trade_notify_suspend_baidu(date: &str) -> Result<Df> {
    let items = fetch_baidu_calendar(date, "notify_suspend")?;
    build_calendar_df(
        &items,
        &[
            ("code", "股票代码"),
            ("name", "股票简称"),
            ("exchange", "交易所代码"),
            ("start", "停牌时间"),
            ("reason", "停牌事项说明"),
            ("marketValue", "市值"),
            ("date", "公告日期"),
            ("time", "公告时间"),
            ("type", "证券类型"),
            ("market", "市场类型"),
            ("isSkip", "是否跳过"),
            ("end", "复牌时间"),
        ],
        &["复牌时间"],
        &[
            "股票代码", "股票简称", "交易所代码", "停牌时间", "复牌时间", "停牌事项说明", "市值",
            "公告日期", "公告时间", "证券类型", "市场类型", "是否跳过",
        ],
        &[],
        &[],
        Some("-"),
        &[],
    )
}

/// 百度股市通-交易提醒-分红派息（对应 akshare [`news_trade_notify_dividend_baidu`]）。
///
/// # 返回列
/// `股票代码, 除权日, 分红, 送股, 转增, 实物, 交易所, 股票简称, 报告期`
pub fn news_trade_notify_dividend_baidu(date: &str) -> Result<Df> {
    let items = fetch_baidu_calendar(date, "notify_divide")?;
    build_calendar_df(
        &items,
        &[
            ("code", "股票代码"),
            ("exchange", "交易所"),
            ("name", "股票简称"),
            ("diviDate", "除权日"),
            ("date", "报告期"),
            ("diviCash", "分红"),
            ("shareDivide", "送股"),
            ("transfer", "转增"),
            ("physical", "实物"),
        ],
        &["分红", "实物", "送股", "转增"],
        &[
            "股票代码", "除权日", "分红", "送股", "转增", "实物", "交易所", "股票简称", "报告期",
        ],
        &[],
        &["除权日", "报告期"],
        Some("-"),
        &[],
    )
}

/// 百度股市通-财报披露（对应 akshare [`news_report_time_baidu`]）。
///
/// # 返回列
/// `股票代码, 股票简称, 交易所, 财报类型, 发布时间, 市值, 发布日期`
pub fn news_report_time_baidu(date: &str) -> Result<Df> {
    let items = fetch_baidu_calendar(date, "report_time")?;
    build_calendar_df(
        &items,
        &[
            ("code", "股票代码"),
            ("name", "股票简称"),
            ("exchange", "交易所"),
            ("reportType", "财报类型"),
            ("time", "发布时间"),
            ("marketValue", "市值"),
            ("capitalization", "总市值"),
            ("date", "发布日期"),
        ],
        &["财报类型", "发布时间"],
        &[
            "股票代码", "股票简称", "交易所", "财报类型", "发布时间", "市值", "发布日期",
        ],
        &["市值"],
        &["发布日期"],
        Some("-"),
        &[("市值", "capitalization")],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_columns_union_of_keys() {
        let items = vec![
            serde_json::json!({"date":"2025-11-26","pubVal":"1.0","country":"美国"}),
            serde_json::json!({"date":"2025-11-27","pubVal":"2.0"}),
        ];
        let df = build_calendar_df(
            &items,
            &[
                ("date", "日期"),
                ("pubVal", "公布"),
                ("country", "国家"),
                ("indicateVal", "预期"),
            ],
            &["预期"],
            &["日期", "国家", "公布", "预期"],
            &["公布"],
            &["日期"],
            None,
            &[],
        )
        .unwrap();
        // country 出现于任一行的键 → 列在；预期 为 required → 列在
        assert_eq!(df.column_names(), vec!["日期", "国家", "公布", "预期"]);
    }
}
