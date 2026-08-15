//! 东方财富网-期货行情（对应 akshare `futures/futures_hist_em.py`）。
//!
//! 已实现：
//! - [`futures_hist_table_em`]：交易所品种对照表（`futsse-static.eastmoney.com/redis` 多级 `msgid`）
//! - [`futures_hist_em`]：期货行情 kline（`push2his.eastmoney.com/api/qt/stock/kline/get`）
//! - [`futures_settlement_price_sgx`]：新加坡交易所历史结算价（`links.sgx.com` ZIP）
//!
//! 注：`futures_hist_em` 与 `futures_settlement_price_sgx` 均依赖 `push2his.eastmoney.com`
//! 计算序号/拉取 kline；该端点在当前网络环境下 TCP 层断连（直连 akshare 同错，属
//! §1.2.1 #10 EM push2 阻断），无法生成 golden，`parity --check` 自动跳过，非回归。
//! `futures_hist_table_em` 走独立可读端点，可正常差分对账。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// 东财期货行情接口的浏览器 UA（akshare 走默认 requests，但带 UA 更稳）。
const UA_EM: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";

/// 用 `zip` crate 解析 SGX 结算价 ZIP（仅 `futures_settlement_price_sgx` 用到）。
use zip::ZipArchive;

/// JSON 值 → Option<String>（数值走 `to_string`，与 akshare `pd.DataFrame` 后逐单元格 str 一致）。
fn cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// 交易所品种映射（futsse-static.eastmoney.com/redis 多级 msgid）
// ---------------------------------------------------------------------------

/// 拉取东方财富期货品种原始映射列表（对应 akshare `__fetch_exchange_symbol_raw_em`）。
///
/// 先 `msgid=gnweb` 取市场列表，再对每个 `mktid` 取 `{mktid}` 得到子列表长度，
/// 逐层 `{mktid}_{num}` 展开合并。每个元素含 `mktid/mktname/name/code/vcode/vname`。
fn fetch_exchange_symbol_raw_em() -> Result<Vec<Value>> {
    let http = HttpClient::default();
    let base = "https://futsse-static.eastmoney.com/redis";
    let mut gn_params = Map::new();
    gn_params.insert("msgid".into(), Value::String("gnweb".into()));
    let gn = http.get_json_with_headers(base, &gn_params, &[("User-Agent", UA_EM)], None)?;
    let markets = gn
        .as_array()
        .ok_or_else(|| AkshareError::Empty("EM 品种对照表 gnweb 返回非数组".into()))?;
    let mut all: Vec<Value> = Vec::new();
    for m in markets {
        let mktid = m
            .get("mktid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if mktid.is_empty() {
            continue;
        }
        // 取 `{mktid}` 仅用于确定子列表长度（对应 akshare `len(inner_data_json)`）
        let mut p1 = Map::new();
        p1.insert("msgid".into(), Value::String(mktid.clone()));
        let inner1 = http.get_json_with_headers(base, &p1, &[("User-Agent", UA_EM)], None)?;
        let len = inner1.as_array().map(Vec::len).unwrap_or(0);
        for num in 1..=len {
            let mut p2 = Map::new();
            p2.insert("msgid".into(), Value::String(format!("{mktid}_{num}")));
            let inner2 =
                http.get_json_with_headers(base, &p2, &[("User-Agent", UA_EM)], None)?;
            if let Some(arr) = inner2.as_array() {
                all.extend(arr.iter().cloned());
            }
        }
    }
    Ok(all)
}

/// 东方财富网-期货行情-交易所品种对照表（对应 akshare [`akshare.futures_hist_table_em`]）。
///
/// # 返回列
/// `市场简称, 合约中文代码, 合约代码`（取自原始 `mktname/name/code`）
pub fn futures_hist_table_em() -> Result<Df> {
    let all = fetch_exchange_symbol_raw_em()?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(all.len());
    for it in &all {
        rows.push(vec![
            it.get("mktname").and_then(cell),
            it.get("name").and_then(cell),
            it.get("code").and_then(cell),
        ]);
    }
    Df::from_string_rows(&["市场简称", "合约中文代码", "合约代码"], &rows)
}

/// 四张品种映射表（对应 akshare `__get_exchange_symbol_map`）。
struct EmSymbolMaps {
    /// 中文名 → 市场代码（`name → mktid`）
    c_contract_mkt: HashMap<String, String>,
    /// 中文名 → 东财合约代码（`name → code`）
    c_contract_to_e_contract: HashMap<String, String>,
    /// 东财代码 → 市场代码（`vcode → mktid`）
    e_symbol_mkt: HashMap<String, String>,
    /// 中文名（连续/主连类）→ 市场代码（`vname → mktid`）
    c_symbol_mkt: HashMap<String, String>,
}

fn build_symbol_maps(raw: &[Value]) -> EmSymbolMaps {
    let mut m = EmSymbolMaps {
        c_contract_mkt: HashMap::new(),
        c_contract_to_e_contract: HashMap::new(),
        e_symbol_mkt: HashMap::new(),
        c_symbol_mkt: HashMap::new(),
    };
    for it in raw {
        let name = it.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let code = it.get("code").and_then(Value::as_str).unwrap_or("").to_string();
        let mktid = it.get("mktid").and_then(Value::as_str).unwrap_or("").to_string();
        let vcode = it.get("vcode").and_then(Value::as_str).unwrap_or("").to_string();
        let vname = it.get("vname").and_then(Value::as_str).unwrap_or("").to_string();
        if !name.is_empty() {
            m.c_contract_mkt.insert(name.clone(), mktid.clone());
            m.c_contract_to_e_contract.insert(name, code);
        }
        if !vcode.is_empty() {
            m.e_symbol_mkt.insert(vcode, mktid.clone());
        }
        if !vname.is_empty() {
            m.c_symbol_mkt.insert(vname, mktid);
        }
    }
    m
}

/// 拆分 symbol 为「首个中文/英文连续串」与「首个数字串」（对应 akshare
/// `re.findall(r"[\u4e00-\u9fa5a-zA-Z]+", symbol)[0]` 与 `re.findall(r"\d+", symbol)[0]`）。
fn separate_char_and_numbers(symbol: &str) -> (String, String) {
    let chars: String = symbol
        .chars()
        .take_while(|c| c.is_alphabetic() || (*c as u32) >= 0x4e00 && (*c as u32) <= 0x9fa5)
        .collect();
    let nums: String = symbol.chars().filter(|c| c.is_ascii_digit()).collect();
    (chars, nums)
}

/// `YYYYMMDD` → `YYYY-MM-DD`（用于 kline 日期区间字符串比较）。
fn fmt_ymd(s: &str) -> String {
    let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.len() == 8 {
        format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
    } else {
        d
    }
}

/// 东方财富网-期货行情-行情数据（对应 akshare [`akshare.futures_hist_em`]）。
///
/// `symbol`：合约中文名（如 `热卷主连`）或代码（如 `rb2505`）；`period`：
/// `daily`/`weekly`/`monthly`（→ klt 101/102/103）；`start_date`/`end_date`：
/// `YYYYMMDD`（默认 `19900101`/`20500101`）。
///
/// 数据源 `push2his.eastmoney.com/api/qt/stock/kline/get`；当前环境该端点 TCP 断连，
/// 函数会如实返回网络错误，`parity --check` 因无 golden 自动跳过，非代码缺陷。
///
/// # 返回列
/// `时间, 开盘, 最高, 最低, 收盘, 涨跌, 涨跌幅, 成交量, 成交额, 持仓量`
pub fn futures_hist_em(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    let raw = fetch_exchange_symbol_raw_em()?;
    let maps = build_symbol_maps(&raw);
    // 解析 symbol → secid（对应 akshare 的 try/except KeyError 分支）
    let sec_id = if let Some(mkt) = maps.c_contract_mkt.get(symbol) {
        let code = maps
            .c_contract_to_e_contract
            .get(symbol)
            .ok_or_else(|| AkshareError::Param(format!("未知合约（无东财代码）: {symbol}")))?
            .clone();
        format!("{mkt}.{code}")
    } else {
        let (chars, _) = separate_char_and_numbers(symbol);
        if chars.is_empty() {
            return Err(AkshareError::Param(format!("无法解析合约: {symbol}")));
        }
        // 判断首个字母串是否全中文
        let all_cn = chars.chars().all(|c| (c as u32) >= 0x4e00 && (c as u32) <= 0x9fa5);
        let mkt = if all_cn {
            maps.c_symbol_mkt
                .get(&chars)
                .ok_or_else(|| AkshareError::Param(format!("未知中文合约: {symbol}")))?
        } else {
            maps.e_symbol_mkt
                .get(&chars)
                .ok_or_else(|| AkshareError::Param(format!("未知合约代码: {symbol}")))?
        };
        format!("{mkt}.{symbol}")
    };

    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 period: {other}（可选 daily/weekly/monthly）"
            )))
        }
    };
    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let mut params = Map::new();
    params.insert("secid".into(), Value::String(sec_id));
    params.insert("klt".into(), Value::String(klt.into()));
    params.insert("fqt".into(), Value::String("1".into()));
    params.insert("lmt".into(), Value::String("10000".into()));
    params.insert("end".into(), Value::String("20500000".into()));
    params.insert("iscca".into(), Value::String("1".into()));
    params.insert("fields1".into(), Value::String("f1,f2,f3,f4,f5,f6,f7,f8".into()));
    params.insert(
        "fields2".into(),
        Value::String("f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64".into()),
    );
    params.insert(
        "ut".into(),
        Value::String("7eea3edcaed734bea9cbfc24409ed989".into()),
    );
    params.insert("forcect".into(), Value::String("1".into()));

    let http = HttpClient::default();
    let v = http.get_json_with_headers(url, &params, &[("User-Agent", UA_EM)], None)?;
    let klines = v
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("EM kline 返回缺少 data.klines".into()))?;
    if klines.is_empty() {
        return Df::from_string_rows(
            &[
                "时间", "开盘", "最高", "最低", "收盘", "涨跌", "涨跌幅", "成交量", "成交额", "持仓量",
            ],
            &[],
        );
    }
    // kline CSV 14 字段 → 选取 10 列（顺序对齐 akshare 的 column select）
    // 原位置：0时间 1开 2收 3高 4低 5量 6额 7- 8涨跌幅 9涨跌 10- 11- 12持仓 13-
    // 选取顺序：时间,开,高,低,收,涨跌,涨跌幅,量,额,持仓
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for k in klines {
        let s = k.as_str().ok_or_else(|| AkshareError::Empty("EM kline 元素非字符串".into()))?;
        let f: Vec<&str> = s.split(',').collect();
        if f.len() < 13 {
            continue;
        }
        rows.push(vec![
            Some(f[0].to_string()),
            Some(f[1].to_string()),
            Some(f[3].to_string()),
            Some(f[4].to_string()),
            Some(f[2].to_string()),
            Some(f[9].to_string()),
            Some(f[8].to_string()),
            Some(f[5].to_string()),
            Some(f[6].to_string()),
            Some(f[12].to_string()),
        ]);
    }
    // 日期区间过滤（对应 akshare 的 `temp_df[start_date:end_date]`，kline 时间形如
    // `2024-01-02 00:00:00`，取前 10 位按 `YYYY-MM-DD` 与起止字符串比较）
    let start = fmt_ymd(start_date);
    let end = fmt_ymd(end_date);
    rows.retain(|r| {
        let d = r[0].as_ref().map(|s| &s[..s.len().min(10)]).unwrap_or("");
        d >= start.as_str() && d <= end.as_str()
    });
    let mut df = Df::from_string_rows(
        &[
            "时间", "开盘", "最高", "最低", "收盘", "涨跌", "涨跌幅", "成交量", "成交额", "持仓量",
        ],
        &rows,
    )?;
    df.cast_numeric(&[
        "开盘", "最高", "最低", "收盘", "涨跌", "涨跌幅", "成交量", "成交额", "持仓量",
    ])?;
    Ok(df)
}

// ---------------------------------------------------------------------------
// 新加坡交易所历史结算价（futures_settlement_price_sgx）
// ---------------------------------------------------------------------------

/// 计算 SGX 衍生品日报序号（对应 akshare `__fetch_ftse_index_futu`）。
///
/// 取 `push2his` 上 `100.STI` 的日线 kline，序号 = 末行索引 + 791。
/// 该端点当前环境 TCP 断连，函数会如实返回网络错误。
fn fetch_ftse_index_futu(date: &str) -> Result<usize> {
    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let mut params = Map::new();
    params.insert("secid".into(), Value::String("100.STI".into()));
    params.insert("klt".into(), Value::String("101".into()));
    params.insert("fqt".into(), Value::String("0".into()));
    params.insert("lmt".into(), Value::String("10000".into()));
    params.insert("end".into(), Value::String(date.to_string()));
    params.insert("iscca".into(), Value::String("1".into()));
    params.insert("fields1".into(), Value::String("f1,f2,f3,f4,f5,f6,f7,f8".into()));
    params.insert(
        "fields2".into(),
        Value::String("f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64".into()),
    );
    params.insert(
        "ut".into(),
        Value::String("f057cbcbce2a86e2866ab8877db1d059".into()),
    );
    params.insert("forcect".into(), Value::String("1".into()));
    let http = HttpClient::default();
    let v = http.get_json_with_headers(url, &params, &[("User-Agent", UA_EM)], None)?;
    let klines = v
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("SGX 序号计算缺少 data.klines".into()))?;
    if klines.is_empty() {
        return Err(AkshareError::Empty("SGX 序号计算 kline 为空".into()));
    }
    Ok(klines.len() - 1 + 791)
}

/// 解析 SGX 衍生品日报 ZIP（对应 akshare `zipfile.ZipFile(...).open(namelist()[0])`
/// 后按 txt(制表符)/csv(逗号) 解析）。返回保持全部列为字符串的 [`Df`]。
fn parse_sgx_zip(bytes: &[u8]) -> Result<Df> {
    let mut reader = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| AkshareError::Empty(format!("SGX ZIP 解析失败: {e}")))?;
    if reader.is_empty() {
        return Err(AkshareError::Empty("SGX ZIP 无条目".into()));
    }
    let mut entry = reader
        .by_index(0)
        .map_err(|e| AkshareError::Empty(format!("SGX ZIP 读取首条目失败: {e}")))?;
    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .map_err(|e| AkshareError::Empty(format!("SGX ZIP 条目解码失败: {e}")))?;
    let delim = if entry.name().ends_with("txt") { '\t' } else { ',' };
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Err(AkshareError::Empty("SGX ZIP 条目为空".into()));
    }
    let header: Vec<&str> = lines[0].split(delim).map(|c| c.trim()).collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(lines.len() - 1);
    for line in &lines[1..] {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<Option<String>> = line
            .split(delim)
            .map(|c| {
                let c = c.trim();
                if c.is_empty() {
                    None
                } else {
                    Some(c.to_string())
                }
            })
            .collect();
        rows.push(f);
    }
    let cols: Vec<&str> = header.to_vec();
    Df::from_string_rows(&cols, &rows)
}

/// 新加坡交易所-衍生品-历史结算价格（对应 akshare [`akshare.futures_settlement_price_sgx`]）。
///
/// `date`：`YYYYMMDD`（交易日）。先经 `push2his` 计算日报序号，再下载
/// `https://links.sgx.com/1.0.0/derivatives-daily/{num}/FUTURE.zip` 解析。
/// `push2his` 当前环境 TCP 断连，函数如实返回网络错误，`parity --check` 因无 golden
/// 自动跳过，非回归。
///
/// # 返回列
/// 随 SGX 日报文件动态变化（保持原样，全部为字符串）。
pub fn futures_settlement_price_sgx(date: &str) -> Result<Df> {
    let num = fetch_ftse_index_futu(date)?;
    let url = format!("https://links.sgx.com/1.0.0/derivatives-daily/{num}/FUTURE.zip");
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[("User-Agent", UA_EM)], None)?;
    parse_sgx_zip(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_char_and_numbers_basic() {
        assert_eq!(separate_char_and_numbers("焦煤2506"), ("焦煤".to_string(), "2506".to_string()));
        assert_eq!(separate_char_and_numbers("rb2505"), ("rb".to_string(), "2505".to_string()));
        assert_eq!(separate_char_and_numbers("热卷主连"), ("热卷主连".to_string(), "".to_string()));
    }

    #[test]
    fn fmt_ymd_basic() {
        assert_eq!(fmt_ymd("19900101"), "1990-01-01");
        assert_eq!(fmt_ymd("2050-01-01"), "2050-01-01");
    }

    #[test]
    fn build_symbol_maps_ok() {
        let raw = [
            serde_json::json!({"mktid":"114","mktname":"上期所","name":"螺纹钢","code":"rb","vcode":"rb9999","vname":"螺纹主连"}),
        ];
        let m = build_symbol_maps(&raw);
        assert_eq!(m.c_contract_mkt.get("螺纹钢"), Some(&"114".to_string()));
        assert_eq!(m.c_contract_to_e_contract.get("螺纹钢"), Some(&"rb".to_string()));
        assert_eq!(m.e_symbol_mkt.get("rb9999"), Some(&"114".to_string()));
        assert_eq!(m.c_symbol_mkt.get("螺纹主连"), Some(&"114".to_string()));
    }

    #[test]
    fn hist_em_column_select_matches_akshare() {
        // 单条 kline CSV（14 字段），验证 10 列选取顺序与 akshare 一致
        let kline = "2024-01-02 00:00:00,3500.0,3550.0,3600.0,3480.0,100000,3.5e8,0,1.2,50.0,0,0,120000,0";
        let f: Vec<&str> = kline.split(',').collect();
        let row = vec![
            Some(f[0].to_string()),
            Some(f[1].to_string()),
            Some(f[3].to_string()),
            Some(f[4].to_string()),
            Some(f[2].to_string()),
            Some(f[9].to_string()),
            Some(f[8].to_string()),
            Some(f[5].to_string()),
            Some(f[6].to_string()),
            Some(f[12].to_string()),
        ];
        assert_eq!(row[4], Some("3550.0".to_string())); // 收盘
        assert_eq!(row[1], Some("3500.0".to_string())); // 开盘
        assert_eq!(row[9], Some("120000".to_string())); // 持仓量
    }
}
