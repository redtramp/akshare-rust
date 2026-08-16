//! 新浪财经-商品现货价格指数（`finance.sina.com.cn`）。
//!
//! 对应 akshare `index/index_spot.py::spot_goods`：
//! `GoodsIndexService.get_goods_index` 接口返回 JSON，列名逐字对齐。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

const GOODS_URL: &str =
    "https://stock.finance.sina.com.cn/futures/api/openapi.php/GoodsIndexService.get_goods_index";

/// 新浪商品现货价格指数（对应 akshare [`spot_goods`]）。
///
/// `symbol`：`波罗的海干散货指数` / `钢坯价格指数` / `澳大利亚粉矿价格`。
///
/// # 返回列
/// `日期, 指数, 涨跌额, 涨跌幅`
pub fn spot_goods(symbol: &str) -> Result<Df> {
    let symbol_map = [
        ("波罗的海干散货指数", "BDI"),
        ("钢坯价格指数", "GP"),
        ("澳大利亚粉矿价格", "PB"),
    ];
    let code = symbol_map
        .iter()
        .find(|(cn, _)| *cn == symbol)
        .map(|(_, code)| *code)
        .ok_or_else(|| AkshareError::Param(format!("未知现货指数: {symbol}")))?;

    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("symbol".into(), Value::String(code.into()));
    params.insert("table".into(), Value::String("0".into()));
    let json = http.get_json(GOODS_URL, &params, Some("https://finance.sina.com.cn/"))?;
    let data = json
        .pointer("/result/data/data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("新浪现货指数 data 缺失".into()))?;

    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for item in &data {
        let date = item
            .get("opendate")
            .and_then(Value::as_str)
            .map(str::to_string);
        let price = item
            .get("price")
            .and_then(Value::as_str)
            .map(str::to_string);
        let zde = item.get("zde").and_then(Value::as_str).map(str::to_string);
        let zdf = item.get("zdf").and_then(Value::as_str).map(str::to_string);
        // akshare dropna：日期缺失的行丢弃
        if date.is_none() {
            continue;
        }
        rows.push(vec![date, price, zde, zdf]);
    }

    let mut df = Df::from_string_rows(&["日期", "指数", "涨跌额", "涨跌幅"], &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["指数", "涨跌额", "涨跌幅"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_map_resolves() {
        for s in ["波罗的海干散货指数", "钢坯价格指数", "澳大利亚粉矿价格"] {
            let r = spot_goods(s);
            // 网络不可达时返回 Err，但不应因 symbol 不匹配返回 Param
            if let Err(AkshareError::Param(msg)) = &r {
                panic!("symbol 解析错误: {msg}");
            }
        }
    }
}
