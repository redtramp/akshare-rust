//! 期货数据（对应 akshare `futures/` 目录）。
//!
//! 首批实现：五家期货交易所结算参数（对应 akshare `futures/futures_settle.py`）：
//! - [`futures_settle_cffex`]：中金所（CSV，GBK）
//! - [`futures_settle_czce`]：郑商所（管道符分隔 txt）
//! - [`futures_settle_gfex`]：广期所（POST JSON）
//! - [`futures_settle_shfe`]：上期所（JSON）
//! - [`futures_settle_ine`]：上能中心（JSON）
//!
//! 大商所（DCE）因网站反爬保护（412）暂缓，与 akshare 上游状态一致。
//! 各接口均为「指定日期 → 该交易所全部期货合约的保证金/手续费/涨跌停参数」，
//! 失败（无此日期数据 / 页面不存在）时返回空表（对应 akshare `pd.DataFrame()`）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{Map, Value};

/// 将「请求失败」映射为 akshare 语义的结果：
/// 非 2xx（页面不存在/数据未发布）→ 空表（akshare `if status != 200: return pd.DataFrame()`）；
/// 传输/反爬/登录态错误 → 如实上报（akshare 对连接异常直接抛错，项目 §2.1.4 要求拦截时明确失败）。
fn or_empty<T>(r: Result<T>) -> Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(AkshareError::Status { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// akshare 全局 UA（`futures_settle.py` 使用 `akshare.utils.cons.headers`）。
const UA_CHROME: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";
/// 上期所/上能中心要求的老旧 UA（对应 akshare `cons.shfe_headers`）。
const UA_MSIE: &str = "Mozilla/4.0 (compatible; MSIE 5.5; Windows NT)";
/// 广期所 POST 请求头（对应 akshare `gfex_headers`）。
const GFEX_HEADERS: [(&str, &str); 6] = [
    ("Accept", "application/json, text/javascript, */*; q=0.01"),
    ("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8"),
    ("Origin", "http://www.gfex.com.cn"),
    ("Referer", "http://www.gfex.com.cn/gfex/rjycs/ywcs.shtml"),
    ("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"),
    ("X-Requested-With", "XMLHttpRequest"),
];

/// 空表（0 列 0 行，对应 akshare 数据缺失时的 `pd.DataFrame()`）。
fn empty_df() -> Result<Df> {
    Df::from_json_rows(&[])
}

/// 日期归一化为 `YYYYMMDD`（对应 akshare `cons.convert_date` 支持的三种写法）。
///
/// 非法日期返回 [`AkshareError::Param`]（akshare 对非法日期 `convert_date` 返回 None
/// 后 `.strftime` 抛 AttributeError，同样是失败语义；Rust 侧显式报错更清晰）。
fn convert_date(s: &str) -> Result<String> {
    let s = s.trim();
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 8 && (s.len() == 8 || s.len() == 10) {
        let m = digits[4..6].parse::<u32>().unwrap_or(0);
        let d = digits[6..8].parse::<u32>().unwrap_or(0);
        if (1..=12).contains(&m) && (1..=31).contains(&d) {
            return Ok(digits);
        }
    }
    Err(AkshareError::Param(format!(
        "无效日期: {s}（应为 YYYYMMDD / YYYY-MM-DD / YYYY/MM/DD）"
    )))
}

/// 提取合约品种（对应 akshare `symbol.str.extract(r"([A-Za-z]+)")`）。
fn variety_of(symbol: &str) -> String {
    symbol
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect()
}

/// JSON 值 → Option<String>（数值走 `to_string`，与 akshare `pd.DataFrame` 后逐单元格 str 一致）。
fn cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// 中金所（CFFEX）：CSV，GBK，跳过标题行
// ---------------------------------------------------------------------------

const CFFEX_COLS: [&str; 8] = [
    "date",
    "symbol",
    "variety",
    "long_margin_ratio",
    "short_margin_ratio",
    "trade_fee_ratio",
    "delivery_fee_ratio",
    "close_today_fee_ratio",
];

/// 中国金融期货交易所-结算参数（对应 akshare [`akshare.futures_settle_cffex`]）。
///
/// `date`: `YYYYMMDD` / `YYYY-MM-DD` / `YYYY/MM/DD`（必填；缺省数据对应 akshare
/// 默认日期 20260119）。
/// 数据源 `http://www.cffex.com.cn/sj/jscs/{YYYYMM}/{DD}/{date}_1.csv`。
///
/// # 返回列
/// `date, symbol, variety, long_margin_ratio, short_margin_ratio, trade_fee_ratio,
/// delivery_fee_ratio, close_today_fee_ratio`
pub fn futures_settle_cffex(date: &str) -> Result<Df> {
    let d = convert_date(date)?;
    let url = format!(
        "http://www.cffex.com.cn/sj/jscs/{}/{}/{}_1.csv",
        &d[..6],
        &d[6..8],
        d
    );
    let http = HttpClient::default();
    let text = match or_empty(
        http.get_text_with_headers(&url, &Map::new(), &[("User-Agent", UA_CHROME)], None),
    )? {
        Some(t) => t,
        None => return empty_df(),
    };
    parse_cffex(&text, &d)
}

fn parse_cffex(text: &str, date: &str) -> Result<Df> {
    let t = text.trim_start();
    // 页面不存在 / 数据未发布：返回 HTML 错误页 → 空表
    if t.starts_with('<') || t.contains("要查看的页面不存在") {
        return empty_df();
    }
    // 固定跳过前两行（表名 + 表头，对应 akshare `read_csv(skiprows=1)` 后位置重命名）。
    // 注意：若上游 CSV 格式变化（如取消表名行），首行数据会被静默丢弃——当前实测稳定，
    // 字段无引号/内嵌逗号，手动 split(',') 与 pandas read_csv 等价。
    let mut lines = t.lines();
    let _ = lines.next(); // 跳过表名行（如「期货合约结算业务参数表（20260119）」）
    let _ = lines.next(); // 跳过表头行（中文列名，akshare 用 skiprows=1 + 位置重命名）
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').map(|x| x.trim()).collect();
        if f.len() < 6 {
            continue;
        }
        // 仅保留 `^[A-Z]+` 开头的合约行（对应 akshare `str.contains(r"^[A-Z]+")`）
        if !f[0].chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            continue;
        }
        rows.push(vec![
            Some(date.to_string()),
            Some(f[0].to_string()),
            Some(variety_of(f[0])),
            Some(f[1].to_string()),
            Some(f[2].to_string()),
            Some(f[3].to_string()),
            Some(f[4].to_string()),
            Some(f[5].to_string()),
        ]);
    }
    if rows.len() < 5 {
        return empty_df();
    }
    Df::from_string_rows(&CFFEX_COLS, &rows)
}

// ---------------------------------------------------------------------------
// 郑商所（CZCE）：管道符分隔 txt
// ---------------------------------------------------------------------------

const CZCE_COLS: [&str; 14] = [
    "date",
    "symbol",
    "variety",
    "settle_price",
    "is_single_market",
    "single_market_days",
    "margin_ratio",
    "limit_ratio",
    "trade_fee",
    "fee_type",
    "delivery_fee",
    "close_today_fee",
    "position_limit",
    "trade_limit",
];

/// 郑州商品交易所-结算参数（对应 akshare [`akshare.futures_settle_czce`]）。
///
/// `date`: 格式同 [`futures_settle_cffex`]（必填）。数据源
/// `http://www.czce.com.cn/cn/DFSStaticFiles/Future/{YYYY}/{date}/FutureDataClearParams.txt`。
///
/// # 返回列
/// `date, symbol, variety, settle_price, is_single_market, single_market_days, margin_ratio,
/// limit_ratio, trade_fee, fee_type, delivery_fee, close_today_fee, position_limit, trade_limit`
pub fn futures_settle_czce(date: &str) -> Result<Df> {
    let d = convert_date(date)?;
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Future/{}/{}/FutureDataClearParams.txt",
        &d[..4],
        d
    );
    let http = HttpClient::default();
    let text = match or_empty(
        http.get_text_with_headers(&url, &Map::new(), &[("User-Agent", UA_CHROME)], None),
    )? {
        Some(t) => t,
        None => return empty_df(),
    };
    parse_czce(&text, &d)
}

fn parse_czce(text: &str, date: &str) -> Result<Df> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return empty_df();
    }
    // 第 0 行是表名，第 1 行是表头（管道符分隔），之后是数据（对应 akshare `_parse_pipe_data`）
    let ncols = lines[1].split('|').count();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for line in &lines[2..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('|').map(|x| x.trim()).collect();
        if f.len() < ncols {
            continue;
        }
        // 过滤合计/小计/总计行（对应 akshare `~str.contains("小计|合计|总计")`）
        if f[0].is_empty()
            || f[0].contains("小计")
            || f[0].contains("合计")
            || f[0].contains("总计")
        {
            continue;
        }
        let mut row = vec![
            Some(date.to_string()),
            Some(f[0].to_string()),
            Some(variety_of(f[0])),
        ];
        // 剩余字段即 akshare 的 11 个参数字段（第 0 个 symbol 已前置）
        for x in f.iter().skip(1).take(ncols - 1) {
            row.push(Some(x.to_string()));
        }
        rows.push(row);
    }
    if rows.len() < 5 {
        return empty_df();
    }
    Df::from_string_rows(&CZCE_COLS, &rows)
}

// ---------------------------------------------------------------------------
// 广期所（GFEX）：POST JSON
// ---------------------------------------------------------------------------

const GFEX_COLS: [&str; 13] = [
    "date",
    "symbol",
    "variety",
    "spec_buy_rate",
    "spec_buy",
    "hedge_buy_rate",
    "hedge_buy",
    "rise_limit_rate",
    "rise_limit",
    "fall_limit",
    "agent_tot_buy_posi_quota",
    "self_tot_buy_posi_quota",
    "client_buy_posi_quota",
];

const GFEX_NUMERIC: [&str; 10] = [
    "spec_buy_rate",
    "spec_buy",
    "hedge_buy_rate",
    "hedge_buy",
    "rise_limit_rate",
    "rise_limit",
    "fall_limit",
    "agent_tot_buy_posi_quota",
    "self_tot_buy_posi_quota",
    "client_buy_posi_quota",
];

/// 广州期货交易所-结算参数（对应 akshare [`akshare.futures_settle_gfex`]）。
///
/// `date`: 格式同 [`futures_settle_cffex`]（必填）。数据源
/// `http://www.gfex.com.cn/u/interfacesWebTtQueryTradPara/loadDayList`
/// （POST `trade_type=0`，仅保留期货合约，过滤期权 `-` 合约）。
///
/// # 返回列
/// `date, symbol, variety, spec_buy_rate, spec_buy, hedge_buy_rate, hedge_buy,
/// rise_limit_rate, rise_limit, fall_limit, agent_tot_buy_posi_quota,
/// self_tot_buy_posi_quota, client_buy_posi_quota`
pub fn futures_settle_gfex(date: &str) -> Result<Df> {
    let d = convert_date(date)?;
    let url = "http://www.gfex.com.cn/u/interfacesWebTtQueryTradPara/loadDayList";
    let mut params = Map::new();
    params.insert("trade_type".into(), Value::String("0".into()));
    let http = HttpClient::default();
    // 广期所要求表单体（裸 query 参数会因无 Content-Length 被拒 411）
    let json = match or_empty(http.post_form(url, &params, &GFEX_HEADERS))? {
        Some(j) => j,
        None => return empty_df(),
    };
    parse_gfex(&json, &d)
}

fn parse_gfex(json: &Value, date: &str) -> Result<Df> {
    if json.get("code").and_then(Value::as_str) != Some("0") {
        return empty_df();
    }
    let items = json
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return empty_df();
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for item in &items {
        let Some(symbol) = item.get("contractId").and_then(Value::as_str) else {
            continue;
        };
        if symbol.contains('-') {
            continue; // 过滤期权合约
        }
        let mut row = vec![
            Some(date.to_string()),
            Some(symbol.to_string()),
            Some(variety_of(symbol)),
        ];
        for key in [
            "specBuyRate",
            "specBuy",
            "hedgeBuyRate",
            "hedgeBuy",
            "riseLimitRate",
            "riseLimit",
            "fallLimit",
            "agentTotBuyPosiQuota",
            "selfTotBuyPosiQuota",
            "clientBuyPosiQuota",
        ] {
            row.push(item.get(key).and_then(cell));
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return empty_df();
    }
    let mut df = Df::from_string_rows(&GFEX_COLS, &rows)?;
    df.cast_numeric(&GFEX_NUMERIC)?;
    Ok(df)
}

// ---------------------------------------------------------------------------
// 上期所（SHFE）/ 上能中心（INE）：JSON o_cursor
// ---------------------------------------------------------------------------

const SHFE_COLS: [&str; 11] = [
    "date",
    "symbol",
    "variety",
    "settle_price",
    "spec_long_margin_ratio",
    "hedge_long_margin_ratio",
    "spec_short_margin_ratio",
    "hedge_short_margin_ratio",
    "trade_fee_ratio",
    "close_today_fee_ratio",
    "is_close_today",
];

const SHFE_NUMERIC: [&str; 8] = [
    "settle_price",
    "spec_long_margin_ratio",
    "hedge_long_margin_ratio",
    "spec_short_margin_ratio",
    "hedge_short_margin_ratio",
    "trade_fee_ratio",
    "close_today_fee_ratio",
    "is_close_today",
];

/// 解析 SHFE/INE 的 `js{date}.dat` JSON（两站结构一致，仅 host 不同）。
fn parse_shfe_ine(json: &Value, date: &str) -> Result<Df> {
    let items = json
        .get("o_cursor")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return empty_df();
    }
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for item in &items {
        let Some(symbol) = item.get("INSTRUMENTID").and_then(Value::as_str) else {
            continue;
        };
        let mut row = vec![
            Some(date.to_string()),
            Some(symbol.to_string()),
            Some(variety_of(symbol)),
        ];
        for key in [
            "SETTLEMENTPRICE",
            "SPECLONGMARGINRATIO",
            "HEDGLONGMARGINRATIO",
            "SPECSHORTMARGINRATIO",
            "HEDGSHORTMARGINRATIO",
            "TRADEFEERATIO",
            "TTRADEFEERATIO",
            "ISUNITODAY",
        ] {
            row.push(item.get(key).and_then(cell));
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return empty_df();
    }
    let mut df = Df::from_string_rows(&SHFE_COLS, &rows)?;
    df.cast_numeric(&SHFE_NUMERIC)?;
    Ok(df)
}

/// 上海期货交易所-结算参数（对应 akshare [`akshare.futures_settle_shfe`]）。
///
/// `date`: 格式同 [`futures_settle_cffex`]（必填）。数据源
/// `https://www.shfe.com.cn/data/tradedata/future/dailydata/js{date}.dat`。
///
/// # 返回列
/// `date, symbol, variety, settle_price, spec_long_margin_ratio, hedge_long_margin_ratio,
/// spec_short_margin_ratio, hedge_short_margin_ratio, trade_fee_ratio,
/// close_today_fee_ratio, is_close_today`
pub fn futures_settle_shfe(date: &str) -> Result<Df> {
    let d = convert_date(date)?;
    let url = format!("https://www.shfe.com.cn/data/tradedata/future/dailydata/js{d}.dat");
    let http = HttpClient::default();
    let json = match or_empty(
        http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", UA_MSIE)], None),
    )? {
        Some(j) => j,
        None => return empty_df(),
    };
    parse_shfe_ine(&json, &d)
}

/// 上海国际能源交易中心-结算参数（对应 akshare [`akshare.futures_settle_ine`]）。
///
/// `date`: 格式同 [`futures_settle_cffex`]（必填）。数据源
/// `https://www.ine.cn/data/tradedata/future/dailydata/js{date}.dat`。
/// 返回列同 [`futures_settle_shfe`]。
pub fn futures_settle_ine(date: &str) -> Result<Df> {
    let d = convert_date(date)?;
    let url = format!("https://www.ine.cn/data/tradedata/future/dailydata/js{d}.dat");
    let http = HttpClient::default();
    let json = match or_empty(
        http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", UA_MSIE)], None),
    )? {
        Some(j) => j,
        None => return empty_df(),
    };
    parse_shfe_ine(&json, &d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_date_formats() {
        assert_eq!(convert_date("20260119").unwrap(), "20260119");
        assert_eq!(convert_date("2026-01-19").unwrap(), "20260119");
        assert_eq!(convert_date("2026/01/19").unwrap(), "20260119");
        assert!(convert_date("2026-13-01").is_err());
        assert!(convert_date("abc").is_err());
    }

    #[test]
    fn cffex_parse_ok() {
        let text = "\u{feff}期货合约结算业务参数表（20260119）\n\
                    期货合约,合约多头保证金标准,合约空头保证金标准,交易手续费标准,交割手续费标准,平今仓收取率\n\
                    IC2602,12%,12%,万分之0.23,万分之0.5,1000%\n\
                    IF2603,15%,15%,万分之0.23,万分之0.5,1000%\n\
                    IF2606,15%,15%,万分之0.23,万分之0.5,1000%\n\
                    IF2609,15%,15%,万分之0.23,万分之0.5,1000%\n\
                    T2606,2%,2%,3元/手,2.5元/手,0%\n\
                    T2609,2%,2%,3元/手,2.5元/手,0%\n\
                    合计,,,,,\n";
        let df = parse_cffex(text, "20260119").unwrap();
        // 只保留 6 行合约（合计行被 `^[A-Z]+` 过滤）
        assert_eq!(df.height(), 6);
        assert_eq!(df.column_names(), CFFEX_COLS);
        let sym = df.inner().column("symbol").unwrap().str().unwrap();
        assert_eq!(sym.get(0), Some("IC2602"));
        let var = df.inner().column("variety").unwrap().str().unwrap();
        assert_eq!(var.get(0), Some("IC"));
        let fee = df.inner().column("trade_fee_ratio").unwrap().str().unwrap();
        assert_eq!(fee.get(0), Some("万分之0.23"));
    }

    #[test]
    fn cffex_html_page_gives_empty() {
        assert!(parse_cffex("<!DOCTYPE html><html>要查看的页面不存在</html>", "20260119")
            .unwrap()
            .height()
            == 0);
    }

    #[test]
    fn czce_parse_ok() {
        let text = "郑州商品交易所期货结算参数表(2026-01-19)\n\
                    合约代码|当日结算价|是否单边市|连续单边市天数|交易保证金率(%)|涨跌停板(%)|交易手续费|手续费收取方式|交割手续费|日内平今仓交易手续费|日持仓限额|交易限额\n\
                    AP603    |9,436.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP604    |9,389.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP605    |9,400.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP610    |8,113.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP611    |7,929.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    合计      |           |           |               |                |           |           |                 |           |           |           |\n";
        let df = parse_czce(text, "20260119").unwrap();
        assert_eq!(df.height(), 5);
        assert_eq!(df.column_names(), CZCE_COLS);
        let sym = df.inner().column("symbol").unwrap().str().unwrap();
        assert_eq!(sym.get(0), Some("AP603"));
        let price = df.inner().column("settle_price").unwrap().str().unwrap();
        assert_eq!(price.get(0), Some("9,436.00"));
    }

    #[test]
    fn gfex_parse_ok_and_filters_options() {
        let json = serde_json::json!({
            "code": "0",
            "data": [
                {"contractId": "lc2608", "specBuyRate": 0.2, "specBuy": 28944.0, "hedgeBuyRate": 0.2,
                 "hedgeBuy": 28944.0, "riseLimitRate": 0.13, "riseLimit": 163520.0, "fallLimit": 125920,
                 "agentTotBuyPosiQuota": -1.0, "selfTotBuyPosiQuota": 300.0, "clientBuyPosiQuota": 300.0,
                 "selfTotBuySerLimit": 300.0, "clientBuySerLimit": 300.0, "tradeType": "0"},
                {"contractId": "lc2608-C-500", "specBuyRate": 0.2, "specBuy": 28944.0, "hedgeBuyRate": 0.2,
                 "hedgeBuy": 28944.0, "riseLimitRate": 0.13, "riseLimit": 163520.0, "fallLimit": 125920,
                 "agentTotBuyPosiQuota": -1.0, "selfTotBuyPosiQuota": 300.0, "clientBuyPosiQuota": 300.0,
                 "selfTotBuySerLimit": 300.0, "clientBuySerLimit": 300.0, "tradeType": "0"}
            ]
        });
        let df = parse_gfex(&json, "20260119").unwrap();
        assert_eq!(df.height(), 1); // 期权 `-` 合约被过滤
        assert_eq!(df.column_names(), GFEX_COLS);
        let rate = df.inner().column("spec_buy_rate").unwrap().f64().unwrap();
        assert_eq!(rate.get(0), Some(0.2));
    }

    #[test]
    fn shfe_parse_ok() {
        let json = serde_json::json!({
            "o_cursor": [
                {"INSTRUMENTID": "cu2602", "TRADEFEERATIO": 0.05, "TTRADEFEERATIO": 0.025,
                 "COMMODITYDELIVFEEUNIT": 0, "SPECLONGMARGINRATIO": 0.1, "HEDGLONGMARGINRATIO": 0.1,
                 "COMMODITYDELIVFEERATIO": 0, "PRODUCTID": "cu_f", "PRODUCTNAME": "铜",
                 "TTRADEFEEUNIT": 0, "TRADEFEEUNIT": 0, "HEDGSHORTMARGINRATIO": 0.1,
                 "SETTLEMENTPRICE": 100610, "UNIDIRECT": "1", "SPECSHORTMARGINRATIO": 0.1,
                 "ISUNITODAY": 0}
            ]
        });
        let df = parse_shfe_ine(&json, "20260119").unwrap();
        assert_eq!(df.height(), 1);
        assert_eq!(df.column_names(), SHFE_COLS);
        let price = df.inner().column("settle_price").unwrap().f64().unwrap();
        assert_eq!(price.get(0), Some(100610.0));
        let sym = df.inner().column("symbol").unwrap().str().unwrap();
        assert_eq!(sym.get(0), Some("cu2602"));
        let var = df.inner().column("variety").unwrap().str().unwrap();
        assert_eq!(var.get(0), Some("cu"));
    }

    #[test]
    fn empty_responses_give_empty_df() {
        assert_eq!(parse_cffex("", "20260119").unwrap().height(), 0);
        assert_eq!(parse_czce("只有一行", "20260119").unwrap().height(), 0);
        assert_eq!(
            parse_gfex(&serde_json::json!({"code": "0", "data": []}), "20260119")
                .unwrap()
                .height(),
            0
        );
        assert_eq!(
            parse_shfe_ine(&serde_json::json!({"o_cursor": []}), "20260119")
                .unwrap()
                .height(),
            0
        );
    }
}
