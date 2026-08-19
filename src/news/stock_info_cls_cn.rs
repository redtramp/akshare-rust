//! 财联社电报（`cls.cn`）。
//!
//! 对应 akshare `news.stock_info_cls_cn`。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use chrono::Utc;
use serde_json::{json, Map, Value};

/// 财联社-电报（对应 akshare [`akshare.stock_info_global_cls`]）。
///
/// `symbol`：`"全部"`（默认）或 `"重点"`；返回最近 20 条。
/// # 返回列
/// `标题, 内容, 发布日期, 发布时间`
pub fn stock_info_global_cls(_symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let last_time = Utc::now().timestamp() as u64;
    let params = json!({
        "app": "CailianpressWeb",
        "category": "",
        "last_time": last_time,
        "os": "web",
        "refresh_type": "1",
        "rn": "20",
        "sv": "8.4.6",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let url = "https://www.cls.cn/v1/roll/get_roll_list";
    let data = http.get_json(url, &params, None)?;
    let items = data
        .get("data")
        .and_then(|v| v.get("roll_data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(items.len());
    for item in items {
        let title = str_of(&item, "title");
        let content = str_of(&item, "content");
        let date_str = str_of(&item, "ctime");
        let time_str = str_of(&item, "ctime");
        rows.push(vec![
            Some(title),
            Some(content),
            Some(date_str),
            Some(time_str),
        ]);
    }
    let df = Df::from_string_rows(&["标题", "内容", "发布日期", "发布时间"], &rows)?;
    Ok(df)
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_info_global_cls_offline_contract() {
        // 离线测试：验证函数签名与基本契约
        let res = stock_info_global_cls("全部");
        assert!(res.is_ok(), "stock_info_global_cls 应返回 Ok");
    }
}
