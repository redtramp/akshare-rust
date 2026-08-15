//! 新浪期货集群（批次 29 子组 B）。
//!
//! 对应 akshare：
//! - `futures/futures_zh_sina.py`：`futures_symbol_mark` / `futures_zh_realtime` /
//!   `futures_zh_spot` / `futures_zh_daily_sina` / `futures_zh_minute_sina`
//! - `futures/futures_hq_sina.py`：`futures_hq_subscribe_exchange_symbol` /
//!   `futures_foreign_commodity_realtime` / `futures_foreign_commodity_subscribe_exchange_symbol`
//! - `futures/futures_foreign.py`：`futures_foreign_detail` / `futures_foreign_hist`
//!
//! 实时类接口（`*_spot` / `*_realtime`）数值随时间变化，parity 用例使用 `loose`
//! 仅校验列契约 + dtype；历史/分钟/日线类数据稳定，同样以 `loose` 放行浮点末位。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::core::html::read_html_tables;
use crate::core::js_engine::js_literal_to_json;
use crate::sources::eastmoney::finalize_report;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// 去首尾空白与引号（新浪行情字段常带 `"` 包裹）。
fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

/// 新浪 `hq.sinajs.cn` 行情行：`var nf_X="a,b,c";` → 逗号分割的字段列表（已去首尾引号）。
fn parse_sina_quote_lines(text: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for item in text.split(';') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let after_eq = match item.split_once('=') {
            Some((_, v)) => v,
            None => continue,
        };
        let fields: Vec<String> = after_eq.split(',').map(unquote).collect();
        if !fields.is_empty() {
            out.push(fields);
        }
    }
    out
}

/// 新浪 JSONP 响应剥离外壳：返回首个 `[` 到最后一个 `]` 之间的 JSON 数组文本。
///
/// 新浪 jsonp 端点包裹形如 `var _X=([...]);` 或 `=([...]);`（`(` 与 `)` 为回调外壳），
/// 故以数组首尾括号截取，而非 `];`（中间夹着 `)`）。
fn strip_jsonp(body: &str) -> Result<String> {
    let start = body
        .find('[')
        .ok_or_else(|| AkshareError::Empty("JSONP 响应未找到 '['".into()))?;
    let end = body
        .rfind(']')
        .ok_or_else(|| AkshareError::Empty("JSONP 响应未找到 ']'".into()))?;
    Ok(body[start..=end].to_string())
}

/// 从 JS 文本中提取从 `open_idx`（须指向某个 `{`）开始、括号配平的片段。
///
/// 用于从 `ARRFUTURESNODES = { ... };` 这类「对象字面量后接 JS 函数」的脚本中取纯对象：
/// 简单的 `find('{')`/`rfind('}')` 会一路截到文件末尾的函数体，必须按括号深度配平。
/// 字符串内的括号不参与计数（避免误判）。
fn extract_balanced_braces(text: &str, open_idx: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut quote: u8 = 0;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(open_idx) {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == quote {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' | b'\'' => {
                in_str = true;
                quote = b;
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open_idx..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// 读取字符串列（缺失/类型不符返回空 Vec）。
fn col_str(df: &Df, name: &str) -> Vec<Option<String>> {
    match df.inner().column(name) {
        Ok(s) => match s.str() {
            Ok(ca) => ca.iter().map(|v| v.map(str::to_string)).collect(),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

/// 读取数值列（含整数列）为 `Option<f64>`。
fn col_f64(df: &Df, name: &str) -> Vec<Option<f64>> {
    match df.inner().column(name) {
        Ok(s) => match s.f64() {
            Ok(ca) => ca.iter().collect(),
            Err(_) => match s.i64() {
                Ok(ca) => ca.iter().map(|v| v.map(|x| x as f64)).collect(),
                Err(_) => Vec::new(),
            },
        },
        Err(_) => Vec::new(),
    }
}

/// 外盘期货品种中文名 → 代码（对应 akshare `futures_hq_subscribe_exchange_symbol` 硬编码字典）。
const HF_SYMBOL_CODE: &[(&str, &str)] = &[
    ("新加坡铁矿石", "FEF"),
    ("马棕油", "FCPO"),
    ("日橡胶", "RSS3"),
    ("美国原糖", "RS"),
    ("CME比特币期货", "BTC"),
    ("NYBOT-棉花", "CT"),
    ("LME镍3个月", "NID"),
    ("LME铅3个月", "PBD"),
    ("LME锡3个月", "SND"),
    ("LME锌3个月", "ZSD"),
    ("LME铝3个月", "AHD"),
    ("LME铜3个月", "CAD"),
    ("CBOT-黄豆", "S"),
    ("CBOT-小麦", "W"),
    ("CBOT-玉米", "C"),
    ("CBOT-黄豆油", "BO"),
    ("CBOT-黄豆粉", "SM"),
    ("日本橡胶", "TRB"),
    ("COMEX铜", "HG"),
    ("NYMEX天然气", "NG"),
    ("NYMEX原油", "CL"),
    ("COMEX白银", "SI"),
    ("COMEX黄金", "GC"),
    ("CME-瘦肉猪", "LHC"),
    ("布伦特原油", "OIL"),
    ("伦敦金", "XAU"),
    ("伦敦银", "XAG"),
    ("伦敦铂金", "XPT"),
    ("伦敦钯金", "XPD"),
    ("欧洲碳排放", "EUA"),
];

// ---------------------------------------------------------------------------
// 国内期货（futures_zh_sina.py）
// ---------------------------------------------------------------------------

/// 新浪日线接口短键 → 标准列名（对应 akshare 重命名：`d`→`date` 等）。
const DAILY_KEY_MAP: &[(&str, &str)] = &[
    ("d", "date"),
    ("o", "open"),
    ("h", "high"),
    ("l", "low"),
    ("c", "close"),
    ("v", "volume"),
    ("p", "hold"),
    ("s", "settle"),
];
const DAILY_SELECT: &[&str] = &[
    "date", "open", "high", "low", "close", "volume", "hold", "settle",
];
const DAILY_NUMERIC: &[&str] = &[
    "open", "high", "low", "close", "volume", "hold", "settle",
];

/// 新浪分钟线接口短键 → 标准列名。
const MINUTE_KEY_MAP: &[(&str, &str)] = &[
    ("d", "datetime"),
    ("o", "open"),
    ("h", "high"),
    ("l", "low"),
    ("c", "close"),
    ("v", "volume"),
    ("p", "hold"),
];
const MINUTE_SELECT: &[&str] = &[
    "datetime", "open", "high", "low", "close", "volume", "hold",
];
const MINUTE_NUMERIC: &[&str] = &[
    "open", "high", "low", "close", "volume", "hold",
];

/// 期货品种与代码映射（对应 akshare [`akshare.futures_symbol_mark`]）。
///
/// 数据源 `qihuohangqing.js`（gb2312 → GBK 解码），`demjson` 宽松解析的对象字面量
/// 以 QuickJS `JSON.stringify` 等价还原；五大交易所各自首项为市场名，其后为
/// `[品种代码, 市场代码]` 对。
pub fn futures_symbol_mark() -> Result<Df> {
    let url = "https://vip.stock.finance.sina.com.cn/quotes_service/view/js/qihuohangqing.js";
    let http = HttpClient::default();
    let text = http.get_text(url, &Map::new(), None)?;
    // 形如 `ARRFUTURESNODES = { ... };` 后接 JS 函数，须按括号深度截取纯对象字面量，
    // 不能 rfind('}')（会截到文件末尾的函数体）。
    let start = text
        .find("ARRFUTURESNODES")
        .and_then(|i| text[i..].find('{').map(|j| i + j))
        .ok_or_else(|| AkshareError::Empty("qihuohangqing 未找到数据对象".into()))?;
    let obj_text = extract_balanced_braces(&text, start)
        .ok_or_else(|| AkshareError::Empty("qihuohangqing 括号不匹配".into()))?;
    let json = js_literal_to_json(obj_text)?;
    let obj: Map<String, Value> = serde_json::from_str(&json)
        .map_err(|e| AkshareError::json("qihuohangqing JSON 解析失败", e.to_string()))?;
    let exchanges = ["czce", "dce", "shfe", "cffex", "gfex"];
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for ex in exchanges {
        let arr = match obj.get(ex).and_then(Value::as_array) {
            Some(a) => a,
            None => continue,
        };
        let market = arr
            .first()
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_str)
            .unwrap_or(ex)
            .to_string();
        for item in arr.iter().skip(1) {
            let pair = match item.as_array() {
                Some(p) => p,
                None => continue,
            };
            let symbol = pair.first().and_then(Value::as_str).unwrap_or("").to_string();
            let mark = pair.get(1).and_then(Value::as_str).unwrap_or("").to_string();
            rows.push(vec![Some(market.clone()), Some(symbol), Some(mark)]);
        }
    }
    Df::from_string_rows(&["exchange", "symbol", "mark"], &rows)
}

/// 期货品种当前时刻所有可交易合约实时数据（对应 akshare [`akshare.futures_zh_realtime`]）。
///
/// `symbol` 为品种中文名（如 `工业硅`），经 `futures_symbol_mark` 映射为市场代码后请求
/// `Market_Center.getHQFuturesData`；列序取接口返回对象键序，18 个数值列转 float64。
pub fn futures_zh_realtime(symbol: &str) -> Result<Df> {
    let mark_df = futures_symbol_mark()?;
    let sym_col = col_str(&mark_df, "symbol");
    let mark_col = col_str(&mark_df, "mark");
    let mut map: HashMap<String, String> = HashMap::new();
    for (s, m) in sym_col.into_iter().zip(mark_col) {
        if let (Some(s), Some(m)) = (s, m) {
            map.insert(s, m);
        }
    }
    let node = map.get(symbol).ok_or_else(|| {
        AkshareError::Param(format!("未知品种: {symbol}（可用 futures_symbol_mark() 查询）"))
    })?;
    let url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQFuturesData";
    let mut params = Map::new();
    params.insert("page".into(), Value::String("1".into()));
    params.insert("sort".into(), Value::String("position".into()));
    params.insert("asc".into(), Value::String("0".into()));
    params.insert("node".into(), Value::String(node.clone()));
    params.insert("base".into(), Value::String("futures".into()));
    let http = HttpClient::default();
    let v = http.get_json(url, &params, None)?;
    let arr = v.as_array().cloned().unwrap_or_default();
    let keys: Vec<String> = arr
        .first()
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let rename: Vec<(&str, &str)> = keys.iter().map(|k| (k.as_str(), k.as_str())).collect();
    let select: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
    let numeric = [
        "trade",
        "settlement",
        "presettlement",
        "open",
        "high",
        "low",
        "close",
        "bidprice1",
        "askprice1",
        "bidvol1",
        "askvol1",
        "volume",
        "position",
        "preclose",
        "changepercent",
        "bid",
        "ask",
        "prevsettlement",
    ];
    finalize_report(&arr, &rename, &select, &numeric, None)
}

/// 期货实时行情（对应 akshare [`akshare.futures_zh_spot`]）。
///
/// `symbol` 逗号分隔合约名（如 `V2309`），`market="CF"` 商品期货，`adjust="0"` 默认路径。
/// 默认返回 15 列：品种代码经新浪 `hq.sinajs.cn`（`nf_` 前缀 + Referer）行情行解析，
/// 数值列转 float64；`current_price` 为空行丢弃。
///
/// 注：`adjust="1"` 需在 15 列基础上追加 `exchange/contract/contract_min_change`
/// （依赖 `futures_contract_detail` 跨模块查询），本实现聚焦默认路径，非默认参数仍返回
/// 15 列默认布局（实时行情主契约一致）。
pub fn futures_zh_spot(symbol: &str, market: &str, adjust: &str) -> Result<Df> {
    let subscribe = symbol
        .split(',')
        .map(|s| format!("nf_{}", s.trim()))
        .collect::<Vec<_>>()
        .join(",");
    let url = format!("https://hq.sinajs.cn/list={subscribe}");
    let http = HttpClient::default();
    let text = http.get_text_with_headers(
        &url,
        &Map::new(),
        &[("Referer", "https://vip.stock.finance.sina.com.cn/")],
        None,
    )?;
    let lines = parse_sina_quote_lines(&text);
    let cols = [
        "symbol",
        "time",
        "open",
        "high",
        "low",
        "current_price",
        "bid_price",
        "ask_price",
        "buy_vol",
        "sell_vol",
        "hold",
        "volume",
        "avg_price",
        "last_close",
        "last_settle_price",
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for f in &lines {
        if f.len() < 15 {
            continue;
        }
        let get = |i: usize| Some(f.get(i).cloned().unwrap_or_default());
        let current_price = f.get(8).cloned().unwrap_or_default();
        // 丢弃 current_price 为空（无法数值化）的行，与 akshare dropna 对齐
        if current_price.is_empty() {
            continue;
        }
        rows.push(vec![
            get(0),
            get(1),
            get(2),
            get(3),
            get(4),
            Some(current_price),
            get(6),
            get(7),
            get(11),
            get(12),
            get(13),
            get(14),
            get(9),
            get(5),
            get(10),
        ]);
    }
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&[
        "open",
        "high",
        "low",
        "current_price",
        "bid_price",
        "ask_price",
        "buy_vol",
        "sell_vol",
        "hold",
        "volume",
        "avg_price",
        "last_close",
        "last_settle_price",
    ])?;
    let _ = market;
    let _ = adjust;
    Ok(df)
}

/// 中国各品种期货日频率数据（对应 akshare [`akshare.futures_zh_daily_sina`]）。
///
/// 数据源 `InnerFuturesNewService.getDailyKLine`（JSONP，日期参数固定 `20210412` 与
/// akshare 同）；剥离外壳后按对象键序建表，`date` 保留字符串，其余数值列转 float64。
pub fn futures_zh_daily_sina(symbol: &str) -> Result<Df> {
    let date = "20210412";
    let url = "https://stock2.finance.sina.com.cn/futures/api/jsonp.php/var%20_V21052021_4_12=/InnerFuturesNewService.getDailyKLine";
    let mut params = Map::new();
    params.insert("symbol".into(), Value::String(symbol.into()));
    params.insert(
        "type".into(),
        Value::String(format!("{}_{}_{}", &date[0..4], &date[4..6], &date[6..8])),
    );
    let http = HttpClient::default();
    let text = http.get_text(url, &params, None)?;
    let arr_text = strip_jsonp(&text)?;
    let arr: Vec<Value> = serde_json::from_str(&arr_text)
        .map_err(|e| AkshareError::json("daily kline JSON 解析失败", e.to_string()))?;
    finalize_report(&arr, DAILY_KEY_MAP, DAILY_SELECT, DAILY_NUMERIC, None)
}

/// 中国各品种期货分钟频率数据（对应 akshare [`akshare.futures_zh_minute_sina`]）。
///
/// `period`：`1`/`5`/`15`/`30`/`60`。数据源 `InnerFuturesNewService.getFewMinLine`
/// （JSONP），`datetime` 保留字符串，其余数值列转 float64。
pub fn futures_zh_minute_sina(symbol: &str, period: &str) -> Result<Df> {
    let url = "https://stock2.finance.sina.com.cn/futures/api/jsonp.php/=/InnerFuturesNewService.getFewMinLine";
    let mut params = Map::new();
    params.insert("symbol".into(), Value::String(symbol.into()));
    params.insert("type".into(), Value::String(period.into()));
    let http = HttpClient::default();
    let text = http.get_text(url, &params, None)?;
    let arr_text = strip_jsonp(&text)?;
    let arr: Vec<Value> = serde_json::from_str(&arr_text)
        .map_err(|e| AkshareError::json("minute kline JSON 解析失败", e.to_string()))?;
    finalize_report(&arr, MINUTE_KEY_MAP, MINUTE_SELECT, MINUTE_NUMERIC, None)
}

// ---------------------------------------------------------------------------
// 外盘期货（futures_hq_sina.py / futures_foreign.py）
// ---------------------------------------------------------------------------

/// 外盘期货品种对应表（对应 akshare [`akshare.futures_hq_subscribe_exchange_symbol`]）。
///
/// 硬编码字典（中文名 → 代码），无网络请求；返回 `symbol`(中文名)/`code` 两列。
pub fn futures_hq_subscribe_exchange_symbol() -> Result<Df> {
    let rows: Vec<Vec<Option<String>>> = HF_SYMBOL_CODE
        .iter()
        .map(|(name, code)| vec![Some((*name).to_string()), Some((*code).to_string())])
        .collect();
    Df::from_string_rows(&["symbol", "code"], &rows)
}

/// 外盘期货可订阅行情代码（对应 akshare [`akshare.futures_foreign_commodity_subscribe_exchange_symbol`]）。
///
/// akshare 原函数返回 `list`（非 DataFrame），这里包装为单列 `code` 的 `Df` 以兼容
/// 差分测试基础设施；数据源 `hf.html` 的 `oHF_1` 对象键即为代码。
pub fn futures_foreign_commodity_subscribe_exchange_symbol() -> Result<Df> {
    let url = "https://finance.sina.com.cn/money/future/hf.html";
    let http = HttpClient::default();
    let text = http.get_text(url, &Map::new(), None)?;
    let s = text
        .find("var oHF_1 = ")
        .map(|i| i + 12)
        .ok_or_else(|| AkshareError::Empty("hf.html 未找到 'var oHF_1 = '".into()))?;
    let e = text
        .find("var oHF_2 = ")
        .ok_or_else(|| AkshareError::Empty("hf.html 未找到 'var oHF_2 = '".into()))?;
    let raw = &text[s..e.saturating_sub(2)];
    let json = js_literal_to_json(raw)?;
    let obj: Map<String, Value> = serde_json::from_str(&json)
        .map_err(|e| AkshareError::json("oHF_1 JSON 解析失败", e.to_string()))?;
    let rows: Vec<Vec<Option<String>>> =
        obj.keys().map(|k| vec![Some(k.clone())]).collect();
    Df::from_string_rows(&["code"], &rows)
}

/// 新浪-外盘期货-行情数据（对应 akshare [`akshare.futures_foreign_commodity_realtime`]）。
///
/// `symbol` 逗号分隔代码（如 `CT,NID`）；行情行经 `hq.sinajs.cn`（`hf_` 前缀 + Referer）
/// 解析，中文名由 `HF_SYMBOL_CODE` 反向映射；`人民币报价` 由 `最新价 × 品种乘数 × 美元人民币`
/// 计算（乘数取自 `hf.html` 的 `oHF_1`，汇率取自 `hq.sinajs.cn/?list=USDCNY`）。
pub fn futures_foreign_commodity_realtime(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let codes: Vec<String> = symbol.split(',').map(|s| s.trim().to_string()).collect();
    let subscribe = codes
        .iter()
        .map(|c| format!("hf_{c}"))
        .collect::<Vec<_>>()
        .join(",");

    // 1) 品种乘数：hf.html 的 oHF_1，按中文名索引（value[1] 首字符为乘数）
    let hf_text = http.get_text(
        "https://finance.sina.com.cn/money/future/hf.html",
        &Map::new(),
        None,
    )?;
    let raw = {
        let s = hf_text.find("oHF_1 = ").map(|i| i + 8).unwrap_or(0);
        let e = hf_text.find("oHF_2").unwrap_or(hf_text.len());
        &hf_text[s..e]
    };
    let obj_json = {
        let s = raw.find('{').unwrap_or(0);
        let e = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
        &raw[s..e]
    };
    let obj: Map<String, Value> = serde_json::from_str(&js_literal_to_json(obj_json)?)
        .map_err(|e| AkshareError::json("oHF_1 JSON 解析失败", e.to_string()))?;
    let mut name_mul: HashMap<String, f64> = HashMap::new();
    for val in obj.values() {
        let arr = match val.as_array() {
            Some(a) => a,
            None => continue,
        };
        let nm = arr.first().and_then(Value::as_str).unwrap_or("").to_string();
        if arr.len() >= 2 {
            if let Some(s) = arr[1].as_str() {
                if let Some(ch) = s.chars().next() {
                    if let Ok(m) = ch.to_string().parse::<f64>() {
                        name_mul.insert(nm, m);
                    }
                }
            }
        }
    }

    // 2) 美元人民币中间价
    let us_text = http.get_text_with_headers(
        "https://hq.sinajs.cn/?list=USDCNY",
        &Map::new(),
        &[("Referer", "https://finance.sina.com.cn/")],
        None,
    )?;
    let usd_rmb = {
        let s = us_text.find('"').unwrap_or(0) + 1;
        let e = us_text.find(",美元人民币").unwrap_or(us_text.len());
        let seg = &us_text[s..e];
        seg.split(',')
            .next_back()
            .unwrap_or("")
            .trim()
            .parse::<f64>()
            .unwrap_or(1.0)
    };

    // 3) 行情行
    let url = format!("https://hq.sinajs.cn/?list={subscribe}");
    let text = http.get_text_with_headers(
        &url,
        &Map::new(),
        &[("Referer", "https://finance.sina.com.cn/")],
        None,
    )?;
    let lines = parse_sina_quote_lines(&text);
    let name_of: HashMap<&str, &str> = HF_SYMBOL_CODE.iter().map(|(n, c)| (*c, *n)).collect();
    let cols = [
        "名称",
        "最新价",
        "人民币报价",
        "买价",
        "卖价",
        "最高价",
        "最低价",
        "行情时间",
        "昨日结算价",
        "开盘价",
        "持仓量",
        "日期",
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for f in &lines {
        if f.len() < 13 {
            continue;
        }
        let get = |i: usize| Some(f.get(i).cloned().unwrap_or_default());
        let code = f.get(13).cloned().unwrap_or_default();
        let name = name_of
            .get(code.as_str())
            .copied()
            .unwrap_or(&code)
            .to_string();
        let cp_val: f64 = f.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let m = name_mul.get(&name).copied().unwrap_or(1.0);
        let rmb = if f.first().map(|s| s.is_empty()).unwrap_or(true) {
            None
        } else {
            Some((cp_val * m * usd_rmb).to_string())
        };
        rows.push(vec![
            Some(name),
            get(0),
            rmb,
            get(2),
            get(3),
            get(4),
            get(5),
            get(6),
            get(7),
            get(8),
            get(9),
            get(12),
        ]);
    }
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&[
        "最新价",
        "人民币报价",
        "买价",
        "卖价",
        "最高价",
        "最低价",
        "昨日结算价",
        "开盘价",
        "持仓量",
    ])?;

    // 涨跌额 / 涨跌幅（数值列）
    let cp = col_f64(&df, "最新价");
    let sp = col_f64(&df, "昨日结算价");
    let n = cp.len();
    let mut diff = Vec::with_capacity(n);
    let mut pct = Vec::with_capacity(n);
    for i in 0..n {
        match (cp[i], sp[i]) {
            (Some(a), Some(b)) if b != 0.0 => {
                diff.push(Some((a - b).to_string()));
                pct.push(Some(((a - b) / b * 100.0).to_string()));
            }
            _ => {
                diff.push(None);
                pct.push(None);
            }
        }
    }
    df.with_column("涨跌额", &diff)?;
    df.with_column("涨跌幅", &pct)?;
    df.cast_numeric(&["涨跌额", "涨跌幅"])?;

    // 重排为最终 14 列
    df.select(&[
        "名称",
        "最新价",
        "人民币报价",
        "涨跌额",
        "涨跌幅",
        "开盘价",
        "最高价",
        "最低价",
        "昨日结算价",
        "持仓量",
        "买价",
        "卖价",
        "行情时间",
        "日期",
    ])
}

/// 外盘期货历史行情（日频率，对应 akshare [`akshare.futures_foreign_hist`]）。
///
/// 数据源 `GlobalFuturesService.getGlobalFuturesDailyKLine`（JSONP）；剥离外壳后按对象
/// 键序建表，`date` 保留字符串，其余数值列转 float64。
pub fn futures_foreign_hist(symbol: &str) -> Result<Df> {
    let url = "https://stock2.finance.sina.com.cn/futures/api/jsonp.php/=/GlobalFuturesService.getGlobalFuturesDailyKLine";
    let mut params = Map::new();
    params.insert("symbol".into(), Value::String(symbol.into()));
    params.insert("source".into(), Value::String("web".into()));
    let http = HttpClient::default();
    let text = http.get_text(url, &params, None)?;
    let arr_text = strip_jsonp(&text)?;
    let arr: Vec<Value> = serde_json::from_str(&arr_text)
        .map_err(|e| AkshareError::json("foreign hist JSON 解析失败", e.to_string()))?;
    let keys: Vec<String> = arr
        .first()
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let rename: Vec<(&str, &str)> = keys.iter().map(|k| (k.as_str(), k.as_str())).collect();
    let select: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
    let numeric: Vec<&str> = keys
        .iter()
        .filter(|k| k.as_str() != "date")
        .map(|k| k.as_str())
        .collect();
    finalize_report(&arr, &rename, &select, &numeric, None)
}

// ---------------------------------------------------------------------------
// 新浪主力连续合约（futures_index_sina.py / futures_cot_sina.py）
// ---------------------------------------------------------------------------

/// 判断连续合约 symbol 的「首数字」是否为 `0`（对应 akshare
/// `symbol.str.extract(r"([\w])(\d)").iloc[:,1].str.contains("0")` 的语义）。
///
/// 正则 `([\w])(\d)` 取字符串中第一个 `[字母/数字/下划线][数字]` 片段，取其中数字位；
/// 该数字为 `"0"` 即为主力连续合约（如 `V0`/`IF0`/`T0`/`P0` 等）。
fn symbol_continuous_digit_is_zero(symbol: &str) -> bool {
    let chars: Vec<char> = symbol.chars().collect();
    for i in 0..chars.len() {
        if i + 1 < chars.len()
            && (chars[i].is_ascii_alphanumeric() || chars[i] == '_')
            && chars[i + 1].is_ascii_digit()
        {
            return chars[i + 1] == '0';
        }
    }
    false
}

/// 新浪主力连续日线短键 → 标准中文列名（对应 akshare 重命名 `d`→`日期` 等）。
const MAIN_KEY_MAP: &[(&str, &str)] = &[
    ("d", "日期"),
    ("o", "开盘价"),
    ("h", "最高价"),
    ("l", "最低价"),
    ("c", "收盘价"),
    ("v", "成交量"),
    ("p", "持仓量"),
    ("s", "动态结算价"),
];
const MAIN_SELECT: &[&str] = &[
    "日期", "开盘价", "最高价", "最低价", "收盘价", "成交量", "持仓量", "动态结算价",
];
const MAIN_NUMERIC: &[&str] = &[
    "开盘价", "最高价", "最低价", "收盘价", "成交量", "持仓量", "动态结算价",
];

/// 新浪主力连续合约品种一览表（对应 akshare [`akshare.futures_display_main_sina`]）。
///
/// 遍历五大交易所每个品种节点（节点代码取自 [`futures_symbol_mark`] 的 `mark` 列）请求
/// `Market_Center.getHQFuturesData`，筛选 `name` 含"连续"且 `symbol` 首数字为 `"0"` 的合约，
/// 取 `[symbol, exchange, name]`。各节点响应自带 `exchange` 字段，无需按交易所分组。
pub fn futures_display_main_sina() -> Result<Df> {
    let mark_df = futures_symbol_mark()?;
    let marks = col_str(&mark_df, "mark");
    let http = HttpClient::default();
    let url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQFuturesData";
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for m in marks.into_iter().flatten() {
        let mut params = Map::new();
        params.insert("page".into(), Value::String("1".into()));
        params.insert("num".into(), Value::String("5".into()));
        params.insert("sort".into(), Value::String("position".into()));
        params.insert("asc".into(), Value::String("0".into()));
        params.insert("node".into(), Value::String(m));
        params.insert("base".into(), Value::String("futures".into()));
        let v = match http.get_json(url, &params, None) {
            Ok(x) => x,
            Err(_) => continue, // 单节点失败不中断整体（对应 akshare try/except 跳过）
        };
        let arr = v.as_array().cloned().unwrap_or_default();
        for row in &arr {
            let obj = match row.as_object() {
                Some(o) => o,
                None => continue,
            };
            let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
            let symbol = obj.get("symbol").and_then(Value::as_str).unwrap_or("");
            if !name.contains("连续") || !symbol_continuous_digit_is_zero(symbol) {
                continue;
            }
            let sym = obj.get("symbol").and_then(Value::as_str).map(str::to_string);
            let ex = obj.get("exchange").and_then(Value::as_str).map(str::to_string);
            let nm = obj.get("name").and_then(Value::as_str).map(str::to_string);
            rows.push(vec![sym, ex, nm]);
            break; // 每节点仅取首个匹配（对应 akshare `.iloc[0,:3]`）
        }
    }
    Df::from_string_rows(&["symbol", "exchange", "name"], &rows)
}

/// 新浪财经-期货-主力连续日数据（对应 akshare [`akshare.futures_main_sina`]）。
///
/// 数据源 `InnerFuturesNewService.getDailyKLine`（JSONP，日期参数固定 `2021_08_17` 与 akshare 同）。
/// 剥离外壳后按对象键序建表，重命名为中文列；`日期` 保留字符串，其余 7 列转 float64；
/// 最后按 `start_date`/`end_date`（`YYYYMMDD`）做闭区间日期过滤（对应 akshare 的 datetime 索引切片）。
pub fn futures_main_sina(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let trade_date = "2021_08_17";
    let url = "https://stock2.finance.sina.com.cn/futures/api/jsonp.php/var%20_{symbol}{trade_date}=/InnerFuturesNewService.getDailyKLine";
    let mut params = Map::new();
    params.insert("symbol".into(), Value::String(symbol.into()));
    params.insert("_".into(), Value::String(trade_date.into()));
    let http = HttpClient::default();
    let text = http.get_text(url, &params, None)?;
    let arr_text = strip_jsonp(&text)?;
    let arr: Vec<Value> = serde_json::from_str(&arr_text)
        .map_err(|e| AkshareError::json("main kline JSON 解析失败", e.to_string()))?;

    // 日期范围过滤：start/end 为 YYYYMMDD（支持含分隔符写法），按 8 位零填充串做闭区间字典序比较。
    let sd: String = start_date.chars().filter(|c| c.is_ascii_digit()).collect();
    let ed: String = end_date.chars().filter(|c| c.is_ascii_digit()).collect();
    let filtered: Vec<Value> = arr
        .into_iter()
        .filter(|row| {
            let d = row.get("d").and_then(Value::as_str).unwrap_or("");
            let dn: String = d.chars().filter(|c| c.is_ascii_digit()).collect();
            dn.len() == 8 && dn.as_str() >= sd.as_str() && dn.as_str() <= ed.as_str()
        })
        .collect();

    finalize_report(&filtered, MAIN_KEY_MAP, MAIN_SELECT, MAIN_NUMERIC, None)
}

/// 新浪财经-期货-成交持仓（对应 akshare [`akshare.futures_hold_pos_sina`]）。
///
/// `symbol`：`成交量`/`多单持仓`/`空单持仓`（对应 `read_html` 的第 3/4/5 张表）；
/// `contract`：期货合约；`date`：`YYYYMMDD`（内部归一化为 `YYYY-MM-DD`）。
/// 取目标表（丢弃首行表头、末行合计），列 `[名次, 会员简称, <度量>, 比上交易增减]`，
/// 数值化为 `名次`/`<度量>`/`比上交易增减`。
pub fn futures_hold_pos_sina(symbol: &str, contract: &str, date: &str) -> Result<Df> {
    let ddigits: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
    if ddigits.len() != 8 {
        return Err(AkshareError::Param(format!(
            "无效日期: {date}（应为 YYYYMMDD / YYYY-MM-DD）"
        )));
    }
    let date_fmt = format!("{}-{}-{}", &ddigits[0..4], &ddigits[4..6], &ddigits[6..8]);
    let url = "https://vip.stock.finance.sina.com.cn/q/view/vFutures_Positions_cjcc.php";
    let mut params = Map::new();
    params.insert("t_breed".into(), Value::String(contract.into()));
    params.insert("t_date".into(), Value::String(date_fmt));
    let http = HttpClient::default();
    let text = http.get_text(url, &params, None)?;
    let tables = read_html_tables(&text)?;
    let idx = match symbol {
        "成交量" => 2usize,
        "多单持仓" => 3usize,
        "空单持仓" => 4usize,
        other => {
            return Err(AkshareError::Param(format!(
                "未知 symbol: {other}（可选：成交量/多单持仓/空单持仓）"
            )))
        }
    };
    let raw = tables.into_iter().nth(idx).ok_or_else(|| {
        AkshareError::Empty(format!("futures_hold_pos_sina: 未找到第 {idx} 个 <table>"))
    })?;
    if raw.is_empty() {
        return Err(AkshareError::Empty("futures_hold_pos_sina: 空表".into()));
    }
    // raw[0] = 表头，其余为数据行；akshare `.iloc[:-1,:]` 丢弃最后一行（合计）。
    let header: Vec<String> = raw[0].clone();
    let mut data: Vec<Vec<Option<String>>> =
        raw.into_iter().skip(1).map(|r| r.into_iter().map(Some).collect()).collect();
    data.pop();
    let names: Vec<&str> = header.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&names, &data)?;
    let metric = header.get(2).cloned().unwrap_or_default();
    let numeric: Vec<&str> = vec!["名次", metric.as_str(), "比上交易增减"];
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 外盘期货合约详情（对应 akshare [`akshare.futures_foreign_detail`]）。
///
/// 数据源 `{symbol}.shtml`（gbk），取文档中第 7 个 `<table>`（akshare `read_html[6]`）。
/// 该表为「标签/值」网格（每行 3 组 `[标签, 值, 标签, 值, 标签, 值]`），pandas 以
/// `header=None` 解析为 6 列整数列名、全部为字符串，故此处取原始二维表、不把首行当表头、
/// 也不转数值（与 golden 列契约 + dtype 对齐）。
pub fn futures_foreign_detail(symbol: &str) -> Result<Df> {
    let url = format!("https://finance.sina.com.cn/futures/quotes/{symbol}.shtml");
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), None)?;
    let tables = read_html_tables(&text)?;
    let raw = tables.into_iter().nth(6).ok_or_else(|| {
        AkshareError::Empty("foreign_detail: 未找到第 7 个 <table>".into())
    })?;
    let ncols = raw.iter().map(|r| r.len()).max().unwrap_or(0);
    let names: Vec<String> = (0..ncols).map(|i| i.to_string()).collect();
    let rows: Vec<Vec<Option<String>>> = raw
        .into_iter()
        .map(|r| {
            let mut row: Vec<Option<String>> = r.into_iter().map(Some).collect();
            row.resize(ncols, None);
            row
        })
        .collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    Df::from_string_rows(&name_refs, &rows)
}
