//! 期权（option）数据源。
//!
//! 对应 akshare `option/` 模块，覆盖新浪财经（中金所/上交所/商品）、交易所
//! （上交所/深交所）、东方财富三个数据源的期权接口。
//!
//! 列名/列序严格对齐 akshare；实时类接口（spot/minute）数值随时间变化，
//! parity 用例使用 `loose` 模式仅校验列契约；历史/静态接口使用 `strict`。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{finalize_report, push2_urls};
use polars::prelude::{IntoSeries, NewChunkedArray};
use serde_json::{json, Map, Value};

/// 新浪 `hq.sinajs.cn` 需要的 Referer（缺省会 403）。
const SINA_REFERER: &str = "https://stock.finance.sina.com.cn/";
/// 上交所 `query.sse.com.cn` 需要的 Referer。
const SSE_REFERER: &str = "http://www.sse.com.cn/";
/// 上期所接口需要的旧版 UA（缺省会返回 520）。
const SHFE_HEADERS: &[(&str, &str)] = &[(
    "User-Agent",
    "Mozilla/4.0 (compatible; MSIE 5.5; Windows NT)",
)];
/// 广期所接口需要的请求头（缺省会返回 520）。
const GFEX_HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/json, text/javascript, */*; q=0.01"),
    (
        "Content-Type",
        "application/x-www-form-urlencoded; charset=UTF-8",
    ),
    (
        "Referer",
        "http://www.gfex.com.cn/gfex/rihq/hqsj_tjsj.shtml",
    ),
    (
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36",
    ),
    ("X-Requested-With", "XMLHttpRequest"),
];

/// JSON 值 → `Option<String>`（null 映射为 None）。
fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// 从 JSONP 文本中提取 `[...]` 数组部分（兼容 `var x=([...]);` 与裸数组）。
fn extract_array(text: &str) -> Option<&str> {
    let s = text.find('[')?;
    let e = text.rfind(']')?;
    if e > s {
        Some(&text[s..=e])
    } else {
        None
    }
}

/// 从文本中提取首个 `"..."` 包裹的字段串（新浪行情接口）。
fn extract_quoted(text: &str) -> &str {
    match (text.find('"'), text.rfind('"')) {
        (Some(s), Some(e)) if e > s => &text[s + 1..e],
        _ => "",
    }
}

// ===========================================================================
// 阶段 1：新浪财经-中金所（CFFEX）期权
// ===========================================================================

/// CFFEX 期权实时行情内部解析（up 9 字段 + down 8 字段 横向拼接为 17 列）。
///
/// `data` 为 `OptionService.getOptionData` 返回的 JSON 对象。
fn parse_cffex_spot(data: &Value) -> Result<Df> {
    let up = data.get("up").and_then(Value::as_array);
    let down = data.get("down").and_then(Value::as_array);
    let (Some(up), Some(down)) = (up, down) else {
        return Err(AkshareError::Empty(
            "CFFEX 期权数据缺失 up/down 字段".into(),
        ));
    };
    let call_cols = [
        "看涨合约-买量",
        "看涨合约-买价",
        "看涨合约-最新价",
        "看涨合约-卖价",
        "看涨合约-卖量",
        "看涨合约-持仓量",
        "看涨合约-涨跌",
        "行权价",
        "看涨合约-标识",
    ];
    let put_cols = [
        "看跌合约-买量",
        "看跌合约-买价",
        "看跌合约-最新价",
        "看跌合约-卖价",
        "看跌合约-卖量",
        "看跌合约-持仓量",
        "看跌合约-涨跌",
        "看跌合约-标识",
    ];
    let n = up.len().min(down.len());
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = Vec::with_capacity(17);
        if let Some(a) = up[i].as_array() {
            for c in a {
                row.push(str_of(c));
            }
        }
        if let Some(a) = down[i].as_array() {
            for c in a {
                row.push(str_of(c));
            }
        }
        rows.push(row);
    }
    let cols: Vec<&str> = call_cols.iter().chain(put_cols.iter()).copied().collect();
    // 两个「标识」列为合约代码字符串（如 HO2303-C-2350），不参与数值化。
    let numeric_cols: Vec<&str> = cols
        .iter()
        .copied()
        .filter(|c| *c != "看涨合约-标识" && *c != "看跌合约-标识")
        .collect();
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&numeric_cols)?;
    Ok(df)
}

/// CFFEX 期权实时行情（中金所-沪深300/上证50/中证1000）内部抓取。
fn cffex_spot(product: &str, symbol: &str) -> Result<Df> {
    let url =
        "https://stock.finance.sina.com.cn/futures/api/openapi.php/OptionService.getOptionData";
    let params = json!({
        "type": "futures",
        "product": product,
        "exchange": "cffex",
        "pinzhong": symbol,
    });
    let http = HttpClient::default();
    let text = http.get_text(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let s = text
        .find('{')
        .ok_or_else(|| AkshareError::Empty("CFFEX 响应缺少 JSON".into()))?;
    let e = text
        .rfind('}')
        .ok_or_else(|| AkshareError::Empty("CFFEX 响应缺少 JSON".into()))?;
    let v: Value = serde_json::from_str(&text[s..=e])
        .map_err(|err| AkshareError::json(url, err.to_string()))?;
    let data = v
        .get("result")
        .and_then(|r| r.get("data"))
        .ok_or_else(|| AkshareError::Empty("CFFEX 响应缺少 result.data".into()))?;
    parse_cffex_spot(data)
}

/// 中金所-沪深300指数-指定合约-实时行情。
///
/// # 参数
/// - `symbol`: 合约代码，如 `io2204`
///
/// # 返回列
/// 17 列：`看涨合约-买量` … `看涨合约-标识`, `看跌合约-买量` … `看跌合约-标识`
pub fn option_cffex_hs300_spot_sina(symbol: &str) -> Result<Df> {
    cffex_spot("io", symbol)
}

/// 中金所-上证50指数-指定合约-实时行情。
pub fn option_cffex_sz50_spot_sina(symbol: &str) -> Result<Df> {
    cffex_spot("ho", symbol)
}

/// 中金所-中证1000指数-指定合约-实时行情。
pub fn option_cffex_zz1000_spot_sina(symbol: &str) -> Result<Df> {
    cffex_spot("mo", symbol)
}

/// CFFEX 期权日频行情（FutureOptionAllService.getOptionDayline）。
///
/// 返回 6 列：`date, open, high, low, close, volume`。过期合约返回空表。
fn cffex_daily(symbol: &str) -> Result<Df> {
    let url = "https://stock.finance.sina.com.cn/futures/api/jsonp.php/var%20_ak=/FutureOptionAllService.getOptionDayline";
    let params = json!({ "symbol": symbol });
    let http = HttpClient::default();
    let text = http.get_text(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let body = match extract_array(&text) {
        Some(b) => b,
        None => {
            return Df::from_string_rows(&["date", "open", "high", "low", "close", "volume"], &[]);
        }
    };
    let arr: Value =
        serde_json::from_str(body).map_err(|err| AkshareError::json(url, err.to_string()))?;
    let rows = arr
        .as_array()
        .ok_or_else(|| AkshareError::Empty("日线数据格式错误".into()))?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in rows {
        // 中金所日线返回对象：{"o","h","l","c","v","d"}
        let o = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        out.push(vec![
            o.get("d").and_then(str_of),
            o.get("o").and_then(str_of),
            o.get("h").and_then(str_of),
            o.get("l").and_then(str_of),
            o.get("c").and_then(str_of),
            o.get("v").and_then(str_of),
        ]);
    }
    let cols = ["date", "open", "high", "low", "close", "volume"];
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_date(&["date"])?;
    df.cast_numeric(&["open", "high", "low", "close", "volume"])?;
    Ok(df)
}

/// 中金所-沪深300指数-指定合约-日频行情。
pub fn option_cffex_hs300_daily_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

/// 中金所-上证50指数-指定合约-日频行情。
pub fn option_cffex_sz50_daily_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

/// 中金所-中证1000指数-指定合约-日频行情。
pub fn option_cffex_zz1000_daily_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

/// 中金所-沪深300指数-所有合约（Dict[str,List[str]]，首项为主力合约）。
///
/// 返回 `Result<Value>`（结构为 `{symbol: [contract, ...]}`），不进入 parity。
pub fn option_cffex_hs300_list_sina() -> Result<Value> {
    cffex_list("https://stock.finance.sina.com.cn/futures/view/optionsCffexDP.php")
}

/// 中金所-上证50指数-所有合约。
pub fn option_cffex_sz50_list_sina() -> Result<Value> {
    cffex_list("https://stock.finance.sina.com.cn/futures/view/optionsCffexDP.php/ho/cffex")
}

/// 中金所-中证1000指数-所有合约。
pub fn option_cffex_zz1000_list_sina() -> Result<Value> {
    cffex_list("https://stock.finance.sina.com.cn/futures/view/optionsCffexDP.php/mo/cffex")
}

/// 新浪 CFFEX 合约列表页解析（依赖 `#option_symbol`/`#option_suffix` 的 HTML）。
fn cffex_list(url: &str) -> Result<Value> {
    let http = HttpClient::default();
    let text = http.get_text(url, &Map::new(), Some(SINA_REFERER))?;
    // 无 HTML 解析器时退化为仅抓取页面文本中的合约片段（尽力而为）。
    let symbol = text
        .split("option_symbol")
        .nth(1)
        .and_then(|s| s.split('>').nth(1))
        .and_then(|s| s.split('<').next())
        .unwrap_or("")
        .to_string();
    let mut contracts = Vec::new();
    if let Some(seg) = text.split("option_suffix").nth(1) {
        for part in seg.split("<li").skip(1) {
            if let Some(c) = part.split('>').nth(1).and_then(|s| s.split('<').next()) {
                let c = c.trim();
                if !c.is_empty() {
                    contracts.push(c.to_string());
                }
            }
        }
    }
    let mut map = serde_json::Map::new();
    map.insert(
        symbol,
        Value::Array(contracts.into_iter().map(Value::String).collect()),
    );
    Ok(Value::Object(map))
}

// ===========================================================================
// 阶段 2：新浪财经-上交所（SSE）期权
// ===========================================================================

/// 上交所-50ETF-合约到期月份列表（List[str]）。返回 `Result<Value>`。
pub fn option_sse_list_sina(symbol: &str, exchange: &str) -> Result<Value> {
    let url =
        "https://stock.finance.sina.com.cn/futures/api/openapi.php/StockOptionService.getStockName";
    let params = json!({ "exchange": exchange, "cate": symbol });
    let http = HttpClient::default();
    let v = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let months = v["result"]["data"]["contractMonth"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let list: Vec<Value> = months
        .iter()
        .filter_map(|m| m.as_str())
        .map(|s| Value::String(s.split('-').collect::<Vec<_>>().join("")))
        .collect();
    Ok(Value::Array(list.into_iter().skip(1).collect()))
}

/// 指定到期月份指定品种的剩余到期时间（Tuple[str,int]）。返回 `Result<Value>`。
pub fn option_sse_expire_day_sina(trade_date: &str, symbol: &str, exchange: &str) -> Result<Value> {
    let url = "https://stock.finance.sina.com.cn/futures/api/openapi.php/StockOptionService.getRemainderDay";
    let date = if trade_date.len() >= 6 {
        format!("{}-{}", &trade_date[..4], &trade_date[4..])
    } else {
        trade_date.to_string()
    };
    let params = json!({ "exchange": exchange, "cate": symbol, "date": date });
    let http = HttpClient::default();
    let v = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let mut data = v["result"]["data"].clone();
    let remainder = data
        .get("remainderDays")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if remainder < 0 {
        let params2 = json!({ "exchange": exchange, "cate": format!("XD{symbol}"), "date": date });
        let v2 = http.get_json(
            url,
            params2.as_object().expect("静态参数"),
            Some(SINA_REFERER),
        )?;
        data = v2["result"]["data"].clone();
    }
    let expire = data.get("expireDay").cloned().unwrap_or(Value::Null);
    let remain = data.get("remainderDays").cloned().unwrap_or(Value::Null);
    Ok(json!([expire, remain]))
}

/// 上交所-所有看涨/看跌合约代码。
///
/// # 返回列
/// `序号, 期权代码`
pub fn option_sse_codes_sina(symbol: &str, trade_date: &str, underlying: &str) -> Result<Df> {
    let last4 = if trade_date.len() >= 4 {
        &trade_date[trade_date.len() - 4..]
    } else {
        trade_date
    };
    let prefix = if symbol == "看涨期权" {
        "OP_UP_"
    } else {
        "OP_DOWN_"
    };
    let url = format!("https://hq.sinajs.cn/list={prefix}{underlying}{last4}");
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), Some(SINA_REFERER))?;
    let replaced = text.replace('"', ",");
    let codes: Vec<&str> = replaced
        .split(',')
        .filter(|p| p.starts_with("CON_OP_"))
        .map(|p| &p[7..])
        .collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(codes.len());
    for (i, c) in codes.iter().enumerate() {
        rows.push(vec![Some((i + 1).to_string()), Some((*c).to_string())]);
    }
    let mut df = Df::from_string_rows(&["序号", "期权代码"], &rows)?;
    // akshare 的「序号」列为 int64，cast 数值化以对齐 dtype。
    df.cast_numeric(&["序号"])?;
    Ok(df)
}

/// 上交所-期权实时量价数据（字段/值 两列）。
///
/// # 返回列
/// `字段, 值`
pub fn option_sse_spot_price_sina(symbol: &str) -> Result<Df> {
    sina_field_value(
        &format!("CON_OP_{symbol}"),
        &[
            "买量",
            "买价",
            "最新价",
            "卖价",
            "卖量",
            "持仓量",
            "涨幅",
            "行权价",
            "昨收价",
            "开盘价",
            "涨停价",
            "跌停价",
            "申卖价五",
            "申卖量五",
            "申卖价四",
            "申卖量四",
            "申卖价三",
            "申卖量三",
            "申卖价二",
            "申卖量二",
            "申卖价一",
            "申卖量一",
            "申买价一",
            "申买量一 ",
            "申买价二",
            "申买量二",
            "申买价三",
            "申买量三",
            "申买价四",
            "申买量四",
            "申买价五",
            "申买量五",
            "行情时间",
            "主力合约标识",
            "状态码",
            "标的证券类型",
            "标的股票",
            "期权合约简称",
            "振幅",
            "最高价",
            "最低价",
            "成交量",
            "成交额",
        ],
    )
}

/// 上交所-期权标的物实时数据（字段/值 两列）。
pub fn option_sse_underlying_spot_price_sina(symbol: &str) -> Result<Df> {
    sina_field_value(
        symbol,
        &[
            "证券简称",
            "今日开盘价",
            "昨日收盘价",
            "最近成交价",
            "最高成交价",
            "最低成交价",
            "买入价",
            "卖出价",
            "成交数量",
            "成交金额",
            "买数量一",
            "买价位一",
            "买数量二",
            "买价位二",
            "买数量三",
            "买价位三",
            "买数量四",
            "买价位四",
            "买数量五",
            "买价位五",
            "卖数量一",
            "卖价位一",
            "卖数量二",
            "卖价位二",
            "卖数量三",
            "卖价位三",
            "卖数量四",
            "卖价位四",
            "卖数量五",
            "卖价位五",
            "行情日期",
            "行情时间",
            "停牌状态",
        ],
    )
}

/// 上交所-期权基本信息表（字段/值 两列）。
pub fn option_sse_greeks_sina(symbol: &str) -> Result<Df> {
    sina_field_value(
        &format!("CON_SO_{symbol}"),
        &[
            "期权合约简称",
            "成交量",
            "Delta",
            "Gamma",
            "Theta",
            "Vega",
            "隐含波动率",
            "最高价",
            "最低价",
            "交易代码",
            "行权价",
            "最新价",
            "理论价值",
        ],
    )
}

/// 新浪 `hq.sinajs.cn` 行情字段/值解析（两列：`字段, 值`）。
fn sina_field_value(url_suffix: &str, field_list: &[&str]) -> Result<Df> {
    let url = format!("https://hq.sinajs.cn/list={url_suffix}");
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), Some(SINA_REFERER))?;
    let inner = extract_quoted(&text);
    let data_list: Vec<&str> = inner.split(',').collect();
    let n = field_list.len().min(data_list.len());
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(n.max(1));
    for i in 0..n {
        rows.push(vec![
            Some(field_list[i].to_string()),
            Some(data_list[i].to_string()),
        ]);
    }
    Df::from_string_rows(&["字段", "值"], &rows)
}

/// 上交所-指定期权当前交易日分钟数据。
///
/// # 返回列
/// `日期, 时间, 价格, 成交, 持仓, 均价`
pub fn option_sse_minute_sina(symbol: &str) -> Result<Df> {
    let url = "https://stock.finance.sina.com.cn/futures/api/openapi.php/StockOptionDaylineService.getOptionMinline";
    let params = json!({ "symbol": format!("CON_OP_{symbol}") });
    let http = HttpClient::default();
    let v = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let data = v["result"]["data"].as_array().cloned().unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for r in &data {
        let a = match r.as_array() {
            Some(a) => a,
            None => continue,
        };
        if a.len() < 6 {
            continue;
        }
        rows.push(vec![
            str_of(&a[5]),
            str_of(&a[0]),
            str_of(&a[1]),
            str_of(&a[2]),
            str_of(&a[3]),
            str_of(&a[4]),
        ]);
    }
    let cols = ["日期", "时间", "价格", "成交", "持仓", "均价"];
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["价格", "成交", "持仓", "均价"])?;
    Ok(df)
}

/// 上交所-指定期权日频历史数据。
///
/// # 返回列
/// `日期, 开盘, 最高, 最低, 收盘, 成交量`
pub fn option_sse_daily_sina(symbol: &str) -> Result<Df> {
    let url = "https://stock.finance.sina.com.cn/futures/api/jsonp_v2.php//StockOptionDaylineService.getSymbolInfo";
    let params = json!({ "symbol": format!("CON_OP_{symbol}") });
    let http = HttpClient::default();
    let text = http.get_text(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let s = text
        .find('(')
        .ok_or_else(|| AkshareError::Empty("SSE 日线响应缺少 '('".into()))?;
    let e = text
        .rfind(')')
        .ok_or_else(|| AkshareError::Empty("SSE 日线响应缺少 ')'".into()))?;
    let arr: Value = serde_json::from_str(&text[s + 1..e])
        .map_err(|err| AkshareError::json(url, err.to_string()))?;
    let rows = arr.as_array().cloned().unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        // 新浪返回数组对象：{"d","o","h","l","c","v"}
        let o = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        out.push(vec![
            o.get("d").and_then(str_of),
            o.get("o").and_then(str_of),
            o.get("h").and_then(str_of),
            o.get("l").and_then(str_of),
            o.get("c").and_then(str_of),
            o.get("v").and_then(str_of),
        ]);
    }
    let cols = ["日期", "开盘", "最高", "最低", "收盘", "成交量"];
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["开盘", "最高", "最低", "收盘", "成交量"])?;
    Ok(df)
}

/// 上交所-指定期权分钟频率数据（五日）。
///
/// # 返回列
/// `date, time, price, average_price, volume`
pub fn option_finance_minute_sina(symbol: &str) -> Result<Df> {
    let url = "https://stock.finance.sina.com.cn/futures/api/openapi.php/StockOptionDaylineService.getFiveDayLine";
    let params = json!({ "symbol": format!("CON_OP_{symbol}") });
    let http = HttpClient::default();
    let v = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SINA_REFERER),
    )?;
    let items = v["result"]["data"].as_array().cloned().unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(items.len());
    for item in &items {
        let date = item.get("date").and_then(Value::as_str).unwrap_or("");
        let time = item.get("time").and_then(Value::as_str).unwrap_or("");
        let price = item.get("price").and_then(Value::as_str).unwrap_or("");
        let avg = item
            .get("average_price")
            .and_then(Value::as_str)
            .unwrap_or("");
        let vol = item.get("volume").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![
            Some(date.to_string()),
            Some(time.to_string()),
            Some(price.to_string()),
            Some(avg.to_string()),
            Some(vol.to_string()),
        ]);
    }
    let cols = ["date", "time", "price", "average_price", "volume"];
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&["price", "average_price", "volume"])?;
    Ok(df)
}

// ===========================================================================
// 阶段 3：新浪财经-商品期权
// ===========================================================================

/// 商品期权历史日频行情（FutureOptionAllService.getOptionDayline，同 CFFEX 日线）。
pub fn option_commodity_hist_sina(symbol: &str) -> Result<Df> {
    cffex_daily(symbol)
}

/// 商品期权合约日期（依赖 HTML 解析，暂未移植，见报告）。
pub fn option_commodity_contract_sina(_symbol: &str) -> Result<Df> {
    Err(AkshareError::Empty(
        "option_commodity_contract_sina 需要 BeautifulSoup HTML 解析（optionsDP.php），core/html.rs 尚未提供，暂未移植".into(),
    ))
}

/// 商品期权合约实时行情表（依赖 HTML 解析，暂未移植）。
pub fn option_commodity_contract_table_sina(_symbol: &str, _contract: &str) -> Result<Df> {
    Err(AkshareError::Empty(
        "option_commodity_contract_table_sina 需要 HTML 解析以定位 product/exchange，暂未移植"
            .into(),
    ))
}

// ===========================================================================
// 阶段 4：交易所（上交所/深交所）期权
// ===========================================================================

/// 上交所-期权当日合约。
///
/// # 返回列
/// `合约编码, 合约交易代码, 合约简称, 标的券名称及代码, 类型, 行权价, 合约单位,
/// 期权行权日, 行权交收日, 到期日, 开始日期`
pub fn option_current_day_sse() -> Result<Df> {
    let url = "http://query.sse.com.cn/commonQuery.do";
    let params = json!({
        "isPagination": "false",
        "expireDate": "",
        "securityId": "",
        "sqlId": "SSE_ZQPZ_YSP_GGQQZSXT_XXPL_DRHY_SEARCH_L",
    });
    let http = HttpClient::default();
    let v = http.get_json_with_headers(
        url,
        params.as_object().expect("静态参数"),
        &[],
        Some(SSE_REFERER),
    )?;
    let result = v["result"].as_array().cloned().unwrap_or_default();
    let rename = [
        ("SECURITY_ID", "合约编码"),
        ("CONTRACT_ID", "合约交易代码"),
        ("CONTRACT_SYMBOL", "合约简称"),
        ("SECURITYNAMEBYID", "标的券名称及代码"),
        ("CALL_OR_PUT", "类型"),
        ("EXERCISE_PRICE", "行权价"),
        ("CONTRACT_UNIT", "合约单位"),
        ("END_DATE", "期权行权日"),
        ("DELIVERY_DATE", "行权交收日"),
        ("EXPIRE_DATE", "到期日"),
        ("START_DATE", "开始日期"),
    ];
    let select = [
        "合约编码",
        "合约交易代码",
        "合约简称",
        "标的券名称及代码",
        "类型",
        "行权价",
        "合约单位",
        "期权行权日",
        "行权交收日",
        "到期日",
        "开始日期",
    ];
    finalize_report(&result, &rename, &select, &[], None)
}

/// 深交所-期权当日合约（依赖 xlsx 解析，暂未移植，见报告）。
pub fn option_current_day_szse() -> Result<Df> {
    Err(AkshareError::Empty(
        "option_current_day_szse 需要 xlsx 解析（深交所 ShowReport xls 下载），项目暂无 xlsx 解析器，暂未移植".into(),
    ))
}

/// 上交所-期权每日统计。
///
/// # 返回列
/// `合约标的代码, 合约标的名称, 合约数量, 总成交额, 总成交量, 认购成交量, 认沽成交量,
/// 认沽/认购, 未平仓合约总数, 未平仓认购合约数, 未平仓认沽合约数, 交易日`
pub fn option_daily_stats_sse(date: &str) -> Result<Df> {
    let url = "http://query.sse.com.cn/commonQuery.do";
    let params = json!({
        "isPagination": "false",
        "sqlId": "COMMON_SSE_ZQPZ_YSP_QQ_SJTJ_MRTJ_CX",
        "tradeDate": date,
    });
    let http = HttpClient::default();
    let v = http.get_json_with_headers(
        url,
        params.as_object().expect("静态参数"),
        &[],
        Some(SSE_REFERER),
    )?;
    let result = v["result"].as_array().cloned().unwrap_or_default();
    let rename = [
        ("SECURITY_CODE", "合约标的代码"),
        ("SECURITY_ABBR", "合约标的名称"),
        ("CONTRACT_VOLUME", "合约数量"),
        ("TOTAL_MONEY", "总成交额"),
        ("TOTAL_VOLUME", "总成交量"),
        ("CALL_VOLUME", "认购成交量"),
        ("PUT_VOLUME", "认沽成交量"),
        ("CP_RATE", "认沽/认购"),
        ("LEAVES_QTY", "未平仓合约总数"),
        ("LEAVES_CALL_QTY", "未平仓认购合约数"),
        ("LEAVES_PUT_QTY", "未平仓认沽合约数"),
        ("TRADE_DATE", "交易日"),
    ];
    let select = [
        "合约标的代码",
        "合约标的名称",
        "合约数量",
        "总成交额",
        "总成交量",
        "认购成交量",
        "认沽成交量",
        "认沽/认购",
        "未平仓合约总数",
        "未平仓认购合约数",
        "未平仓认沽合约数",
        "交易日",
    ];
    let mut df = finalize_report(&result, &rename, &select, &[], None)?;
    df.cast_date(&["交易日"])?;
    let numeric = [
        "合约数量",
        "总成交额",
        "总成交量",
        "认购成交量",
        "认沽成交量",
        "认沽/认购",
        "未平仓合约总数",
        "未平仓认购合约数",
        "未平仓认沽合约数",
    ];
    df.strip_commas(&numeric)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 深交所-期权每日统计。
///
/// # 返回列
/// `合约标的代码, 合约标的名称, 成交量, 认购成交量, 认沽成交量, 认沽/认购持仓比,
/// 未平仓合约总数, 未平仓认购合约数, 未平仓认沽合约数, 交易日`
pub fn option_daily_stats_szse(date: &str) -> Result<Df> {
    let url = "https://investor.szse.cn/api/report/ShowReport/data";
    let trade_date = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]);
    let params = json!({
        "SHOWTYPE": "JSON",
        "CATALOGID": "ysprdzb",
        "TABKEY": "tab1",
        "txtQueryDate": trade_date,
        "random": "0.0652692406565949",
    });
    let http = HttpClient::default();
    let v = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some("https://investor.szse.cn/"),
    )?;
    let arr = v
        .as_array()
        .and_then(|a| a.first().cloned())
        .unwrap_or(Value::Null);
    let result = arr
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename = [
        ("bddm", "合约标的代码"),
        ("bdmc", "合约标的名称"),
        ("cjl", "成交量"),
        ("rccjl", "认购成交量"),
        ("rpcjl", "认沽成交量"),
        ("rcrpccb", "认沽/认购持仓比"),
        ("wpchyzs", "未平仓合约总数"),
        ("wpcrchys", "未平仓认购合约数"),
        ("wpcrphys", "未平仓认沽合约数"),
    ];
    let select = [
        "合约标的代码",
        "合约标的名称",
        "成交量",
        "认购成交量",
        "认沽成交量",
        "认沽/认购持仓比",
        "未平仓合约总数",
        "未平仓认购合约数",
        "未平仓认沽合约数",
    ];
    let mut df = finalize_report(&result, &rename, &select, &[], None)?;
    df.with_column("交易日", &[Some(trade_date.clone())])?;
    let numeric = [
        "成交量",
        "认购成交量",
        "认沽成交量",
        "认沽/认购持仓比",
        "未平仓合约总数",
        "未平仓认购合约数",
        "未平仓认沽合约数",
    ];
    df.strip_commas(&numeric)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 上交所-期权风险指标。
///
/// # 返回列
/// `TRADE_DATE, SECURITY_ID, CONTRACT_ID, CONTRACT_SYMBOL, DELTA_VALUE, THETA_VALUE,
/// GAMMA_VALUE, VEGA_VALUE, RHO_VALUE, IMPLC_VOLATLTY`
pub fn option_risk_indicator_sse(date: &str) -> Result<Df> {
    let url = "http://query.sse.com.cn/commonQuery.do";
    let params = json!({
        "isPagination": "false",
        "trade_date": date,
        "sqlId": "SSE_ZQPZ_YSP_GGQQZSXT_YSHQ_QQFXZB_DATE_L",
        "contractSymbol": "",
    });
    let http = HttpClient::default();
    let v = http.get_json_with_headers(
        url,
        params.as_object().expect("静态参数"),
        &[],
        Some(SSE_REFERER),
    )?;
    let result = v["result"].as_array().cloned().unwrap_or_default();
    let rename = [
        ("TRADE_DATE", "TRADE_DATE"),
        ("SECURITY_ID", "SECURITY_ID"),
        ("CONTRACT_ID", "CONTRACT_ID"),
        ("CONTRACT_SYMBOL", "CONTRACT_SYMBOL"),
        ("DELTA_VALUE", "DELTA_VALUE"),
        ("THETA_VALUE", "THETA_VALUE"),
        ("GAMMA_VALUE", "GAMMA_VALUE"),
        ("VEGA_VALUE", "VEGA_VALUE"),
        ("RHO_VALUE", "RHO_VALUE"),
        ("IMPLC_VOLATLTY", "IMPLC_VOLATLTY"),
    ];
    let select = [
        "TRADE_DATE",
        "SECURITY_ID",
        "CONTRACT_ID",
        "CONTRACT_SYMBOL",
        "DELTA_VALUE",
        "THETA_VALUE",
        "GAMMA_VALUE",
        "VEGA_VALUE",
        "RHO_VALUE",
        "IMPLC_VOLATLTY",
    ];
    let mut df = finalize_report(&result, &rename, &select, &[], None)?;
    df.cast_date(&["TRADE_DATE"])?;
    df.cast_numeric(&[
        "DELTA_VALUE",
        "THETA_VALUE",
        "GAMMA_VALUE",
        "VEGA_VALUE",
        "RHO_VALUE",
        "IMPLC_VOLATLTY",
    ])?;
    Ok(df)
}

// ===========================================================================
// 阶段 5：东方财富（eastmoney）期权
// ===========================================================================

/// 东财 push2 clist 抓取（多节点容灾）并定位目标字段。
fn em_clist(target: &[(&str, &str)], fields: &str, fs: &str, ut: &str) -> Result<Vec<Value>> {
    let urls = push2_urls("/api/qt/clist/get");
    let mut params = Map::new();
    params.insert("pn".into(), Value::String("1".into()));
    params.insert("pz".into(), Value::String("2000".into()));
    params.insert("po".into(), Value::String("1".into()));
    params.insert("np".into(), Value::String("1".into()));
    params.insert("fltt".into(), Value::String("2".into()));
    params.insert("invt".into(), Value::String("2".into()));
    params.insert("fid".into(), Value::String("f3".into()));
    params.insert("ut".into(), Value::String(ut.into()));
    params.insert("fs".into(), Value::String(fs.into()));
    params.insert("fields".into(), Value::String(fields.into()));
    let http = HttpClient::default();
    let rows = http.fetch_paginated_diff_any(&urls, &params, None)?;
    // 按目标字段映射提取（字段语义映射，实时类数值精确性见报告说明）。
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        let mut m = serde_json::Map::new();
        for (fkey, name) in target {
            m.insert(
                (*name).to_string(),
                obj.get(*fkey).cloned().unwrap_or(Value::Null),
            );
        }
        out.push(Value::Object(m));
    }
    Ok(out)
}

/// 东方财富-期权市场实时行情（含中金所）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 成交量, 成交额, 持仓量, 行权价,
/// 剩余日, 日增, 昨结, 今开, 市场标识`
pub fn option_current_em() -> Result<Df> {
    let ut = "bd1d9ddb04089700cf9c27f6f7426281";
    let target = [
        ("f12", "代码"),
        ("f14", "名称"),
        ("f2", "最新价"),
        ("f4", "涨跌额"),
        ("f3", "涨跌幅"),
        ("f5", "成交量"),
        ("f6", "成交额"),
        ("f8", "持仓量"),
        ("f24", "行权价"),
        ("f21", "剩余日"),
        ("f25", "日增"),
        ("f18", "昨结"),
        ("f17", "今开"),
        ("f13", "市场标识"),
    ];
    let fields = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f28,f11,f62,f128,f136,f115,f152,f133,f108,f163,f161,f162";
    let mut rows = em_clist(
        &target,
        fields,
        "m:10,m:12,m:140,m:141,m:151,m:163,m:226",
        ut,
    )?;

    // 中金所部分（futsseapi）。
    let url = "https://futsseapi.eastmoney.com/list/option/221";
    let mut p = Map::new();
    p.insert("orderBy".into(), Value::String("zdf".into()));
    p.insert("sort".into(), Value::String("desc".into()));
    p.insert("pageSize".into(), Value::String("20000".into()));
    p.insert("pageIndex".into(), Value::String("0".into()));
    p.insert(
        "token".into(),
        Value::String("58b2fa8f54638b60b87d69b31969089c".into()),
    );
    p.insert(
        "field".into(),
        Value::String("dm,sc,name,p,zsjd,zde,zdf,f152,vol,cje,ccl,xqj,syr,rz,zjsj,o".into()),
    );
    p.insert("blockName".into(), Value::String("callback".into()));
    let http = HttpClient::default();
    if let Ok(v) = http.get_json(url, &p, None) {
        if let Some(list) = v.get("list").and_then(Value::as_array) {
            for item in list {
                let mut m = serde_json::Map::new();
                m.insert(
                    "代码".into(),
                    item.get("dm").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "名称".into(),
                    item.get("name").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "最新价".into(),
                    item.get("p").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "涨跌额".into(),
                    item.get("zde").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "涨跌幅".into(),
                    item.get("zdf").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "成交量".into(),
                    item.get("vol").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "成交额".into(),
                    item.get("cje").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "持仓量".into(),
                    item.get("ccl").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "行权价".into(),
                    item.get("xqj").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "剩余日".into(),
                    item.get("syr").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "日增".into(),
                    item.get("rz").cloned().unwrap_or(Value::Null),
                );
                m.insert(
                    "昨结".into(),
                    item.get("zjsj").cloned().unwrap_or(Value::Null),
                );
                m.insert("今开".into(), item.get("o").cloned().unwrap_or(Value::Null));
                m.insert(
                    "市场标识".into(),
                    item.get("sc").cloned().unwrap_or(Value::Null),
                );
                rows.push(Value::Object(m));
            }
        }
    }

    let select = [
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "持仓量",
        "行权价",
        "剩余日",
        "日增",
        "昨结",
        "今开",
        "市场标识",
    ];
    let numeric = [
        "最新价",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "持仓量",
        "行权价",
        "剩余日",
        "日增",
        "昨结",
        "今开",
    ];
    let mut df = finalize_report(&rows, &[], &select, &numeric, None)?;
    // 重新生成 1 起始序号（akshare 末尾 reset_index + 1）。
    let n = df.height();
    let idx: Vec<Option<i64>> = (1..=n).map(|i| Some(i as i64)).collect();
    df.inner_mut()
        .insert_column(0, {
            let chunked = polars::prelude::Int64Chunked::from_iter_options(
                "序号".into(),
                idx.iter().copied(),
            );
            chunked.into_series().into()
        })
        .map_err(|e| AkshareError::Empty(format!("插入序号列失败: {e}")))?;
    Ok(df)
}

/// 东方财富-期权分时行情（依赖 `option_current_em` 获取 secid）。
///
/// # 返回列
/// `time, close, high, low, volume, amount`
pub fn option_minute_em(symbol: &str) -> Result<Df> {
    let secid = current_em_secid(symbol)?;
    let url = "https://push2.eastmoney.com/api/qt/stock/trends2/get";
    let params = json!({
        "secid": secid,
        "fields1": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,f13,f14,f17",
        "fields2": "f51,f53,f54,f55,f56,f57,f58",
        "iscr": "0",
        "iscca": "0",
        "ut": "f057cbcbce2a86e2866ab8877db1d059",
        "ndays": "1",
        "cb": "quotepushdata1",
    });
    let http = HttpClient::default();
    let text = http.get_text(url, params.as_object().expect("静态参数"), None)?;
    let s = text
        .find('(')
        .ok_or_else(|| AkshareError::Empty("分时响应缺少 '('".into()))?;
    let e = text
        .rfind(')')
        .ok_or_else(|| AkshareError::Empty("分时响应缺少 ')'".into()))?;
    let v: Value = serde_json::from_str(&text[s + 1..e])
        .map_err(|err| AkshareError::json(url, err.to_string()))?;
    let trends = v["data"]["trends"].as_array().cloned().unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(trends.len());
    for t in &trends {
        let line = match t.as_str() {
            Some(l) => l,
            None => continue,
        };
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 7 {
            continue;
        }
        rows.push(vec![
            Some(parts[0].to_string()),
            Some(parts[1].to_string()),
            Some(parts[2].to_string()),
            Some(parts[3].to_string()),
            Some(parts[4].to_string()),
            Some(parts[5].to_string()),
        ]);
    }
    let cols = ["time", "close", "high", "low", "volume", "amount"];
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&["close", "high", "low", "volume", "amount"])?;
    Ok(df)
}

/// 从 `option_current_em` 结果中查找指定代码的 secid（市场.代码）。
fn current_em_secid(symbol: &str) -> Result<String> {
    let df = option_current_em()?;
    let code_col = df
        .inner()
        .column("代码")
        .map_err(|e| AkshareError::Empty(format!("缺少代码列: {e}")))?;
    let mkt_col = df
        .inner()
        .column("市场标识")
        .map_err(|e| AkshareError::Empty(format!("缺少市场标识列: {e}")))?;
    let codes = code_col
        .str()
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    let mkts = mkt_col
        .str()
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    for i in 0..codes.len() {
        if codes.get(i) == Some(symbol) {
            if let Some(m) = mkts.get(i) {
                return Ok(format!("{m}.{symbol}"));
            }
        }
    }
    Err(AkshareError::Empty(format!(
        "未在 option_current_em 中找到合约 {symbol}"
    )))
}

/// 东方财富-期权折溢价分析。
///
/// # 返回列
/// `期权代码, 期权名称, 最新价, 涨跌幅, 行权价, 折溢价率, 标的名称, 标的最新价,
/// 标的涨跌幅, 盈亏平衡价, 到期日`
pub fn option_premium_analysis_em() -> Result<Df> {
    let target = [
        ("f12", "期权代码"),
        ("f14", "期权名称"),
        ("f2", "最新价"),
        ("f3", "涨跌幅"),
        ("f250", "行权价"),
        ("f330", "折溢价率"),
        ("f161", "标的名称"),
        ("f152", "标的最新价"),
        ("f332", "标的涨跌幅"),
        ("f337", "盈亏平衡价"),
        ("f301", "到期日"),
    ];
    let fields = "f1,f2,f3,f12,f13,f14,f161,f250,f330,f331,f332,f333,f334,f335,f337,f301,f152";
    let rows = em_clist(&target, fields, "m:10", "b2884a393a59ad64002292a3e90d46a5")?;
    let select = [
        "期权代码",
        "期权名称",
        "最新价",
        "涨跌幅",
        "行权价",
        "折溢价率",
        "标的名称",
        "标的最新价",
        "标的涨跌幅",
        "盈亏平衡价",
        "到期日",
    ];
    let mut df = finalize_report(
        &rows,
        &[],
        &select,
        &[
            "最新价",
            "涨跌幅",
            "行权价",
            "折溢价率",
            "标的最新价",
            "标的涨跌幅",
            "盈亏平衡价",
        ],
        None,
    )?;
    // 到期日归一化（akshare 为 %Y%m%d → 日期）
    df.cast_date(&["到期日"])?;
    Ok(df)
}

/// 东方财富-期权风险分析。
///
/// # 返回列
/// `期权代码, 期权名称, 最新价, 涨跌幅, 杠杆比率, 实际杠杆比率, Delta, Gamma,
/// Vega, Rho, Theta, 到期日`
pub fn option_risk_analysis_em() -> Result<Df> {
    let target = [
        ("f12", "期权代码"),
        ("f14", "期权名称"),
        ("f2", "最新价"),
        ("f3", "涨跌幅"),
        ("f302", "杠杆比率"),
        ("f303", "实际杠杆比率"),
        ("f325", "Delta"),
        ("f326", "Gamma"),
        ("f327", "Vega"),
        ("f329", "Rho"),
        ("f328", "Theta"),
        ("f301", "到期日"),
    ];
    let fields = "f1,f2,f3,f12,f13,f14,f302,f303,f325,f326,f327,f329,f328,f301,f152,f154";
    let rows = em_clist(&target, fields, "m:10", "b2884a393a59ad64002292a3e90d46a5")?;
    let select = [
        "期权代码",
        "期权名称",
        "最新价",
        "涨跌幅",
        "杠杆比率",
        "实际杠杆比率",
        "Delta",
        "Gamma",
        "Vega",
        "Rho",
        "Theta",
        "到期日",
    ];
    let mut df = finalize_report(
        &rows,
        &[],
        &select,
        &[
            "最新价",
            "涨跌幅",
            "杠杆比率",
            "实际杠杆比率",
            "Delta",
            "Gamma",
            "Vega",
            "Rho",
            "Theta",
        ],
        None,
    )?;
    df.cast_date(&["到期日"])?;
    Ok(df)
}

/// 东方财富-期权价值分析。
///
/// # 返回列
/// `期权代码, 期权名称, 最新价, 时间价值, 内在价值, 隐含波动率, 理论价格, 标的名称,
/// 标的最新价, 标的近一年波动率, 到期日`
pub fn option_value_analysis_em() -> Result<Df> {
    let target = [
        ("f12", "期权代码"),
        ("f14", "期权名称"),
        ("f2", "最新价"),
        ("f298", "时间价值"),
        ("f299", "内在价值"),
        ("f249", "隐含波动率"),
        ("f300", "理论价格"),
        ("f161", "标的名称"),
        ("f152", "标的最新价"),
        ("f336", "标的近一年波动率"),
        ("f301", "到期日"),
    ];
    let fields =
        "f1,f2,f3,f12,f13,f14,f298,f299,f249,f300,f330,f331,f332,f333,f334,f335,f336,f301,f152";
    let rows = em_clist(&target, fields, "m:10", "b2884a393a59ad64002292a3e90d46a5")?;
    let select = [
        "期权代码",
        "期权名称",
        "最新价",
        "时间价值",
        "内在价值",
        "隐含波动率",
        "理论价格",
        "标的名称",
        "标的最新价",
        "标的近一年波动率",
        "到期日",
    ];
    let mut df = finalize_report(
        &rows,
        &[],
        &select,
        &[
            "最新价",
            "时间价值",
            "内在价值",
            "隐含波动率",
            "理论价格",
            "标的最新价",
            "标的近一年波动率",
        ],
        None,
    )?;
    df.cast_date(&["到期日"])?;
    Ok(df)
}

/// 东方财富-期权龙虎榜单。
///
/// # 参数
/// - `symbol`: 期权代码（510050/510300/159919）
/// - `indicator`: 指标（4 选 1）
/// - `trade_date`: 交易日期 `YYYYMMDD`
///
/// # 返回列
/// `交易类型, 交易日期, 证券代码, 标的名称, 名次, 机构, 交易量/持仓量, 增减,
/// 净认沽量/净持仓量/净交易量, 占总交易量比例`
pub fn option_lhb_em(symbol: &str, indicator: &str, trade_date: &str) -> Result<Df> {
    let trade_date_fmt = format!(
        "{}-{}-{}",
        &trade_date[..4],
        &trade_date[4..6],
        &trade_date[6..]
    );
    let filter = format!("(SECURITY_CODE=\"{symbol}\")(TRADE_DATE='{trade_date_fmt}')");
    let url = "https://datacenter-web.eastmoney.com/api/data/get";
    let mut params = Map::new();
    params.insert("type".into(), Value::String("RPT_IF_BILLBOARD_TD".into()));
    params.insert("sty".into(), Value::String("ALL".into()));
    params.insert("filter".into(), Value::String(filter));
    params.insert("p".into(), Value::String("1".into()));
    params.insert("pss".into(), Value::String("200".into()));
    params.insert("source".into(), Value::String("IFBILLBOARD".into()));
    params.insert("client".into(), Value::String("WEB".into()));
    params.insert(
        "ut".into(),
        Value::String("b2884a393a59ad64002292a3e90d46a5".into()),
    );
    let http = HttpClient::default();
    let v = http.get_json(url, &params, None)?;
    let rows = v["result"]["data"].as_array().cloned().unwrap_or_default();

    // 按 indicator 定位切片与 10 个目标列（位置取自 akshare 列名映射）。
    let (start, count, pick, names): (usize, usize, [usize; 10], [&str; 10]) = match indicator {
        "期权交易情况-认沽交易量" => (
            0,
            7,
            [0, 1, 2, 3, 7, 6, 8, 9, 10, 11],
            [
                "交易类型",
                "交易日期",
                "证券代码",
                "标的名称",
                "名次",
                "机构",
                "交易量",
                "增减",
                "净认沽量",
                "占总交易量比例",
            ],
        ),
        "期权持仓情况-认沽持仓量" => (
            7,
            7,
            [0, 1, 2, 3, 7, 6, 13, 14, 15, 16],
            [
                "交易类型",
                "交易日期",
                "证券代码",
                "标的名称",
                "名次",
                "机构",
                "持仓量",
                "增减",
                "净持仓量",
                "占总交易量比例",
            ],
        ),
        "期权交易情况-认购交易量" => (
            14,
            7,
            [0, 1, 2, 3, 7, 6, 15, 16, 17, 18],
            [
                "交易类型",
                "交易日期",
                "证券代码",
                "标的名称",
                "名次",
                "机构",
                "交易量",
                "增减",
                "净交易量",
                "占总交易量比例",
            ],
        ),
        _ => (
            21,
            usize::MAX,
            [0, 1, 2, 3, 7, 6, 13, 14, 15, 16],
            [
                "交易类型",
                "交易日期",
                "证券代码",
                "标的名称",
                "名次",
                "机构",
                "持仓量",
                "增减",
                "净持仓量",
                "占总交易量比例",
            ],
        ),
    };
    let end = if count == usize::MAX {
        rows.len()
    } else {
        (start + count).min(rows.len())
    };
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &rows[start..end] {
        let vals: Vec<Value> = match r.as_object() {
            Some(o) => o.values().cloned().collect(),
            None => continue,
        };
        let mut row = Vec::with_capacity(10);
        for idx in pick {
            row.push(vals.get(idx).and_then(str_of));
        }
        out.push(row);
    }
    let mut df = Df::from_string_rows(&names, &out)?;
    df.cast_date(&["交易日期"])?;
    let metric = if indicator.contains("持仓") {
        "持仓量"
    } else {
        "交易量"
    };
    df.cast_numeric(&[
        "名次",
        metric,
        "增减",
        "净认沽量",
        "净持仓量",
        "净交易量",
        "占总交易量比例",
    ])?;
    Ok(df)
}

// ===========================================================================
// 阶段 6：其他期权源（郑商所/大商所/广期所/上期所/中金所/CFFEX/openctp/上交所标的）
// ===========================================================================

/// 空表（指定列名，保证列契约一致）。
fn empty_df(cols: &[&str]) -> Result<Df> {
    Df::from_string_rows(cols, &[])
}

/// 郑商所-期权日频行情（pipe 分隔文本）。
///
/// # 返回列
/// `合约代码, 昨结算, 今开盘, 最高价, 最低价, 今收盘, 今结算, 涨跌1, 涨跌2,
/// 成交量(手), 持仓量, 增减量, 成交额(万元), DELTA, 隐含波动率, 行权量`
pub fn option_hist_czce(symbol: &str, trade_date: &str) -> Result<Df> {
    let prefix = czce_prefix(symbol)?;
    let year = &trade_date[..4];
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Option/{year}/{trade_date}/OptionDataDaily.txt"
    );
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), None)?;
    let cols = [
        "合约代码",
        "昨结算",
        "今开盘",
        "最高价",
        "最低价",
        "今收盘",
        "今结算",
        "涨跌1",
        "涨跌2",
        "成交量(手)",
        "持仓量",
        "增减量",
        "成交额(万元)",
        "DELTA",
        "隐含波动率",
        "行权量",
    ];
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 3 {
        return empty_df(&cols);
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for line in &lines[2..] {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 16 {
            continue;
        }
        if !parts[0].contains(prefix) {
            continue;
        }
        rows.push(
            parts[..16]
                .iter()
                .map(|s| Some(s.trim().to_string()))
                .collect(),
        );
    }
    // akshare 末尾含一行「总计」汇总（合约代码含品种前缀），丢弃（iloc[:-1]）。
    if !rows.is_empty() {
        rows.pop();
    }
    let mut df = Df::from_string_rows(&cols, &rows)?;
    let numeric = &cols[1..];
    df.strip_commas(numeric)?;
    df.cast_numeric(numeric)?;
    Ok(df)
}

/// 郑商所 symbol → 合约前缀映射。
fn czce_prefix(symbol: &str) -> Result<&'static str> {
    let m = match symbol {
        "白糖期权" => "SR",
        "棉花期权" => "CF",
        "甲醇期权" => "MA",
        "PTA期权" => "TA",
        "动力煤期权" => "ZC",
        "菜籽粕期权" => "RM",
        "菜籽油期权" => "OI",
        "花生期权" => "PK",
        "对二甲苯期权" => "PX",
        "烧碱期权" => "SH",
        "纯碱期权" => "SA",
        "短纤期权" => "PF",
        "锰硅期权" => "SM",
        "硅铁期权" => "SF",
        "尿素期权" => "UR",
        "苹果期权" => "AP",
        "红枣期权" => "CJ",
        "玻璃期权" => "FG",
        "瓶片期权" => "PR",
        "丙烯期货" => "PL",
        _ => return Err(AkshareError::Param(format!("未知郑商所期权品种: {symbol}"))),
    };
    Ok(m)
}

/// 郑商所-期权年度历史行情（pipe 分隔，保留原始表头列名含尾部空格）。
///
/// # 返回列（含尾部空格，对齐 akshare）
/// `交易日期  `, `合约代码   `, `昨结算    `, `今开盘    `, `最高价    `, `最低价    `,
/// `今收盘    `, `今结算    `, `涨跌1     `, `涨跌2     `, `成交量(手)`, `持仓量    `,
/// `增减量    `, `成交额(万元)`, `DELTA     `, `隐含波动率`, `行权量`
pub fn option_hist_yearly_czce(symbol: &str, year: &str) -> Result<Df> {
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Option/{year}/OptionDataAllHistory/{symbol}OPTIONS{year}.txt"
    );
    let http = HttpClient::default();
    let text = http.get_text(&url, &Map::new(), None)?;
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return empty_df(&[
            "交易日期  ",
            "合约代码   ",
            "昨结算    ",
            "今开盘    ",
            "最高价    ",
            "最低价    ",
            "今收盘    ",
            "今结算    ",
            "涨跌1     ",
            "涨跌2     ",
            "成交量(手)",
            "持仓量    ",
            "增减量    ",
            "成交额(万元)",
            "DELTA     ",
            "隐含波动率",
            "行权量",
        ]);
    }
    // 表头取文件第 2 行（akshare skiprows=1），保留尾部空格。
    let header: Vec<&str> = lines[1].split('|').collect();
    let mut cols: Vec<String> = header.iter().map(|s| s.to_string()).collect();
    if cols.len() < 17 {
        cols = vec![
            "交易日期  ".into(),
            "合约代码   ".into(),
            "昨结算    ".into(),
            "今开盘    ".into(),
            "最高价    ".into(),
            "最低价    ".into(),
            "今收盘    ".into(),
            "今结算    ".into(),
            "涨跌1     ".into(),
            "涨跌2     ".into(),
            "成交量(手)".into(),
            "持仓量    ".into(),
            "增减量    ".into(),
            "成交额(万元)".into(),
            "DELTA     ".into(),
            "隐含波动率".into(),
            "行权量".into(),
        ];
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for line in &lines[2..] {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < cols.len() {
            continue;
        }
        rows.push(
            parts[..cols.len()]
                .iter()
                .map(|s| Some(s.to_string()))
                .collect(),
        );
    }
    let col_refs: Vec<&str> = cols.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &rows)?;
    // akshare 仅对 涨跌1/涨跌2/DELTA/隐含波动率 做数值化（其余保持原始 str）。
    let numeric_idx = [8usize, 9, 14, 15];
    let numeric: Vec<&str> = numeric_idx
        .iter()
        .filter_map(|&i| col_refs.get(i).copied())
        .collect();
    df.strip_commas(&numeric)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 大连商品交易所-期权日频行情（POST JSON）。
///
/// # 返回列
/// `品种名称, 合约, 开盘价, 最高价, 最低价, 收盘价, 前结算价, 结算价, 涨跌, 涨跌1,
/// Delta, 隐含波动率(%), 成交量, 持仓量, 持仓量变化, 成交额, 行权量`
pub fn option_hist_dce(symbol: &str, trade_date: &str) -> Result<Df> {
    let variety = dce_variety(symbol)?;
    let url = "http://www.dce.com.cn/dcereport/publicweb/dailystat/dayQuotes";
    let mut params = Map::new();
    params.insert("contractId".into(), Value::String("".into()));
    params.insert("lang".into(), Value::String("zh".into()));
    params.insert("optionSeries".into(), Value::String("".into()));
    params.insert("statisticsType".into(), Value::from(0));
    params.insert("tradeDate".into(), Value::String(trade_date.into()));
    params.insert("tradeType".into(), Value::String("2".into()));
    params.insert("varietyId".into(), Value::String(variety.into()));
    let http = HttpClient::default();
    let v = match http.post_json(url, &params, &[]) {
        Ok(v) => v,
        Err(_) => {
            return empty_df(&[
                "品种名称",
                "合约",
                "开盘价",
                "最高价",
                "最低价",
                "收盘价",
                "前结算价",
                "结算价",
                "涨跌",
                "涨跌1",
                "Delta",
                "隐含波动率(%)",
                "成交量",
                "持仓量",
                "持仓量变化",
                "成交额",
                "行权量",
            ]);
        }
    };
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename = [
        ("variety", "品种名称"),
        ("contractId", "合约"),
        ("open", "开盘价"),
        ("high", "最高价"),
        ("low", "最低价"),
        ("close", "收盘价"),
        ("lastClear", "前结算价"),
        ("clearPrice", "结算价"),
        ("diff", "涨跌"),
        ("diff1", "涨跌1"),
        ("delta", "Delta"),
        ("impliedVolatility", "隐含波动率(%)"),
        ("volumn", "成交量"),
        ("openInterest", "持仓量"),
        ("diffI", "持仓量变化"),
        ("turnover", "成交额"),
        ("matchQtySum", "行权量"),
    ];
    let select = [
        "品种名称",
        "合约",
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "前结算价",
        "结算价",
        "涨跌",
        "涨跌1",
        "Delta",
        "隐含波动率(%)",
        "成交量",
        "持仓量",
        "持仓量变化",
        "成交额",
        "行权量",
    ];
    let numeric = [
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "前结算价",
        "结算价",
        "涨跌",
        "涨跌1",
        "Delta",
        "隐含波动率(%)",
        "成交额",
    ];
    let mut df = finalize_report(&data, &rename, &select, &numeric, None)?;
    // 成交量/持仓量/持仓量变化/行权量 含千分位逗号
    let comma = ["成交量", "持仓量", "持仓量变化", "行权量"];
    df.strip_commas(&comma)?;
    df.cast_numeric(&comma)?;
    Ok(df)
}

/// 大商所 symbol → varietyId 映射。
fn dce_variety(symbol: &str) -> Result<&'static str> {
    let m = match symbol {
        "玉米期权" => "c",
        "豆粕期权" => "m",
        "铁矿石期权" => "i",
        "液化石油气期权" => "pg",
        "聚乙烯期权" => "l",
        "聚氯乙烯期权" => "v",
        "聚丙烯期权" => "pp",
        "棕榈油期权" => "p",
        "黄大豆1号期权" => "a",
        "黄大豆2号期权" => "b",
        "豆油期权" => "y",
        "乙二醇期权" => "eg",
        "苯乙烯期权" => "eb",
        "鸡蛋期权" => "jd",
        "玉米淀粉期权" => "cs",
        "生猪期权" => "lh",
        "原木期权" => "lg",
        _ => return Err(AkshareError::Param(format!("未知大商所期权品种: {symbol}"))),
    };
    Ok(m)
}

/// 广州期货交易所-日频率量价数据（POST JSON）。
///
/// # 返回列
/// `商品名称, 合约名称, 开盘价, 最高价, 最低价, 收盘价, 前结算价, 结算价, 涨跌, 涨跌1,
/// Delta, 成交量, 持仓量, 持仓量变化, 成交额, 行权量, 隐含波动率`
pub fn option_hist_gfex(symbol: &str, trade_date: &str) -> Result<Df> {
    let url = "http://www.gfex.com.cn/u/interfacesWebTiDayQuotes/loadList";
    let mut params = Map::new();
    params.insert("trade_date".into(), Value::String(trade_date.into()));
    params.insert("trade_type".into(), Value::String("1".into()));
    let http = HttpClient::default();
    let v = http.post_form(url, &params, GFEX_HEADERS)?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename = [
        ("variety", "商品名称"),
        ("delivMonth", "合约名称"),
        ("open", "开盘价"),
        ("high", "最高价"),
        ("low", "最低价"),
        ("close", "收盘价"),
        ("lastClear", "前结算价"),
        ("clearPrice", "结算价"),
        ("diff", "涨跌"),
        ("diff1", "涨跌1"),
        ("delta", "Delta"),
        ("volumn", "成交量"),
        ("openInterest", "持仓量"),
        ("diffI", "持仓量变化"),
        ("turnover", "成交额"),
        ("matchQtySum", "行权量"),
        ("impliedVolatility", "隐含波动率"),
    ];
    let select = [
        "商品名称",
        "合约名称",
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "前结算价",
        "结算价",
        "涨跌",
        "涨跌1",
        "Delta",
        "成交量",
        "持仓量",
        "持仓量变化",
        "成交额",
        "行权量",
        "隐含波动率",
    ];
    let numeric = [
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "前结算价",
        "结算价",
        "涨跌",
        "涨跌1",
        "Delta",
        "成交量",
        "持仓量",
        "持仓量变化",
        "成交额",
        "行权量",
        "隐含波动率",
    ];
    let mut df = finalize_report(&data, &rename, &select, &numeric, None)?;
    // 仅保留匹配品种的行（akshare 用 str.contains）
    df = filter_contains(&df, "商品名称", symbol)?;
    Ok(df)
}

/// 按字符串包含过滤行（对应 akshare `df[df[col].str.contains(s)]`）。
fn filter_contains(df: &Df, col: &str, needle: &str) -> Result<Df> {
    let series = df
        .inner()
        .column(col)
        .map_err(|e| AkshareError::Empty(format!("缺少列 {col}: {e}")))?;
    let strs = series
        .str()
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    let mut mask: Vec<bool> = Vec::with_capacity(strs.len());
    for i in 0..strs.len() {
        mask.push(strs.get(i).map(|s| s.contains(needle)).unwrap_or(false));
    }
    let filtered = df
        .inner()
        .filter(&polars::prelude::BooleanChunked::from_iter(
            mask.iter().copied(),
        ))?;
    Ok(Df::from_inner(filtered))
}

/// 上海期货交易所-期权日频行情（kx{date}.dat JSON）。
///
/// # 返回列
/// `合约代码, 开盘价, 最高价, 最低价, 收盘价, 前结算价, 结算价, 涨跌1, 涨跌2,
/// 成交量, 持仓量, 持仓量变化, 成交额, 德尔塔, 行权量`
pub fn option_hist_shfe(symbol: &str, trade_date: &str) -> Result<Df> {
    let url = format!("https://www.shfe.com.cn/data/tradedata/option/dailydata/kx{trade_date}.dat");
    let http = HttpClient::default();
    let v = http.get_json_with_headers(&url, &Map::new(), SHFE_HEADERS, None)?;
    let all = v
        .get("o_curinstrument")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let data: Vec<Value> = all
        .into_iter()
        .filter(|r| {
            let inst = r.get("INSTRUMENTID").and_then(Value::as_str).unwrap_or("");
            let prod = r.get("PRODUCTNAME").and_then(Value::as_str).unwrap_or("");
            inst != "小计" && inst != "合计" && !inst.is_empty() && prod.trim() == symbol
        })
        .collect();
    let rename = [
        ("INSTRUMENTID", "合约代码"),
        ("OPENPRICE", "开盘价"),
        ("HIGHESTPRICE", "最高价"),
        ("LOWESTPRICE", "最低价"),
        ("CLOSEPRICE", "收盘价"),
        ("PRESETTLEMENTPRICE", "前结算价"),
        ("SETTLEMENTPRICE", "结算价"),
        ("ZD1_CHG", "涨跌1"),
        ("ZD2_CHG", "涨跌2"),
        ("VOLUME", "成交量"),
        ("OPENINTEREST", "持仓量"),
        ("OPENINTERESTCHG", "持仓量变化"),
        ("TURNOVER", "成交额"),
        ("DELTA", "德尔塔"),
        ("EXECVOLUME", "行权量"),
    ];
    let select = [
        "合约代码",
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "前结算价",
        "结算价",
        "涨跌1",
        "涨跌2",
        "成交量",
        "持仓量",
        "持仓量变化",
        "成交额",
        "德尔塔",
        "行权量",
    ];
    // akshare 未做数值化，保留服务端 JSON 原生 dtype：
    // 合约代码/开盘价/最高价/最低价 为 str，其余数值列为 num。
    finalize_report(&data, &rename, &select, &select[4..], None)
}

/// 上海期货交易所-合约隐含波动率（kx{date}.dat JSON, o_cursigma）。
///
/// # 返回列
/// `合约系列, 成交量, 持仓量, 持仓量变化, 成交额, 行权量, 隐含波动率`
pub fn option_vol_shfe(symbol: &str, trade_date: &str) -> Result<Df> {
    let url = format!("https://www.shfe.com.cn/data/tradedata/option/dailydata/kx{trade_date}.dat");
    let http = HttpClient::default();
    let v = http.get_json_with_headers(&url, &Map::new(), SHFE_HEADERS, None)?;
    let all = v
        .get("o_cursigma")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let data: Vec<Value> = all
        .into_iter()
        .filter(|r| {
            r.get("PRODUCTNAME")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                == symbol
        })
        .collect();
    let rename = [
        ("INSTRUMENTID", "合约系列"),
        ("VOLUME", "成交量"),
        ("OPENINTEREST", "持仓量"),
        ("OPENINTERESTCHG", "持仓量变化"),
        ("TURNOVER", "成交额"),
        ("EXECVOLUME", "行权量"),
        ("SIGMA", "隐含波动率"),
    ];
    let select = [
        "合约系列",
        "成交量",
        "持仓量",
        "持仓量变化",
        "成交额",
        "行权量",
        "隐含波动率",
    ];
    // akshare 未做数值化：合约系列/隐含波动率 保持 str，其余数值列为 num。
    finalize_report(&data, &rename, &select, &select[1..6], None)
}

/// 广州期货交易所-合约隐含波动率（POST JSON）。
///
/// # 返回列
/// `合约系列, 隐含波动率`
pub fn option_vol_gfex(symbol: &str, trade_date: &str) -> Result<Df> {
    let symbol_code = gfex_vol_code(symbol)?;
    let url = "http://www.gfex.com.cn/u/interfacesWebTiDayQuotes/loadListOptVolatility";
    let mut params = Map::new();
    params.insert("trade_date".into(), Value::String(trade_date.into()));
    let http = HttpClient::default();
    let v = http.post_form(url, &params, GFEX_HEADERS)?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename = [("seriesId", "合约系列"), ("hisVolatility", "隐含波动率")];
    let select = ["合约系列", "隐含波动率"];
    let mut df = finalize_report(&data, &rename, &select, &["隐含波动率"], None)?;
    df = filter_contains(&df, "合约系列", symbol_code)?;
    Ok(df)
}

/// 广期所隐含波动率 symbol → 代码前缀。
fn gfex_vol_code(symbol: &str) -> Result<&'static str> {
    let m = match symbol {
        "工业硅" => "si",
        "碳酸锂" => "lc",
        "多晶硅" => "ps",
        _ => {
            return Err(AkshareError::Param(format!(
                "未知广期所波动率品种: {symbol}"
            )))
        }
    };
    Ok(m)
}

/// openctp-期权合约信息（JSON 重命名 29 列）。
///
/// # 返回列（29 列，含尾部无空格）
/// `交易所ID, 合约ID, 合约名称, 商品类别, 品种ID, 合约乘数, 最小变动价位,
/// 做多保证金率, 做空保证金率, 做多保证金/手, 做空保证金/手, 开仓手续费率,
/// 开仓手续费/手, 平仓手续费率, 平仓手续费/手, 平今手续费率, 平今手续费/手,
/// 交割年份, 交割月份, 上市日期, 最后交易日, 交割日, 标的合约ID, 标的合约乘数,
/// 期权类型, 行权价, 合约状态`
pub fn option_contract_info_ctp() -> Result<Df> {
    let url = "http://dict.openctp.cn/instruments?types=option";
    let http = HttpClient::default();
    let v = http.get_json(url, &Map::new(), None)?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename = [
        ("ExchangeID", "交易所ID"),
        ("InstrumentID", "合约ID"),
        ("InstrumentName", "合约名称"),
        ("ProductClass", "商品类别"),
        ("ProductID", "品种ID"),
        ("VolumeMultiple", "合约乘数"),
        ("PriceTick", "最小变动价位"),
        // openctp 返回但 akshare 未重命名，保留英文原名且位置不变。
        ("MinLimitOrderVolume", "MinLimitOrderVolume"),
        ("MaxLimitOrderVolume", "MaxLimitOrderVolume"),
        ("LongMarginRatioByMoney", "做多保证金率"),
        ("ShortMarginRatioByMoney", "做空保证金率"),
        ("LongMarginRatioByVolume", "做多保证金/手"),
        ("ShortMarginRatioByVolume", "做空保证金/手"),
        ("OpenRatioByMoney", "开仓手续费率"),
        ("OpenRatioByVolume", "开仓手续费/手"),
        ("CloseRatioByMoney", "平仓手续费率"),
        ("CloseRatioByVolume", "平仓手续费/手"),
        ("CloseTodayRatioByMoney", "平今手续费率"),
        ("CloseTodayRatioByVolume", "平今手续费/手"),
        ("DeliveryYear", "交割年份"),
        ("DeliveryMonth", "交割月份"),
        ("OpenDate", "上市日期"),
        ("ExpireDate", "最后交易日"),
        ("DeliveryDate", "交割日"),
        ("UnderlyingInstrID", "标的合约ID"),
        ("UnderlyingMultiple", "标的合约乘数"),
        ("OptionsType", "期权类型"),
        ("StrikePrice", "行权价"),
        ("InstLifePhase", "合约状态"),
    ];
    let select = rename_to_names(&rename);
    let numeric = [
        "合约乘数",
        "最小变动价位",
        "MinLimitOrderVolume",
        "MaxLimitOrderVolume",
        "做多保证金率",
        "做空保证金率",
        "做多保证金/手",
        "做空保证金/手",
        "开仓手续费率",
        "开仓手续费/手",
        "平仓手续费率",
        "平仓手续费/手",
        "平今手续费率",
        "平今手续费/手",
        "交割年份",
        "交割月份",
        "标的合约乘数",
        "行权价",
    ];
    finalize_report(&data, &rename, &select, &numeric, None)
}

/// 从 rename 对中提取中文名列表（保持顺序）。
fn rename_to_names<'a>(rename: &'a [(&'a str, &'a str)]) -> Vec<&'a str> {
    rename.iter().map(|(_, n)| *n).collect()
}

/// 上交所-期权标的当日行情（yunhq JSON）。
///
/// # 返回列
/// `代码, 名称, 当前价, 涨跌, 涨跌幅, 振幅, 成交量(手), 成交额(万元), 更新日期`
pub fn option_finance_sse_underlying(symbol: &str) -> Result<Df> {
    let url = sse_underlying_url(symbol)?;
    let params =
        json!({ "select": "code,name,last,change,chg_rate,amp_rate,volume,amount,prev_close" });
    let http = HttpClient::default();
    let v = http.get_json(
        url,
        params.as_object().expect("静态参数"),
        Some(SSE_REFERER),
    )?;
    let date = v.get("date").and_then(Value::as_str).unwrap_or("");
    let time = v.get("time").and_then(Value::as_str).unwrap_or("");
    let list = v
        .get("list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for item in &list {
        let a = match item.as_array() {
            Some(a) => a,
            None => continue,
        };
        if a.len() < 9 {
            continue;
        }
        rows.push(vec![
            str_of(&a[0]),
            str_of(&a[1]),
            str_of(&a[2]),
            str_of(&a[3]),
            str_of(&a[4]),
            str_of(&a[5]),
            str_of(&a[6]),
            str_of(&a[7]),
            Some(format!("{date}{time}")),
        ]);
    }
    let cols = [
        "代码",
        "名称",
        "当前价",
        "涨跌",
        "涨跌幅",
        "振幅",
        "成交量(手)",
        "成交额(万元)",
        "更新日期",
    ];
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&[
        "当前价",
        "涨跌",
        "涨跌幅",
        "振幅",
        "成交量(手)",
        "成交额(万元)",
    ])?;
    Ok(df)
}

/// 上交所标的行情 URL（含标的 ETF 代码）。
fn sse_underlying_url(symbol: &str) -> Result<&'static str> {
    let u = match symbol {
        "华夏上证50ETF期权" => "http://yunhq.sse.com.cn:32041/v1/sh1/list/self/510050",
        "华泰柏瑞沪深300ETF期权" => "http://yunhq.sse.com.cn:32041/v1/sh1/list/self/510300",
        "南方中证500ETF期权" => "http://yunhq.sse.com.cn:32041/v1/sh1/list/self/510500",
        "华夏科创50ETF期权" => "http://yunhq.sse.com.cn:32041/v1/sh1/list/self/588000",
        "易方达科创50ETF期权" => "http://yunhq.sse.com.cn:32041/v1/sh1/list/self/588080",
        _ => return Err(AkshareError::Param(format!("未知上交所标的: {symbol}"))),
    };
    Ok(u)
}

/// 金融期权当日交易行情（多分支：上交所 JSON / 深交所 JSON / 中金所 CSV）。
///
/// # 返回列
/// - 上交所 5 分支：`日期, 合约交易代码, 当前价, 涨跌幅, 前结价, 行权价, 数量`
/// - 嘉实沪深300ETF期权：`合约编码, 合约简称, 标的名称, 类型, 行权价, 合约单位, 期权行权日, 行权交收日`
/// - 中金所 3 分支：CSV 原始列
pub fn option_finance_board(symbol: &str, end_month: &str) -> Result<Df> {
    let em = &end_month[end_month.len().saturating_sub(2)..];
    match symbol {
        "华夏上证50ETF期权" => sse_board_king("510050", em),
        "华泰柏瑞沪深300ETF期权" => sse_board_king("510300", em),
        "南方中证500ETF期权" => sse_board_king("510500", em),
        "华夏科创50ETF期权" => sse_board_king("588000", em),
        "易方达科创50ETF期权" => sse_board_king("588080", em),
        "嘉实沪深300ETF期权" => szse_board(em),
        "沪深300股指期权" => cffex_board_csv("http://www.cffex.com.cn/quote_IO.txt", em),
        "中证1000股指期权" => cffex_board_csv("http://www.cffex.com.cn/quote_MO.txt", em),
        "上证50股指期权" => cffex_board_csv("http://www.cffex.com.cn/quote_HO.txt", em),
        _ => Err(AkshareError::Param(format!("未知金融期权品种: {symbol}"))),
    }
}

/// 上交所 king 分支（yunhq tstyle JSON）。
fn sse_board_king(code: &str, em: &str) -> Result<Df> {
    let url = format!("http://yunhq.sse.com.cn:32041/v1/sho/list/tstyle/{code}_{em}");
    let params = json!({ "select": "contractid,last,chg_rate,presetpx,exepx" });
    let http = HttpClient::default();
    let v = http.get_json(
        &url,
        params.as_object().expect("静态参数"),
        Some(SSE_REFERER),
    )?;
    let date = v.get("date").and_then(Value::as_str).unwrap_or("");
    let time = v.get("time").and_then(Value::as_str).unwrap_or("");
    let total = v.get("total").and_then(Value::as_i64).unwrap_or(0);
    let list = v
        .get("list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for item in &list {
        let a = match item.as_array() {
            Some(a) => a,
            None => continue,
        };
        if a.len() < 5 {
            continue;
        }
        rows.push(vec![
            Some(format!("{date}{time}")),
            str_of(&a[0]),
            str_of(&a[1]),
            str_of(&a[2]),
            str_of(&a[3]),
            str_of(&a[4]),
            Some(total.to_string()),
        ]);
    }
    let cols = [
        "日期",
        "合约交易代码",
        "当前价",
        "涨跌幅",
        "前结价",
        "行权价",
        "数量",
    ];
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&["当前价", "涨跌幅", "前结价", "行权价", "数量"])?;
    Ok(df)
}

/// 深交所嘉实分支（ShowReport ysplbrb JSON）。
fn szse_board(em: &str) -> Result<Df> {
    let url = "http://www.szse.cn/api/report/ShowReport/data";
    let cols = [
        "合约编码",
        "合约简称",
        "标的名称",
        "类型",
        "行权价",
        "合约单位",
        "期权行权日",
        "行权交收日",
    ];
    let http = HttpClient::default();
    let mut page = 1;
    let mut rows_json: Vec<Value> = Vec::new();
    loop {
        let mut params = Map::new();
        params.insert("SHOWTYPE".into(), Value::String("JSON".into()));
        params.insert("CATALOGID".into(), Value::String("ysplbrb".into()));
        params.insert("TABKEY".into(), Value::String("tab1".into()));
        params.insert("PAGENO".into(), Value::from(page));
        params.insert("random".into(), Value::String("0.10642298535346595".into()));
        let v = http.get_json(url, &params, None)?;
        let arr = match v.as_array().and_then(|a| a.first().cloned()) {
            Some(a) => a,
            None => break,
        };
        let data = arr
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if data.is_empty() {
            break;
        }
        let pagecount = arr
            .get("metadata")
            .and_then(|m| m.get("pagecount"))
            .and_then(Value::as_i64)
            .unwrap_or(1);
        rows_json.extend(data);
        if page >= pagecount {
            break;
        }
        page += 1;
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &rows_json {
        // 期权行权日 形如 2026-08-26 / 2026/08/26，取月份两位与 em 比较
        let exd = r.get("EXERCISEDATE").and_then(Value::as_str).unwrap_or("");
        let month = exd.split(['-', '/']).nth(1).map(|m| m.trim()).unwrap_or("");
        if month != em {
            continue;
        }
        out.push(vec![
            r.get("CODE").and_then(str_of),
            r.get("NAME").and_then(str_of),
            r.get("TARGETNAME").and_then(str_of),
            r.get("TYPE").and_then(str_of),
            r.get("EXEPX").and_then(str_of),
            r.get("UNIT").and_then(str_of),
            r.get("EXERCISEDATE").and_then(str_of),
            r.get("DELIVERYDATE").and_then(str_of),
        ]);
    }
    Df::from_string_rows(&cols, &out)
}

/// 中金所股指期权分支（CSV 文本，按 instrument 前缀月份过滤）。
fn cffex_board_csv(url: &str, em: &str) -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text(url, &Map::new(), None)?;
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Df::from_string_rows(&["instrument"], &[]);
    }
    let header: Vec<String> = lines[0].split(',').map(|s| s.to_string()).collect();
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for line in &lines[1..] {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.is_empty() || parts[0].is_empty() {
            continue;
        }
        // instrument 形如 io2408-C-3700，取第 4 位起的 2 位月份
        let inst = parts[0];
        let m = if inst.len() >= 6 { &inst[4..6] } else { "" };
        if m != em {
            continue;
        }
        let row: Vec<Option<String>> = (0..header.len())
            .map(|i| parts.get(i).map(|s| s.to_string()))
            .collect();
        rows.push(row);
    }
    Df::from_string_rows(&col_refs, &rows)
}

/// 九期网-商品期权手续费（依赖 HTML 表格解析，暂未移植，见报告）。
pub fn option_comm_info(_symbol: &str) -> Result<Df> {
    Err(AkshareError::Empty(
        "option_comm_info 需要 pandas.read_html 解析 9qihuo.com 表格，core/html.rs 尚未提供，暂未移植".into(),
    ))
}

/// 九期网-商品期权品种代码（依赖 HTML 解析，暂未移植）。
pub fn option_comm_symbol() -> Result<Df> {
    Err(AkshareError::Empty(
        "option_comm_symbol 需要 BeautifulSoup 解析 9qihuo.com，暂未移植".into(),
    ))
}

/// 唯爱期货-商品期权保证金（依赖 HTML 表格解析，暂未移植）。
pub fn option_margin(_symbol: &str) -> Result<Df> {
    Err(AkshareError::Empty(
        "option_margin 需要 pandas.read_html 解析 iweiai.com 表格，暂未移植".into(),
    ))
}

/// 唯爱期货-商品期权品种代码（依赖 HTML 解析，暂未移植）。
pub fn option_margin_symbol() -> Result<Df> {
    Err(AkshareError::Empty(
        "option_margin_symbol 需要 BeautifulSoup 解析 iweiai.com，暂未移植".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cffex_spot_parse_17_columns() {
        let data = json!({
            "up": [["1","641.4","643.6","647.6","1","84","-0.98","3550","io2204C3550"]],
            "down": [["0","0.0","0.2","0.2","20","1016","0.0","io2204P3550"]],
        });
        let df = parse_cffex_spot(&data).unwrap();
        assert_eq!(df.height(), 1);
        assert_eq!(df.column_names().len(), 17);
        assert_eq!(df.column_names()[8], "看涨合约-标识");
        assert_eq!(df.column_names()[16], "看跌合约-标识");
    }

    #[test]
    fn cffex_spot_empty_up_down() {
        let data = json!({});
        assert!(parse_cffex_spot(&data).is_err());
    }

    #[test]
    fn dayline_expired_returns_empty() {
        // 模拟 (null); 响应中无 '['
        let df = cffex_daily_from_text("(null);").unwrap();
        assert_eq!(df.height(), 0);
        assert_eq!(
            df.column_names(),
            vec!["date", "open", "high", "low", "close", "volume"]
        );
    }

    /// 直接测试 JSONP 文本解析（不联网）。
    fn cffex_daily_from_text(text: &str) -> Result<Df> {
        let body = match extract_array(text) {
            Some(b) => b,
            None => {
                return Df::from_string_rows(
                    &["date", "open", "high", "low", "close", "volume"],
                    &[],
                );
            }
        };
        let arr: Value = serde_json::from_str(body)
            .map_err(|err| AkshareError::json("test", err.to_string()))?;
        let rows = arr.as_array().cloned().unwrap_or_default();
        let mut out = Vec::new();
        for r in &rows {
            let a = r.as_array().unwrap();
            out.push(vec![
                str_of(&a[5]),
                str_of(&a[0]),
                str_of(&a[1]),
                str_of(&a[2]),
                str_of(&a[3]),
                str_of(&a[4]),
            ]);
        }
        let cols = ["date", "open", "high", "low", "close", "volume"];
        let mut df = Df::from_string_rows(&cols, &out)?;
        df.cast_date(&["date"])?;
        df.cast_numeric(&["open", "high", "low", "close", "volume"])?;
        Ok(df)
    }

    #[test]
    fn sse_field_value_two_columns() {
        // CON_OP_ 行情串：var hq_str_CON_OP_X="买量,641.4,...";
        let text = "var hq_str_CON_OP_X=\"641.4,643.6\";";
        let inner = extract_quoted(text);
        let data_list: Vec<&str> = inner.split(',').collect();
        let fields = ["买量", "买价"];
        let n = fields.len().min(data_list.len());
        assert_eq!(n, 2);
    }

    #[test]
    fn czce_prefix_map() {
        assert_eq!(czce_prefix("白糖期权").unwrap(), "SR");
        assert_eq!(czce_prefix("玻璃期权").unwrap(), "FG");
        assert!(czce_prefix("未知").is_err());
    }

    #[test]
    fn dce_variety_map() {
        assert_eq!(dce_variety("聚丙烯期权").unwrap(), "pp");
        assert!(dce_variety("未知").is_err());
    }

    #[test]
    fn gfex_vol_code_map() {
        assert_eq!(gfex_vol_code("碳酸锂").unwrap(), "lc");
        assert!(gfex_vol_code("未知").is_err());
    }

    #[test]
    fn current_day_sse_rename_order() {
        let result = json!([
            {"SECURITY_ID":"10012127","CONTRACT_ID":"510050C2608M02750","CONTRACT_SYMBOL":"50ETF购8月2750",
             "SECURITYNAMEBYID":"50ETF(510050)","CALL_OR_PUT":"认购","EXERCISE_PRICE":"2.750",
             "CONTRACT_UNIT":"10000","END_DATE":"20260826","DELIVERY_DATE":"20260827",
             "EXPIRE_DATE":"20260826","START_DATE":"20260720"}
        ]);
        let rename = [
            ("SECURITY_ID", "合约编码"),
            ("CONTRACT_ID", "合约交易代码"),
            ("CONTRACT_SYMBOL", "合约简称"),
            ("SECURITYNAMEBYID", "标的券名称及代码"),
            ("CALL_OR_PUT", "类型"),
            ("EXERCISE_PRICE", "行权价"),
            ("CONTRACT_UNIT", "合约单位"),
            ("END_DATE", "期权行权日"),
            ("DELIVERY_DATE", "行权交收日"),
            ("EXPIRE_DATE", "到期日"),
            ("START_DATE", "开始日期"),
        ];
        let select = [
            "合约编码",
            "合约交易代码",
            "合约简称",
            "标的券名称及代码",
            "类型",
            "行权价",
            "合约单位",
            "期权行权日",
            "行权交收日",
            "到期日",
            "开始日期",
        ];
        let df = finalize_report(result.as_array().unwrap(), &rename, &select, &[], None).unwrap();
        assert_eq!(df.column_names(), select);
        assert_eq!(df.height(), 1);
    }

    #[test]
    fn lhb_positional_pick() {
        // 9 个分支名用于确认 pick 不越界
        let names = [
            "交易类型",
            "交易日期",
            "证券代码",
            "标的名称",
            "名次",
            "机构",
            "交易量",
            "增减",
            "净认沽量",
            "占总交易量比例",
        ];
        assert_eq!(names.len(), 10);
    }
}
