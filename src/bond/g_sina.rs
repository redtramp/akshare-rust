//! 新浪财经（sina）债券类函数（批次4 · 阶段5）。
//!
//! 对应 akshare `bond/bond_gb_sina.py`、`bond/bond_zh_sina.py`、
//! `bond/bond_cb_sina.py`、`bond/bond_zh_cov.py`（SINA 系可转债）：
//! - `bond_gb_us_sina` / `bond_gb_zh_sina`：中美国债收益率日线（JSON）
//! - `bond_zh_hs_spot`：沪深债券实时行情（分页）
//! - `bond_zh_hs_daily`：沪深债券历史日 K（hk_js_decode 解密）
//! - `bond_cb_profile_sina` / `bond_cb_summary_sina`：可转债详情/概况（HTML 表）
//! - `bond_zh_hs_cov_daily` / `bond_zh_hs_cov_spot`：可转债日 K / 实时（hk_js_decode）

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

/// 中国国债收益率品种映射（`symbol` → 接口代码）。
const ZH_GB_MAP: &[(&str, &str)] = &[
    ("中国1年期国债", "CN1YT"),
    ("中国2年期国债", "CN2YT"),
    ("中国3年期国债", "CN3YT"),
    ("中国5年期国债", "CN5YT"),
    ("中国7年期国债", "CN7YT"),
    ("中国10年期国债", "CN10YT"),
    ("中国15年期国债", "CN15YT"),
    ("中国20年期国债", "CN20YT"),
    ("中国30年期国债", "CN30YT"),
];

/// 美国国债收益率品种映射（`symbol` → 接口代码）。
const US_GB_MAP: &[(&str, &str)] = &[
    ("美国1月期国债", "US1MT"),
    ("美国2月期国债", "US2MT"),
    ("美国3月期国债", "US3MT"),
    ("美国4月期国债", "US4MT"),
    ("美国6月期国债", "US6MT"),
    ("美国1年期国债", "US1YT"),
    ("美国2年期国债", "US2YT"),
    ("美国3年期国债", "US3YT"),
    ("美国5年期国债", "US5YT"),
    ("美国7年期国债", "US7YT"),
    ("美国10年期国债", "US10YT"),
    ("美国20年期国债", "US20YT"),
    ("美国30年期国债", "US30YT"),
];

/// 在映射表中查找接口代码，找不到返回 `Empty` 错误。
fn lookup<'a>(map: &'a [(&str, &str)], symbol: &str) -> Result<&'a str> {
    map.iter()
        .find(|(k, _)| *k == symbol)
        .map(|(_, v)| *v)
        .ok_or_else(|| {
            crate::core::error::AkshareError::Empty(format!("未知债券品种: {symbol}"))
        })
}

/// 抓取新浪国债收益率日线（对应 akshare [`bond_gb_zh_sina`] / [`bond_gb_us_sina`]）。
///
/// 接口返回 `result.data` 数组，每行键序 `d,o,h,l,c,v`，位置重命名为
/// `date,open,high,low,close,volume`；`date` 保持字符串（对应 akshare
/// `pd.to_datetime(...).dt.date` → object 列，dtype 为 str）。
fn gb_sina(symbol: &str, map: &[(&str, &str)]) -> Result<Df> {
    let code = lookup(map, symbol)?;
    let url = format!("https://bond.finance.sina.com.cn/hq/gb/daily?symbol={code}");
    let http = HttpClient::default();
    let data = http.get_json(&url, &Map::new(), None)?;
    let rows = data
        .get("result")
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            crate::core::error::AkshareError::Empty("新浪国债响应缺少 result.data".into())
        })?;
    if rows.is_empty() {
        return Df::from_string_rows(&["date", "open", "high", "low", "close", "volume"], &[]);
    }
    let mut df = Df::from_json_rows(&rows)?;
    df.rename_columns(&["date", "open", "high", "low", "close", "volume"])?;
    df.cast_numeric(&["open", "high", "low", "close", "volume"])?;
    Ok(df)
}

/// 中国国债收益率行情（对应 akshare [`bond_gb_zh_sina`]）。
///
/// # 返回列
/// `date, open, high, low, close, volume`
pub fn bond_gb_zh_sina(symbol: &str) -> Result<Df> {
    gb_sina(symbol, ZH_GB_MAP)
}

/// 美国国债收益率行情（对应 akshare [`bond_gb_us_sina`]）。
///
/// # 返回列
/// `date, open, high, low, close, volume`
pub fn bond_gb_us_sina(symbol: &str) -> Result<Df> {
    gb_sina(symbol, US_GB_MAP)
}
