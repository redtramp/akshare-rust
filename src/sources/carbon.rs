//! 碳排放交易数据源（批次 5 长尾 · energy）。
//!
//! 对应 akshare `energy/energy_carbon.py`：
//! - 广州碳排放权交易中心行情（`energy_carbon_gz`）
//! - 湖北碳排放权交易中心现货每日概况（`energy_carbon_hb`）
//!
//! 注：`energy_carbon_domestic`（连接 `k.tanjiaoyi.com:8080` 失败）、
//! `energy_carbon_sz` / `energy_carbon_eu`（页面结构已变，解析 NoneType）、
//! `energy_carbon_bj`（多页抓取约 110s，超出 parity 120s 超时）均不可达或不宜对账，跳过。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::html::read_html_tables;
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

const GZ_URL: &str = "http://ets.cnemission.com/carbon/portalIndex/markethistory";
const HB_URL: &str = "https://www.hbets.cn/";

const GZ_HEADERS: &[(&str, &str)] = &[(
    "user-agent",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
)];

/// 广州碳排放权交易中心-行情信息（对应 akshare [`energy_carbon_gz`]）。
///
/// # 返回列
/// `日期, 品种, 开盘价, 收盘价, 最高价, 最低价, 涨跌, 涨跌幅, 成交数量, 成交金额`
pub fn energy_carbon_gz() -> Result<Df> {
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("Top".into(), Value::String("1".into()));
    params.insert("beginTime".into(), Value::String("2010-01-01".into()));
    params.insert("endTime".into(), Value::String("2030-09-12".into()));

    let text = http.get_text_with_headers(GZ_URL, &params, GZ_HEADERS, None)?;
    let tables = read_html_tables(&text)?;
    let table = tables
        .get(1)
        .ok_or_else(|| AkshareError::Empty("广州碳市场未解析到行情表".into()))?;
    if table.len() < 2 {
        return Err(AkshareError::Empty("广州碳市场行情表为空".into()));
    }
    // 跳过首行表头（pd.read_html(header=0) 语义）
    let data_rows: Vec<Vec<Option<String>>> = table[1..]
        .iter()
        .map(|r| r.iter().map(|c| Some(c.clone())).collect())
        .collect();

    let names = [
        "日期",
        "品种",
        "开盘价",
        "收盘价",
        "最高价",
        "最低价",
        "涨跌",
        "涨跌幅",
        "成交数量",
        "成交金额",
    ];
    let mut df = Df::from_string_rows(&names, &data_rows)?;
    df.cast_date(&["日期"])?;
    // 涨跌幅带百分号，先剥离再数值化
    df.strip_suffix(&["涨跌幅"], "%")?;
    df.cast_numeric(&[
        "开盘价",
        "收盘价",
        "最高价",
        "最低价",
        "涨跌",
        "涨跌幅",
        "成交数量",
        "成交金额",
    ])?;
    // akshare 按日期升序
    let sorted = df.sort_by("日期", true, false)?;
    Ok(sorted)
}

/// 湖北碳排放权交易中心-现货交易数据-配额-每日概况（对应 akshare [`energy_carbon_hb`]）。
///
/// # 返回列
/// `日期, 成交价, 成交量, 最新, 涨跌`
pub fn energy_carbon_hb() -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text_with_headers(HB_URL, &Map::new(), GZ_HEADERS, None)?;

    // 提取内联脚本中 `cjj = '[...]'` 的 JSON 数组（与 akshare 切片逻辑一致）
    let start = text
        .find("cjj = '[")
        .ok_or_else(|| AkshareError::Empty("湖北碳市场未找到 cjj 数组".into()))?
        + 7;
    let end = text
        .rfind("cjj =")
        .ok_or_else(|| AkshareError::Empty("湖北碳市场未找到 cjj 结束".into()))?
        - 31;
    if end <= start {
        return Err(AkshareError::Empty("湖北碳市场 cjj 数组切片异常".into()));
    }
    let sub = &text[start..end];
    let arr: Vec<Value> =
        serde_json::from_str(sub).map_err(|e| AkshareError::json(HB_URL, e.to_string()))?;

    let rows: Vec<Vec<Option<String>>> = arr
        .iter()
        .map(|v| {
            let get = |k: &str| -> Option<String> {
                v.get(k).and_then(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    Value::Null => None,
                    other => Some(other.to_string()),
                })
            };
            vec![get("riqi"), get("cjj"), get("cjl"), get("zx"), get("zd")]
        })
        .collect();

    let mut df = Df::from_string_rows(&["日期", "成交价", "成交量", "最新", "涨跌"], &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["成交价", "成交量", "最新", "涨跌"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gz_strips_percent_before_numeric() {
        // 涨跌幅 "0.47%" → 剥离后数值化
        let rows = vec![vec![
            Some("20260811".into()),
            Some("GDEA".into()),
            Some("38.15".into()),
            Some("38.33".into()),
            Some("38.5".into()),
            Some("37.95".into()),
            Some("0.18".into()),
            Some("0.47%".into()),
            Some("68620".into()),
            Some("2629623.29".into()),
        ]];
        let mut df = Df::from_string_rows(
            &[
                "日期",
                "品种",
                "开盘价",
                "收盘价",
                "最高价",
                "最低价",
                "涨跌",
                "涨跌幅",
                "成交数量",
                "成交金额",
            ],
            &rows,
        )
        .unwrap();
        df.cast_date(&["日期"]).unwrap();
        df.strip_suffix(&["涨跌幅"], "%").unwrap();
        df.cast_numeric(&["涨跌幅"]).unwrap();
        assert_eq!(df.column_names()[7], "涨跌幅");
    }
}
