//! 期货数据（对应 akshare `futures/` 目录）。
//!
//! 已实现：
//! - 五家期货交易所结算参数（对应 akshare `futures/futures_settle.py`）：
//!   [`futures_settle_cffex`]：中金所（CSV，GBK）、[`futures_settle_czce`]：郑商所（管道符 txt）、
//!   [`futures_settle_gfex`]：广期所（POST JSON）、[`futures_settle_shfe`]：上期所（JSON）、
//!   [`futures_settle_ine`]：上能中心（JSON）
//! - 统一入口 [`futures_settle`]：按 `market` 分派到上述五家，输出 akshare 统一的
//!   20 列规范字段（`SETTLE_OUTPUT_COLUMNS`，对应 `_normalize_settle_columns`）
//! - 新浪期货合约详情 [`futures_contract_detail`]（对应 akshare `futures/futures_contract_detail.py`）
//! - 东财 datacenter 期货库存：
//!   [`futures_comex_inventory`]（COMEX 黄金/白银库存，`RPT_FUTUOPT_GOLDSIL`）、
//!   [`futures_inventory_em`]（期货品种库存/增减，`RPT_FUTU_STOCKDATA`）
//!
//! 大商所（DCE）因网站反爬保护（412）暂缓，与 akshare 上游状态一致。
//! 各接口均为「指定日期 → 该交易所全部期货合约的保证金/手续费/涨跌停参数」，
//! 失败（无此日期数据 / 页面不存在）时返回空表（对应 akshare `pd.DataFrame()`）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::finalize_report;
use crate::stock_feature::{datacenter, report_extra};
use polars::prelude::*;
use scraper::{Html, Selector};
use serde_json::{Map, Value};

// 批次 29 子组 A：东方财富国际期货 + 中证商品指数 + 东财期货规则
pub mod em_global;
pub use em_global::*;

// 批次 29 子组 B：新浪期货集群（国内 sina + 外盘 hq/foreign）
pub mod sina;
pub use sina::*;

// 批次 29 子组 C：交易所官方数据（合约信息 / 仓单 / 交割 / 期转现 / 历史行情）
pub mod exchange;
pub use exchange::*;

// 批次 29 子组 D：东方财富期货行情（品种对照表 / kline / SGX 结算价）
pub mod em;
pub use em::*;

// 批次 29 子组 E：期货杂项 / 独立数据源集群（手续费 / 库存 / 现货 / 合约详情）
pub mod misc;
pub use misc::*;

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

// ---------------------------------------------------------------------------
// 统一结算参数入口（futures_settle）：20 列规范化
// ---------------------------------------------------------------------------

/// 统一的结算参数字段（对应 akshare `SETTLE_OUTPUT_COLUMNS`，20 列）。
const SETTLE_OUTPUT_COLUMNS: [&str; 20] = [
    "date",
    "symbol",
    "variety",
    "settle_price",
    "long_margin_ratio",
    "short_margin_ratio",
    "spec_long_margin_ratio",
    "spec_short_margin_ratio",
    "hedge_long_margin_ratio",
    "hedge_short_margin_ratio",
    "trade_fee_ratio",
    "close_today_fee_ratio",
    "delivery_fee_ratio",
    "is_single_market",
    "single_market_days",
    "limit_ratio",
    "position_limit",
    "trade_limit",
    "rise_limit_rate",
    "fall_limit_rate",
];

/// 每交易所的「统一列 → 源列」映射（`None` = 无来源，输出空列）。
///
/// 对应 akshare `_normalize_settle_columns` 的 `field_mapping` 在各自交易所
/// 原始列集合上求值的结果：目标列已存在则原样保留，否则取第一个命中的来源列；
/// 一个来源列可被多个统一列复用（如 GFEX `spec_buy_rate` 同时映射到
/// `long_margin_ratio` / `spec_long_margin_ratio` / `hedge_short_margin_ratio`，
/// 含 akshare 上游的映射怪癖 `hedge_short_margin_ratio ← spec_buy_rate`）。
fn settle_mapping(market: &str) -> &'static [(&'static str, Option<&'static str>)] {
    const CFFEX: &[(&str, Option<&str>)] = &[
        ("date", Some("date")),
        ("symbol", Some("symbol")),
        ("variety", Some("variety")),
        ("settle_price", None),
        ("long_margin_ratio", Some("long_margin_ratio")),
        ("short_margin_ratio", Some("short_margin_ratio")),
        ("spec_long_margin_ratio", None),
        ("spec_short_margin_ratio", None),
        ("hedge_long_margin_ratio", None),
        ("hedge_short_margin_ratio", None),
        ("trade_fee_ratio", Some("trade_fee_ratio")),
        ("close_today_fee_ratio", Some("close_today_fee_ratio")),
        ("delivery_fee_ratio", Some("delivery_fee_ratio")),
        ("is_single_market", None),
        ("single_market_days", None),
        ("limit_ratio", None),
        ("position_limit", None),
        ("trade_limit", None),
        ("rise_limit_rate", None),
        ("fall_limit_rate", None),
    ];
    const CZCE: &[(&str, Option<&str>)] = &[
        ("date", Some("date")),
        ("symbol", Some("symbol")),
        ("variety", Some("variety")),
        ("settle_price", Some("settle_price")),
        ("long_margin_ratio", Some("margin_ratio")),
        ("short_margin_ratio", None),
        ("spec_long_margin_ratio", None),
        ("spec_short_margin_ratio", None),
        ("hedge_long_margin_ratio", None),
        ("hedge_short_margin_ratio", None),
        ("trade_fee_ratio", None),
        ("close_today_fee_ratio", None),
        ("delivery_fee_ratio", None),
        ("is_single_market", Some("is_single_market")),
        ("single_market_days", Some("single_market_days")),
        ("limit_ratio", Some("limit_ratio")),
        ("position_limit", Some("position_limit")),
        ("trade_limit", Some("trade_limit")),
        ("rise_limit_rate", None),
        ("fall_limit_rate", None),
    ];
    const GFEX: &[(&str, Option<&str>)] = &[
        ("date", Some("date")),
        ("symbol", Some("symbol")),
        ("variety", Some("variety")),
        ("settle_price", None),
        ("long_margin_ratio", Some("spec_buy_rate")),
        ("short_margin_ratio", Some("hedge_buy_rate")),
        ("spec_long_margin_ratio", Some("spec_buy_rate")),
        ("spec_short_margin_ratio", Some("hedge_buy_rate")),
        ("hedge_long_margin_ratio", Some("hedge_buy_rate")),
        ("hedge_short_margin_ratio", Some("spec_buy_rate")),
        ("trade_fee_ratio", None),
        ("close_today_fee_ratio", None),
        ("delivery_fee_ratio", None),
        ("is_single_market", None),
        ("single_market_days", None),
        ("limit_ratio", None),
        ("position_limit", Some("client_buy_posi_quota")),
        ("trade_limit", None),
        ("rise_limit_rate", Some("rise_limit_rate")),
        ("fall_limit_rate", Some("fall_limit")),
    ];
    const SHFE_INE: &[(&str, Option<&str>)] = &[
        ("date", Some("date")),
        ("symbol", Some("symbol")),
        ("variety", Some("variety")),
        ("settle_price", Some("settle_price")),
        ("long_margin_ratio", None),
        ("short_margin_ratio", None),
        ("spec_long_margin_ratio", Some("spec_long_margin_ratio")),
        ("spec_short_margin_ratio", Some("spec_short_margin_ratio")),
        ("hedge_long_margin_ratio", Some("hedge_long_margin_ratio")),
        ("hedge_short_margin_ratio", Some("hedge_short_margin_ratio")),
        ("trade_fee_ratio", Some("trade_fee_ratio")),
        ("close_today_fee_ratio", Some("close_today_fee_ratio")),
        ("delivery_fee_ratio", None),
        ("is_single_market", None),
        ("single_market_days", None),
        ("limit_ratio", None),
        ("position_limit", None),
        ("trade_limit", None),
        ("rise_limit_rate", None),
        ("fall_limit_rate", None),
    ];
    const UNSUPPORTED: &[(&str, Option<&str>)] = &[
        ("date", None),
        ("symbol", None),
        ("variety", None),
        ("settle_price", None),
        ("long_margin_ratio", None),
        ("short_margin_ratio", None),
        ("spec_long_margin_ratio", None),
        ("spec_short_margin_ratio", None),
        ("hedge_long_margin_ratio", None),
        ("hedge_short_margin_ratio", None),
        ("trade_fee_ratio", None),
        ("close_today_fee_ratio", None),
        ("delivery_fee_ratio", None),
        ("is_single_market", None),
        ("single_market_days", None),
        ("limit_ratio", None),
        ("position_limit", None),
        ("trade_limit", None),
        ("rise_limit_rate", None),
        ("fall_limit_rate", None),
    ];
    match market {
        "CFFEX" => CFFEX,
        "CZCE" => CZCE,
        "GFEX" => GFEX,
        "SHFE" | "INE" => SHFE_INE,
        _ => UNSUPPORTED,
    }
}

/// 统一结算参数规范化（对应 akshare `_normalize_settle_columns`）。
///
/// 按 `settle_mapping` 复制源列（保留 dtype，float64 仍是 float64），
/// 无来源的统一列输出全空列；空表输入输出 20 列空表。
fn normalize_settle(df: &Df, market: &str) -> Result<Df> {
    if df.height() == 0 {
        // 对应 akshare `if df.empty: return pd.DataFrame(columns=SETTLE_OUTPUT_COLUMNS)`
        return Df::from_string_rows(&SETTLE_OUTPUT_COLUMNS, &[]);
    }
    let n = df.height();
    let mut columns: Vec<Column> = Vec::with_capacity(SETTLE_OUTPUT_COLUMNS.len());
    for (target, source) in settle_mapping(market) {
        let col: Column = match source {
            Some(src) => {
                let mut c = df
                    .inner()
                    .column(src)
                    .map_err(|e| AkshareError::Empty(format!("统一结算表缺源列 {src}: {e}")))?
                    .clone();
                c.rename(PlSmallStr::from_str(target));
                c
            }
            None => StringChunked::from_iter_options(
                PlSmallStr::from_str(target),
                std::iter::repeat_n(None::<&str>, n),
            )
            .into_series()
            .into(),
        };
        columns.push(col);
    }
    let inner = DataFrame::new(n, columns)
        .map_err(|e| AkshareError::Empty(format!("构建统一结算表失败: {e}")))?;
    Ok(Df::from_inner(inner))
}

/// 期货交易所结算参数（统一入口，对应 akshare [`akshare.futures_settle`]）。
///
/// `date`: 格式同 [`futures_settle_cffex`]（必填）。
/// `market`: 交易所代码，`CFFEX` 中金所 / `CZCE` 郑商所 / `SHFE` 上期所 /
/// `INE` 上能中心 / `GFEX` 广期所；不支持的代码返回 20 列空表
/// （对应 akshare `print(f"Unsupported market: {market}")` 后返回空 DataFrame）。
///
/// # 返回列（20 列统一规范，部分列按交易所无数据为全空）
/// `date, symbol, variety, settle_price, long_margin_ratio, short_margin_ratio,
/// spec_long_margin_ratio, spec_short_margin_ratio, hedge_long_margin_ratio,
/// hedge_short_margin_ratio, trade_fee_ratio, close_today_fee_ratio,
/// delivery_fee_ratio, is_single_market, single_market_days, limit_ratio,
/// position_limit, trade_limit, rise_limit_rate, fall_limit_rate`
pub fn futures_settle(date: &str, market: &str) -> Result<Df> {
    let m = market.trim().to_ascii_uppercase();
    let raw = match m.as_str() {
        "CFFEX" => futures_settle_cffex(date)?,
        "CZCE" => futures_settle_czce(date)?,
        "SHFE" => futures_settle_shfe(date)?,
        "GFEX" => futures_settle_gfex(date)?,
        "INE" => futures_settle_ine(date)?,
        _ => return Df::from_string_rows(&SETTLE_OUTPUT_COLUMNS, &[]),
    };
    normalize_settle(&raw, &m)
}

// ---------------------------------------------------------------------------
// 新浪期货合约详情（futures_contract_detail）
// ---------------------------------------------------------------------------

/// 新浪财经-期货合约详情（对应 akshare [`akshare.futures_contract_detail`]）。
///
/// `symbol`: 合约代码（如 `V2201`）。数据源
/// `https://finance.sina.com.cn/futures/quotes/{symbol}.shtml`（GB2312 页面）。
/// 页面第 7 张表（`id="table-futures-basic-data"`，akshare 用 `pd.read_html[6]`）
/// 每行 6 个单元格（th/td 交替），akshare 拆成 3 组 `(item, value)` 对后纵向拼接。
///
/// # 返回列
/// `item, value`
pub fn futures_contract_detail(symbol: &str) -> Result<Df> {
    let url = format!("https://finance.sina.com.cn/futures/quotes/{symbol}.shtml");
    let http = HttpClient::default();
    let text = match or_empty(http.get_text(&url, &Map::new(), None))? {
        Some(t) => t,
        None => return empty_df(),
    };
    parse_contract_detail(&text)
}

fn parse_contract_detail(text: &str) -> Result<Df> {
    let table_sel = Selector::parse("#table-futures-basic-data")
        .map_err(|e| AkshareError::Empty(format!("解析表格选择器失败: {e}")))?;
    let tr_sel =
        Selector::parse("tr").map_err(|e| AkshareError::Empty(format!("解析行选择器失败: {e}")))?;
    let cell_sel = Selector::parse("td, th")
        .map_err(|e| AkshareError::Empty(format!("解析单元格选择器失败: {e}")))?;

    let doc = Html::parse_document(text);
    let table = doc.select(&table_sel).next().ok_or_else(|| {
        AkshareError::Empty("新浪期货合约详情页缺少基础数据表".into())
    })?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for tr in table.select(&tr_sel) {
        // 单元格文本按空白折叠（对应 pandas read_html 的 `_remove_whitespace`：
        // 上游 `交易时间` 等单元格含连续空格，pandas 折叠为单个空格）
        let cells: Vec<String> = tr
            .select(&cell_sel)
            .map(|c| {
                c.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        rows.push(cells);
    }
    if rows.is_empty() {
        return empty_df();
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    // akshare 按列组纵向拼接：`data_one=iloc[:, :2]`、`data_two=iloc[:, 2:4]`、
    // `data_three=iloc[:, 4:]` 各自取全部行后 concat(axis=0)。因此输出顺序是
    // 先所有行的 (0,1) 列组、再所有行的 (2,3) 列组、最后所有行的 (4,5) 列组。
    let mut pairs: Vec<[Option<String>; 2]> = Vec::new();
    let mut col = 0;
    while col + 1 < width {
        for row in &rows {
            let item = row.get(col).filter(|s| !s.is_empty()).cloned();
            let value = row.get(col + 1).filter(|s| !s.is_empty()).cloned();
            pairs.push([item, value]);
        }
        col += 2;
    }
    let rows_out: Vec<Vec<Option<String>>> =
        pairs.into_iter().map(|p| p.to_vec()).collect();
    Df::from_string_rows(&["item", "value"], &rows_out)
}

// ============ 东财 datacenter 期货库存（RPT_FUTUOPT_GOLDSIL / RPT_FUTU_*） ============

/// JSON 值转字符串（兼容 str / 数值，对应 datacenter 单元格的多种类型）。
fn cell_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// COMEX 库存数据（对应 akshare [`akshare.futures_comex_inventory`]）。
///
/// `symbol`：`黄金` → `EMI00069026`、`白银` → `EMI00069027`（akshare `symbol_map`）。
/// 报表 `RPT_FUTUOPT_GOLDSIL`，过滤 `(@STORAGE_TON<>"NULL")` 去掉空库存行，按日期降序。
/// 列名随 `symbol` 动态拼接（`COMEX黄金库存量-吨` / `COMEX黄金库存量-盎司` 等）。
///
/// # 返回列
/// `序号, 日期, COMEX{黄金|白银}库存量-吨, COMEX{黄金|白银}库存量-盎司`
/// （`日期` 归一化为 `YYYY-MM-DD`；两个库存量列转 float64）。
pub fn futures_comex_inventory(symbol: &str) -> Result<Df> {
    let indicator = match symbol {
        "黄金" => "EMI00069026",
        "白银" => "EMI00069027",
        other => {
            return Err(AkshareError::Param(format!(
                "未知 symbol: {other}（可选：黄金/白银）"
            )))
        }
    };
    let filter = format!(r#"(INDICATOR_ID1="{indicator}")(@STORAGE_TON<>"NULL")"#);
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), Some(""), None, None);
    let rows = datacenter("RPT_FUTUOPT_GOLDSIL", "ALL", &extra, "500")?;

    let ton = format!("COMEX{symbol}库存量-吨");
    let ounce = format!("COMEX{symbol}库存量-盎司");
    let rename: [(&str, &str); 3] = [
        ("REPORT_DATE", "日期"),
        ("STORAGE_TON", ton.as_str()),
        ("STORAGE_OUNCE", ounce.as_str()),
    ];
    let select: [&str; 3] = ["日期", ton.as_str(), ounce.as_str()];
    let numeric: [&str; 2] = [ton.as_str(), ounce.as_str()];
    let mut df = finalize_report(&rows, &rename, &select, &numeric, Some("序号"))?;
    df.cast_date(&["日期"])?;
    Ok(df)
}

/// 期货库存 `symbol` 兜底映射（对应 akshare `futures.cons.futures_inventory_em_symbol_dict`，
/// 仅取非 `None` 项）。优先用东财 `RPT_FUTU_POSITIONCODE` 返回的 `TRADE_TYPE → TRADE_CODE`
/// 主合约映射；命中不到时回退到此表（覆盖东财主合约表里缺失的品种，如 `a → A`）。
const INVENTORY_SYMBOL_MAP: &[(&str, &str)] = &[
    ("a", "A"),
    ("ag", "AG"),
    ("al", "AL"),
    ("ao", "AO"),
    ("AP", "AP"),
    ("au", "AU"),
    ("b", "B"),
    ("br", "BR"),
    ("bu", "BU"),
    ("c", "C"),
    ("CF", "CF"),
    ("CJ", "CJ"),
    ("cs", "CS"),
    ("cu", "CU"),
    ("CY", "CY"),
    ("eb", "EB"),
    ("ec", "ec"),
    ("eg", "EG"),
    ("FG", "FG"),
    ("PL", "PL"),
    ("fu", "FU"),
    ("hc", "HC"),
    ("i", "I"),
    ("IC", "IC"),
    ("IF", "IF"),
    ("IH", "IH"),
    ("IM", "IM"),
    ("j", "J"),
    ("jd", "JD"),
    ("jm", "JM"),
    ("l", "L"),
    ("lc", "lc"),
    ("lh", "LH"),
    ("lu", "lu"),
    ("m", "M"),
    ("MA", "MA"),
    ("ni", "NI"),
    ("nr", "nr"),
    ("OI", "OI"),
    ("p", "P"),
    ("pb", "PB"),
    ("PF", "PF"),
    ("pg", "PG"),
    ("PK", "PK"),
    ("pp", "PP"),
    ("PX", "PX"),
    ("rb", "RB"),
    ("RM", "RM"),
    ("RS", "RS"),
    ("ru", "RU"),
    ("SA", "SA"),
    ("SF", "SF"),
    ("SH", "SH"),
    ("si", "si"),
    ("SM", "SM"),
    ("sn", "SN"),
    ("sp", "SP"),
    ("SR", "SR"),
    ("ss", "SS"),
    ("T", "T"),
    ("TA", "TA"),
    ("TF", "TF"),
    ("TL", "TL"),
    ("TS", "TS"),
    ("UR", "UR"),
    ("v", "V"),
    ("y", "Y"),
    ("zn", "ZN"),
];

/// 期货库存数据（对应 akshare [`akshare.futures_inventory_em`]）。
///
/// `symbol`：品种代码或中文名（默认 `"a"`）。先查 `RPT_FUTU_POSITIONCODE`（`IS_MAINCODE="1"`）
/// 取主合约映射 `TRADE_TYPE → TRADE_CODE`；命中不到时回退到 [`INVENTORY_SYMBOL_MAP`]
/// （对应 akshare 的 `futures_inventory_em_symbol_dict`）。再查 `RPT_FUTU_STOCKDATA`
/// （`SECURITY_CODE=产品代码`、`TRADE_DATE>='2020-10-28'`），按日期降序。
///
/// # 返回列
/// `日期, 库存, 增减`（`日期` 归一化为 `YYYY-MM-DD`；`库存`/`增减` 转 float64）。
pub fn futures_inventory_em(symbol: &str) -> Result<Df> {
    // 1) 主合约映射：TRADE_TYPE -> TRADE_CODE
    let extra0 = report_extra("", "", Some(r#"(IS_MAINCODE="1")"#), None, None, None);
    let rows0 = datacenter(
        "RPT_FUTU_POSITIONCODE",
        "TRADE_MARKET_CODE,TRADE_CODE,TRADE_TYPE",
        &extra0,
        "500",
    )?;
    let mut code_by_type: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for r in &rows0 {
        if let (Some(t), Some(c)) = (
            r.get("TRADE_TYPE").and_then(cell_str),
            r.get("TRADE_CODE").and_then(cell_str),
        ) {
            code_by_type.insert(t, c);
        }
    }
    // akshare 解析顺序：先命中 datacenter 主合约表，再回退到硬编码品种表，否则报错。
    let product_id = if let Some(code) = code_by_type.get(symbol) {
        code.clone()
    } else if let Some((_, code)) = INVENTORY_SYMBOL_MAP
        .iter()
        .find(|(s, _)| *s == symbol)
    {
        (*code).to_string()
    } else {
        return Err(AkshareError::Param(format!(
            "未找到品种: {symbol}（可选项见东财期货库存数据页的品种列表）"
        )));
    };

    // 2) 库存数据：SECURITY_CODE + 起始日期过滤
    let filter = format!(r#"(SECURITY_CODE="{product_id}")(TRADE_DATE>='2020-10-28')"#);
    let extra = report_extra("TRADE_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter(
        "RPT_FUTU_STOCKDATA",
        "SECURITY_CODE,TRADE_DATE,ON_WARRANT_NUM,ADDCHANGE",
        &extra,
        "500",
    )?;
    let rename: [(&str, &str); 3] = [
        ("TRADE_DATE", "日期"),
        ("ON_WARRANT_NUM", "库存"),
        ("ADDCHANGE", "增减"),
    ];
    let select: [&str; 3] = ["日期", "库存", "增减"];
    let numeric: [&str; 2] = ["库存", "增减"];
    let mut df = finalize_report(&rows, &rename, &select, &numeric, None)?;
    df.cast_date(&["日期"])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 抽取列名（与 parity export_parity 同口径），用于断言列契约顺序。
    fn col_names(df: &Df) -> Vec<String> {
        df.export_parity(0)["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

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

    #[test]
    fn settle_mapping_is_20_columns() {
        for m in ["CFFEX", "CZCE", "GFEX", "SHFE", "INE"] {
            let map = settle_mapping(m);
            assert_eq!(map.len(), 20, "{m} 映射应为 20 列");
            let targets: Vec<&str> = map.iter().map(|(t, _)| *t).collect();
            assert_eq!(targets, SETTLE_OUTPUT_COLUMNS, "{m} 列序须与规范一致");
        }
    }

    #[test]
    fn normalize_settle_czce_maps_and_keeps_dtypes() {
        // 模拟 CZCE 原始表（全字符串列）
        let text = "郑州商品交易所期货结算参数表(2026-01-19)\n\
                    合约代码|当日结算价|是否单边市|连续单边市天数|交易保证金率(%)|涨跌停板(%)|交易手续费|手续费收取方式|交割手续费|日内平今仓交易手续费|日持仓限额|交易限额\n\
                    AP603    |9,436.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP604    |9,389.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP605    |9,400.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP610    |8,113.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    AP611    |7,929.00   |N          |0              |10              |±9         |5.00       |绝对值           |0.00       |20.00      |1000       |\n\
                    合计      |           |           |               |                |           |           |                 |           |           |           |\n";
        let raw = parse_czce(text, "20260119").unwrap();
        let unified = normalize_settle(&raw, "CZCE").unwrap();
        assert_eq!(unified.column_names(), SETTLE_OUTPUT_COLUMNS);
        assert_eq!(unified.height(), 5);
        // 有映射的列保留值
        let price = unified.inner().column("settle_price").unwrap().str().unwrap();
        assert_eq!(price.get(0), Some("9,436.00"));
        let margin = unified.inner().column("long_margin_ratio").unwrap().str().unwrap();
        assert_eq!(margin.get(0), Some("10"));
        // 无映射的列为全空
        let sr = unified.inner().column("short_margin_ratio").unwrap().str().unwrap();
        assert!(sr.get(0).is_none());
        let tf = unified.inner().column("trade_fee_ratio").unwrap().str().unwrap();
        assert!(tf.get(0).is_none());
    }

    #[test]
    fn normalize_settle_gfex_copies_numeric_dtype() {
        let json = serde_json::json!({
            "code": "0",
            "data": [
                {"contractId": "lc2608", "specBuyRate": 0.2, "specBuy": 28944.0, "hedgeBuyRate": 0.2,
                 "hedgeBuy": 28944.0, "riseLimitRate": 0.13, "riseLimit": 163520.0, "fallLimit": 125920,
                 "agentTotBuyPosiQuota": -1.0, "selfTotBuyPosiQuota": 300.0, "clientBuyPosiQuota": 300.0,
                 "selfTotBuySerLimit": 300.0, "clientBuySerLimit": 300.0, "tradeType": "0"}
            ]
        });
        let raw = parse_gfex(&json, "20260119").unwrap();
        let unified = normalize_settle(&raw, "GFEX").unwrap();
        assert_eq!(unified.column_names(), SETTLE_OUTPUT_COLUMNS);
        assert_eq!(unified.height(), 1);
        // 数值源列复制后仍是 float64
        let lmr = unified.inner().column("long_margin_ratio").unwrap().f64().unwrap();
        assert_eq!(lmr.get(0), Some(0.2));
        let hsr = unified.inner().column("hedge_short_margin_ratio").unwrap().f64().unwrap();
        assert_eq!(hsr.get(0), Some(0.2)); // akshare 上游怪癖：← spec_buy_rate
        let pos = unified.inner().column("position_limit").unwrap().f64().unwrap();
        assert_eq!(pos.get(0), Some(300.0));
        // 无来源列空
        assert!(unified
            .inner()
            .column("settle_price")
            .unwrap()
            .str()
            .unwrap()
            .get(0)
            .is_none());
    }

    #[test]
    fn normalize_settle_empty_input_gives_20_empty_columns() {
        let empty = empty_df().unwrap();
        let unified = normalize_settle(&empty, "CZCE").unwrap();
        assert_eq!(unified.height(), 0);
        assert_eq!(unified.column_names(), SETTLE_OUTPUT_COLUMNS);
    }

    #[test]
    fn parse_contract_detail_ok() {
        let html = r#"<html><body>
        <table id="table-futures-basic-data" class="table">
        <tr><th>交易品种</th><td>聚氯乙烯</td><th>交易单位</th><td>5吨/手</td><th>报价单位</th><td>元(人民币/吨)</td></tr>
        <tr><th>交易时间</th><td>上午 09:00-10:15 10:30-11:30  下午 13:30-15:00  夜间 21:00-23:00</td><th>最后交易日</th><td>合约月份第10个交易日</td><th>最后交割日</th><td>最后交易日后第3个交易日</td></tr>
        <tr><th>交割方式</th><td>实物交割</td><th>交易代码</th><td>V</td><th>上市交易所</th><td>大连商品交易所</td></tr>
        </table>
        </body></html>"#;
        let df = parse_contract_detail(html).unwrap();
        assert_eq!(df.column_names(), vec!["item", "value"]);
        assert_eq!(df.height(), 9); // 3 行 × 3 列组
        let item = df.inner().column("item").unwrap().str().unwrap();
        let value = df.inner().column("value").unwrap().str().unwrap();
        // 列组纵向拼接：先所有行的 (0,1)，再 (2,3)，最后 (4,5)
        assert_eq!(item.get(0), Some("交易品种"));
        assert_eq!(value.get(0), Some("聚氯乙烯"));
        assert_eq!(item.get(1), Some("交易时间"));
        // 连续空格折叠为单个（对应 pandas read_html）
        assert_eq!(value.get(1), Some("上午 09:00-10:15 10:30-11:30 下午 13:30-15:00 夜间 21:00-23:00"));
        assert_eq!(item.get(2), Some("交割方式"));
        assert_eq!(value.get(2), Some("实物交割"));
        assert_eq!(item.get(3), Some("交易单位"));
        assert_eq!(value.get(3), Some("5吨/手"));
        assert_eq!(item.get(8), Some("上市交易所"));
        assert_eq!(value.get(8), Some("大连商品交易所"));
    }

    #[test]
    fn parse_contract_detail_missing_table_is_err() {
        assert!(parse_contract_detail("<html><body>无表格</body></html>").is_err());
    }

    #[test]
    fn comex_inventory_offline_contract() {
        // 复刻 futures_comex_inventory（symbol=黄金）的 finalize 契约：
        // 序号（1 起始）+ 日期 + COMEX黄金库存量-吨 + COMEX黄金库存量-盎司
        let rows = vec![json!({
            "REPORT_DATE": "2024-01-05 00:00:00",
            "STORAGE_TON": 12345.67,
            "STORAGE_OUNCE": 398765.4,
        })];
        let ton = "COMEX黄金库存量-吨";
        let ounce = "COMEX黄金库存量-盎司";
        let rename: [(&str, &str); 3] = [
            ("REPORT_DATE", "日期"),
            ("STORAGE_TON", ton),
            ("STORAGE_OUNCE", ounce),
        ];
        let select: [&str; 3] = ["日期", ton, ounce];
        let numeric: [&str; 2] = [ton, ounce];
        let mut df = finalize_report(&rows, &rename, &select, &numeric, Some("序号")).unwrap();
        df.cast_date(&["日期"]).unwrap();

        assert_eq!(
            col_names(&df),
            vec!["序号", "日期", "COMEX黄金库存量-吨", "COMEX黄金库存量-盎司"]
        );
        let idx = df.inner().column("序号").unwrap().f64().unwrap().get(0);
        assert_eq!(idx, Some(1.0));
        let d = df.inner().column("日期").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2024-01-05"));
        let t = df.inner().column(ton).unwrap().f64().unwrap().get(0).unwrap();
        assert!(approx(t, 12345.67));
    }

    #[test]
    fn inventory_em_offline_contract() {
        // 复刻 futures_inventory_em 第二段的 finalize 契约：日期 + 库存 + 增减
        let rows = vec![json!({
            "TRADE_DATE": "2024-02-08 00:00:00",
            "ON_WARRANT_NUM": 123456.0,
            "ADDCHANGE": -789.0,
        })];
        let rename: [(&str, &str); 3] = [
            ("TRADE_DATE", "日期"),
            ("ON_WARRANT_NUM", "库存"),
            ("ADDCHANGE", "增减"),
        ];
        let select: [&str; 3] = ["日期", "库存", "增减"];
        let numeric: [&str; 2] = ["库存", "增减"];
        let mut df = finalize_report(&rows, &rename, &select, &numeric, None).unwrap();
        df.cast_date(&["日期"]).unwrap();

        assert_eq!(col_names(&df), vec!["日期", "库存", "增减"]);
        let d = df.inner().column("日期").unwrap().str().unwrap().get(0);
        assert_eq!(d, Some("2024-02-08"));
        let n = df.inner().column("库存").unwrap().f64().unwrap().get(0).unwrap();
        assert!(approx(n, 123456.0));
        let c = df.inner().column("增减").unwrap().f64().unwrap().get(0).unwrap();
        assert!(approx(c, -789.0));
    }
}
