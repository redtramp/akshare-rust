//! 新浪财经（sina）债券类函数（批次4 · 阶段5）。
//!
//! 对应 akshare `bond/bond_gb_sina.py`、`bond/bond_zh_sina.py`、
//! `bond/bond_cb_sina.py`、`bond/bond_zh_cov.py`（SINA 系可转债）：
//! - `bond_gb_us_sina` / `bond_gb_zh_sina`：中美国债收益率日线（JSON）
//! - `bond_zh_hs_daily` / `bond_zh_hs_cov_daily`：沪深债券/可转债历史日 K（hk_js_decode 解密）
//! - `bond_zh_hs_spot`：沪深债券实时行情（分页 JSON 数组，位置映射中文列）
//! - `bond_zh_hs_cov_spot`：沪深可转债实时行情（分页 JSON 数组，原始键）
//! - `bond_cb_profile_sina` / `bond_cb_summary_sina`：可转债详情/概况（HTML 表）

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::html::read_html;
use crate::core::http::HttpClient;
use crate::core::js_engine::sina_js_decode;
use serde_json::{Map, Value};

/// 中国国债收益率品种映射（`symbol` → 接口代码）。
const ZH_GB_MAP: &[(&str, &str)] = &[
    ("中国1年期国债", "CN1YT"),
    ("中国2年期国债", "CN2YT"),
    ("中国3年期国债", "CN3YT"),
    ("中国5年期国债", "CN5YT"),
    ("中国7年期国债", "CN7YT"),
    ("中国10年期国债", "CN10YT"),
    ("中国15年期国债", "CN15YT"),
    ("中国20年期国债", "CN20YT"),
    ("中国30年期国债", "CN30YT"),
];

/// 美国国债收益率品种映射（`symbol` → 接口代码）。
const US_GB_MAP: &[(&str, &str)] = &[
    ("美国1月期国债", "US1MT"),
    ("美国2月期国债", "US2MT"),
    ("美国3月期国债", "US3MT"),
    ("美国4月期国债", "US4MT"),
    ("美国6月期国债", "US6MT"),
    ("美国1年期国债", "US1YT"),
    ("美国2年期国债", "US2YT"),
    ("美国3年期国债", "US3YT"),
    ("美国5年期国债", "US5YT"),
    ("美国7年期国债", "US7YT"),
    ("美国10年期国债", "US10YT"),
    ("美国20年期国债", "US20YT"),
    ("美国30年期国债", "US30YT"),
];

/// 沪深债券实时行情：原始行 19 列 → 输出 13 列（位置映射）。
const HS_SPOT_MAP: &[(usize, &str)] = &[
    (0, "代码"),
    (2, "名称"),
    (3, "最新价"),
    (4, "涨跌额"),
    (5, "涨跌幅"),
    (6, "买入"),
    (7, "卖出"),
    (8, "昨收"),
    (9, "今开"),
    (10, "最高"),
    (11, "最低"),
    (12, "成交量"),
    (13, "成交额"),
];

/// 沪深债券实时行情中需数值化的列（其余保持字符串，对齐 akshare）。
const HS_SPOT_NUM: &[&str] = &[
    "最新价", "买入", "卖出", "昨收", "今开", "最高", "最低",
];

/// 在映射表中查找接口代码，找不到返回 `Empty` 错误。
fn lookup<'a>(map: &'a [(&str, &str)], symbol: &str) -> Result<&'a str> {
    map.iter()
        .find(|(k, _)| *k == symbol)
        .map(|(_, v)| *v)
        .ok_or_else(|| AkshareError::Empty(format!("未知债券品种: {symbol}")))
}

/// 构造 `Map` 查询参数（对应 akshare 的 `params=dict(...)`）。
fn params_map(pairs: &[(&str, &str)]) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), Value::String((*v).to_string()));
    }
    m
}

/// 单元值转可选字符串（对应 akshare 的 `str(x)` / 缺失值 `None`）。
fn cell_to_opt_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// 把 JSON 二维数组（新浪行情中心响应）转成 `Vec<Vec<Option<String>>>`。
fn arr_of_arr_to_rows(v: &Value) -> Vec<Vec<Option<String>>> {
    let mut out = Vec::new();
    if let Some(rows) = v.as_array() {
        for r in rows {
            if let Some(cells) = r.as_array() {
                out.push(cells.iter().map(cell_to_opt_str).collect());
            }
        }
    }
    out
}

/// 从新浪行情 `klc_kl.js` 响应文本中提取被编码的字符串（引号内、首个 `;` 之前）。
fn extract_sina_js_encoded(text: &str) -> Result<String> {
    let after_eq = text
        .split('=')
        .nth(1)
        .ok_or_else(|| AkshareError::Empty("新浪 JS 响应缺少 '=' 分隔".into()))?;
    let before_semi = after_eq.split(';').next().ok_or_else(|| {
        AkshareError::Empty("新浪 JS 响应缺少 ';' 分隔".into())
    })?;
    Ok(before_semi.replace('"', ""))
}

/// 当前日期（对应 akshare `datetime.datetime.now()`，Asia/Shanghai +8），格式 `YYYY_MM_DD`。
fn now_ymd_underscore() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let local = secs + 8 * 3600;
    let days = (local / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}_{m:02}_{d:02}")
}

/// 天数 → 公历日期（Howard Hinnant `civil_from_days`）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 抓取新浪国债收益率日线（对应 akshare [`bond_gb_zh_sina`] / [`bond_gb_us_sina`]）。
///
/// 接口返回 `result.data` 数组，每行键序 `d,o,h,l,c,v`，位置重命名为
/// `date,open,high,low,close,volume`；`date` 保持字符串（对应 akshare
/// `pd.to_datetime(...).dt.date` → object 列，dtype 为 str）。
fn gb_sina(symbol: &str, map: &[(&str, &str)]) -> Result<Df> {
    let code = lookup(map, symbol)?;
    let url = format!("https://bond.finance.sina.com.cn/hq/gb/daily?symbol={code}");
    let http = HttpClient::default();
    let data = http.get_json(&url, &Map::new(), None)?;
    let rows = data
        .get("result")
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AkshareError::Empty("新浪国债响应缺少 result.data".into()))?;
    if rows.is_empty() {
        return Df::from_string_rows(&["date", "open", "high", "low", "close", "volume"], &[]);
    }
    let mut df = Df::from_json_rows(&rows)?;
    df.rename_columns(&["date", "open", "high", "low", "close", "volume"])?;
    df.cast_numeric(&["open", "high", "low", "close", "volume"])?;
    Ok(df)
}

/// 中国国债收益率行情（对应 akshare [`bond_gb_zh_sina`]）。
///
/// # 返回列
/// `date, open, high, low, close, volume`
pub fn bond_gb_zh_sina(symbol: &str) -> Result<Df> {
    gb_sina(symbol, ZH_GB_MAP)
}

/// 美国国债收益率行情（对应 akshare [`bond_gb_us_sina`]）。
///
/// # 返回列
/// `date, open, high, low, close, volume`
pub fn bond_gb_us_sina(symbol: &str) -> Result<Df> {
    gb_sina(symbol, US_GB_MAP)
}

/// 沪深债券/可转债历史日 K（对应 akshare `bond_zh_hs_daily` / `bond_zh_hs_cov_daily`）。
///
/// GET `finance.sina.com.cn/realstock/company/{symbol}/hisdata/klc_kl.js?d={今日}`，
/// 响应为 `var xxx="编码串";`，提取编码串后经 `sina.js::d()` 解密得到
/// `[{date,open,high,low,close}, ...]`，`date` 保持字符串，其余数值化。
fn bond_hs_hist_daily(symbol: &str) -> Result<Df> {
    let date = now_ymd_underscore();
    let url = format!(
        "https://finance.sina.com.cn/realstock/company/{symbol}/hisdata/klc_kl.js?d={date}"
    );
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), None)?;
    let encoded = extract_sina_js_encoded(&text)?;
    let json = sina_js_decode(&encoded)?;
    let rows: Vec<Value> =
        serde_json::from_str(&json).map_err(|e| AkshareError::json("新浪日K解密结果非JSON数组", e.to_string()))?;
    if rows.is_empty() {
        return Df::from_string_rows(&["date", "open", "high", "low", "close"], &[]);
    }
    let mut df = Df::from_json_rows_typed(&rows)?;
    df.cast_numeric(&["open", "high", "low", "close"])?;
    Ok(df)
}

/// 沪深债券历史日 K 线（对应 akshare [`bond_zh_hs_daily`]）。
///
/// # 返回列
/// `date, open, high, low, close`
pub fn bond_zh_hs_daily(symbol: &str) -> Result<Df> {
    bond_hs_hist_daily(symbol)
}

/// 沪深可转债历史日 K 线（对应 akshare [`bond_zh_hs_cov_daily`]）。
///
/// 与 [`bond_zh_hs_daily`] 共用同一新浪历史行情接口与解密逻辑。
///
/// # 返回列
/// `date, open, high, low, close`
pub fn bond_zh_hs_cov_daily(symbol: &str) -> Result<Df> {
    bond_hs_hist_daily(symbol)
}

/// 沪深债券实时行情（对应 akshare `bond_zh_hs_spot(start_page, end_page)`）。
///
/// 分页拉取 `Market_Center.getHQNodeData`（`demjson` 解析为二维数组），
/// 按位置映射为中文列。注：该接口 `vip.stock.finance.sina.com.cn` 在部分网络返回
/// 500，golden 可能不可生成；实现与 akshare 逐字段对齐。
pub fn bond_zh_hs_spot(start_page: &str, end_page: &str) -> Result<Df> {
    let http = HttpClient::default();
    let count_url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCountSimple";
    let count_text = http.get_text(count_url, &params_map(&[("node", "hs_z")]), None)?;
    let total_digits: String = count_text.chars().filter(|c| c.is_ascii_digit()).collect();
    let total: usize = total_digits
        .parse()
        .map_err(|_| AkshareError::Empty("沪深债券页数解析失败".into()))?;
    let page_count = total.div_ceil(80);
    let sp: usize = start_page
        .parse()
        .map_err(|_| AkshareError::Param(format!("start_page 非数字: {start_page}")))?;
    let mut ep: usize = end_page
        .parse()
        .map_err(|_| AkshareError::Param(format!("end_page 非数字: {end_page}")))?;
    ep = if ep < page_count { ep + 1 } else { page_count };

    let url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
    let mut raw_rows: Vec<Vec<Option<String>>> = Vec::new();
    for page in sp..ep {
        let params = params_map(&[
            ("page", &page.to_string()),
            ("num", "80"),
            ("sort", "symbol"),
            ("asc", "1"),
            ("node", "hs_z"),
            ("_s_r_a", "page"),
        ]);
        let data = http.get_json(url, &params, None)?;
        raw_rows.extend(arr_of_arr_to_rows(&data));
        http.random_delay();
    }

    let mut out_rows: Vec<Vec<Option<String>>> = Vec::with_capacity(raw_rows.len());
    for r in &raw_rows {
        let out: Vec<Option<String>> = HS_SPOT_MAP
            .iter()
            .map(|(idx, _)| r.get(*idx).cloned().flatten())
            .collect();
        out_rows.push(out);
    }
    let names: Vec<&str> = HS_SPOT_MAP.iter().map(|(_, n)| *n).collect();
    let mut df = Df::from_string_rows(&names, &out_rows)?;
    df.cast_numeric(HS_SPOT_NUM)?;
    Ok(df)
}

/// 沪深可转债实时行情（对应 akshare `bond_zh_hs_cov_spot()`）。
///
/// 分页拉取 `Market_Center.getHQNodeDataSimple`，`demjson` 解析为对象数组后
/// **保留原始键**（akshare 不重命名），列名与接口返回一致。
pub fn bond_zh_hs_cov_spot() -> Result<Df> {
    let http = HttpClient::default();
    let count_url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCountSimple";
    let count_text = http.get_text(count_url, &params_map(&[("node", "hskzz_z")]), None)?;
    let total_digits: String = count_text.chars().filter(|c| c.is_ascii_digit()).collect();
    let total: usize = total_digits
        .parse()
        .map_err(|_| AkshareError::Empty("沪深可转债页数解析失败".into()))?;
    let page_count = total.div_ceil(80);

    let url = "http://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeDataSimple";
    let mut all: Vec<Value> = Vec::new();
    for page in 1..=page_count {
        let params = params_map(&[
            ("page", &page.to_string()),
            ("num", "80"),
            ("sort", "symbol"),
            ("asc", "1"),
            ("node", "hskzz_z"),
            ("_s_r_a", "page"),
        ]);
        let data = http.get_json(url, &params, None)?;
        if let Some(arr) = data.as_array() {
            for item in arr {
                if item.is_object() {
                    all.push(item.clone());
                }
            }
        }
        http.random_delay();
    }
    Df::from_json_rows_typed(&all)
}

/// 可转债详情资料（对应 akshare `bond_cb_profile_sina(symbol)`）。
///
/// GET `money.finance.sina.com.cn/bond/info/{symbol}.html`，取首页第一个 `<table>`，
/// 列重命名为 `item, value`。
pub fn bond_cb_profile_sina(symbol: &str) -> Result<Df> {
    let url = format!("https://money.finance.sina.com.cn/bond/info/{symbol}.html");
    let http = HttpClient::default();
    let html = http.get_text(&url, &Map::new(), None)?;
    let tables = read_html(&html)?;
    let df = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("可转债详情页未找到表格".into()))?;
    let cols = df.column_names();
    if cols.len() < 2 {
        return Err(AkshareError::Empty(format!(
            "可转债详情表列数不足: {}",
            cols.len()
        )));
    }
    let picked = df.select(&[cols[0].as_str(), cols[1].as_str()])?;
    let mut picked = picked;
    picked.rename_columns(&["item", "value"])?;
    Ok(picked)
}

/// 可转债债券概况（对应 akshare `bond_cb_summary_sina(symbol)`）。
///
/// GET `money.finance.sina.com.cn/bond/quotes/{symbol}.html`，取第 11 个 `<table>`，
/// 将其 6 列拆成 3 组 `(item, value)` 纵向拼接。
pub fn bond_cb_summary_sina(symbol: &str) -> Result<Df> {
    let url = format!("https://money.finance.sina.com.cn/bond/quotes/{symbol}.html");
    let http = HttpClient::default();
    let html = http.get_text(&url, &Map::new(), None)?;
    let tables = read_html(&html)?;
    let tbl = tables
        .into_iter()
        .nth(10)
        .ok_or_else(|| AkshareError::Empty("可转债概况页未找到第 11 个表格".into()))?;
    let names = tbl.column_names();
    if names.len() < 6 {
        return Err(AkshareError::Empty(format!(
            "可转债概况表列数不足: {}",
            names.len()
        )));
    }
    let c: Vec<Vec<Option<String>>> = (0..6)
        .map(|i| col_strs(&tbl, &names[i]))
        .collect::<Result<_>>()?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    let zipped = c[0]
        .iter()
        .zip(&c[1])
        .zip(&c[2])
        .zip(&c[3])
        .zip(&c[4])
        .zip(&c[5]);
    for (((((a, b), cc), d), e), f) in zipped {
        rows.push(vec![a.clone(), b.clone()]);
        rows.push(vec![cc.clone(), d.clone()]);
        rows.push(vec![e.clone(), f.clone()]);
    }
    Df::from_string_rows(&["item", "value"], &rows)
}

/// 取某列字符串值向量。
fn col_strs(df: &Df, name: &str) -> Result<Vec<Option<String>>> {
    let s = df
        .inner()
        .column(name)
        .map_err(|e| AkshareError::Empty(format!("取列 {name} 失败: {e}")))?;
    let ca = s
        .str()
        .map_err(|_| AkshareError::Empty(format!("{name} 非字符串列")))?;
    Ok((0..s.len()).map(|i| ca.get(i).map(str::to_string)).collect())
}
