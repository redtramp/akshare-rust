//! 百度股市通-外汇-行情榜单（fx 分类）。
//!
//! 对应 akshare `fx/fx_quote_baidu.py`：
//! - [`fx_quote_baidu`]：外汇行情榜单（人民币/美元），分页抓取 `getforeignrank`
//!
//! 源端 `finance.pae.baidu.com` 需要 `acs-token`（目标网站复制）才有数据；
//! 未携带 token 时返回码非 `0`，与 akshare 一致返回空表（不伪造数据）。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use serde_json::{json, Map, Value};

const BAIDU_RANK_URL: &str = "https://finance.pae.baidu.com/api/getforeignrank";

/// 百度外汇榜单请求头（对应 akshare `fx_quote_baidu` 的 headers）。
fn rank_headers(token: &str) -> Vec<(&'static str, &str)> {
    vec![
        ("accept", "application/json, text/plain, */*"),
        ("accept-language", "zh-CN,zh;q=0.9"),
        ("origin", "https://finance.baidu.com"),
        ("referer", "https://finance.baidu.com/"),
        ("acs-token", token),
    ]
}

/// 百度股市通-外汇-行情榜单（对应 akshare [`akshare.fx_quote_baidu`]）。
///
/// - `symbol`: `"人民币"` / `"美元"`
/// - `token`: 目标网站复制的 `acs-token`（无 token 时接口返回非 0 码，返回空表）
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌额, 涨跌幅`
pub fn fx_quote_baidu(symbol: &str, token: &str) -> Result<Df> {
    let symbol_map = match symbol {
        "人民币" => "rmb",
        "美元" => "dollar",
        other => {
            return Err(crate::core::error::AkshareError::Param(format!(
                "无效 symbol: {other}，可选 人民币/美元"
            )))
        }
    };
    let headers = rank_headers(token);
    let http = HttpClient::default();

    let mut out_rows: Vec<Vec<Option<String>>> = Vec::new();
    let mut page = 0usize;
    loop {
        let params = json!({
            "type": symbol_map,
            "pn": page,
            "rn": "20",
            "finClientType": "pc",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

        let value = match http.get_json_with_headers(BAIDU_RANK_URL, &params, &headers, None) {
            Ok(v) => v,
            // 网络/反爬失败：与 akshare 一样中止分页，返回已取数据（可能为空）
            Err(_) => break,
        };
        // ResultCode != "0" → akshare 打印异常后 break，返回已取数据（通常为空）
        if value.get("ResultCode").and_then(Value::as_str) != Some("0") {
            break;
        }
        let result = value
            .get("Result")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if result.is_empty() {
            break;
        }

        // 每行含 `list` 嵌套（外层字段 + 嵌套 ["0"=列名, "1"=值] 两行结构），
        // 与 akshare `pd.DataFrame(item).T.iloc[1]`（值行）语义一致。
        let mut page_rows: Vec<Vec<Option<String>>> = Vec::new();
        let mut value_row: Option<Vec<String>> = None;
        for row in &result {
            let Some(obj) = row.as_object() else { continue };
            let Some(list) = obj.get("list") else {
                continue;
            };
            let Some(list_obj) = list.as_object() else {
                continue;
            };
            let names: Vec<&str> = match list_obj.get("0") {
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
                _ => continue,
            };
            let values: Vec<String> = match list_obj.get("1") {
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect(),
                _ => continue,
            };
            let row_map: std::collections::HashMap<String, String> = names
                .iter()
                .zip(values.iter())
                .map(|(k, v)| ((*k).to_string(), v.clone()))
                .collect();
            // 目标列：代码/名称/最新价/涨跌额/涨跌幅（缺失置空）
            let pick = |k: &str| row_map.get(k).cloned();
            page_rows.push(vec![
                pick("代码"),
                pick("名称"),
                pick("最新价"),
                pick("涨跌额"),
                pick("涨跌幅"),
            ]);
            value_row = Some(values);
        }
        if value_row.is_none() {
            break;
        }
        // 本页不足 20 条即末页（与 akshare 一致）
        let last = page_rows.len();
        out_rows.extend(page_rows);
        if last < 20 {
            break;
        }
        page += 20;
    }

    let mut df = Df::from_string_rows(&["代码", "名称", "最新价", "涨跌额", "涨跌幅"], &out_rows)?;
    df.cast_numeric(&["最新价", "涨跌额"])?;
    // 涨跌幅带 % 号：剥除后 ÷100（对应 akshare strip("%") / 100）
    df.strip_suffix(&["涨跌幅"], "%")?;
    df.cast_numeric(&["涨跌幅"])?;
    df.scale("涨跌幅", 100.0)?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_symbol_rejected() {
        assert!(fx_quote_baidu("欧元", "").is_err());
    }

    #[test]
    fn nested_list_parse_offline() {
        // 模拟 Result 单行 {"list": {"0": [...], "1": [...]}} 的结构抽取
        let row = json!({
            "market": "人民币",
            "list": {
                "0": ["代码", "名称", "最新价", "涨跌额", "涨跌幅"],
                "1": ["USDCNY", "美元/人民币", "7.1200", "-0.01", "-0.14%"],
            },
            "status": "1",
        });
        let obj = row.as_object().unwrap();
        let list = obj.get("list").unwrap().as_object().unwrap();
        let names: Vec<&str> = list
            .get("0")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let values: Vec<String> = list
            .get("1")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        assert_eq!(names, vec!["代码", "名称", "最新价", "涨跌额", "涨跌幅"]);
        assert_eq!(values.len(), 5);
    }
}
