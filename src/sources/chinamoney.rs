//! 外汇交易中心暨全国银行间同业拆借中心（chinamoney）数据源。
//!
//! 对应 akshare `bond/bond_china.py`、`bond/bond_china_money.py`、
//! `bond/bond_info_cm.py`：所有接口走 `https://www.chinamoney.com.cn/ags/...`
//! 的 POST（表单体）或 GET，返回 JSON（含 `records` / `data.resultList`）。
//! 实测无需登录态即可直连取数（akshare 的 `__bond_register_service` 兜底分支
//! 在本环境用简单请求即可命中）。

use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

/// chinamoney 请求 UA（对应 akshare `utils/cons.headers` 的浏览器 UA）。
const CM_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36";

/// POST 表单体到 chinamoney 接口并返回 JSON（对应 akshare `requests.post(url, data=..., headers=...)`）。
///
/// `payload` 使用所有权字符串以便调用方拼接分页/动态参数（`pageNo`、`bondCode` 等）。
pub fn cm_post(url: &str, payload: &[(String, String)]) -> Result<Value> {
    let mut params = Map::new();
    for (k, v) in payload {
        params.insert(k.clone(), Value::String(v.clone()));
    }
    let headers = [("User-Agent", CM_UA)];
    HttpClient::default().post_form(url, &params, &headers)
}

/// GET chinamoney 接口并返回 JSON（带 UA）。
pub fn cm_get(url: &str, params: &[(String, String)]) -> Result<Value> {
    let mut p = Map::new();
    for (k, v) in params {
        p.insert(k.clone(), Value::String(v.clone()));
    }
    let headers = [("User-Agent", CM_UA)];
    HttpClient::default().get_json_with_headers(url, &p, &headers, None)
}

/// 抽取 `records` 数组（对应 akshare `data_json["records"]`）。
pub(crate) fn records_of(data: &Value) -> Vec<Value> {
    data.get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// 抽取 `data.resultList` 数组（bond_info_cm 列表接口）。
pub(crate) fn result_list_of(data: &Value) -> Vec<Value> {
    data.get("data")
        .and_then(|d| d.get("resultList"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// 从响应取分页总数（`pageTotal` / `pageTotalSize`）。
pub(crate) fn page_total_of(data: &Value) -> u64 {
    if let Some(d) = data.get("data") {
        if let Some(n) = d.get("pageTotal").and_then(Value::as_u64) {
            return n;
        }
        if let Some(n) = d.get("pageTotalSize").and_then(Value::as_u64) {
            return n;
        }
    }
    if let Some(n) = data.get("pageTotal").and_then(Value::as_u64) {
        return n;
    }
    if let Some(n) = data.get("pageTotalSize").and_then(Value::as_u64) {
        return n;
    }
    1
}

/// 随机延迟 0.5~1.5s（对应 akshare 翻页限流；§9 生产标准）。
pub(crate) fn random_delay() {
    let delay: f64 = rand::random_range(0.5..1.5);
    std::thread::sleep(std::time::Duration::from_secs_f64(delay));
}

/// 收盘收益率曲线映射表（bond_china_close_return 内部用于 symbol→code 解析）。
///
/// 对应 akshare `bond_china_close_return_map()`：GET
/// `ags/ms/cm-u-bk-currency/ClsYldCurvCurvGO`，返回 `records`
/// （含 `value`/`cnLabel`/`enLabel` 等键）。
pub fn close_return_map() -> Result<Vec<Value>> {
    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-bk-currency/ClsYldCurvCurvGO";
    let data = cm_get(url, &[])?;
    Ok(records_of(&data))
}

/// 空参数表（便于调用 `cm_post`/`cm_get` 的无参接口）。
fn no_params() -> Vec<(String, String)> {
    Vec::new()
}

/// 债券信息查询-筛选条件查询（对应 akshare `bond_info_cm_query`）。
///
/// `symbol` ∈ {"主承销商", "债券类型", "息票类型", "发行年份", "评级等级"}。
/// 返回数组元素形如 `{name, code}`（主承销商）或单键数组（其他）。
pub fn info_cm_query(symbol: &str) -> Result<Vec<Value>> {
    if symbol == "主承销商" {
        let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-bond-md/EntyFullNameSearchCondition";
        let data = cm_post(url, &no_params())?;
        let enty = data
            .get("data")
            .and_then(|d| d.get("enty"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        return Ok(enty);
    }
    let symbol_map = [
        ("债券类型", "bondType"),
        ("息票类型", "couponType"),
        ("发行年份", "issueYear"),
        ("评级等级", "bondRtngShrt"),
    ];
    let key = symbol_map
        .iter()
        .find(|(cn, _)| *cn == symbol)
        .map(|(_, k)| *k)
        .ok_or_else(|| AkshareError::Param(format!("未知查询指标: {symbol}")))?;
    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-bond-md/BondBaseInfoSearchCondition";
    let data = cm_post(url, &no_params())?;
    let arr = data
        .get("data")
        .and_then(|d| d.get(key))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(arr)
}
