//! 期货杂项 / 独立数据源集群（对应 akshare `futures/` 下分散的杂项函数）。
//!
//! 已实现（批次 29 子组 E）：
//! - [`futures_comm_info`]：九期网期货手续费（`9qihuo.com/qihuoshouxufei`）
//! - [`futures_comm_js`]：金十期货手续费（`mp-api.jin10.com`，`x-app-id` 头）
//! - [`futures_fees_info`]：openctp 期货交易费用参照表（`openctp.cn/fees.html`）
//! - [`futures_rule`]：国泰君安期货交易日历（`gtjaqh.com/pc/calendar`）
//! - [`futures_news_shmet`]：上海金属网快讯（`shmet.com` POST JSON）
//! - [`futures_inventory_99`]：99 期货网大宗商品库存（`99qh.com` + `fx168api.com`）
//! - [`futures_spot_stock`]：东财现货与股票上下游（`data.eastmoney.com/ifdata/xhgp.html`）
//! - [`futures_stock_shfe_js`]：金十上期所库存周报（`datacenter-api.jin10.com`）
//! - [`futures_spot_sys`]：生意社现期图（`100ppi.com/sf/792.html`）
//! - [`futures_contract_detail_em`]：东财期货合约详情（`quote.eastmoney.com` + `futsse-static`）
//!
//! 注：`futures_derivative` 在 akshare 中是子包（模块），并非可调用函数，不在 1094
//! 公开函数目标内，故本子组不含该函数（详见 PLAN.md 批次 29-E 说明）。
//!
//! 网络依赖：以上源当前环境多数可达，但 `9qihuo`/`gtjaqh`/`100ppi` 等存在反爬或
//! DNS 限制；无法生成 golden 的用例 `parity --check` 自动跳过，非代码缺陷。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::html::read_html_tables;
use crate::core::http::HttpClient;
use crate::core::js_engine::js_literal_to_json;
use chrono::{LocalResult, TimeZone, Utc};
use scraper::{Html, Selector};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// 浏览器 UA（多数源无特别要求，带 UA 更稳）。
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";

/// JSON 值 → Option<String>（数值走 `to_string`，与 akshare `pd.DataFrame` 后逐单元格 str 一致）。
fn cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// 字符串索引越界安全的单元格取值（缺列补空串）。
fn rget(row: &[String], i: usize) -> &str {
    row.get(i).map(|s| s.as_str()).unwrap_or("")
}

// ===========================================================================
// 金十期货手续费（futures_comm_js）
// ===========================================================================

/// 金十期货手续费（对应 akshare [`futures_comm_js`]）。
///
/// `date`：`YYYYMMDD`。走 `mp-api.jin10.com/api/dynamic-data/child`，需 `x-app-id` 头。
///
/// # 返回列
/// `日期, 合约品种, 合约代码, 手续费公布时间, 价格公布时间, 现价, 涨停板, 跌停板,
/// 保证金/买开, 保证金/卖开, 保证金/每手, 每手跳数, 开仓, 平昨, 平今, 每跳毛利, 每跳净利, 交易所`
pub fn futures_comm_js(date: &str) -> Result<Df> {
    let formatted = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
    let mut params = Map::new();
    params.insert("tb_name".into(), Value::String("_vir_26".into()));
    // akshare 用 json.dumps(...) → 字符串；reqwest 的 query 不支持嵌套对象，必须传字符串
    let search = json!({ "range,date": format!("{},{}", formatted, formatted), "status": 1 });
    params.insert(
        "search".into(),
        Value::String(serde_json::to_string(&search).unwrap_or_default()),
    );
    params.insert("order".into(), Value::String("date,desc".into()));
    let url = "https://mp-api.jin10.com/api/dynamic-data/child";
    let headers = [
        ("user-agent", UA),
        ("x-app-id", "fiXF2nOnDycGutVA"),
        ("x-version", "1.0"),
        ("referer", "https://www.jin10.com/"),
        ("origin", "https://www.jin10.com"),
    ];
    let http = HttpClient::default();
    let v = http.get_json_with_headers(url, &params, &headers, None)?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("金十期货手续费返回缺少 data".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for it in data {
        rows.push(vec![
            cell(&it["date"]),
            cell(&it["heyue_name"]),
            cell(&it["heyue_code"]),
            cell(&it["pub_date_commission"]),
            cell(&it["pub_date_price"]),
            cell(&it["heyue_price"]),
            cell(&it["up_limit_num"]),
            cell(&it["down_limit_num"]),
            cell(&it["buy_ratio"]),
            cell(&it["sell_ratio"]),
            cell(&it["per_lot_price"]),
            cell(&it["buy_commission"]),
            cell(&it["sell_cur_commission"]),
            cell(&it["sell_yesterday_commission"]),
            cell(&it["per_ratio"]),
            cell(&it["per_commission_price"]),
            cell(&it["per_net_profit"]),
            cell(&it["jys"]),
        ]);
    }
    let cols = [
        "日期",
        "合约品种",
        "合约代码",
        "手续费公布时间",
        "价格公布时间",
        "现价",
        "涨停板",
        "跌停板",
        "保证金/买开",
        "保证金/卖开",
        "保证金/每手",
        "开仓",
        "平今",
        "平昨",
        "每手跳数",
        "每跳毛利",
        "每跳净利",
        "交易所",
    ];
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&[
        "现价",
        "涨停板",
        "跌停板",
        "每手跳数",
        "每跳毛利",
        "每跳净利",
    ])?;
    Ok(df)
}

// ===========================================================================
// openctp 期货交易费用参照表（futures_fees_info）
// ===========================================================================

/// openctp 期货交易费用参照表（对应 akshare [`futures_fees_info`]）。
///
/// 抓取 `openctp.cn/fees.html` 首个表格，并附加「更新时间」列（来自页面首段 `Generated at`）。
///
/// # 返回列
/// 随页面表格动态变化 + 末尾 `更新时间`
pub fn futures_fees_info() -> Result<Df> {
    let url = "http://openctp.cn/fees.html";
    let http = HttpClient::default();
    let text = http.get_text_with_headers(url, &Map::new(), &[("User-Agent", UA)], None)?;
    // 更新时间：页面首段形如 "Generated at 2024-01-01 12:00:00."
    let update_time = if let Some(i) = text.find("Generated at ") {
        let rest = &text[i + "Generated at ".len()..];
        let end = rest.find('.').unwrap_or(rest.len());
        rest[..end].trim().to_string()
    } else {
        String::new()
    };
    let tables = read_html_tables(&text)?;
    let raw = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("openctp 费用表无表格".into()))?;
    if raw.len() < 2 {
        // 无数据：返回仅含表头（含更新时间）的空表
        let header: Vec<String> = if !raw.is_empty() {
            raw[0].clone()
        } else {
            Vec::new()
        };
        let mut cols = header;
        cols.push("更新时间".to_string());
        return Df::from_string_rows(&cols.iter().map(String::as_str).collect::<Vec<_>>(), &[]);
    }
    let header = &raw[0];
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(raw.len() - 1);
    for r in &raw[1..] {
        let mut row: Vec<Option<String>> = r.iter().map(|c| Some(c.clone())).collect();
        row.push(Some(update_time.clone()));
        rows.push(row);
    }
    let mut cols: Vec<String> = header.clone();
    cols.push("更新时间".to_string());
    let mut df = Df::from_string_rows(&cols.iter().map(String::as_str).collect::<Vec<_>>(), &rows)?;
    // 对齐 akshare pd.read_html 的列类型推断（数值列 → float64）
    df.infer_numeric()?;
    Ok(df)
}

// ===========================================================================
// 国泰君安期货交易日历（futures_rule）
// ===========================================================================

/// 国泰君安期货交易日历数据表（对应 akshare [`futures_rule`]）。
///
/// `date`：交易日（`YYYYMMDD` 或 `YYYY-MM-DD`）。抓取 `gtjaqh.com/pc/calendar`，
/// 取第 2 行作表头的表格，去 `%` 并数值化保证金/涨跌停等列。
///
/// # 返回列
/// 随页面表格动态变化（含 `交易保证金比例` / `涨跌停板幅度` / `合约乘数` 等）
pub fn futures_rule(date: &str) -> Result<Df> {
    let url = "https://www.gtjaqh.com/pc/calendar";
    let mut params = Map::new();
    params.insert("date".into(), Value::String(date.to_string()));
    let http = HttpClient::default();
    let text = http.get_text_with_headers(url, &params, &[("User-Agent", UA)], None)?;
    let tables = read_html_tables(&text)?;
    let raw = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("国泰君安交易日历无表格".into()))?;
    // akshare 用 header=1（第 2 行作表头）
    if raw.len() < 2 {
        let header: Vec<String> = if !raw.is_empty() {
            raw[0].clone()
        } else {
            Vec::new()
        };
        return Df::from_string_rows(&header.iter().map(String::as_str).collect::<Vec<_>>(), &[]);
    }
    let header = raw[1].clone();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(raw.len() - 2);
    for r in &raw[2..] {
        // 与 akshare `pd.to_numeric(errors="coerce")` 对齐：缺失标记 `--` 视为空，
        // 这样 `infer_numeric` 会把整列推断为 float64（空单元格不参与判定）。
        rows.push(
            r.iter()
                .map(|c| {
                    if c.trim() == "--" {
                        Some(String::new())
                    } else {
                        Some(c.clone())
                    }
                })
                .collect(),
        );
    }
    let mut df = Df::from_string_rows(
        &header.iter().map(String::as_str).collect::<Vec<_>>(),
        &rows,
    )?;
    let names = df.column_names();
    let has = |c: &str| names.iter().any(|n| n == c);
    // 先去掉百分比后缀（对应 akshare str.strip("%")），再按列推断数值类型
    let pct_cols: Vec<&str> = ["交易保证金比例", "涨跌停板幅度"]
        .iter()
        .copied()
        .filter(|c| has(c))
        .collect();
    if !pct_cols.is_empty() {
        df.strip_suffix(&pct_cols, "%")?;
    }
    df.infer_numeric()?;
    Ok(df)
}

// ===========================================================================
// 上海金属网快讯（futures_news_shmet）
// ===========================================================================

/// 毫秒时间戳 → Asia/Shanghai（UTC+8，无夏令时）`YYYY-MM-DD HH:MM:SS`。
fn ms_to_shanghai(ms: i64) -> String {
    // +8h（毫秒）
    let secs = (ms + 28_800_000) / 1000;
    match Utc.timestamp_opt(secs, 0) {
        LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => String::new(),
    }
}

/// 上海金属网快讯（对应 akshare [`futures_news_shmet`]）。
///
/// `symbol`：`全部` / `要闻` / `VIP` / `财经` / `铜` / `铝` / ... / `小金属`。
/// POST `shmet.com/api/rest/news/queryNewsflashList`，返回「发布时间, 内容」。
///
/// # 返回列
/// `发布时间, 内容`
pub fn futures_news_shmet(symbol: &str) -> Result<Df> {
    let url = "https://www.shmet.com/api/rest/news/queryNewsflashList";
    let symbol_map: HashMap<&str, &str> = [
        ("要闻", "0"),
        ("VIP", "100"),
        ("财经", "999"),
        ("铜", "1002"),
        ("铝", "1003"),
        ("铅", "1005"),
        ("锌", "1004"),
        ("镍", "1006"),
        ("锡", "1007"),
        ("贵金属", "1008"),
        ("小金属", "1009"),
    ]
    .into_iter()
    .collect();
    let mut payload = Map::new();
    if symbol == "全部" {
        payload.insert("currentPage".into(), Value::from(1));
        payload.insert("pageSize".into(), Value::from(100));
    } else {
        let tag = symbol_map
            .get(symbol)
            .ok_or_else(|| AkshareError::Param(format!("未知快讯分类: {symbol}")))?;
        payload.insert("currentPage".into(), Value::from(1));
        payload.insert("pageSize".into(), Value::from(2000));
        payload.insert("content".into(), Value::String(String::new()));
        payload.insert("flashTag".into(), Value::String((*tag).to_string()));
    }
    let headers = [("User-Agent", UA)];
    let http = HttpClient::default();
    let v = http.post_json_body(url, &Value::Object(payload), &headers)?;
    let list = v
        .get("data")
        .and_then(|d| d.get("dataList"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("上海金属网快讯返回缺少 data.dataList".into()))?;
    // 解析每条：发布时间(ms) → 上海时间字符串；内容
    let mut parsed: Vec<(i64, String, String)> = Vec::with_capacity(list.len());
    for it in list {
        let content = it
            .get("内容")
            .and_then(Value::as_str)
            .unwrap_or_else(|| it.get("content").and_then(Value::as_str).unwrap_or(""))
            .to_string();
        let ms = match it.get("发布时间").or_else(|| it.get("publishTime")) {
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Some(Value::String(s)) => s.parse::<i64>().ok(),
            _ => None,
        }
        .unwrap_or(0);
        let time_str = ms_to_shanghai(ms);
        parsed.push((ms, time_str, content));
    }
    // 按发布时间升序（对应 akshare sort_values）
    parsed.sort_by_key(|a| a.0);
    let rows: Vec<Vec<Option<String>>> = parsed
        .into_iter()
        .map(|(_, t, c)| vec![Some(t), Some(c)])
        .collect();
    Df::from_string_rows(&["发布时间", "内容"], &rows)
}

// ===========================================================================
// 99 期货网大宗商品库存（futures_inventory_99）
// ===========================================================================

/// 99 期货网品种代码对照表（对应 akshare `__get_99_symbol_map`）。
///
/// 抓取 `99qh.com/data/stockIn` 的 `__NEXT_DATA__`，构建 名称/代码 → productId 映射。
fn get_99_symbol_map(text: &str) -> Result<(HashMap<String, String>, HashMap<String, String>)> {
    let doc = Html::parse_document(text);
    let sel =
        Selector::parse("script#__NEXT_DATA__").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let raw = doc
        .select(&sel)
        .next()
        .map(|e| e.text().collect::<Vec<_>>().join(""))
        .ok_or_else(|| AkshareError::Empty("99qh 缺少 __NEXT_DATA__".into()))?;
    let v: Value = serde_json::from_str(&raw)
        .map_err(|e| AkshareError::json("99qh __NEXT_DATA__ 解析失败", e.to_string()))?;
    let variety = v
        .pointer("/props/pageProps/data/varietyListData")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("99qh 品种列表缺失".into()))?;
    let mut name_map = HashMap::new();
    let mut code_map = HashMap::new();
    for item in variety {
        let products = item.get("productList").and_then(Value::as_array);
        if let Some(products) = products {
            for p in products {
                let name = p
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let code = p
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let pid = match p.get("productId") {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => continue,
                };
                if !name.is_empty() {
                    name_map.insert(name, pid.clone());
                }
                if !code.is_empty() {
                    code_map.insert(code, pid);
                }
            }
        }
    }
    Ok((name_map, code_map))
}

/// 99 期货网大宗商品库存数据（对应 akshare [`futures_inventory_99`]）。
///
/// `symbol`：品种中文名（如 `豆一`）或代码。经品种映射取 `productId`，再查
/// `centerapi.fx168api.com/app/qh/api/stock/trend`（需硬编码 `_pcc` 头）。
///
/// # 返回列
/// `日期, 收盘价, 库存`
pub fn futures_inventory_99(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    let page = http.get_text_with_headers(
        "https://www.99qh.com/data/stockIn",
        &Map::new(),
        &[("User-Agent", UA)],
        None,
    )?;
    let (name_map, code_map) = get_99_symbol_map(&page)?;
    let product_id = name_map
        .get(symbol)
        .or_else(|| code_map.get(symbol))
        .cloned()
        .ok_or_else(|| AkshareError::Param(format!("未找到品种 {symbol} 对应的编号")))?;
    let url = "https://centerapi.fx168api.com/app/qh/api/stock/trend";
    let pcc = "DJKijwhimCjFLvYe7p2Evo5OnkSZ/sohOcXWRKQiwxhWKtezlhkQwqkaFeAVaF8h/H8Qx7u6Ew80tAI2ph2bQEQwUP1y+6m8tEecTQSZtLbjtgtqg1FijxNIwgzGaIn9vVfujlOTDFCLkUJWSKuCcTm/diD9X/lhoFSaqJxB56E=";
    let headers = [
        ("Content-Type", "application/json;charset=UTF-8"),
        ("_pcc", pcc),
        ("user-agent", UA),
        ("referer", "https://www.99qh.com"),
        ("origin", "https://www.99qh.com"),
    ];
    let mut params = Map::new();
    params.insert("productId".into(), Value::String(product_id));
    params.insert("type".into(), Value::String("1".into()));
    params.insert("pageNo".into(), Value::String("1".into()));
    params.insert("pageSize".into(), Value::String("5000".into()));
    params.insert("startDate".into(), Value::String(String::new()));
    params.insert("endDate".into(), Value::String(String::new()));
    params.insert("appCategory".into(), Value::String("web".into()));
    let v = http.get_json_with_headers(url, &params, &headers, None)?;
    let list = v
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("99qh 库存返回缺少 data.list".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(list.len());
    for it in list {
        rows.push(vec![
            cell(&it["日期"]).or_else(|| cell(&it["date"])),
            cell(&it["收盘价"]).or_else(|| cell(&it["close"])),
            cell(&it["库存"]).or_else(|| cell(&it["inventory"])),
        ]);
    }
    let mut df = Df::from_string_rows(&["日期", "收盘价", "库存"], &rows)?;
    df.cast_numeric(&["收盘价", "库存"])?;
    Ok(df)
}

// ===========================================================================
// 东财现货与股票上下游（futures_spot_stock）
// ===========================================================================

/// 东财现货与股票上下游对应数据（对应 akshare [`futures_spot_stock`]）。
///
/// `symbol`：`能源`/`化工`/`塑料`/`纺织`/`有色`/`钢铁`/`建材`/`农副`。
/// 解析 `data.eastmoney.com/ifdata/xhgp.html` 中 `pagedata = {...}` JS 字面量。
///
/// # 返回列
/// `商品名称, <4~5 个日期>, 最新价格, 近半年涨跌幅, 生产商, 下游用户`
pub fn futures_spot_stock(symbol: &str) -> Result<Df> {
    let map_dict: HashMap<&str, usize> = [
        ("能源", 0),
        ("化工", 1),
        ("塑料", 2),
        ("纺织", 3),
        ("有色", 4),
        ("钢铁", 5),
        ("建材", 6),
        ("农副", 7),
    ]
    .into_iter()
    .collect();
    let idx = *map_dict
        .get(symbol)
        .ok_or_else(|| AkshareError::Param(format!("未知现货分类: {symbol}")))?;
    let url = "https://data.eastmoney.com/ifdata/xhgp.html";
    let http = HttpClient::default();
    let text = http.get_text_with_headers(url, &Map::new(), &[("User-Agent", UA)], None)?;
    // 截取 pagedata 对象字面量（与 akshare 的 find("pagedata")..find("/newstatic/...") 等价）
    let start = text
        .find("pagedata")
        .ok_or_else(|| AkshareError::Empty("东财现货与股票无 pagedata".into()))?;
    let end = text
        .find("/newstatic/js/common/emdataview.js")
        .unwrap_or(text.len());
    let chunk = &text[start..end];
    let a = chunk
        .find('{')
        .ok_or_else(|| AkshareError::Empty("pagedata 中未找到对象".into()))?;
    let b = chunk
        .rfind('}')
        .ok_or_else(|| AkshareError::Empty("pagedata 中未找到对象结尾".into()))?;
    let obj = &chunk[a..=b];
    let json_text = js_literal_to_json(obj)?;
    let temp_json: Value = serde_json::from_str(&json_text)
        .map_err(|e| AkshareError::json("东财 pagedata JSON 解析失败", e.to_string()))?;
    let dates = temp_json
        .get("dates")
        .and_then(Value::as_object)
        .map(|o| o.values().filter_map(cell).collect::<Vec<String>>())
        .ok_or_else(|| AkshareError::Empty("pagedata 缺少 dates".into()))?;
    let n_dates = dates.len();
    let datas = temp_json
        .get("datas")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("pagedata 缺少 datas".into()))?;
    let category = datas
        .get(idx)
        .and_then(|c| c.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty(format!("pagedata datas[{idx}] 缺少 list")))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(category.len());
    for item in category {
        let name = cell(&item["name"]).or_else(|| cell(&item["商品名称"]));
        let mut row: Vec<Option<String>> = Vec::with_capacity(2 + n_dates);
        row.push(name);
        // 日期列数据存于 item 的 v1..v5（与 dates.values() 顺序一一对应），
        // 而非日期标签本身（MM-DD）。
        for i in 0..n_dates {
            let key = format!("v{}", i + 1);
            row.push(cell(&item[key]));
        }
        row.push(cell(&item["price"]).or_else(|| cell(&item["最新价格"])));
        row.push(cell(&item["zdf"]).or_else(|| cell(&item["近半年涨跌幅"])));
        // 生产商 = scss 列表的 name 拼接；下游用户 = xyyhs 列表的 name 拼接
        let scs = join_names(item.get("scss"));
        let xyyh = join_names(item.get("xyyhs"));
        row.push(Some(scs));
        row.push(Some(xyyh));
        rows.push(row);
    }
    let mut cols: Vec<String> = vec!["商品名称".to_string()];
    cols.extend(dates.iter().cloned());
    cols.push("最新价格".to_string());
    cols.push("近半年涨跌幅".to_string());
    cols.push("生产商".to_string());
    cols.push("下游用户".to_string());
    let mut df = Df::from_string_rows(&cols.iter().map(String::as_str).collect::<Vec<_>>(), &rows)?;
    // 对齐 akshare 源码：仅对前 4 个日期列 + 最新价格 + 近半年涨跌幅 做 to_numeric；
    // 第 5 个（最后一个）日期列故意保持 str（akshare 未对其调用 to_numeric）。
    let mut num_cols: Vec<String> = Vec::new();
    for d in dates.iter().take(n_dates.min(4)) {
        num_cols.push(d.clone());
    }
    num_cols.push("最新价格".to_string());
    num_cols.push("近半年涨跌幅".to_string());
    let num_refs: Vec<&str> = num_cols.iter().map(String::as_str).collect();
    df.cast_numeric(&num_refs)?;
    Ok(df)
}

/// 把 `scss`/`xyyhs` 这类 `[{name: ...}, ...]` 列表拼接为 `", "` 分隔字符串（空列表 → `-`）。
fn join_names(v: Option<&Value>) -> String {
    match v.and_then(Value::as_array) {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .filter_map(|x| x.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(", "),
        _ => "-".to_string(),
    }
}

// ===========================================================================
// 金十上期所库存周报（futures_stock_shfe_js）
// ===========================================================================

/// 金十财经-上海期货交易所指定交割仓库库存周报（对应 akshare [`futures_stock_shfe_js`]）。
///
/// `date`：`YYYYMMDD`。走 `datacenter-api.jin10.com/reports/list`（`x-app-id` 头）。
///
/// # 返回列
/// 随 `data.keys` 动态变化（首列为品种，其余数值列）
pub fn futures_stock_shfe_js(date: &str) -> Result<Df> {
    let formatted = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
    let mut params = Map::new();
    params.insert("category".into(), Value::String("stock".into()));
    params.insert("date".into(), Value::String(formatted));
    params.insert("attr_id".into(), Value::String("1".into()));
    let url = "https://datacenter-api.jin10.com/reports/list";
    let headers = [
        ("user-agent", UA),
        ("x-app-id", "rU6QIu7JHe2gOUeR"),
        ("x-csrf-token", "x-csrf-token"),
        ("x-version", "1.0.0"),
    ];
    let http = HttpClient::default();
    let v = http.get_json_with_headers(url, &params, &headers, None)?;
    let keys = v
        .get("data")
        .and_then(|d| d.get("keys"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("金十库存周报缺少 data.keys".into()))?;
    let columns_list: Vec<String> = keys
        .iter()
        .filter_map(|k| k.get("name").and_then(Value::as_str).map(String::from))
        .collect();
    let values = v
        .get("data")
        .and_then(|d| d.get("values"))
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("金十库存周报缺少 data.values".into()))?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(values.len());
    for row in values {
        let arr = row
            .as_array()
            .ok_or_else(|| AkshareError::Empty("库存周报行非数组".into()))?;
        rows.push(arr.iter().map(cell).collect());
    }
    let mut df = Df::from_string_rows(
        &columns_list.iter().map(String::as_str).collect::<Vec<_>>(),
        &rows,
    )?;
    if columns_list.len() > 1 {
        let num_cols: Vec<&str> = columns_list[1..].iter().map(String::as_str).collect();
        df.cast_numeric(&num_cols)?;
    }
    Ok(df)
}

// ===========================================================================
// 生意社现期图（futures_spot_sys）
// ===========================================================================

/// 生意社品种 → 现期图 URL 字典（对应 akshare `__get_sys_spot_futures_dict`）。
fn get_sys_spot_futures_dict(text: &str) -> Result<HashMap<String, String>> {
    let doc = Html::parse_document(text);
    let sel = Selector::parse("div.q8 li").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let mut map = HashMap::new();
    for li in doc.select(&sel) {
        let a_sel = Selector::parse("a").map_err(|e| AkshareError::Empty(e.to_string()))?;
        if let Some(a) = li.select(&a_sel).next() {
            let name = a.text().collect::<Vec<_>>().join("").trim().to_string();
            let href = a.value().attr("href").unwrap_or("").to_string();
            if !name.is_empty() && !href.is_empty() {
                map.insert(name, href);
            }
        }
    }
    if map.is_empty() {
        return Err(AkshareError::Empty("生意社品种字典为空".into()));
    }
    Ok(map)
}

/// 把生意社现期表（已按 `header=0, index_col=0` 解析为二维字符串）转置为
/// 「日期, <各日期列>」格式（对应 akshare `pd.read_html(...)[idx].T.reset_index()`）。
///
/// `table`：原始表格（含表头行，`table[0]` 为表头，`table[1..]` 首列为索引/日期）。
/// `metric_cols`：转置后「日期」列应出现的指标名（取原表头 `table[0][1..]`）。
fn transpose_spot_table(table: &[Vec<String>], metric_cols: &[String]) -> Df {
    // 日期列 = 各数据行首列
    let dates: Vec<String> = table
        .iter()
        .skip(1)
        .map(|r| r.first().cloned().unwrap_or_default())
        .collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(metric_cols.len());
    for (mj, m) in metric_cols.iter().enumerate() {
        let col = mj + 1; // 原表头第 1 列起为指标数据列
        let mut row = vec![Some(m.clone())];
        for r in table.iter().skip(1) {
            let v = r.get(col).cloned().filter(|s| !s.is_empty());
            row.push(v);
        }
        rows.push(row);
    }
    let mut cols: Vec<String> = vec!["日期".to_string()];
    cols.extend(dates);
    let df = Df::from_string_rows(&cols.iter().map(String::as_str).collect::<Vec<_>>(), &rows)
        .unwrap_or_else(|_| Df::from_string_rows(&["日期"], &[]).unwrap());
    df
}

/// 生意社-商品与期货-现期图（对应 akshare [`futures_spot_sys`]）。
///
/// `symbol`：期货品种（如 `铜`）；`indicator`：`市场价格` / `基差率` / `主力基差`。
/// 取品种对应 URL，解析 `pd.read_html(header=0, index_col=0)` 第 2/3/4 张表并转置。
///
/// # 返回列
/// `日期, <日期序列...>`（每行一个指标：现货价格/主力合约/最近合约 或 基差率 或 主力基差）
pub fn futures_spot_sys(symbol: &str, indicator: &str) -> Result<Df> {
    let url = "https://www.100ppi.com/sf/792.html";
    let http = HttpClient::default();
    let list_text = http.get_text_with_headers(url, &Map::new(), &[("User-Agent", UA)], None)?;
    let dict = get_sys_spot_futures_dict(&list_text)?;
    let href = dict
        .get(symbol)
        .ok_or_else(|| AkshareError::Param(format!("未知现期品种: {symbol}")))?;
    let detail = http.get_text_with_headers(
        &format!("https://www.100ppi.com{href}"),
        &Map::new(),
        &[("User-Agent", UA)],
        None,
    )?;
    let tables = read_html_tables(&detail)?;
    // 选定表索引：市场价格→[1]，基差率→[2]，主力基差→[3]
    let table_idx = match indicator {
        "市场价格" => 1,
        "基差率" => 2,
        "主力基差" => 3,
        other => {
            return Err(AkshareError::Param(format!(
                "无效 indicator: {other}（可选 市场价格/基差率/主力基差）"
            )))
        }
    };
    let raw = tables
        .get(table_idx)
        .ok_or_else(|| AkshareError::Empty(format!("生意社现期表索引 {table_idx} 缺失")))?;
    if raw.len() < 2 {
        return Df::from_string_rows(&["日期"], &[]);
    }
    // 指标名 = 原表头第 1 列起（对应 akshare 转置后的指标行）
    let metric_cols: Vec<String> = if raw[0].len() > 1 {
        raw[0][1..].to_vec()
    } else {
        match indicator {
            "市场价格" => vec![
                "现货价格".to_string(),
                "主力合约".to_string(),
                "最近合约".to_string(),
            ],
            "基差率" => vec!["基差率".to_string()],
            _ => vec!["主力基差".to_string()],
        }
    };
    let mut df = transpose_spot_table(raw, &metric_cols);
    // 数值化除「日期」外的所有列（akshare 对各指标列 to_numeric(coerce)）
    let names = df.column_names();
    let num_cols: Vec<&str> = names
        .iter()
        .filter(|n| *n != "日期")
        .map(String::as_str)
        .collect();
    if !num_cols.is_empty() {
        df.cast_numeric(&num_cols)?;
    }
    Ok(df)
}

// ===========================================================================
// 东财期货合约详情（futures_contract_detail_em）
// ===========================================================================

/// 东财期货合约详情（对应 akshare [`futures_contract_detail_em`]）。
///
/// `symbol`：合约代码（如 `v2602F`）。从 `quote.eastmoney.com/qihuo/{symbol}.html`
/// 解析详情链接，再取 `futsse-static.eastmoney.com/redis?msgid={inner}_info`。
///
/// # 返回列
/// `item, value`（item 为中文合约要素名）
pub fn futures_contract_detail_em(symbol: &str) -> Result<Df> {
    let url = format!("https://quote.eastmoney.com/qihuo/{symbol}.html");
    let http = HttpClient::default();
    let text = http.get_text_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let doc = Html::parse_document(&text);
    let sel = Selector::parse("div.sidertabbox_tsplit div.onet a")
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    let href = doc
        .select(&sel)
        .next()
        .and_then(|e| e.value().attr("href"))
        .ok_or_else(|| AkshareError::Empty("东财合约详情页未找到详情链接".into()))?
        .to_string();
    let inner = href
        .rsplit('#')
        .next()
        .unwrap_or("")
        .trim_start_matches("futures_")
        .to_string();
    if inner.is_empty() {
        return Err(AkshareError::Empty("东财合约详情 inner_symbol 为空".into()));
    }
    let api = format!("https://futsse-static.eastmoney.com/redis?msgid={inner}_info");
    let v = http.get_json_with_headers(&api, &Map::new(), &[("User-Agent", UA)], None)?;
    let obj = v
        .as_object()
        .ok_or_else(|| AkshareError::Empty("东财合约详情 JSON 非对象".into()))?;
    let mapping: HashMap<&str, &str> = [
        ("vname", "交易品种"),
        ("vcode", "交易代码"),
        ("jydw", "交易单位"),
        ("bjdw", "报价单位"),
        ("market", "上市交易所"),
        ("zxbddw", "最小变动价格"),
        ("zdtbfd", "跌涨停板幅度"),
        ("hyjgyf", "合约交割月份"),
        ("jysj", "交易时间"),
        ("zhjyr", "最后交易日"),
        ("zhjgr", "最后交割日"),
        ("jgpj", "交割品级"),
        ("zcjybzj", "最初交易保证金"),
        ("jgfs", "交割方式"),
    ]
    .into_iter()
    .collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(obj.len());
    for (k, val) in obj {
        let item = mapping
            .get(k.as_str())
            .copied()
            .unwrap_or(k.as_str())
            .to_string();
        rows.push(vec![Some(item), cell(val)]);
    }
    Df::from_string_rows(&["item", "value"], &rows)
}

// ===========================================================================
// 九期网期货手续费（futures_comm_info）
// ===========================================================================

/// 交易所名称（用于切片与标注）。
const QIHUO_EXCHANGES: &[&str] = &[
    "上海期货交易所",
    "大连商品交易所",
    "郑州商品交易所",
    "上海国际能源交易中心",
    "广州期货交易所",
    "中国金融期货交易所",
];

/// `合约品种` "名称(代码)" → (名称, 代码)。
fn split_paren(s: &str) -> (String, String) {
    if let Some(p) = s.find('(') {
        let name = s[..p].trim().to_string();
        let code = s[p + 1..].trim_end_matches(')').trim().to_string();
        (name, code)
    } else {
        (s.trim().to_string(), String::new())
    }
}

/// `涨/跌停板` "x / y" → (涨停板, 跌停板)。
fn split_slash(s: &str) -> (String, String) {
    if let Some(p) = s.find('/') {
        (s[..p].trim().to_string(), s[p + 1..].trim().to_string())
    } else {
        (s.trim().to_string(), String::new())
    }
}

/// 手续费标准 "1.2万分之/3元" → (万分之浮点串, 元串)。
///
/// 与 akshare 一致：分别提取「万分之」前的系数（÷10000）与「/」后「元」前的固定值，
/// 二者可共存（同一单元格含两种计费方式）。
fn parse_fee(s: &str) -> (Option<String>, Option<String>) {
    let wan = if let Some(pos) = s.find("万分之") {
        let num_part = s[..pos].trim_end_matches(['/', ' ']).trim();
        num_part
            .parse::<f64>()
            .ok()
            .map(|x| (x / 10000.0).to_string())
    } else {
        None
    };
    let yuan = if s.contains('元') {
        let seg = s
            .rsplit('/')
            .next()
            .unwrap_or(s)
            .trim_end_matches('元')
            .trim();
        if seg.is_empty() {
            None
        } else {
            Some(seg.to_string())
        }
    } else {
        None
    };
    (wan, yuan)
}

/// 从页面文本提取（手续费更新时间, 价格更新时间）。
fn extract_update_times(text: &str) -> (String, String) {
    let comm = text
        .find("手续费更新时间：")
        .map(|i| {
            let r = &text[i + "手续费更新时间：".len()..];
            r.split('，').next().unwrap_or("").trim().to_string()
        })
        .unwrap_or_default();
    let price = text
        .find("价格更新时间：")
        .map(|i| {
            let r = &text[i + "价格更新时间：".len()..];
            let end = r.find('。').or_else(|| r.find('）')).unwrap_or(r.len());
            r[..end].trim().to_string()
        })
        .unwrap_or_default();
    (comm, price)
}

/// 处理单个交易所切片（对应 akshare `_futures_comm_qihuo_process`）。
///
/// `slice`：该交易所的数据行（每行 15 列）；`name`：交易所名称。
/// 返回 21 列行。
fn process_qihuo_slice(
    slice: &[Vec<String>],
    name: &str,
    times: &(String, String),
) -> Vec<Vec<Option<String>>> {
    let mut out = Vec::with_capacity(slice.len());
    for row in slice {
        let contract = rget(row, 0);
        let (cname, ccode) = split_paren(contract);
        let (up, down) = split_slash(rget(row, 2));
        let margin_buy = rget(row, 3).trim_end_matches('%').to_string();
        let margin_sell = rget(row, 4).trim_end_matches('%').to_string();
        let margin_per = rget(row, 5).trim_end_matches('元').trim().to_string();
        let fee_total = rget(row, 10).trim_end_matches('元').trim().to_string();
        let (open_w, open_y) = parse_fee(rget(row, 6));
        let (yest_w, yest_y) = parse_fee(rget(row, 7));
        let (today_w, today_y) = parse_fee(rget(row, 8));
        out.push(vec![
            Some(name.to_string()),          // 交易所名称
            Some(cname),                     // 合约名称
            Some(ccode),                     // 合约代码
            Some(rget(row, 1).to_string()),  // 现价
            Some(up),                        // 涨停板
            Some(down),                      // 跌停板
            Some(margin_buy),                // 保证金-买开
            Some(margin_sell),               // 保证金-卖开
            Some(margin_per),                // 保证金-每手
            open_w,                          // 手续费标准-开仓-万分之
            open_y,                          // 手续费标准-开仓-元
            yest_w,                          // 手续费标准-平昨-万分之
            yest_y,                          // 手续费标准-平昨-元
            today_w,                         // 手续费标准-平今-万分之
            today_y,                         // 手续费标准-平今-元
            Some(rget(row, 9).to_string()),  // 每跳毛利
            Some(fee_total),                 // 手续费
            Some(rget(row, 11).to_string()), // 每跳净利
            Some(rget(row, 12).to_string()), // 备注
            Some(times.0.clone()),           // 手续费更新时间
            Some(times.1.clone()),           // 价格更新时间
        ]);
    }
    out
}

/// 九期网期货手续费（对应 akshare [`futures_comm_info`]）。
///
/// `symbol`：`所有` 或六家交易所名之一。抓取 `9qihuo.com/qihuoshouxufei`，
/// 按交易所名切片并逐项拆分（合约品种/涨跌停/保证金/手续费），附加更新时间。
///
/// # 返回列
/// `交易所名称, 合约名称, 合约代码, 现价, 涨停板, 跌停板, 保证金-买开, 保证金-卖开,
/// 保证金-每手, 手续费标准-开仓-万分之, 手续费标准-开仓-元, 手续费标准-平昨-万分之,
/// 手续费标准-平昨-元, 手续费标准-平今-万分之, 手续费标准-平今-元, 每跳毛利, 手续费,
/// 每跳净利, 备注, 手续费更新时间, 价格更新时间`
pub fn futures_comm_info(symbol: &str) -> Result<Df> {
    let url = "https://www.9qihuo.com/qihuoshouxufei";
    let http = HttpClient::default();
    let text = http.get_text_with_headers(url, &Map::new(), &[("User-Agent", UA)], None)?;
    let tables = read_html_tables(&text)?;
    let raw = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("九期网手续费无表格".into()))?;
    if raw.len() < 2 {
        return Df::from_string_rows(&QIHUO_OUTPUT_COLS, &[]);
    }
    let times = extract_update_times(&text);
    // 收集各交易所锚点行号（按文档顺序）
    let mut anchors: Vec<(usize, &str)> = Vec::new();
    for (i, r) in raw.iter().enumerate() {
        let c0 = rget(r, 0);
        for ex in QIHUO_EXCHANGES {
            if c0.contains(ex) {
                anchors.push((i, ex));
                break;
            }
        }
    }
    // 决定要包含哪些交易所
    let include: Vec<&str> = if symbol == "所有" {
        QIHUO_EXCHANGES.to_vec()
    } else {
        vec![symbol]
    };
    let mut all_rows: Vec<Vec<Option<String>>> = Vec::new();
    for (p, (idx, ex)) in anchors.iter().enumerate() {
        if !include.contains(ex) {
            continue;
        }
        let next_idx = anchors.get(p + 1).map(|a| a.0).unwrap_or(raw.len());
        // 数据从第 idx+3 行开始（跳过锚点行 + 2 个子表头行）
        if *idx + 3 >= next_idx {
            continue;
        }
        let slice = &raw[idx + 3..next_idx];
        all_rows.extend(process_qihuo_slice(slice, ex, &times));
    }
    let mut df = Df::from_string_rows(&QIHUO_OUTPUT_COLS, &all_rows)?;
    df.cast_numeric(&[
        "现价",
        "涨停板",
        "跌停板",
        "保证金-买开",
        "保证金-卖开",
        "保证金-每手",
        "手续费标准-开仓-万分之",
        "手续费标准-平昨-万分之",
        "手续费标准-平今-万分之",
        "每跳毛利",
        "手续费",
        "每跳净利",
    ])?;
    Ok(df)
}

/// `futures_comm_info` 输出列（21 列）。
const QIHUO_OUTPUT_COLS: [&str; 21] = [
    "交易所名称",
    "合约名称",
    "合约代码",
    "现价",
    "涨停板",
    "跌停板",
    "保证金-买开",
    "保证金-卖开",
    "保证金-每手",
    "手续费标准-开仓-万分之",
    "手续费标准-开仓-元",
    "手续费标准-平昨-万分之",
    "手续费标准-平昨-元",
    "手续费标准-平今-万分之",
    "手续费标准-平今-元",
    "每跳毛利",
    "手续费",
    "每跳净利",
    "备注",
    "手续费更新时间",
    "价格更新时间",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_paren_basic() {
        assert_eq!(
            split_paren("白糖(SR)"),
            ("白糖".to_string(), "SR".to_string())
        );
        assert_eq!(
            split_paren("螺纹钢(RB)"),
            ("螺纹钢".to_string(), "RB".to_string())
        );
        // 无括号：整体作为名称，代码为空（与 akshare split("(") 行为一致）
        assert_eq!(split_paren("无代码"), ("无代码".to_string(), String::new()));
    }

    #[test]
    fn split_slash_basic() {
        assert_eq!(split_slash("5 / 4"), ("5".to_string(), "4".to_string()));
        assert_eq!(split_slash("7"), ("7".to_string(), String::new()));
    }

    #[test]
    fn parse_fee_wan_and_yuan() {
        // 1.2万分之/3元 → 万分之=1.2/10000≈0.00012，元=3
        let (w, y) = parse_fee("1.2万分之/3元");
        let wf: f64 = w.unwrap().parse().unwrap();
        assert!((wf - 0.00012).abs() < 1e-12);
        assert_eq!(y, Some("3".to_string()));
        assert_eq!(parse_fee("3元"), (None, Some("3".to_string())));
        // 1.5万分之 → 万分之≈0.00015，无 元 部分
        let (w2, y2) = parse_fee("1.5万分之");
        let wf2: f64 = w2.unwrap().parse().unwrap();
        assert!((wf2 - 0.00015).abs() < 1e-12);
        assert_eq!(y2, None);
    }

    #[test]
    fn ms_to_shanghai_basic() {
        // 1970-01-01 00:00:00 UTC = 1970-01-01 08:00:00 Shanghai
        assert_eq!(ms_to_shanghai(0), "1970-01-01 08:00:00");
        assert_eq!(ms_to_shanghai(1_000), "1970-01-01 08:00:01");
    }

    #[test]
    fn process_qihuo_slice_columns() {
        let row = vec![
            "白糖(SR)".to_string(),
            "6000".to_string(),
            "5 / 4".to_string(),
            "10%".to_string(),
            "10%".to_string(),
            "5000元".to_string(),
            "1.2万分之/3元".to_string(),
            "1万分之/2元".to_string(),
            "1万分之/2元".to_string(),
            "20".to_string(),
            "5元".to_string(),
            "15".to_string(),
            "备注".to_string(),
            String::new(),
            String::new(),
        ];
        let times = ("2024-01-01".to_string(), "2024-01-01".to_string());
        let out = process_qihuo_slice(&[row], "上海期货交易所", &times);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 21);
        assert_eq!(out[0][0], Some("上海期货交易所".to_string()));
        assert_eq!(out[0][1], Some("白糖".to_string()));
        assert_eq!(out[0][2], Some("SR".to_string()));
        assert_eq!(out[0][4], Some("5".to_string()));
        assert_eq!(out[0][5], Some("4".to_string()));
        let wan: f64 = out[0][9].clone().unwrap().parse().unwrap();
        assert!((wan - 0.00012).abs() < 1e-12);
        assert_eq!(out[0][10], Some("3".to_string()));
    }

    #[test]
    fn join_names_empty_is_dash() {
        assert_eq!(join_names(None), "-");
        let v = json!([{"name": "A"}, {"name": "B"}]);
        assert_eq!(join_names(Some(&v)), "A, B");
    }

    #[test]
    fn transpose_spot_table_shape() {
        // 表头: [日期标签, 现货价格, 主力合约]; 两行日期
        let table = vec![
            vec![
                "".to_string(),
                "现货价格".to_string(),
                "主力合约".to_string(),
            ],
            vec![
                "2024-01-01".to_string(),
                "6000".to_string(),
                "5990".to_string(),
            ],
            vec![
                "2024-01-02".to_string(),
                "6010".to_string(),
                "6000".to_string(),
            ],
        ];
        let metrics = vec!["现货价格".to_string(), "主力合约".to_string()];
        let df = transpose_spot_table(&table, &metrics);
        assert_eq!(df.column_names()[0], "日期");
        assert_eq!(df.column_names()[1], "2024-01-01");
        assert_eq!(df.column_names()[2], "2024-01-02");
        assert_eq!(df.height(), 2);
    }
}
