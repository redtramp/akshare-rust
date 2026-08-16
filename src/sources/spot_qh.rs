//! 99 期货期现数据源（`www.99qh.com` / `centerapi.fx168api.com`）。
//!
//! 对应 akshare `spot/spot_price_qh.py`：
//! - 品种对照表从 `spotTrend` 页面的 `__NEXT_DATA__`（Next.js 注入的 JSON）解析
//! - 现货走势需先从 `v.js` 响应头取 `_pcc` token，再请求 `spot/trend`

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use scraper::{Html, Selector};
use serde_json::{Map, Value};

const SPOTTREND_URL: &str = "https://www.99qh.com/data/spotTrend";
const TOKEN_URL: &str = "https://centerapi.fx168api.com/app/common/v.js";
const TREND_URL: &str = "https://centerapi.fx168api.com/app/qh/api/spot/trend";

/// 解析 `spotTrend` 页面的 `__NEXT_DATA__`，返回品种列表
/// （每项含 `qhExchangeName` / `name` / `productId`）。
fn variety_list() -> Result<Vec<Value>> {
    let http = HttpClient::default();
    let text = http.get_text(SPOTTREND_URL, &Map::new(), None)?;
    let doc = Html::parse_document(&text);
    let sel =
        Selector::parse("script#__NEXT_DATA__").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let data_text = doc
        .select(&sel)
        .next()
        .map(|e| e.text().collect::<Vec<_>>().join(""))
        .ok_or_else(|| AkshareError::Empty("99qh 页面缺少 __NEXT_DATA__".into()))?;
    let json: Value = serde_json::from_str(&data_text)
        .map_err(|e| AkshareError::json(SPOTTREND_URL, e.to_string()))?;
    let variety = json
        .pointer("/props/pageProps/data/varietyListData")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("99qh varietyListData 缺失".into()))?;
    let mut products = Vec::new();
    for item in &variety {
        if let Some(list) = item.get("productList").and_then(Value::as_array) {
            for p in list {
                products.push(p.clone());
            }
        }
    }
    Ok(products)
}

/// 99 期货-交易所与品种对照表（对应 akshare [`spot_price_table_qh`]）。
///
/// # 返回列
/// `交易所名称, 品种名称`
pub fn spot_price_table_qh() -> Result<Df> {
    let products = variety_list()?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(products.len());
    for p in &products {
        let exch = p
            .get("qhExchangeName")
            .and_then(Value::as_str)
            .map(str::to_string);
        let name = p.get("name").and_then(Value::as_str).map(str::to_string);
        rows.push(vec![exch, name]);
    }
    Df::from_string_rows(&["交易所名称", "品种名称"], &rows)
}

/// 99 期货-现货走势（对应 akshare [`spot_price_qh`]）。
///
/// `symbol`：品种名称，如 `螺纹钢`（可通过 [`spot_price_table_qh`] 获取）。
///
/// # 返回列
/// `日期, 期货收盘价, 现货价格`
pub fn spot_price_qh(symbol: &str) -> Result<Df> {
    let products = variety_list()?;
    // name -> productId（productId 在 JSON 中为数字或字符串，两种都接受）
    let mut symbol_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for p in &products {
        let name = p.get("name").and_then(Value::as_str);
        let id = match p.get("productId") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };
        if let (Some(n), Some(id)) = (name, id) {
            symbol_map.insert(n.to_string(), id);
        }
    }
    let product_id = symbol_map
        .get(symbol)
        .ok_or_else(|| AkshareError::Param(format!("未知品种: {symbol}")))?
        .clone();

    let http = HttpClient::default();
    let token = http
        .get_response_header_with_headers(
            TOKEN_URL,
            &Map::new(),
            "_pcc",
            &[
                ("Origin", "https://www.99qh.com"),
                ("Referer", "https://www.99qh.com"),
            ],
        )?
        .ok_or_else(|| AkshareError::Empty("99qh 未返回 _pcc token".into()))?;

    let mut params = Map::new();
    params.insert("productId".into(), Value::String(product_id));
    params.insert("pageNo".into(), Value::String("1".into()));
    params.insert("pageSize".into(), Value::String("50000".into()));
    params.insert("startDate".into(), Value::String(String::new()));
    params.insert("endDate".into(), Value::String("2050-01-01".into()));
    params.insert("appCategory".into(), Value::String("web".into()));

    let headers = [
        ("_pcc", token.as_str()),
        ("Origin", "https://www.99qh.com"),
        ("Referer", "https://www.99qh.com"),
    ];
    let json = http.get_json_with_headers(TREND_URL, &params, &headers, None)?;
    let list = json
        .pointer("/data/list")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("99qh spot/trend data.list 缺失".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(list.len());
    for item in &list {
        let date = item.get("date").and_then(Value::as_str).map(str::to_string);
        let fp = item.get("fp").and_then(Value::as_str).map(str::to_string);
        let sp = item.get("sp").and_then(Value::as_str).map(str::to_string);
        rows.push(vec![date, fp, sp]);
    }
    let mut df = Df::from_string_rows(&["日期", "期货收盘价", "现货价格"], &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["期货收盘价", "现货价格"])?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variety_list_empty_ok() {
        // 离线单测：空 productList 不崩溃
        let v = serde_json::json!({"props":{"pageProps":{"data":{"varietyListData":[{"productList":[]},{"productList":[]}]}}}});
        let arr = v
            .pointer("/props/pageProps/data/varietyListData")
            .unwrap()
            .as_array()
            .unwrap();
        let mut count = 0;
        for item in arr {
            if let Some(l) = item.get("productList").and_then(Value::as_array) {
                count += l.len();
            }
        }
        assert_eq!(count, 0);
    }
}
