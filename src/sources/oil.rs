//! 中国油价数据源（批次 5 长尾 · energy）。
//!
//! 对应 akshare `energy/energy_oil_em.py`（东方财富数据中心）：
//! - 汽柴油历史调价信息（`energy_oil_hist`）
//! - 全国各地区汽柴油价格（`energy_oil_detail`）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

const OIL_URL: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
const OIL_TOKEN: &str = "894050c76af8597a853f5b408b759f5d";

/// 通用：GET 东财油价接口并取 `result.data` 数组。
fn fetch_oil_data(report: &str, extra: &[(&str, &str)]) -> Result<Vec<Value>> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("reportName".into(), Value::String(report.into()));
    params.insert("columns".into(), Value::String("ALL".into()));
    params.insert("token".into(), Value::String(OIL_TOKEN.into()));
    params.insert("pageNumber".into(), Value::String("1".into()));
    params.insert("pageSize".into(), Value::String("1000".into()));
    params.insert("source".into(), Value::String("WEB".into()));
    for (k, v) in extra {
        params.insert((*k).into(), Value::String((*v).into()));
    }
    let json = http.get_json(OIL_URL, &params, None)?;
    json.get("result")
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("东财油价数据缺失".into()))
}

/// 取 JSON 对象的字符串字段（None 保持 None）。
fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| match x {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    })
}

/// 汽柴油历史调价信息（对应 akshare [`energy_oil_hist`]）。
///
/// # 返回列
/// `调整日期, 汽油价格, 柴油价格, 汽油涨跌, 柴油涨跌`
pub fn energy_oil_hist() -> Result<Df> {
    let data = fetch_oil_data(
        "RPTA_WEB_YJ_BD",
        &[
            ("sortColumns", "dim_date"),
            ("sortTypes", "-1"),
            ("p", "1"),
            ("pageNo", "1"),
            ("pageNum", "1"),
        ],
    )?;
    let rows: Vec<Vec<Option<String>>> = data
        .iter()
        .map(|v| {
            vec![
                s(v, "DIM_DATE"),
                s(v, "VALUE"),
                s(v, "CY_JG"),
                s(v, "QY_FD"),
                s(v, "CY_FD"),
            ]
        })
        .collect();
    let mut df = Df::from_string_rows(
        &["调整日期", "汽油价格", "柴油价格", "汽油涨跌", "柴油涨跌"],
        &rows,
    )?;
    df.cast_date(&["调整日期"])?;
    df.cast_numeric(&["汽油价格", "柴油价格", "汽油涨跌", "柴油涨跌"])?;
    // akshare 按调整日期升序
    let sorted = df.sort_by("调整日期", true, false)?;
    Ok(sorted)
}

/// 全国各地区汽柴油价格（对应 akshare [`energy_oil_detail`]）。
///
/// 注意：akshare 用「按位置重命名」（`df.columns=[...]`），其列名与字段并非
/// 语义对应（如 `V_92` 实际来自 `V95` 字段）。本实现严格复刻该位置映射，
/// 以保证与 akshare 输出逐列一致。
///
/// # 参数
/// `date`：调整日期 `YYYYMMDD`（如 `"20240118"`）。
///
/// # 返回列
/// `日期, 地区, V_0, V_92, V_95, V_89, ZDE_0, ZDE_92, ZDE_95, ZDE_89, QE_0, QE_92, QE_95, QE_89`
pub fn energy_oil_detail(date: &str) -> Result<Df> {
    if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
        return Err(AkshareError::Empty(format!(
            "energy_oil_detail 日期需为 YYYYMMDD，收到: {date}"
        )));
    }
    let formatted = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
    let data = fetch_oil_data(
        "RPTA_WEB_YJ_JH",
        &[
            ("filter", &format!("(dim_date='{formatted}')")),
            ("sortColumns", "cityname"),
            ("sortTypes", "1"),
        ],
    )?;
    let rows: Vec<Vec<Option<String>>> = data
        .iter()
        .map(|v| {
            // 严格复刻 akshare 的位置重命名：data 字段顺序为
            // DIM_DATE, CITYNAME, V0, V95, V92, V89, ZDE0, ZDE92, ZDE95, ZDE89, QE0, QE92, QE95, QE89
            vec![
                s(v, "DIM_DATE"),
                s(v, "CITYNAME"),
                s(v, "V0"),
                s(v, "V95"),
                s(v, "V92"),
                s(v, "V89"),
                s(v, "ZDE0"),
                s(v, "ZDE92"),
                s(v, "ZDE95"),
                s(v, "ZDE89"),
                s(v, "QE0"),
                s(v, "QE92"),
                s(v, "QE95"),
                s(v, "QE89"),
            ]
        })
        .collect();
    let names = [
        "日期", "地区", "V_0", "V_92", "V_95", "V_89", "ZDE_0", "ZDE_92", "ZDE_95", "ZDE_89",
        "QE_0", "QE_92", "QE_95", "QE_89",
    ];
    let mut df = Df::from_string_rows(&names, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&[
        "V_0", "V_92", "V_95", "V_89", "ZDE_0", "ZDE_92", "ZDE_95", "ZDE_89", "QE_0", "QE_92",
        "QE_95", "QE_89",
    ])?;
    Ok(df)
}
