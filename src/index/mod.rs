//! 指数数据接口。
//!
//! 首批实现（对应 akshare `index/index_zh_em.py`）：
//! - [`index_zh_a_hist`]：中国股票指数历史行情

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    fetch_clist, fetch_kline, fetch_kline_min, fetch_trends, json_value_to_string, kline_to_df,
    min_kline_to_df, push2_urls, KLINE_COLS,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// 指数代码 → 市场标识 映射缓存（对应 akshare `index_code_id_map_em` 的 lru_cache）。
static INDEX_CODE_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

/// 获取指数代码 → 市场标识 映射（对应 akshare `index_code_id_map_em()`）。
pub fn index_code_id_map_em() -> Result<&'static HashMap<String, String>> {
    if let Some(map) = INDEX_CODE_MAP.get() {
        return Ok(map);
    }
    {
        let urls = push2_urls("/api/qt/clist/get");
        let params = json!({
            "pn": "1",
            "pz": "100",
            "po": "1",
            "np": "1",
            "ut": "bd1d9ddb04089700cf9c27f6f7426281",
            "fltt": "2",
            "invt": "2",
            "fid": "f3",
            "fs": "b:MK0010,m:1+t:1,m:0 t:5,m:1+s:3,m:0+t:5,m:2",
            "fields": "f3,f12,f13",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let http = HttpClient::default();
        let df = fetch_clist(&http, &urls, &params)?;

        let mut map = HashMap::new();
        let inner = df.inner();
        if let (Ok(codes), Ok(markets)) = (inner.column("f12"), inner.column("f13")) {
            if let (Ok(codes), Ok(markets)) = (codes.str(), markets.str()) {
                for (c, m) in codes.iter().zip(markets.iter()) {
                    if let (Some(c), Some(m)) = (c, m) {
                        map.insert(c.to_string(), m.to_string());
                    }
                }
            }
        }
        let _ = INDEX_CODE_MAP.set(map);
    }
    INDEX_CODE_MAP
        .get()
        .ok_or_else(|| AkshareError::empty("指数映射初始化失败"))
}

/// 中国股票指数历史行情。
///
/// 对应 akshare [`akshare.index_zh_a_hist`]。
///
/// # 参数
/// - `symbol`: 指数代码，如 `"000001"`（上证指数）
/// - `period`: `daily`/`weekly`/`monthly`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `日期, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 振幅, 涨跌幅, 涨跌额, 换手率`
pub fn index_zh_a_hist(symbol: &str, period: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        _ => return Err(AkshareError::Param(format!("无效 period: {period}"))),
    };
    let http = HttpClient::default();

    // 尝试市场标识：优先查映射，回退 1/0/2/47（对应 akshare 的 fallback 链）
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(map) = index_code_id_map_em() {
        if let Some(m) = map.get(symbol) {
            candidates.push(m.clone());
        }
    }
    for fallback in ["1", "0", "2", "47"] {
        candidates.push(fallback.to_string());
    }

    let mut last_err: Option<AkshareError> = None;
    for market in candidates {
        let secid = format!("{market}.{symbol}");
        match fetch_kline(&http, &secid, klt, "0", start_date, end_date) {
            Ok(klines) if !klines.is_empty() => {
                return kline_to_df(&KLINE_COLS, &klines, None);
            }
            Ok(_) => {
                last_err = Some(AkshareError::empty(format!("{symbol} 无 K 线数据")));
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| AkshareError::empty(format!("{symbol} 无 K 线数据"))))
}

/// 指数分钟级行情（对应 akshare [`akshare.index_zh_a_hist_min_em`]）。
///
/// # 参数
/// - `symbol`: 指数代码，如 `"399006"`（创业板指）
/// - `period`: `"1"`（当日分时）或 `"5"`/`"15"`/`"30"`/`"60"`（分钟 K 线，恒前复权）
/// - `start_date`/`end_date`: `YYYY-MM-DD HH:MM:SS` 区间（含边界）
///
/// # 返回列
/// period=1: `时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 均价`；
/// 其余: `时间, 开盘, 收盘, 最高, 最低, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 换手率`
///
/// 注：akshare 对非 `"1"` 的 period 值直接透传给服务端 `klt`；本实现额外校验
/// 仅接受 `"5"`/`"15"`/`"30"`/`"60"`。分钟级数据为滚动窗口（约最近 8 个月）。
pub fn index_zh_a_hist_min_em(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    if period != "1" && !matches!(period, "5" | "15" | "30" | "60") {
        return Err(AkshareError::Param(format!("无效 period: {period}")));
    }
    let http = HttpClient::default();

    // secid 候选：优先查映射，回退 1/0/47（对应 akshare 的 fallback 链）
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(map) = index_code_id_map_em() {
        if let Some(m) = map.get(symbol) {
            candidates.push(m.clone());
        }
    }
    for fb in ["1", "0", "47"] {
        candidates.push(fb.to_string());
    }

    let mut last_err: Option<AkshareError> = None;
    for market in candidates {
        let secid = format!("{market}.{symbol}");
        let result = if period == "1" {
            let lines = match fetch_trends(&http, &secid, "5", "0") {
                Ok(l) => l,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            if lines.is_empty() {
                last_err = Some(AkshareError::empty(format!("{symbol} 无分时数据")));
                continue;
            }
            let cols = [
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "成交量",
                "成交额",
                "均价",
            ];
            min_kline_to_df(&lines, start_date, end_date, &cols, &cols, &cols[1..])
        } else {
            let lines = match fetch_kline_min(&http, &secid, period, "1") {
                Ok(l) => l,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            if lines.is_empty() {
                last_err = Some(AkshareError::empty(format!("{symbol} 无分钟数据")));
                continue;
            }
            let src = [
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "成交量",
                "成交额",
                "振幅",
                "涨跌幅",
                "涨跌额",
                "换手率",
            ];
            let out = [
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "涨跌幅",
                "涨跌额",
                "成交量",
                "成交额",
                "振幅",
                "换手率",
            ];
            min_kline_to_df(&lines, start_date, end_date, &src, &out, &out[1..])
        };
        match result {
            Ok(df) if df.height() > 0 => return Ok(df),
            Ok(_) => last_err = Some(AkshareError::empty(format!("{symbol} 无分钟数据"))),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| AkshareError::empty(format!("{symbol} 无分钟数据"))))
}

// === BATCH37-C 东财全球指数（push2 clist，fltt=1 原始值百分位）===
//
// 对应 akshare `index/index_global_em.py`。`fs` 固定 60 个全球指数 secid，
// `data.diff` 为「序号→行」对象；数值列 ÷100（fltt=1），最新行情时间为
// 秒级时间戳 → Asia/Shanghai。

/// 东财-全球指数-实时行情（对应 akshare [`akshare.index_global_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 开盘价, 最高价, 最低价, 昨收价,
/// 振幅, 最新行情时间`
pub fn index_global_spot_em() -> Result<Df> {
    const FS: &str =
        "i:1.000001,i:0.399001,i:0.399005,i:0.399006,i:1.000300,i:100.HSI,i:100.HSCEI,i:124.HSCCI,\
i:100.TWII,i:100.N225,i:100.KOSPI200,i:100.KS11,i:100.STI,i:100.SENSEX,i:100.KLSE,i:100.SET,\
i:100.PSI,i:100.KSE100,i:100.VNINDEX,i:100.JKSE,i:100.CSEALL,i:100.SX5E,i:100.FTSE,i:100.MCX,\
i:100.AXX,i:100.FCHI,i:100.GDAXI,i:100.RTS,i:100.IBEX,i:100.PSI20,i:100.OMXC20,i:100.BFX,\
i:100.AEX,i:100.WIG,i:100.OMXSPI,i:100.SSMI,i:100.HEX,i:100.OSEBX,i:100.ATX,i:100.MIB,\
i:100.ASE,i:100.ICEXI,i:100.PX,i:100.ISEQ,i:100.DJIA,i:100.SPX,i:100.NDX,i:100.TSX,\
i:100.BVSP,i:100.MXX,i:100.AS51,i:100.AORD,i:100.NZ50,i:100.UDI,i:100.BDI,i:100.CRB";
    const URL: &str = "https://push2.eastmoney.com/api/qt/clist/get";
    let params = json!({
        "np": "2",
        "fltt": "1",
        "invt": "2",
        "fs": FS,
        "fields": "f12,f13,f14,f292,f1,f2,f4,f3,f152,f17,f18,f15,f16,f7,f124",
        "fid": "f3",
        "pn": "1",
        "pz": "200",
        "po": "1",
        "dect": "1",
        "wbp2u": "|0|0|0|web",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(URL, &params, None)?;
    let diff = value
        .get("data")
        .and_then(|d| d.get("diff"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut rows_vec: Vec<Value> = Vec::new();
    match diff {
        Value::Array(arr) => rows_vec = arr,
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            rows_vec = entries.into_iter().map(|(_, v)| v).collect();
        }
        _ => {}
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows_vec.len());
    for (i, row) in rows_vec.iter().enumerate() {
        let f = |k: &str| row.get(k).and_then(json_value_to_string);
        // fltt=1 原始值百分位 → ÷100（akshare /100）
        let pct = |k: &str| -> Option<String> {
            f(k).and_then(|s| s.parse::<f64>().ok().map(|v| (v / 100.0).to_string()))
        };
        // 最新行情时间：秒级时间戳 → Asia/Shanghai
        let ts = f("f124").and_then(|s| s.parse::<i64>().ok()).map(|sec| {
            use chrono::TimeZone;
            chrono::Utc
                .timestamp_opt(sec, 0)
                .single()
                .map(|dt| {
                    dt.with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_default()
        });
        out.push(vec![
            Some((i + 1).to_string()),
            f("f12"),
            f("f14"),
            pct("f2"),
            pct("f4"),
            pct("f3"),
            pct("f17"),
            pct("f15"),
            pct("f16"),
            pct("f18"),
            pct("f7"),
            ts,
        ]);
    }
    const COLS: [&str; 12] = [
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "开盘价",
        "最高价",
        "最低价",
        "昨收价",
        "振幅",
        "最新行情时间",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&[
        "最新价",
        "涨跌额",
        "涨跌幅",
        "开盘价",
        "最高价",
        "最低价",
        "昨收价",
        "振幅",
    ])?;
    Ok(df)
}

/// 东财-全球指数-历史行情（对应 akshare [`akshare.index_global_hist_em`]）。
///
/// - `symbol`: 指数名称（由 [`index_global_spot_em`] 获取），如 `"美元指数"`
///
/// `push2his` kline（`fqt=1` 后复权），14 字段取 `日期,今开,最新价,最高,最低,振幅`，
/// 附 `代码, 名称`。
///
/// # 返回列
/// `日期, 代码, 名称, 今开, 最新价, 最高, 最低, 振幅`
pub fn index_global_hist_em(symbol: &str) -> Result<Df> {
    let (market, code) = match symbol {
        "波罗的海BDI指数" => ("100", "BDI"),
        "葡萄牙PSI20" => ("100", "PSI20"),
        "菲律宾马尼拉" => ("100", "PSI"),
        "泰国SET" => ("100", "SET"),
        "俄罗斯RTS" => ("100", "RTS"),
        "巴基斯坦卡拉奇" => ("100", "KSE100"),
        "越南胡志明" => ("100", "VNINDEX"),
        "红筹指数" => ("124", "HSCCI"),
        "印尼雅加达综合" => ("100", "JKSE"),
        "希腊雅典ASE" => ("100", "ASE"),
        "墨西哥BOLSA" => ("100", "MXX"),
        "挪威OSEBX" => ("100", "OSEBX"),
        "巴西BOVESPA" => ("100", "BVSP"),
        "波兰WIG" => ("100", "WIG"),
        "印度孟买SENSEX" => ("100", "SENSEX"),
        "布拉格指数" => ("100", "PX"),
        "荷兰AEX" => ("100", "AEX"),
        "冰岛ICEX" => ("100", "ICEXI"),
        "斯里兰卡科伦坡" => ("100", "CSEALL"),
        "富时新加坡海峡时报" => ("100", "STI"),
        "富时意大利MIB" => ("100", "MIB"),
        "路透CRB商品指数" => ("100", "CRB"),
        "比利时BFX" => ("100", "BFX"),
        "富时AIM全股" => ("100", "AXX"),
        "新西兰50" => ("100", "NZ50"),
        "上证指数" => ("1", "000001"),
        "国企指数" => ("100", "HSCEI"),
        "沪深300" => ("1", "000300"),
        "英国富时100" => ("100", "FTSE"),
        "中小100" => ("0", "399005"),
        "瑞士SMI" => ("100", "SSMI"),
        "西班牙IBEX35" => ("100", "IBEX"),
        "瑞典OMXSPI" => ("100", "OMXSPI"),
        "爱尔兰综合" => ("100", "ISEQ"),
        "韩国KOSPI" => ("100", "KS11"),
        "深证成指" => ("0", "399001"),
        "韩国KOSPI200" => ("100", "KOSPI200"),
        "芬兰赫尔辛基" => ("100", "HEX"),
        "恒生指数" => ("100", "HSI"),
        "欧洲斯托克50" => ("100", "SX5E"),
        "美元指数" => ("100", "UDI"),
        "法国CAC40" => ("100", "FCHI"),
        "台湾加权" => ("100", "TWII"),
        "英国富时250" => ("100", "MCX"),
        "富时马来西亚KLCI" => ("100", "KLSE"),
        "OMX哥本哈根20" => ("100", "OMXC20"),
        "道琼斯" => ("100", "DJIA"),
        "奥地利ATX" => ("100", "ATX"),
        "加拿大S&P/TSX" => ("100", "TSX"),
        "德国DAX30" => ("100", "GDAXI"),
        "创业板指" => ("0", "399006"),
        "澳大利亚普通股" => ("100", "AORD"),
        "标普500" => ("100", "SPX"),
        "澳大利亚标普200" => ("100", "AS51"),
        "日经225" => ("100", "N225"),
        "纳斯达克" => ("100", "NDX"),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，请用 index_global_spot_em 获取指数名称"
            )))
        }
    };
    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let params = json!({
        "secid": format!("{market}.{code}"),
        "klt": "101",
        "fqt": "1",
        "lmt": "50000",
        "end": "20500000",
        "iscca": "1",
        "fields1": "f1,f2,f3,f4,f5,f6,f7,f8",
        "fields2": "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64",
        "ut": "f057cbcbce2a86e2866ab8877db1d059",
        "forcect": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(url, &params, None)?;
    let data = value
        .get("data")
        .ok_or_else(|| AkshareError::Empty("全球指数历史无 data".into()))?;
    let klines = data
        .get("klines")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let code_str = data
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or(code)
        .to_string();
    let name_str = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(symbol)
        .to_string();
    // 14 字段：日期,今开,最新价,最高,最低,-,-,振幅,-,-,-,-,-,-（取 0,1,2,3,4,7）
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for line in klines.iter().filter_map(Value::as_str) {
        let f: Vec<&str> = line.split(',').collect();
        let pick = |i: usize| f.get(i).map(|s| Some((*s).to_string())).unwrap_or(None);
        out.push(vec![
            pick(0),
            Some(code_str.clone()),
            Some(name_str.clone()),
            pick(1),
            pick(2),
            pick(3),
            pick(4),
            pick(7),
        ]);
    }
    const COLS: [&str; 8] = [
        "日期",
        "代码",
        "名称",
        "今开",
        "最新价",
        "最高",
        "最低",
        "振幅",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["今开", "最新价", "最高", "最低", "振幅"])?;
    Ok(df)
}

/// 新浪-指数成份股（对应 akshare [`akshare.index_stock_cons_sina`]）。
///
/// - `symbol`: 指数代码（如 `"000300"`；仅部分指数可用）
///
/// 沪深300 走 `Market_Center.getHQNodeData`（node=hs300）分页；其余走
/// `getHQNodeDataSimple`（node=zhishu_{symbol}）。
///
/// # 返回列
/// 原键列（`symbol, code, name, trade, ...`）
pub fn index_stock_cons_sina(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    if symbol == "000300" {
        let count_url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeStockCountSimple";
        let mut count_params = Map::new();
        count_params.insert("node".into(), Value::String("hs300".into()));
        let count_text = http.get_text(count_url, &count_params, None)?;
        let total: u64 = count_text.trim().parse().unwrap_or(0);
        let total_pages = total.div_ceil(80) + 1;

        let url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeData";
        let mut rows: Vec<Value> = Vec::new();
        for page in 1..total_pages {
            let params = json!({
                "page": page.to_string(),
                "num": "80",
                "sort": "symbol",
                "asc": "1",
                "node": "hs300",
                "symbol": "",
                "_s_r_a": "init",
            });
            let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
            match http.get_text(url, &params, None) {
                Ok(t) => {
                    if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&t) {
                        rows.extend(arr);
                    }
                }
                Err(_) => break,
            }
            let delay: f64 = rand::random_range(0.5..1.5);
            std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        }
        Df::from_json_rows(&rows)
    } else {
        let url = "https://vip.stock.finance.sina.com.cn/quotes_service/api/json_v2.php/Market_Center.getHQNodeDataSimple";
        let params = json!({
            "page": "1",
            "num": "3000",
            "sort": "symbol",
            "asc": "1",
            "node": format!("zhishu_{symbol}"),
            "_s_r_a": "setlen",
        });
        let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let text = http.get_text(url, &params, None)?;
        let rows: Vec<Value> =
            serde_json::from_str(&text).map_err(|e| AkshareError::json(url, e.to_string()))?;
        Df::from_json_rows(&rows)
    }
}

/// 聚宽-指数列表（对应 akshare [`akshare.index_stock_info`]）。
///
/// `joinquant.com/data/dict/indexData` 首张 HTML 表，取前 3 列。
///
/// # 返回列
/// `index_code, display_name, publish_date`
pub fn index_stock_info() -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text(
        "https://www.joinquant.com/data/dict/indexData",
        &Map::new(),
        None,
    )?;
    let tables = crate::core::html::read_html_tables(&text)?;
    let table = tables
        .first()
        .ok_or_else(|| AkshareError::Empty("聚宽指数列表页面缺少表格".into()))?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(table.len());
    for r in table.iter().skip(1) {
        let cells: Vec<Option<String>> = r.iter().map(|s| Some(s.clone())).collect();
        if cells.len() < 3 {
            continue;
        }
        // 指数代码形如 "000300.SH"，取 "." 前部分（akshare split）
        let code = cells[0]
            .clone()
            .map(|s| s.split('.').next().unwrap_or("").to_string());
        out.push(vec![code, cells[1].clone(), cells[2].clone()]);
    }
    Df::from_string_rows(&["index_code", "display_name", "publish_date"], &out)
}

// === BATCH37-D 中证指数成分股/权重（csindex xls 下载）===
//
// 对应 akshare `index/index_cons.py` 的 `index_stock_cons_csindex` /
// `index_stock_cons_weight_csindex`。`oss-ch.csindex.com.cn` 下载 xls，
// calamine 解析首个工作表为字符串二维数组。

/// calamine 解析 xls 首个工作表为字符串二维数组。
fn csindex_xls_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    use calamine::{Data, Reader, Xls};
    let cur = std::io::Cursor::new(bytes.to_vec());
    let mut wb = Xls::new(cur).map_err(|e| AkshareError::Empty(format!("xls 解析失败: {e}")))?;
    let range = wb
        .worksheet_range_at(0)
        .ok_or_else(|| AkshareError::Empty("xls 无工作表".into()))?
        .map_err(|e| AkshareError::Empty(format!("读取 xls 工作表失败: {e}")))?;
    let mut rows = Vec::with_capacity(range.height());
    for r in range.rows() {
        let mut row = Vec::with_capacity(r.len());
        for c in r {
            row.push(match c {
                Data::Empty => String::new(),
                Data::String(s) => s.clone(),
                Data::Float(f) => {
                    let v = *f;
                    if v.fract() == 0.0 {
                        format!("{}", v as i64)
                    } else {
                        format!("{v}")
                    }
                }
                Data::Int(i) => i.to_string(),
                Data::Bool(b) => b.to_string(),
                Data::DateTime(_) => String::new(),
                Data::Error(e) => format!("{e:?}"),
                other => other.to_string(),
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// 中证指数 xls 下载（首行表头 + 数据行）。
fn csindex_xls(url: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(url, &Map::new(), &[], None)?;
    let all = csindex_xls_rows(&bytes)?;
    let mut iter = all.into_iter();
    let header = iter.next().unwrap_or_default();
    let data: Vec<Vec<String>> = iter.collect();
    Ok((header, data))
}

/// 中证指数-成份股目录（对应 akshare [`akshare.index_stock_cons_csindex`]）。
///
/// - `symbol`: 指数代码，如 `"000300"`
///
/// # 返回列
/// `日期, 指数代码, 指数名称, 指数英文名称, 成分券代码, 成分券名称,
/// 成分券英文名称, 交易所, 交易所英文名称`
pub fn index_stock_cons_csindex(symbol: &str) -> Result<Df> {
    let url = format!(
        "https://oss-ch.csindex.com.cn/static/html/csindex/public/uploads/file/autofile/cons/{symbol}cons.xls"
    );
    let (header, data) = csindex_xls(&url)?;
    let pick = |row: &[String], i: usize| row.get(i).cloned().map(Some).unwrap_or(None);
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for row in &data {
        let mut r: Vec<Option<String>> = row.iter().map(|s| Some(s.clone())).collect();
        // 指数代码/成分券代码 zfill(6)
        for idx in [1, 4] {
            if let Some(Some(s)) = r.get_mut(idx) {
                *s = format!("{:0>6}", s);
            }
        }
        out.push(r);
    }
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_date(&["日期"])?;
    let _ = pick;
    Ok(df)
}

/// 中证指数-样本权重（对应 akshare [`akshare.index_stock_cons_weight_csindex`]）。
///
/// - `symbol`: 指数代码，如 `"000300"`
///
/// # 返回列
/// `日期, 指数代码, 指数名称, 指数英文名称, 成分券代码, 成分券名称,
/// 成分券英文名称, 交易所, 交易所英文名称, 权重`
pub fn index_stock_cons_weight_csindex(symbol: &str) -> Result<Df> {
    let url = format!(
        "https://oss-ch.csindex.com.cn/static/html/csindex/public/uploads/file/autofile/closeweight/{symbol}closeweight.xls"
    );
    let (header, data) = csindex_xls(&url)?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(data.len());
    for row in &data {
        let mut r: Vec<Option<String>> = row.iter().map(|s| Some(s.clone())).collect();
        for idx in [1, 4] {
            if let Some(Some(s)) = r.get_mut(idx) {
                *s = format!("{:0>6}", s);
            }
        }
        out.push(r);
    }
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["权重"])?;
    Ok(df)
}

// === BATCH38-B 国证指数 cni 系列（index_all_cni / index_hist_cni / index_detail_*）===
//
// 对应 akshare `index/index_cni.py`。indexList JSON、getIndexDailyDataWithDataFormat
// JSON、sample-detail xls 下载（复用 `csindex_xls_rows`）。

/// 国证指数-最近交易日所有指数（对应 akshare [`akshare.index_all_cni`]）。
///
/// `cnindex.com.cn/index/indexList` JSON，25 列位置式 → select 10 列；
/// 成交量/成交额/总市值/自由流通市值 除以 10^5/10^8。
///
/// # 返回列
/// `指数代码, 指数简称, 样本数, 收盘点位, 涨跌幅, PE滚动, 成交量, 成交额, 总市值, 自由流通市值`
pub fn index_all_cni() -> Result<Df> {
    let params = json!({
        "channelCode": "-1",
        "rows": "2000",
        "pageNum": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json("https://www.cnindex.com.cn/index/indexList", &params, None)?;
    let rows = value
        .get("data")
        .and_then(|d| d.get("rows"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 25 列位置式：_,_,指数代码,_,_,_,_,_,指数简称,_,_,_,样本数,收盘点位,涨跌幅,_,PE滚动,_,成交量,成交额,总市值,自由流通市值,_,_,_
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = r.as_object().cloned().unwrap_or_default();
        let values: Vec<Option<String>> = obj
            .values()
            .take(25)
            .map(|v| match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect();
        let pick = |i: usize| values.get(i).cloned().flatten();
        out.push(vec![
            pick(2),
            pick(8),
            pick(12),
            pick(13),
            pick(14),
            pick(16),
            pick(18),
            pick(19),
            pick(20),
            pick(21),
        ]);
    }
    const COLS: [&str; 10] = [
        "指数代码",
        "指数简称",
        "样本数",
        "收盘点位",
        "涨跌幅",
        "PE滚动",
        "成交量",
        "成交额",
        "总市值",
        "自由流通市值",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.scale("成交量", 1.0 / 100000.0)?;
    df.scale("成交额", 1.0 / 100000000.0)?;
    df.scale("总市值", 1.0 / 100000000.0)?;
    df.scale("自由流通市值", 1.0 / 100000000.0)?;
    Ok(df)
}

/// 国证指数-指数历史行情（对应 akshare [`akshare.index_hist_cni`]）。
///
/// - `symbol`: 指数代码；`start_date`/`end_date`: `YYYYMMDD`
///
/// `hq.cnindex.com.cn getIndexDailyDataWithDataFormat` JSON，11 列位置式 → select 8 列；
/// 涨跌幅去 `%` 后 ÷100，按日期升序。
///
/// # 返回列
/// `日期, 开盘价, 最高价, 最低价, 收盘价, 涨跌幅, 成交量, 成交额`
pub fn index_hist_cni(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let sd = cnindex_fmt_date(start_date);
    let ed = cnindex_fmt_date(end_date);
    let params = json!({
        "indexCode": symbol,
        "startDate": sd,
        "endDate": ed,
        "frequency": "day",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(
        "http://hq.cnindex.com.cn/market/market/getIndexDailyDataWithDataFormat",
        &params,
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 11 列位置式：日期,_,最高价,开盘价,最低价,收盘价,_,涨跌幅,成交额,成交量,_
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = r.as_object().cloned().unwrap_or_default();
        let values: Vec<Option<String>> = obj
            .values()
            .take(11)
            .map(|v| match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect();
        let pick = |i: usize| values.get(i).cloned().flatten();
        // 涨跌幅去 % → ÷100
        let pct = pick(7).map(|s| s.replace('%', ""));
        out.push(vec![
            pick(0),
            pick(3),
            pick(2),
            pick(4),
            pick(5),
            pct,
            pick(9),
            pick(8),
        ]);
    }
    const COLS: [&str; 8] = [
        "日期",
        "开盘价",
        "最高价",
        "最低价",
        "收盘价",
        "涨跌幅",
        "成交量",
        "成交额",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&COLS[1..])?;
    df.scale("涨跌幅", 1.0 / 100.0)?;
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

/// `YYYYMMDD` → `YYYY-MM-DD`。
fn cnindex_fmt_date(d: &str) -> String {
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..])
    } else {
        d.to_string()
    }
}

/// 国证指数-样本详情（指定日期样本成份，对应 akshare [`akshare.index_detail_cni`]）。
///
/// - `symbol`: 指数代码，如 `"399001"`
///
/// `sample-detail/download-history` xls 下载，6 列；样本代码 zfill(6)。
///
/// # 返回列
/// `日期, 样本代码, 样本简称, 所属行业, 总市值, 权重`
pub fn index_detail_cni(symbol: &str) -> Result<Df> {
    cnindex_sample_detail(symbol, "download-history")
}

/// 国证指数-样本详情-历史样本（对应 akshare [`akshare.index_detail_hist_cni`]）。
///
/// - `symbol`: 指数代码
///
/// # 返回列
/// `日期, 样本代码, 样本简称, 所属行业, 总市值, 权重`
pub fn index_detail_hist_cni(symbol: &str) -> Result<Df> {
    cnindex_sample_detail(symbol, "download-history")
}

/// 国证指数-样本详情-历史调样（对应 akshare [`akshare.index_detail_hist_adjust_cni`]）。
///
/// - `symbol`: 指数代码
///
/// `sample-detail/download-adjustment` xls 下载，原键列（样本代码 zfill(6)）。
///
/// # 返回列
/// 原键列
pub fn index_detail_hist_adjust_cni(symbol: &str) -> Result<Df> {
    let url =
        format!("https://www.cnindex.com.cn/sample-detail/download-adjustment?indexcode={symbol}");
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[], None)?;
    let all = match csindex_xls_rows(&bytes) {
        Ok(v) => v,
        Err(_) => return Df::from_string_rows(&["样本代码"], &[]),
    };
    let mut iter = all.into_iter();
    let header = iter.next().unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for row in iter {
        let mut r: Vec<Option<String>> = row.iter().map(|s| Some(s.clone())).collect();
        if let Some(Some(s)) = r.first_mut() {
            *s = format!("{:0>6}", s);
        }
        out.push(r);
    }
    let col_refs: Vec<&str> = header.iter().map(String::as_str).collect();
    Df::from_string_rows(&col_refs, &out)
}

/// 国证指数样本 xls 下载公共实现（6 列契约）。
fn cnindex_sample_detail(symbol: &str, endpoint: &str) -> Result<Df> {
    let url = format!("https://www.cnindex.com.cn/sample-detail/{endpoint}?indexcode={symbol}");
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[], None)?;
    let all = csindex_xls_rows(&bytes)?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for row in all.iter().skip(1) {
        let mut r: Vec<Option<String>> = row.iter().map(|s| Some(s.clone())).collect();
        // 样本代码 zfill(6)
        if let Some(Some(s)) = r.get_mut(1) {
            *s = format!("{:0>6}", s);
        }
        rows.push(r);
    }
    const COLS: [&str; 6] = ["日期", "样本代码", "样本简称", "所属行业", "总市值", "权重"];
    let mut df = Df::from_string_rows(&COLS, &rows)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["总市值", "权重"])?;
    Ok(df)
}

// === BATCH39-C 申万宏源研究-指数（index_hist_sw 等）===
//
// 对应 akshare `index/index_research_sw.py`。`swsresearch.com/institute-sw/api/`
// 需跳过 SSL 验证（HttpClient 已 `danger_accept_invalid_certs(true)`）。

/// 申万宏源-指数历史数据（对应 akshare [`akshare.index_hist_sw`]）。
///
/// - `symbol`: 指数代码，如 `"801030"`；`period`: `"day"` / `"week"` / `"month"`
///
/// `institute-sw/api/index_publish/trend/` JSON，键映射后 select 8 列。
///
/// # 返回列
/// `代码, 日期, 收盘, 开盘, 最高, 最低, 成交量, 成交额`
pub fn index_hist_sw(symbol: &str, period: &str) -> Result<Df> {
    let period_map = match period {
        "day" => "DAY",
        "week" => "WEEK",
        "month" => "MONTH",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 period: {other}，可选 day/week/month"
            )))
        }
    };
    let params = json!({
        "swindexcode": symbol,
        "period": period_map,
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://www.swsresearch.com/institute-sw/api/index_publish/trend/",
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("swindexcode"),
            f("bargaindate"),
            f("closeindex"),
            f("openindex"),
            f("maxindex"),
            f("minindex"),
            f("bargainamount"),
            f("bargainsum"),
        ]);
    }
    const COLS: [&str; 8] = [
        "代码",
        "日期",
        "收盘",
        "开盘",
        "最高",
        "最低",
        "成交量",
        "成交额",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&COLS[2..])?;
    Ok(df)
}

/// 申万宏源-指数成分股（对应 akshare [`akshare.index_component_sw`]）。
///
/// - `symbol`: 指数代码，如 `"801001"`
///
/// `institute-sw/api/index_publish/details/component_stocks/` JSON
/// （`data.results`），键映射后 select 5 列。
///
/// # 返回列
/// `序号, 证券代码, 证券名称, 最新权重, 计入日期`
pub fn index_component_sw(symbol: &str) -> Result<Df> {
    let params = json!({
        "swindexcode": symbol,
        "page": "1",
        "page_size": "10000",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://www.swsresearch.com/institute-sw/api/index_publish/details/component_stocks/",
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(|d| d.get("results"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()),
            f("stockcode"),
            f("stockname"),
            f("newweight"),
            f("beginningdate"),
        ]);
    }
    const COLS: [&str; 5] = ["序号", "证券代码", "证券名称", "最新权重", "计入日期"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["计入日期"])?;
    df.cast_numeric(&["最新权重"])?;
    Ok(df)
}

// === BATCH40-A 申万宏源研究-指数（index_min_sw / index_realtime_sw / index_analysis_*）===
//
// 对应 akshare `index/index_research_sw.py`。同 `institute-sw/api` 跳过 SSL 验证。

/// 申万宏源-指数分时数据（对应 akshare [`akshare.index_min_sw`]）。
///
/// - `symbol`: 指数代码，如 `"801001"`
///
/// `institute-sw/api/index_publish/details/timelines/` JSON，键映射后 select 5 列。
///
/// # 返回列
/// `代码, 名称, 价格, 日期, 时间`
pub fn index_min_sw(symbol: &str) -> Result<Df> {
    let params = json!({ "swindexcode": symbol });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://www.swsresearch.com/institute-sw/api/index_publish/details/timelines/",
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("l1"),
            f("l2"),
            f("l8"),
            f("trading_date"),
            f("trading_time"),
        ]);
    }
    const COLS: [&str; 5] = ["代码", "名称", "价格", "日期", "时间"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["价格"])?;
    Ok(df)
}

/// 申万宏源-指数系列实时行情（对应 akshare [`akshare.index_realtime_sw`]）。
///
/// - `symbol`: `"市场表征"` / `"一级行业"` / `"二级行业"` / `"风格指数"` /
///   `"大类风格指数"` / `"金创指数"`
///
/// `institute-sw/api/index_publish/current/` 分页（page_size=50）。
///
/// # 返回列
/// `指数代码, 指数名称, 昨收盘, 今开盘, 最新价, 成交额, 成交量, 最高价, 最低价`
pub fn index_realtime_sw(symbol: &str) -> Result<Df> {
    let url = "https://www.swsresearch.com/institute-sw/api/index_publish/current/";
    let http = HttpClient::default();
    let headers: &[(&str, &str)] = &[(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
    )];
    let mut params = json!({
        "page": "1",
        "page_size": "50",
        "indextype": symbol,
    });
    let params_map: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let first = http.get_json_with_headers(url, &params_map, headers, None)?;
    let total_page = first
        .get("data")
        .and_then(|d| d.get("count"))
        .and_then(Value::as_u64)
        .map(|n| n.div_ceil(50).max(1))
        .unwrap_or(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v
            .get("data")
            .and_then(|d| d.get("results"))
            .and_then(Value::as_array)
        {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=total_page {
        params = json!({
            "page": page.to_string(),
            "page_size": "50",
            "indextype": symbol,
        });
        let pm: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_json_with_headers(url, &pm, headers, None) {
            Ok(v) => append(&v, &mut rows),
            Err(_) => break,
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("swindexcode"),
            f("swindexname"),
            f("precloseindex"),
            f("openindex"),
            f("currentindex"),
            f("bargainsum"),
            f("bargainamount"),
            f("maxindex"),
            f("minindex"),
        ]);
    }
    const COLS: [&str; 9] = [
        "指数代码",
        "指数名称",
        "昨收盘",
        "今开盘",
        "最新价",
        "成交额",
        "成交量",
        "最高价",
        "最低价",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&COLS[2..])?;
    Ok(df)
}

/// 申万宏源-指数分析公共实现（index_analysis_report/，type=DAY/WEEK/MONTH）。
fn index_analysis_sw_base(symbol: &str, start_date: &str, end_date: &str, typ: &str) -> Result<Df> {
    let url = "https://www.swsresearch.com/institute-sw/api/index_analysis/index_analysis_report/";
    let http = HttpClient::default();
    let headers: &[(&str, &str)] = &[(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
    )];
    let sd = cnindex_fmt_date(start_date);
    let ed = cnindex_fmt_date(end_date);
    let mut params = json!({
        "page": "1",
        "page_size": "50",
        "index_type": symbol,
        "start_date": sd,
        "end_date": ed,
        "type": typ,
        "swindexcode": "all",
    });
    let params_map: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let first = http.get_json_with_headers(url, &params_map, headers, None)?;
    let total_page = first
        .get("data")
        .and_then(|d| d.get("count"))
        .and_then(Value::as_u64)
        .map(|n| n.div_ceil(50).max(1))
        .unwrap_or(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v
            .get("data")
            .and_then(|d| d.get("results"))
            .and_then(Value::as_array)
        {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=total_page {
        params = json!({
            "page": page.to_string(),
            "page_size": "50",
            "index_type": symbol,
            "start_date": sd,
            "end_date": ed,
            "type": typ,
            "swindexcode": "all",
        });
        let pm: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_json_with_headers(url, &pm, headers, None) {
            Ok(v) => append(&v, &mut rows),
            Err(_) => break,
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            f("swindexcode"),
            f("swindexname"),
            f("bargaindate"),
            f("closeindex"),
            f("bargainamount"),
            f("markup"),
            f("turnoverrate"),
            f("pe"),
            f("pb"),
            f("meanprice"),
            f("bargainsumrate"),
            f("negotiablessharesum1"),
            f("negotiablessharesum2"),
            f("dp"),
        ]);
    }
    const COLS: [&str; 14] = [
        "指数代码",
        "指数名称",
        "发布日期",
        "收盘指数",
        "成交量",
        "涨跌幅",
        "换手率",
        "市盈率",
        "市净率",
        "均价",
        "成交额占比",
        "流通市值",
        "平均流通市值",
        "股息率",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["发布日期"])?;
    df.cast_numeric(&COLS[3..])?;
    df = df.sort_by("发布日期", true, false)?;
    Ok(df)
}

/// 申万宏源-指数分析-日报（对应 akshare [`akshare.index_analysis_daily_sw`]）。
///
/// - `symbol`: `"市场表征"` / `"一级行业"` / `"二级行业"` / `"风格指数"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `指数代码, 指数名称, 发布日期, 收盘指数, 成交量, 涨跌幅, 换手率, 市盈率,
/// 市净率, 均价, 成交额占比, 流通市值, 平均流通市值, 股息率`
pub fn index_analysis_daily_sw(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    index_analysis_sw_base(symbol, start_date, end_date, "DAY")
}

/// 申万宏源-指数分析-周报（对应 akshare [`akshare.index_analysis_weekly_sw`]）。
pub fn index_analysis_weekly_sw(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    index_analysis_sw_base(symbol, start_date, end_date, "WEEK")
}

/// 申万宏源-指数分析-月报（对应 akshare [`akshare.index_analysis_monthly_sw`]）。
pub fn index_analysis_monthly_sw(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    index_analysis_sw_base(symbol, start_date, end_date, "MONTH")
}

/// 申万宏源-周/月报表-日期序列（对应 akshare [`akshare.index_analysis_week_month_sw`]）。
///
/// - `symbol`: `"week"` / `"month"`
///
/// # 返回列
/// `date`
pub fn index_analysis_week_month_sw(symbol: &str) -> Result<Df> {
    let params = json!({ "type": symbol.to_uppercase() });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://www.swsresearch.com/institute-sw/api/index_analysis/week_month_datetime/",
        &params,
        &[(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
        )],
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(vec![r.get("bargaindate").and_then(json_value_to_string)]);
    }
    let mut df = Df::from_string_rows(&["date"], &out)?;
    df.cast_date(&["date"])?;
    df = df.sort_by("date", true, false)?;
    Ok(df)
}

// === BATCH41-A 财新数据-指数报告（cxIndexTrendInfo，type 分指标）===
//
// 对应 akshare `index/index_cx.py` 的 19 个 `index_*_cx` 函数。同源
// `yun.ccxe.com.cn/api/index/pro/cxIndexTrendInfo`，仅 `type` 参数不同；
// 响应 `data` 每项 `{changeRate, data, time}`（time 为毫秒时间戳），
// 输出 `日期, {指数名}, 变化值/变化幅度` 三列。

/// 财新指数趋势公共实现。
fn cx_index_trend(typ: &str, value_col: &str, index_col: &str) -> Result<Df> {
    let params = json!({ "type": typ });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(
        "https://yun.ccxe.com.cn/api/index/pro/cxIndexTrendInfo",
        &params,
        None,
    )?;
    let rows = value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        // time 毫秒 → Asia/Shanghai 日期
        let date = f("time").and_then(|s| s.parse::<i64>().ok()).map(|ms| {
            use chrono::TimeZone;
            chrono::Utc
                .timestamp_millis_opt(ms)
                .single()
                .map(|dt| {
                    dt.with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
                        .format("%Y-%m-%d")
                        .to_string()
                })
                .unwrap_or_default()
        });
        out.push(vec![date, f("data"), f("changeRate")]);
    }
    let cols = ["日期", index_col, value_col];
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&[cols[1], cols[2]])?;
    Ok(df)
}

macro_rules! cx_index_fn {
    ($name:ident, $type:literal, $value_col:literal, $index_col:literal, $desc:literal) => {
        #[doc = $desc]
        pub fn $name() -> Result<Df> {
            cx_index_trend($type, $value_col, $index_col)
        }
    };
}

cx_index_fn!(
    index_pmi_com_cx,
    "com",
    "变化值",
    "综合PMI",
    "财新-综合PMI（对应 akshare `index_pmi_com_cx`）。"
);
cx_index_fn!(
    index_pmi_man_cx,
    "man",
    "变化值",
    "制造业PMI",
    "财新-制造业PMI（对应 akshare `index_pmi_man_cx`）。"
);
cx_index_fn!(
    index_pmi_ser_cx,
    "ser",
    "变化值",
    "服务业PMI",
    "财新-服务业PMI（对应 akshare `index_pmi_ser_cx`）。"
);
cx_index_fn!(
    index_dei_cx,
    "dei",
    "变化值",
    "数字经济指数",
    "财新-数字经济指数（对应 akshare `index_dei_cx`）。"
);
cx_index_fn!(
    index_ii_cx,
    "ii",
    "变化值",
    "产业指数",
    "财新-产业指数（对应 akshare `index_ii_cx`）。"
);
cx_index_fn!(
    index_si_cx,
    "si",
    "变化值",
    "溢出指数",
    "财新-溢出指数（对应 akshare `index_si_cx`）。"
);
cx_index_fn!(
    index_fi_cx,
    "fi",
    "变化值",
    "融合指数",
    "财新-融合指数（对应 akshare `index_fi_cx`）。"
);
cx_index_fn!(
    index_bi_cx,
    "bi",
    "变化值",
    "基础指数",
    "财新-基础指数（对应 akshare `index_bi_cx`）。"
);
cx_index_fn!(
    index_nei_cx,
    "nei",
    "变化值",
    "中国新经济指数",
    "财新-中国新经济指数（对应 akshare `index_nei_cx`）。"
);
cx_index_fn!(
    index_li_cx,
    "li",
    "变化值",
    "劳动力投入指数",
    "财新-劳动力投入指数（对应 akshare `index_li_cx`）。"
);
cx_index_fn!(
    index_ci_cx,
    "ci",
    "变化值",
    "资本投入指数",
    "财新-资本投入指数（对应 akshare `index_ci_cx`）。"
);
cx_index_fn!(
    index_ti_cx,
    "ti",
    "变化值",
    "科技投入指数",
    "财新-科技投入指数（对应 akshare `index_ti_cx`）。"
);
cx_index_fn!(
    index_neaw_cx,
    "neaw",
    "变化值",
    "新经济行业入职平均工资水平",
    "财新-新经济行业入职平均工资（对应 akshare `index_neaw_cx`）。"
);
cx_index_fn!(
    index_awpr_cx,
    "awpr",
    "变化值",
    "新经济入职工资溢价水平",
    "财新-新经济入职工资溢价（对应 akshare `index_awpr_cx`）。"
);
cx_index_fn!(
    index_cci_cx,
    "cci",
    "变化值",
    "大宗商品指数",
    "财新-大宗商品指数（对应 akshare `index_cci_cx`）。"
);
cx_index_fn!(
    index_qli_cx,
    "qli",
    "变化幅度",
    "高质量因子指数",
    "财新-高质量因子指数（对应 akshare `index_qli_cx`）。"
);
cx_index_fn!(
    index_ai_cx,
    "ai",
    "变化幅度",
    "AI策略指数",
    "财新-AI策略指数（对应 akshare `index_ai_cx`）。"
);
cx_index_fn!(
    index_bei_cx,
    "ind",
    "变化幅度",
    "基石经济指数",
    "财新-基石经济指数（对应 akshare `index_bei_cx`）。"
);
cx_index_fn!(
    index_neei_cx,
    "ind",
    "变化幅度",
    "新动能指数",
    "财新-新动能指数（对应 akshare `index_neei_cx`）。"
);

// === BATCH37-E 新浪全球指数（index_global_sina_symbol_map + gi.finance.sina.com.cn）===
//
// 对应 akshare `index/index_global_sina.py`。

/// 新浪全球指数 symbol 映射（对应 akshare `index_global_sina_symbol_map`）。
fn global_sina_code(symbol: &str) -> Result<&'static str> {
    Ok(match symbol {
        "英国富时100指数" => "UKX",
        "德国DAX 30种股价指数" => "DAX",
        "俄罗斯MICEX指数" => "INDEXCF",
        "法CAC40指数" => "CAC",
        "瑞士股票指数" => "SWI20",
        "富时意大利MIB指数" => "FTSEMIB",
        "荷兰AEX综合指数" => "AEX",
        "西班牙IBEX指数" => "IBEX",
        "欧洲Stoxx50指数" => "SX5E",
        "加拿大S&P/TSX综合指数" => "GSPTSE",
        "墨西哥BOLSA指数" => "MXX",
        "巴西BOVESPA股票指数" => "IBOV",
        "中国台湾加权指数" => "TWJQ",
        "日经225指数" => "NKY",
        "首尔综合指数" => "KOSPI",
        "印度尼西亚雅加达综合指数" => "JCI",
        "印度孟买SENSEX指数" => "SENSEX",
        "澳大利亚标准普尔200指数" => "AS51",
        "新西兰NZSE 50指数" => "NZ250",
        "埃及CASE 30指数" => "CASE",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，请用 index_global_name_table 获取指数名称"
            )))
        }
    })
}

/// 新浪财经-环球市场-名称代码映射表（对应 akshare [`akshare.index_global_name_table`]）。
///
/// # 返回列
/// `指数名称, 代码`
pub fn index_global_name_table() -> Result<Df> {
    let entries = [
        ("英国富时100指数", "UKX"),
        ("德国DAX 30种股价指数", "DAX"),
        ("俄罗斯MICEX指数", "INDEXCF"),
        ("法CAC40指数", "CAC"),
        ("瑞士股票指数", "SWI20"),
        ("富时意大利MIB指数", "FTSEMIB"),
        ("荷兰AEX综合指数", "AEX"),
        ("西班牙IBEX指数", "IBEX"),
        ("欧洲Stoxx50指数", "SX5E"),
        ("加拿大S&P/TSX综合指数", "GSPTSE"),
        ("墨西哥BOLSA指数", "MXX"),
        ("巴西BOVESPA股票指数", "IBOV"),
        ("中国台湾加权指数", "TWJQ"),
        ("日经225指数", "NKY"),
        ("首尔综合指数", "KOSPI"),
        ("印度尼西亚雅加达综合指数", "JCI"),
        ("印度孟买SENSEX指数", "SENSEX"),
        ("澳大利亚标准普尔200指数", "AS51"),
        ("新西兰NZSE 50指数", "NZ250"),
        ("埃及CASE 30指数", "CASE"),
    ];
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(entries.len());
    for (name, code) in entries {
        out.push(vec![Some(name.to_string()), Some(code.to_string())]);
    }
    Df::from_string_rows(&["指数名称", "代码"], &out)
}

/// 新浪财经-环球市场-历史行情（对应 akshare [`akshare.index_global_hist_sina`]）。
///
/// - `symbol`: 指数名称（由 [`index_global_name_table`] 获取），如 `"OMX"` → 实为 `"日经225指数"` 等
///
/// `gi.finance.sina.com.cn/hq/daily`（num=10000），短键映射 + 6 列。
///
/// # 返回列
/// `date, open, high, low, close, volume`
pub fn index_global_hist_sina(symbol: &str) -> Result<Df> {
    let code = global_sina_code(symbol)?;
    let url = "https://gi.finance.sina.com.cn/hq/daily";
    let params = json!({ "symbol": code, "num": "10000" });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json(url, &params, None)?;
    let rows = value
        .get("result")
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![f("d"), f("o"), f("h"), f("l"), f("c"), f("v")]);
    }
    const COLS: [&str; 6] = ["date", "open", "high", "low", "close", "volume"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["date"])?;
    df.cast_numeric(&COLS[1..])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klt_mapping_rejects_bad_period() {
        let r = index_zh_a_hist("000001", "bad", "20240101", "20240131");
        assert!(matches!(r, Err(AkshareError::Param(_))));
    }
}
