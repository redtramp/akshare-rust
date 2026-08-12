//! 巨潮资讯（cninfo）数据源。
//!
//! 对应 akshare `stock/stock_profile_cninfo.py` 等模块：
//! - 所有接口走 `https://webapi.cninfo.com.cn/api/sysapi/p_sysapiXXXX` POST
//! - 请求头需携带 `Accept-Enckey`：由内置 `cninfo.js` 的 `getResCode1()`
//!   （AES-CBC(时间戳) → base64）生成，对应 akshare 的 `py_mini_racer` 执行
//! - 响应形如 `{count, records: [...]}`，列序 = records 首对象键序

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::core::js_engine::cninfo_get_res_code;
use serde_json::{json, Map, Value};

/// cninfo 公共请求头（对应 akshare 各 cninfo 函数的 headers 字典）。
fn cninfo_headers(mcode: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Accept", "*/*".to_string()),
        ("Accept-Enckey", mcode.to_string()),
        ("Accept-Encoding", "gzip, deflate".to_string()),
        ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8".to_string()),
        ("Cache-Control", "no-cache".to_string()),
        ("Content-Length", "0".to_string()),
        ("Host", "webapi.cninfo.com.cn".to_string()),
        ("Origin", "https://webapi.cninfo.com.cn".to_string()),
        ("Pragma", "no-cache".to_string()),
        ("Proxy-Connection", "keep-alive".to_string()),
        ("Referer", "https://webapi.cninfo.com.cn/".to_string()),
        ("X-Requested-With", "XMLHttpRequest".to_string()),
    ]
}

/// POST sysapi 端点并返回 `records` 数组。
///
/// 对应 akshare 每个 cninfo 函数中重复的
/// `js_code.call("getResCode1") → requests.post(url, params, headers)` 流程。
pub(crate) fn post_sysapi(endpoint: &str, params: &Map<String, Value>) -> Result<Vec<Value>> {
    let mcode = cninfo_get_res_code()?;
    let headers: Vec<(&str, String)> = cninfo_headers(&mcode);
    let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let url = format!("https://webapi.cninfo.com.cn/api/sysapi/{endpoint}");
    let http = HttpClient::default();
    let data = http.post_json(&url, params, &header_refs)?;
    let records = data
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(records)
}

/// 单条记录转 DataFrame：按 `records[0]` 键序取前 `n` 个值，列名映射为 `columns`。
///
/// 对应 akshare 的 `pd.Series(records_json).to_frame().T` + 列名覆盖。
/// 键序由 JSON `preserve_order` 保证与响应一致。
fn single_record_df(records: &[Value], columns: &[&str]) -> Result<Df> {
    let Some(record) = records.first() else {
        return Df::from_string_rows(columns, &[]);
    };
    let obj = record
        .as_object()
        .ok_or_else(|| AkshareError::Empty("records[0] 不是 JSON 对象".into()))?;
    let keys: Vec<&String> = obj.keys().collect();
    let n = columns.len().min(keys.len());
    let row: Vec<Option<String>> = keys[..n]
        .iter()
        .map(|k| match obj.get(*k) {
            Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(other) => Some(other.to_string()),
            None => None,
        })
        .collect();
    Df::from_string_rows(columns, &[row])
}

/// 多记录 DataFrame：records 数组转表，列名覆盖为 `columns`。
///
/// 对应 akshare `pd.DataFrame(records)` + 列名覆盖。
/// 注意：多条记录可能字段不一致，akshare 以首条键序为准。
fn multi_record_df(records: &[Value], columns: &[&str]) -> Result<Df> {
    if records.is_empty() {
        return Df::from_string_rows(columns, &[]);
    }
    let Some(first) = records.first().and_then(Value::as_object) else {
        return Err(AkshareError::Empty("records[0] 不是 JSON 对象".into()));
    };
    let keys: Vec<&String> = first.keys().collect();
    let n = columns.len().min(keys.len());
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(records.len());
    for r in records {
        let obj = r
            .as_object()
            .ok_or_else(|| AkshareError::Empty("records 元素不是 JSON 对象".into()))?;
        let row: Vec<Option<String>> = keys[..n]
            .iter()
            .map(|k| match obj.get(*k) {
                Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(other) => Some(other.to_string()),
                None => None,
            })
            .collect();
        rows.push(row);
    }
    Df::from_string_rows(columns, &rows)
}

/// 列重命名（基于新列名集合构建子表，对应 akshare `df[cols]` 重排）。
fn reorder_df(df: &Df, columns: &[&str]) -> Result<Df> {
    df.select(columns)
}

/// 巨潮资讯-个股-公司概况（对应 akshare [`akshare.stock_profile_cninfo`]）。
///
/// # 返回列
/// `公司名称, 英文名称, 曾用简称, A股代码, A股简称, B股代码, B股简称,
/// H股代码, H股简称, 入选指数, 所属市场, 所属行业, 法人代表, 注册资金,
/// 成立日期, 上市日期, 官方网站, 电子邮箱, 联系电话, 传真, 注册地址,
/// 办公地址, 邮政编码, 主营业务, 经营范围, 机构简介`
pub fn stock_profile_cninfo(symbol: &str) -> Result<Df> {
    let params = json!({ "scode": symbol });
    let records = post_sysapi("p_sysapi1133", params.as_object().expect("scode 参数"))?;
    const COLUMNS: [&str; 26] = [
        "公司名称",
        "英文名称",
        "曾用简称",
        "A股代码",
        "A股简称",
        "B股代码",
        "B股简称",
        "H股代码",
        "H股简称",
        "入选指数",
        "所属市场",
        "所属行业",
        "法人代表",
        "注册资金",
        "成立日期",
        "上市日期",
        "官方网站",
        "电子邮箱",
        "联系电话",
        "传真",
        "注册地址",
        "办公地址",
        "邮政编码",
        "主营业务",
        "经营范围",
        "机构简介",
    ];
    single_record_df(&records, &COLUMNS)
}

/// 巨潮资讯-个股-上市相关（对应 akshare [`akshare.stock_ipo_summary_cninfo`]）。
///
/// 日期列保留原字符串（akshare 转 `date` 类型，字符串形式一致）。
///
/// # 返回列
/// `股票代码, 招股公告日期, 中签率公告日, 每股面值, 总发行数量,
/// 发行前每股净资产, 摊薄发行市盈率, 募集资金净额, 上网发行日期,
/// 上市日期, 发行价格, 发行费用总额, 发行后每股净资产, 上网发行中签率, 主承销商`
pub fn stock_ipo_summary_cninfo(symbol: &str) -> Result<Df> {
    let params = json!({ "scode": symbol });
    let records = post_sysapi("p_sysapi1134", params.as_object().expect("scode 参数"))?;
    const COLUMNS: [&str; 15] = [
        "股票代码",
        "招股公告日期",
        "中签率公告日",
        "每股面值",
        "总发行数量",
        "发行前每股净资产",
        "摊薄发行市盈率",
        "募集资金净额",
        "上网发行日期",
        "上市日期",
        "发行价格",
        "发行费用总额",
        "发行后每股净资产",
        "上网发行中签率",
        "主承销商",
    ];
    let mut df = single_record_df(&records, &COLUMNS)?;
    df.cast_date(&["招股公告日期", "中签率公告日", "上网发行日期", "上市日期"])?;
    df.cast_numeric(&[
        "每股面值",
        "总发行数量",
        "发行前每股净资产",
        "摊薄发行市盈率",
        "募集资金净额",
        "发行价格",
        "发行费用总额",
        "发行后每股净资产",
        "上网发行中签率",
    ])?;
    Ok(df)
}

/// 巨潮资讯-个股-历史分红（对应 akshare [`akshare.stock_dividend_cninfo`]）。
///
/// 多记录：列重命名 + 数值化 + 按实施方案公告日期排序（升序）。
///
/// # 返回列
/// `实施方案公告日期, 分红类型, 送股比例, 转增比例, 派息比例, 股权登记日,
/// 除权日, 派息日, 股份到账日, 实施方案分红说明, 报告时间`
pub fn stock_dividend_cninfo(symbol: &str) -> Result<Df> {
    let params = json!({ "scode": symbol });
    let records = post_sysapi("p_sysapi1139", params.as_object().expect("scode 参数"))?;

    // 对应 akshare：`pd.DataFrame(records)`（列序=响应键序）→ `rename`（位置不变）
    // → 日期/数值转换 → 排序 → 重排到输出列序。
    // 注意：响应键序可能 ≠ RENAME 数组序，必须保持响应键序再按名重排。
    const RENAME: [(&str, &str); 11] = [
        ("F006D", "实施方案公告日期"),
        ("F044V", "分红类型"),
        ("F011N", "转增比例"),
        ("F010N", "送股比例"),
        ("F012N", "派息比例"),
        ("F018D", "股权登记日"),
        ("F020D", "除权日"),
        ("F023D", "派息日"),
        ("F025D", "股份到账日"),
        ("F007V", "实施方案分红说明"),
        ("F001V", "报告时间"),
    ];
    // 1) 保持响应键序构建（对应 pd.DataFrame(records)）
    let mut df = Df::from_json_rows(&records)?;
    // 2) 按名重命名（仅重命名存在的键，位置不变，对应 rename）
    let cur_names = df.column_names();
    let renamed: Vec<String> = cur_names
        .iter()
        .map(|n| {
            RENAME
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| n.clone())
        })
        .collect();
    let refs: Vec<&str> = renamed.iter().map(String::as_str).collect();
    df.rename_columns(&refs)?;
    // 3) 日期/数值转换
    df.cast_date(&["实施方案公告日期", "股权登记日", "除权日", "派息日"])?;
    df.cast_numeric(&["送股比例", "转增比例", "派息比例"])?;
    // 4) 按日期升序排序（ISO 字符串序 = 时间序）
    let sorted = df.sort_by("实施方案公告日期", true, false)?;
    // 5) 重排到 akshare 输出列序
    reorder_df(
        &sorted,
        &[
            "实施方案公告日期",
            "分红类型",
            "送股比例",
            "转增比例",
            "派息比例",
            "股权登记日",
            "除权日",
            "派息日",
            "股份到账日",
            "实施方案分红说明",
            "报告时间",
        ],
    )
}

/// 巨潮资讯-数据中心-新股数据-新股发行（对应 akshare [`akshare.stock_new_ipo_cninfo`]）。
///
/// # 返回列
/// `证劵代码, 证券简称, 上市日期, 申购日期, 发行价, 总发行数量, 发行市盈率,
/// 上网发行中签率, 摇号结果公告日, 中签公告日, 中签缴款日, 网上申购上限, 上网发行数量`
pub fn stock_new_ipo_cninfo() -> Result<Df> {
    let params = json!({ "timetype": "36", "market": "ALL" });
    let records = post_sysapi("p_sysapi1097", params.as_object().expect("timetype 参数"))?;
    const COLUMNS: [&str; 13] = [
        "摇号结果公告日",
        "中签公告日",
        "证券简称",
        "上市日期",
        "中签缴款日",
        "申购日期",
        "发行价",
        "证劵代码",
        "上网发行中签率",
        "总发行数量",
        "发行市盈率",
        "上网发行数量",
        "网上申购上限",
    ];
    let mut df = multi_record_df(&records, &COLUMNS)?;
    df.cast_date(&[
        "摇号结果公告日",
        "中签公告日",
        "上市日期",
        "中签缴款日",
        "申购日期",
    ])?;
    df.cast_numeric(&[
        "发行价",
        "上网发行中签率",
        "总发行数量",
        "发行市盈率",
        "上网发行数量",
        "网上申购上限",
    ])?;
    reorder_df(
        &df,
        &[
            "证劵代码",
            "证券简称",
            "上市日期",
            "申购日期",
            "发行价",
            "总发行数量",
            "发行市盈率",
            "上网发行中签率",
            "摇号结果公告日",
            "中签公告日",
            "中签缴款日",
            "网上申购上限",
            "上网发行数量",
        ],
    )
}

/// 巨潮资讯-数据中心-新股数据-新股过会（对应 akshare [`akshare.stock_new_gh_cninfo`]）。
///
/// # 返回列
/// `公司名称, 上会日期, 审核类型, 审议内容, 审核结果, 审核公告日`
pub fn stock_new_gh_cninfo() -> Result<Df> {
    let records = post_sysapi("p_sysapi1098", &Map::new())?;
    const COLUMNS: [&str; 6] = [
        "公司名称",
        "上会日期",
        "审核类型",
        "审议内容",
        "审核结果",
        "审核公告日",
    ];
    let mut df = multi_record_df(&records, &COLUMNS)?;
    df.cast_date(&["上会日期", "审核公告日"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_record_picks_first_n_keys() {
        let records = vec![json!({
            "K1": "v1", "K2": "v2", "K3": "v3", "K4": "v4",
        })];
        let df = single_record_df(&records, &["A", "B"]).unwrap();
        assert_eq!(df.column_names(), vec!["A", "B"]);
        assert_eq!(df.height(), 1);
        assert!(!df.head_preview(1).is_empty());
    }

    #[test]
    fn single_record_empty_gives_empty_df() {
        let df = single_record_df(&[], &["A", "B"]).unwrap();
        assert_eq!(df.height(), 0);
        assert_eq!(df.column_names(), vec!["A", "B"]);
    }

    #[test]
    fn multi_record_key_order_from_first() {
        let records = vec![
            json!({"F006D": "2024-01-01", "F044V": "派息"}),
            json!({"F006D": "2023-01-01", "F044V": "转增"}),
        ];
        let df = multi_record_df(&records, &["日期", "类型"]).unwrap();
        assert_eq!(df.column_names(), vec!["日期", "类型"]);
        assert_eq!(df.height(), 2);
    }

    #[test]
    fn reorder_keeps_given_order() {
        let df = Df::from_string_rows(
            &["b", "a"],
            &[vec![Some("1".into())], vec![Some("2".into())]],
        )
        .unwrap();
        let out = reorder_df(&df, &["a", "b"]).unwrap();
        assert_eq!(out.column_names(), vec!["a", "b"]);
    }
}
