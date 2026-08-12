//! 搜猪网数据源（`www.soozhu.com` 生猪/饲料大数据）。
//!
//! 对应 akshare `spot/spot_hog_soozhu.py`：先 GET 数据中心页提取
//! `csrfmiddlewaretoken`，再以表单形式 POST `act=...` 取 JSON 数据。
//!
//! 返回列约定：
//! - `spot_hog_soozhu`：`省份, 价格, 涨跌幅`
//! - 其余走势类：`日期, 价格`

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use scraper::{Html, Selector};
use serde_json::{Map, Value};

const SOOZHU_URL: &str = "https://www.soozhu.com/price/data/center/";

/// 提取页面中的 `csrfmiddlewaretoken`（对应 akshare BeautifulSoup 解析）。
///
/// 复用同一 `HttpClient` 以维持会话 cookie（akshare 用单个 `requests.Session`，
/// GET 首页写入的 cookie 需带到后续 POST，否则服务端返回 403）。
fn fetch_csrf(http: &HttpClient) -> Result<String> {
    let text = http.get_text(SOOZHU_URL, &Map::new(), None)?;
    let doc = Html::parse_document(&text);
    let sel = Selector::parse("input[name=\"csrfmiddlewaretoken\"]")
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    doc.select(&sel)
        .next()
        .and_then(|e| e.value().attr("value").map(str::to_string))
        .ok_or_else(|| AkshareError::Empty("搜猪网未找到 csrfmiddlewaretoken".into()))
}

/// POST 数据中心接口（带 csrf），返回解析后的 JSON。
fn post_data(act: &str, indid: Option<&str>) -> Result<Value> {
    let http = HttpClient::default();
    let token = fetch_csrf(&http)?;
    let mut params = Map::new();
    params.insert("act".into(), Value::String(act.into()));
    params.insert("csrfmiddlewaretoken".into(), Value::String(token));
    if let Some(id) = indid {
        params.insert("indid".into(), Value::String(id.into()));
    }
    http.post_form(SOOZHU_URL, &params, &[])
}

/// 从 `[date, price]` 数组行构建 `日期, 价格` 表。
fn build_trend(rows: &[Value]) -> Result<Df> {
    let mut data: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in rows {
        let arr = r.as_array().cloned().unwrap_or_default();
        if arr.len() < 2 {
            continue;
        }
        let date = arr[0].as_str().map(str::to_string);
        let price = arr[1].as_str().map(str::to_string);
        data.push(vec![date, price]);
    }
    let mut df = Df::from_string_rows(&["日期", "价格"], &data)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["价格"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// 搜猪-各省均价实时排行榜（对应 akshare [`spot_hog_soozhu`]）。
///
/// # 返回列
/// `省份, 价格, 涨跌幅`（涨跌幅保留 2 位小数）
pub fn spot_hog_soozhu() -> Result<Df> {
    let json = post_data("mapdata", None)?;
    let vlist = json
        .get("vlist")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("搜猪网 vlist 缺失".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(vlist.len());
    for item in &vlist {
        let name = item.get("name").and_then(Value::as_str).map(str::to_string);
        let value = item.get("value").and_then(Value::as_array).cloned();
        let (price, pct) = match value.and_then(|v| {
            let mut it = v.into_iter();
            let first = it.next();
            let second = it.next();
            match (first, second) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            }
        }) {
            Some((p, rest)) => {
                let price = p.as_str().map(str::to_string);
                let pct = rest.as_str().map(str::to_string);
                (price, pct)
            }
            None => (None, None),
        };
        rows.push(vec![name, price, pct]);
    }
    let mut df = Df::from_string_rows(&["省份", "价格", "涨跌幅"], &rows)?;
    df.cast_numeric(&["价格"])?;
    df.round_column("涨跌幅", 2)?;
    Ok(df)
}

/// 搜猪-今年以来全国出栏均价走势（对应 akshare [`spot_hog_year_trend_soozhu`]）。
///
/// # 返回列
/// `日期, 价格`
pub fn spot_hog_year_trend_soozhu() -> Result<Df> {
    let json = post_data("yeartrend", None)?;
    let list = json
        .get("nationlist")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("搜猪网 nationlist 缺失".into()))?;
    build_trend(&list)
}

/// 通用「价格走势」接口（对应 akshare `pricetrend` 系列）。
///
/// `indid` 为空表示全国瘦肉型肉猪；其余取具体品种 ID。
fn spot_hog_pricetrend(indid: Option<&str>) -> Result<Df> {
    let json = post_data("pricetrend", indid)?;
    let list = json
        .get("datalist")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("搜猪网 datalist 缺失".into()))?;
    build_trend(&list)
}

/// 全国瘦肉型肉猪（对应 akshare [`spot_hog_lean_price_soozhu`]）。
pub fn spot_hog_lean_price_soozhu() -> Result<Df> {
    spot_hog_pricetrend(None)
}

/// 全国三元仔猪（对应 akshare [`spot_hog_three_way_soozhu`]）。
pub fn spot_hog_three_way_soozhu() -> Result<Df> {
    spot_hog_pricetrend(Some("4"))
}

/// 全国后备二元母猪（对应 akshare [`spot_hog_crossbred_soozhu`]）。
pub fn spot_hog_crossbred_soozhu() -> Result<Df> {
    spot_hog_pricetrend(Some("6"))
}

/// 全国玉米价格走势（对应 akshare [`spot_corn_price_soozhu`]）。
pub fn spot_corn_price_soozhu() -> Result<Df> {
    spot_hog_pricetrend(Some("8"))
}

/// 全国豆粕价格走势（对应 akshare [`spot_soybean_price_soozhu`]）。
pub fn spot_soybean_price_soozhu() -> Result<Df> {
    spot_hog_pricetrend(Some("9"))
}

/// 全国育肥猪合料半月走势（对应 akshare [`spot_mixed_feed_soozhu`]）。
pub fn spot_mixed_feed_soozhu() -> Result<Df> {
    spot_hog_pricetrend(Some("11"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_trend_sorts_and_numerics() {
        let rows = vec![
            Value::Array(vec![Value::String("2024-03-01".into()), Value::from(11.0)]),
            Value::Array(vec![Value::String("2024-01-01".into()), Value::from(10.0)]),
        ];
        let df = build_trend(&rows).unwrap();
        assert_eq!(df.height(), 2);
        let dates = df.inner().column("日期").unwrap().str().unwrap();
        assert_eq!(dates.get(0), Some("2024-01-01"));
        assert_eq!(dates.get(1), Some("2024-03-01"));
        assert_eq!(
            *df.inner().column("价格").unwrap().dtype(),
            polars::datatypes::DataType::Float64
        );
    }

    #[test]
    fn build_trend_skips_short_rows() {
        let rows = vec![
            Value::Array(vec![Value::String("2024-01-01".into())]),
            Value::Array(vec![Value::String("2024-02-01".into()), Value::from(9.5)]),
        ];
        let df = build_trend(&rows).unwrap();
        assert_eq!(df.height(), 1);
    }
}
