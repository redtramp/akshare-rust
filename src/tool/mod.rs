//! tool 工具类模块（新浪交易日历）。
//!
//! 对应 akshare `tool/trade_date_hist.py`：
//! - [`tool_trade_date_hist_sina`]：新浪交易日历（sina.js `d()` 解码）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::core::js_engine::sina_js_decode;
use serde_json::{Map, Value};

/// 新浪财经-交易日历-历史数据（对应 akshare [`akshare.tool_trade_date_hist_sina`]）。
///
/// 数据源 `https://finance.sina.com.cn/realstock/company/klc_td_sh.txt`，
/// 返回 `var KLC_KL_xxx="<编码>"` 形式的 JS 赋值，经 sina.js `d()` 解码为日期数组。
/// akshare 会补充 1992-05-04（该日期为交易日但源数据缺失）后升序排序。
///
/// # 返回列
/// `trade_date`
pub fn tool_trade_date_hist_sina() -> Result<Df> {
    const URL: &str = "https://finance.sina.com.cn/realstock/company/klc_td_sh.txt";
    let http = HttpClient::default();
    let text = http.get_text(URL, &Map::new(), None)?;

    // 与 akshare 一致：split("=")[1].split(";")[0].replace('"', "")
    let encoded = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪交易日历响应缺少 = 分隔".into()))?
        .split(';')
        .next()
        .ok_or_else(|| AkshareError::Empty("新浪交易日历响应缺少 ; 分隔".into()))?
        .replace('"', "");

    let decoded = sina_js_decode(&encoded)?;
    let arr: Vec<Value> =
        serde_json::from_str(&decoded).map_err(|e| AkshareError::json(URL, e.to_string()))?;

    // akshare：pd.DataFrame(dict_list) 单列 → columns=["trade_date"]
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(arr.len() + 1);
    for v in &arr {
        let s = match v {
            Value::String(s) => Some(s.clone()),
            Value::Null => None,
            other => Some(other.to_string()),
        };
        rows.push(vec![s]);
    }
    // 该日期是交易日但新浪返回缺失，补充（akshare 源码固定追加）
    rows.push(vec![Some("1992-05-04".into())]);

    let mut df = Df::from_string_rows(&["trade_date"], &rows)?;
    df.cast_date(&["trade_date"])?;
    let sorted = df.sort_by("trade_date", true, false)?;
    Ok(sorted)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_encoded_format_offline() {
        // 模拟 klc_td_sh.txt 的 var KLC_KL_xxx="..." 结构
        let text = r#"var KLC_KL_xxx="abc123";"#;
        let encoded = text
            .split('=')
            .nth(1)
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .replace('"', "");
        assert_eq!(encoded, "abc123");
    }
}
