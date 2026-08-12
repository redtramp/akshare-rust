//! 金十数据源（`datacenter-api.jin10.com` 经济数据中心报表）。
//!
//! 对应 akshare `economic/macro_china.py::__macro_china_base_func`：
//! - `GET /reports/list_v2?max_date=&category=ec&attr_id={id}&_={ts}`
//!   携带 `x-app-id` / `x-csrf-token` / `x-version` 头
//! - 响应 `data.values` 每页约 20 行（新→旧），翻页用「末行日期 − 1 天」作为
//!   下一页 `max_date`，直到返回空
//! - 每行 4 个元素：`[日期, 今值, 预测值, 前值]`（与 akshare 位置重命名一致）
//!
//! 返回统一 5 列契约：`商品, 日期, 今值, 预测值, 前值`，其中 商品 = 报表名，
//! 日期为 `YYYY-MM-DD`（对应 pandas `.dt.date`），三个数值列转 Float64。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

/// 金十接口要求的关键头（对应 akshare `__macro_china_base_func` 的 headers）。
const JIN10_HEADERS: [(&str, &str); 4] = [
    (
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/107.0.0.0 Safari/537.36",
    ),
    ("x-app-id", "rU6QIu7JHe2gOUeR"),
    ("x-csrf-token", "x-csrf-token"),
    ("x-version", "1.0.0"),
];

const JIN10_COLS: [&str; 5] = ["商品", "日期", "今值", "预测值", "前值"];
const JIN10_NUMERIC: [&str; 3] = ["今值", "预测值", "前值"];

/// 当前毫秒时间戳（对应 akshare `str(int(round(t * 1000)))` 的 `_` 参数）。
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `YYYY-MM-DD` 减一天（对应 akshare `strptime(...) - timedelta(days=1)`）。
fn prev_day(date: &str) -> String {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return date.to_string();
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        date[0..4].parse::<i32>(),
        date[5..7].parse::<i32>(),
        date[8..10].parse::<i32>(),
    ) else {
        return date.to_string();
    };
    // 天数转「自 1970-01-01 的天序号」，减一后转回（借用 chrono 不可用时的轻量算法）
    let days_from_civil = |y: i32, m: i32, d: i32| -> i32 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    };
    let civil_from_days = |z: i32| -> (i32, i32, i32) {
        let z = z + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    };
    let n = days_from_civil(y, m, d) - 1;
    let (ny, nm, nd) = civil_from_days(n);
    format!("{ny:04}-{nm:02}-{nd:02}")
}

/// JSON 值 → Option<String>（数值走 `to_string`，与 pandas 逐单元格 str 一致）。
fn cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// 金十数据中心-中国经济指标报表（对应 akshare `__macro_china_base_func`）。
///
/// `symbol`：报表名（输出 `商品` 列值，如 `中国CPI年率报告`）；
/// `attr_id`：金十指标 ID（如 `56`）。翻页抓取全部历史后按 `日期` 升序返回。
///
/// # 返回列
/// `商品, 日期, 今值, 预测值, 前值`
pub fn macro_china_base(symbol: &str, attr_id: &str) -> Result<Df> {
    let url = "https://datacenter-api.jin10.com/reports/list_v2";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("max_date".into(), Value::String(String::new()));
    params.insert("category".into(), Value::String("ec".into()));
    params.insert("attr_id".into(), Value::String(attr_id.into()));

    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    // 上一页 max_date：翻页不前进（上游返回非 ISO 日期或忽略 max_date）时终止，
    // 防御死循环；正常路径每页末行日期递减。
    let mut prev_max = String::new();
    loop {
        params.insert("_".into(), Value::from(now_ms()));
        let json = http.get_json_with_headers(url, &params, &JIN10_HEADERS, None)?;
        let values = json
            .get("data")
            .and_then(|d| d.get("values"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if values.is_empty() {
            break;
        }
        for v in &values {
            let arr = v.as_array().cloned().unwrap_or_default();
            // 每行 4 元素：日期/今值/预测值/前值（对应 akshare 位置重命名）
            let mut row: Vec<Option<String>> = (0..4).map(|i| arr.get(i).and_then(cell)).collect();
            row.insert(0, Some(symbol.to_string()));
            rows.push(row);
        }
        // 翻页：末行日期 − 1 天作为下一页 max_date
        let last_date = values
            .last()
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let next_max = prev_day(&last_date);
        if next_max == prev_max {
            break;
        }
        prev_max = next_max.clone();
        params.insert("max_date".into(), Value::String(next_max));
    }

    let mut df = Df::from_string_rows(&JIN10_COLS, &rows)?;
    df.cast_numeric(&JIN10_NUMERIC)?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 拉取金十 cdn 公开报表 JSON（`cdn.jin10.com/data_center/reports/{file}`）。
///
/// 对应 akshare 各 `macro_china_*` 的金十 cdn 实现（如 `sge.json` / `il_1.json` /
/// `fs_1.json` / `exchange_rate.json` 等）。返回解析后的 JSON 根对象。
pub(crate) fn fetch_jin10_cdn(file: &str) -> Result<Value> {
    let url = format!("https://cdn.jin10.com/data_center/reports/{file}");
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("_".into(), Value::from(now_ms()));
    http.get_json(&url, &params, None)
}

/// 拉取金十 cdn 公开报表原始文本（用于 `.js` 包裹的 JSON，如日度能源报告）。
///
/// 返回响应文本，调用方自行剥离 `var xxx = ` 前缀并解析 JSON。
pub(crate) fn fetch_jin10_cdn_text(file: &str) -> Result<String> {
    let url = format!("https://cdn.jin10.com/dc/reports/{file}");
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("v".into(), Value::from(now_ms()));
    params.insert("_".into(), Value::from(now_ms() + 90));
    let text = http.get_text(&url, &params, None)?;
    let start = text
        .find('{')
        .ok_or_else(|| AkshareError::Empty("金十 cdn js 未找到 JSON".into()))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| AkshareError::Empty("金十 cdn js 未找到 JSON".into()))?;
    Ok(text[start..=end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prev_day_arithmetic() {
        assert_eq!(prev_day("2026-03-01"), "2026-02-28");
        assert_eq!(prev_day("2024-03-01"), "2024-02-29"); // 闰年
        assert_eq!(prev_day("2026-01-01"), "2025-12-31");
        assert_eq!(prev_day("2026-03-09"), "2026-03-08");
        // 非 ISO 输入原样返回（翻页前不会出现）
        assert_eq!(prev_day("2026/03/09"), "2026/03/09");
    }
}
