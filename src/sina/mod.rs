//! 新浪财经数据源。
//!
//! 首批实现（对应 akshare `stock/stock_hk_sina.py::stock_hk_spot` 与
//! `stock/stock_zh_a_sina.py::stock_zh_a_minute` 基础形态）：
//! - [`stock_hk_spot`]：港股实时行情（分页）
//! - [`stock_zh_a_minute`]：A 股分钟线（JSONP，不复权）
//!
//! 说明：新浪日 K（`stock_zh_a_daily`）需要 17KB JS 解密（`hk_js_decode`），
//! 且与东财 `stock_zh_a_hist` 功能等价，暂不移植；复权分钟线依赖它，同样延后。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{json, Value};

/// 新浪财经-港股实时行情（对应 akshare [`akshare.stock_hk_spot`]）。
///
/// 分页抓取（最多 99 页 × 60 条，空页停止）。
///
/// # 返回列
/// `日期时间, 代码, 中文名称, 英文名称, 交易类型, 最新价, 涨跌额, 涨跌幅,
/// 昨收, 今开, 最高, 最低, 成交量, 成交额, 买一, 卖一`
pub fn stock_hk_spot() -> Result<Df> {
    let url =
        "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHKStockData";
    let http = HttpClient::default();
    let mut all: Vec<Value> = Vec::new();

    for page in 1..=99 {
        let params = json!({
            "page": page.to_string(),
            "num": "60",
            "sort": "symbol",
            "asc": "1",
            "node": "qbgg_hk",
            "_s_r_a": "init",
        });
        let data = http.get_json(
            url,
            params.as_object().expect("静态参数"),
            Some("https://vip.stock.finance.sina.com.cn/mkt/"),
        )?;
        let rows = data.as_array().cloned().unwrap_or_default();
        if rows.is_empty() {
            break;
        }
        all.extend(rows);
        if all.len().is_multiple_of(600) {
            // 每 10 页稍歇，避免触发限流
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    if all.is_empty() {
        return Err(AkshareError::Empty("新浪港股现货无数据".into()));
    } // 列映射：响应键 → akshare 列名（只保留 16 个目标列，未列出的键丢弃）
    let rename = [
        ("symbol", "代码"),
        ("name", "中文名称"),
        ("engname", "英文名称"),
        ("tradetype", "交易类型"),
        ("lasttrade", "最新价"),
        ("prevclose", "昨收"),
        ("open", "今开"),
        ("high", "最高"),
        ("low", "最低"),
        ("volume", "成交量"),
        ("amount", "成交额"),
        ("ticktime", "日期时间"),
        ("buy", "买一"),
        ("sell", "卖一"),
    ];
    let df = Df::from_json_rows(&all)?;
    // 按映射选出存在的列（保持响应键序）
    let mut keep: Vec<&str> = Vec::new();
    for (k, _) in &rename {
        if df.column_names().iter().any(|c| c == k) {
            keep.push(k);
        }
    }
    let mut out = df.select(&keep)?;
    let renamed: Vec<String> = keep
        .iter()
        .map(|c| {
            let cstr: &str = c;
            rename
                .iter()
                .find(|(k, _)| *k == cstr)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| cstr.to_string())
        })
        .collect();
    let refs: Vec<&str> = renamed.iter().map(String::as_str).collect();
    out.rename_columns(&refs)?; // 涨跌额/涨跌幅为派生列（最新价-昨收），akshare 输出 16 列含此两列
    out = compute_change(&out)?;
    out.cast_numeric(&[
        "最新价",
        "涨跌额",
        "涨跌幅",
        "昨收",
        "今开",
        "最高",
        "最低",
        "成交量",
        "成交额",
        "买一",
        "卖一",
    ])?;
    Ok(out)
}

/// 计算涨跌额/涨跌幅列（响应中为派生值，akshare 从原始列映射，此处直接计算）。
fn compute_change(df: &Df) -> Result<Df> {
    let mut out = df.clone();
    let last = col_f64(&out, "最新价")?;
    let prev = col_f64(&out, "昨收")?;
    let n = out.height();
    let mut chg_amt: Vec<Option<String>> = Vec::with_capacity(n);
    let mut chg_pct: Vec<Option<String>> = Vec::with_capacity(n);
    for i in 0..n {
        match (last[i], prev[i]) {
            (Some(l), Some(p)) => {
                let diff = l - p;
                chg_amt.push(Some(format_float(diff)));
                chg_pct.push(Some(format_float(if p != 0.0 {
                    diff / p * 100.0
                } else {
                    0.0
                })));
            }
            _ => {
                chg_amt.push(None);
                chg_pct.push(None);
            }
        }
    }
    out.with_column("涨跌额", &chg_amt)?;
    out.with_column("涨跌幅", &chg_pct)?;
    // 重排到 akshare 16 列顺序（仅保留存在的列，兼容测试的列子集）
    const TARGET: [&str; 16] = [
        "日期时间",
        "代码",
        "中文名称",
        "英文名称",
        "交易类型",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "昨收",
        "今开",
        "最高",
        "最低",
        "成交量",
        "成交额",
        "买一",
        "卖一",
    ];
    let present: Vec<&str> = TARGET
        .iter()
        .copied()
        .filter(|c| out.column_names().iter().any(|n| n == c))
        .collect();
    out.select(&present)
}

/// 提取列浮点值数组。
fn col_f64(df: &Df, name: &str) -> Result<Vec<Option<f64>>> {
    let c = df
        .inner()
        .column(name)
        .map_err(|e| AkshareError::Empty(format!("缺少列 {name}: {e}")))?;
    if let Ok(s) = c.f64() {
        return Ok((0..c.len()).map(|i| s.get(i)).collect());
    }
    if let Ok(s) = c.str() {
        return Ok((0..c.len())
            .map(|i| s.get(i).and_then(|v| v.trim().parse::<f64>().ok()))
            .collect());
    }
    Ok(vec![None; c.len()])
}

/// 浮点字符串化（整数不带小数点）。
fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// 新浪财经-A 股分钟线（对应 akshare [`akshare.stock_zh_a_minute`] 不复权形态）。
///
/// `symbol`: 如 `sh600519`；`period`: `1/5/15/30/60` 分钟。
///
/// # 返回列
/// `day, open, high, low, close, volume, amount`（akshare 取前 7 列，列名原样）
pub fn stock_zh_a_minute(symbol: &str, period: &str, _adjust: &str) -> Result<Df> {
    let url = "https://quotes.sina.cn/cn/api/jsonp_v2.php/=/CN_MarketDataService.getKLineData";
    let params = json!({
        "symbol": symbol,
        "scale": period,
        "ma": "no",
        "datalen": "1970",
    });
    let http = HttpClient::default();
    let text = http.get_text(url, params.as_object().expect("静态参数"), None)?;
    // JSONP 解包：`=([...])` 或带前缀 `/*...*/\n=([...])`
    let start = text
        .find("=(")
        .ok_or_else(|| AkshareError::Empty("新浪分钟线响应缺少 '=(' 前缀".into()))?;
    let body = &text[start + 2..];
    let end = body
        .find(");")
        .ok_or_else(|| AkshareError::Empty("新浪分钟线响应缺少 ');' 后缀".into()))?;
    let json_text = &body[..end];
    let rows: Vec<Value> =
        serde_json::from_str(json_text).map_err(|e| AkshareError::json(url, e.to_string()))?;
    let df = Df::from_json_rows(&rows)?;
    // 只取前 7 列
    let names = df.column_names();
    let take: Vec<&str> = names.iter().take(7).map(String::as_str).collect();
    df.select(&take)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_float_rules() {
        assert_eq!(format_float(73.25), "73.25");
        assert_eq!(format_float(73.0), "73");
        assert_eq!(format_float(-1.5), "-1.5");
        assert_eq!(format_float(0.0), "0");
    }

    #[test]
    fn col_f64_from_str() {
        let df = Df::from_string_rows(
            &["a"],
            &[vec![Some("73.25".into())], vec![Some("-".into())]],
        )
        .unwrap();
        let v = col_f64(&df, "a").unwrap();
        assert_eq!(v, vec![Some(73.25), None]);
    }

    #[test]
    fn compute_change_adds_columns() {
        let df = Df::from_string_rows(
            &["日期时间", "代码", "最新价", "昨收"],
            &[vec![
                Some("2026/08/07 16:08:20".into()),
                Some("00001".into()),
                Some("73.25".into()),
                Some("72.25".into()),
            ]],
        )
        .unwrap();
        let out = compute_change(&df).unwrap();
        let names = out.column_names();
        assert_eq!(
            names,
            vec!["日期时间", "代码", "最新价", "涨跌额", "涨跌幅", "昨收"]
        );
        let amt = out.inner().column("涨跌额").unwrap().str().unwrap();
        assert_eq!(amt.get(0), Some("1"));
        let pct = out.inner().column("涨跌幅").unwrap().str().unwrap();
        let pct_v: f64 = pct.get(0).unwrap().parse().unwrap();
        assert!((pct_v - 1.3840830449826985).abs() < 1e-12);
    }
}
