//! 上海黄金交易所（SGE）数据源（`www.sge.com.cn`）。
//!
//! 对应 akshare `spot/spot_sge.py`：行情走势/基准价/实时行情等 JSON 接口。
//! 部分接口经 `graph/...` POST 表单返回 JSON，列名与 akshare 逐字一致。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

/// 毫秒时间戳 → `YYYY-MM-DD`（对应 pandas `to_datetime(unit="ms").dt.date`）。
fn ms_to_date(ms: i64) -> Option<String> {
    const MS_PER_DAY: i64 = 86_400_000;
    let days = ms / MS_PER_DAY;
    // 1970-01-01 起的天序号 → 公历日期（轻量算法，避免引入 chrono）
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    Some(format!("{year:04}-{m:02}-{d:02}"))
}

/// JSON 单元格 → 字符串：字符串原样返回，数字格式化为文本（对应 akshare
/// `pd.DataFrame` 直接吃入混合类型数组，`pd.to_numeric` 再解析）。
fn json_cell_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// "HH:MM" 时间 → 当日秒数（用于排序/过滤）。
fn time_to_secs(t: &str) -> Option<i32> {
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let h: i32 = parts[0].parse().ok()?;
    let m: i32 = parts[1].parse().ok()?;
    let s: i32 = if parts.len() > 2 {
        parts[2].parse().unwrap_or(0)
    } else {
        0
    };
    Some(h * 3600 + m * 60 + s)
}

/// 品种静态表（对应 akshare [`spot_symbol_table_sge`]）。
///
/// # 返回列
/// `序号, 品种`（序号从 1 开始）
pub fn spot_symbol_table_sge() -> Result<Df> {
    let symbols = [
        "Au99.99", "Au99.95", "Au100g", "Pt99.95", "Ag(T+D)", "Au(T+D)", "mAu(T+D)",
        "Au(T+N1)", "Au(T+N2)", "Ag99.99", "iAu99.99", "Au99.5", "iAu100g", "iAu99.5",
        "PGC30g", "NYAuTN06", "NYAuTN12",
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(symbols.len());
    for (i, sym) in symbols.iter().enumerate() {
        rows.push(vec![Some((i + 1).to_string()), Some((*sym).to_string())]);
    }
    let mut df = Df::from_string_rows(&["序号", "品种"], &rows)?;
    df.cast_numeric(&["序号"])?;
    Ok(df)
}

/// 上海金基准价历史数据（对应 akshare [`spot_golden_benchmark_sge`]）。
///
/// # 返回列
/// `交易时间, 晚盘价, 早盘价`
pub fn spot_golden_benchmark_sge() -> Result<Df> {
    fetch_benchmark("https://www.sge.com.cn/graph/DayilyJzj")
}

/// 上海银基准价历史数据（对应 akshare [`spot_silver_benchmark_sge`]）。
///
/// # 返回列
/// `交易时间, 晚盘价, 早盘价`
pub fn spot_silver_benchmark_sge() -> Result<Df> {
    fetch_benchmark("https://www.sge.com.cn/graph/DayilyShsilverJzj")
}

/// 基准价通用解析：wp = 晚盘、zp = 早盘，按行对齐。
fn fetch_benchmark(url: &str) -> Result<Df> {
    let http = HttpClient::default();
    let params = Map::new();
    let json = http.post_form(url, &params, &[])?;
    let wp = json
        .get("wp")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("SGE 基准价 wp 缺失".into()))?;
    let zp = json
        .get("zp")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let n = wp.len().max(zp.len());
    let mut date: Vec<Option<String>> = Vec::with_capacity(n);
    let mut eve: Vec<Option<String>> = Vec::with_capacity(n);
    let mut morn: Vec<Option<String>> = Vec::with_capacity(n);
    for i in 0..n {
        let w = wp.get(i).and_then(Value::as_array).cloned().unwrap_or_default();
        let z = zp.get(i).and_then(Value::as_array).cloned().unwrap_or_default();
        let ms = w.first().and_then(Value::as_i64);
        date.push(ms.and_then(ms_to_date));
        eve.push(w.get(1).and_then(Value::as_str).map(str::to_string));
        morn.push(z.get(1).and_then(Value::as_str).map(str::to_string));
    }
    let mut df = Df::from_string_rows(&["交易时间", "晚盘价", "早盘价"], &[date, eve, morn])?;
    df.cast_numeric(&["晚盘价", "早盘价"])?;
    Ok(df)
}

/// SGE 历史行情（对应 akshare [`spot_hist_sge`]）。
///
/// `symbol`：品种，如 `Au99.99`（可通过 [`spot_symbol_table_sge`] 获取）。
///
/// # 返回列
/// `date, open, close, low, high`
pub fn spot_hist_sge(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("instid".into(), Value::String(symbol.to_string()));
    let json = http.post_form("https://www.sge.com.cn/graph/Dailyhq", &params, &[])?;
    let time = json
        .get("time")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("SGE 历史行情 time 缺失".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(time.len());
    for r in &time {
        let arr = r.as_array().cloned().unwrap_or_default();
        if arr.len() < 5 {
            continue;
        }
        let cells: Vec<Option<String>> = arr.iter().map(json_cell_to_string).collect();
        rows.push(cells);
    }
    let mut df = Df::from_string_rows(&["date", "open", "close", "low", "high"], &rows)?;
    df.cast_date(&["date"])?;
    df.cast_numeric(&["open", "close", "low", "high"])?;
    Ok(df)
}

/// SGE 实时行情（对应 akshare [`spot_quotations_sge`]）。
///
/// `symbol`：品种，如 `Au99.99`。
///
/// # 返回列
/// `品种, 时间, 现价, 更新时间`（过滤掉晚于更新时间的时点后按时间升序）
pub fn spot_quotations_sge(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("instid".into(), Value::String(symbol.to_string()));
    let json = http.get_json(
        "https://www.sge.com.cn/graph/quotations",
        &params,
        Some("https://www.sge.com.cn/"),
    )?;
    let heyue = json
        .get("heyue")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let times = json
        .get("times")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let data = json
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let delaystr = json
        .get("delaystr")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    // 更新时间的时点部分（如 "15:30:00"）
    let update_secs = delaystr
        .split_whitespace()
        .nth(1)
        .and_then(time_to_secs);

    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(times.len());
    for (i, t) in times.iter().enumerate() {
        let time_str = t.as_str().map(str::to_string);
        let price = data.get(i).and_then(Value::as_str).map(str::to_string);
        // 过滤晚于更新时间的时点（对应 akshare 时间 < 更新时间）
        if let (Some(ts), Some(us)) = (time_str.as_deref().and_then(time_to_secs), update_secs) {
            if ts >= us {
                continue;
            }
        }
        rows.push(vec![
            Some(heyue.clone()),
            time_str,
            price,
            Some(delaystr.clone()),
        ]);
    }
    let mut df = Df::from_string_rows(&["品种", "时间", "现价", "更新时间"], &rows)?;
    df.cast_numeric(&["现价"])?;
    df = df.sort_by("时间", true, false)?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_to_date_basics() {
        // 1970-01-01 00:00:00 UTC
        assert_eq!(ms_to_date(0), Some("1970-01-01".into()));
        // 2024-01-01 00:00:00 UTC = 1704067200000
        assert_eq!(ms_to_date(1_704_067_200_000), Some("2024-01-01".into()));
        // 2024-05-20 00:00:00 UTC
        assert_eq!(ms_to_date(1_716_163_200_000), Some("2024-05-20".into()));
    }

    #[test]
    fn time_to_secs_parses() {
        assert_eq!(time_to_secs("09:00"), Some(32_400));
        assert_eq!(time_to_secs("15:30:00"), Some(55_800));
        assert_eq!(time_to_secs("bad"), None);
    }
}
