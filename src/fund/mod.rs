//! 基金数据接口。
//!
//! 首批实现（对应 akshare `fund/fund_etf_em.py`、`fund/fund_lof_em.py`）：
//! - [`fund_etf_spot_em`]：ETF 实时行情
//! - [`fund_lof_spot_em`]：LOF 实时行情
//!
//! 说明：akshare 的 ETF 实时行情主用 `push2delay` 延迟节点，本实现走
//! [`push2_urls`] 多节点容灾（同簇数据，避免单节点限流）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    fetch_clist, fetch_kline, fetch_kline_min, fetch_trends, json_value_to_string, kline_to_df,
    push2_urls, KLINE_COLS,
};
use serde_json::{json, Map, Value};

/// ETF 实时行情（对应 akshare [`akshare.fund_etf_spot_em`]）。
///
/// # 返回列
/// `代码, 名称, 最新价, IOPV实时估值, 基金折价率, 涨跌额, 涨跌幅, 成交量, 成交额, 开盘价,
/// 最高价, 最低价, 昨收, 振幅, 换手率, 量比, 委比, 外盘, 内盘, 主力净流入-净额,
/// 主力净流入-净占比, 超大单净流入-净额, 超大单净流入-净占比, 大单净流入-净额,
/// 大单净流入-净占比, 中单净流入-净额, 中单净流入-净占比, 小单净流入-净额,
/// 小单净流入-净占比, 现手, 买一, 卖一, 最新份额, 流通市值, 总市值, 数据日期, 更新时间`
///
/// 注：akshare 将 `数据日期`/`更新时间` 转为时间类型，本实现保留字符串（dtype 偏差，
/// 见 PLAN 已知偏差清单）。
pub fn fund_etf_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "wbp2u": "|0|0|0|web",
        "fid": "f12",
        "fs": "b:MK0021,b:MK0022,b:MK0023,b:MK0024,b:MK0827",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f30,f31,f32,f33,f34,f35,f38,f62,f63,f64,f65,f66,f69,f72,f75,f78,f81,f84,f87,f115,f124,f128,f136,f152,f184,f297,f402,f441",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let df = fetch_clist(&http, &urls, &params)?;

    let rename = [
        ("f12", "代码"),
        ("f14", "名称"),
        ("f2", "最新价"),
        ("f4", "涨跌额"),
        ("f3", "涨跌幅"),
        ("f5", "成交量"),
        ("f6", "成交额"),
        ("f7", "振幅"),
        ("f17", "开盘价"),
        ("f15", "最高价"),
        ("f16", "最低价"),
        ("f18", "昨收"),
        ("f8", "换手率"),
        ("f10", "量比"),
        ("f30", "现手"),
        ("f31", "买一"),
        ("f32", "卖一"),
        ("f33", "委比"),
        ("f34", "外盘"),
        ("f35", "内盘"),
        ("f62", "主力净流入-净额"),
        ("f184", "主力净流入-净占比"),
        ("f66", "超大单净流入-净额"),
        ("f69", "超大单净流入-净占比"),
        ("f72", "大单净流入-净额"),
        ("f75", "大单净流入-净占比"),
        ("f78", "中单净流入-净额"),
        ("f81", "中单净流入-净占比"),
        ("f84", "小单净流入-净额"),
        ("f87", "小单净流入-净占比"),
        ("f38", "最新份额"),
        ("f21", "流通市值"),
        ("f20", "总市值"),
        ("f402", "基金折价率"),
        ("f441", "IOPV实时估值"),
        ("f297", "数据日期"),
        ("f124", "更新时间"),
    ];
    let select = [
        "代码",
        "名称",
        "最新价",
        "IOPV实时估值",
        "基金折价率",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "开盘价",
        "最高价",
        "最低价",
        "昨收",
        "振幅",
        "换手率",
        "量比",
        "委比",
        "外盘",
        "内盘",
        "主力净流入-净额",
        "主力净流入-净占比",
        "超大单净流入-净额",
        "超大单净流入-净占比",
        "大单净流入-净额",
        "大单净流入-净占比",
        "中单净流入-净额",
        "中单净流入-净占比",
        "小单净流入-净额",
        "小单净流入-净占比",
        "现手",
        "买一",
        "卖一",
        "最新份额",
        "流通市值",
        "总市值",
        "数据日期",
        "更新时间",
    ];
    let numeric = [
        "最新价",
        "IOPV实时估值",
        "基金折价率",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "开盘价",
        "最高价",
        "最低价",
        "昨收",
        "振幅",
        "换手率",
        "量比",
        "委比",
        "外盘",
        "内盘",
        "主力净流入-净额",
        "主力净流入-净占比",
        "超大单净流入-净额",
        "超大单净流入-净占比",
        "大单净流入-净额",
        "大单净流入-净占比",
        "中单净流入-净额",
        "中单净流入-净占比",
        "小单净流入-净额",
        "小单净流入-净占比",
        "现手",
        "买一",
        "卖一",
        "最新份额",
        "流通市值",
        "总市值",
    ];
    crate::sources::eastmoney::finalize_spot(df, &rename, &select, &numeric)
}

/// LOF 实时行情（对应 akshare [`akshare.fund_lof_spot_em`]）。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌额, 涨跌幅, 成交量, 成交额, 开盘价, 最高价, 最低价, 昨收,
/// 换手率, 流通市值, 总市值`
pub fn fund_lof_spot_em() -> Result<Df> {
    let urls = push2_urls("/api/qt/clist/get");
    let params = json!({
        "pn": "1",
        "pz": "100",
        "po": "1",
        "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2",
        "invt": "2",
        "wbp2u": "|0|0|0|web",
        "fid": "f3",
        "fs": "b:MK0404,b:MK0405,b:MK0406,b:MK0407",
        "fields": "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let df = fetch_clist(&http, &urls, &params)?;

    let rename = [
        ("f12", "代码"),
        ("f14", "名称"),
        ("f2", "最新价"),
        ("f4", "涨跌额"),
        ("f3", "涨跌幅"),
        ("f5", "成交量"),
        ("f6", "成交额"),
        ("f17", "开盘价"),
        ("f15", "最高价"),
        ("f16", "最低价"),
        ("f18", "昨收"),
        ("f8", "换手率"),
        ("f21", "流通市值"),
        ("f20", "总市值"),
    ];
    let select = [
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "开盘价",
        "最高价",
        "最低价",
        "昨收",
        "换手率",
        "流通市值",
        "总市值",
    ];
    let numeric = [
        "最新价",
        "涨跌额",
        "涨跌幅",
        "成交量",
        "成交额",
        "开盘价",
        "最高价",
        "最低价",
        "昨收",
        "换手率",
        "流通市值",
        "总市值",
    ];
    crate::sources::eastmoney::finalize_spot(df, &rename, &select, &numeric)
}

/// 同花顺理财-基金数据-每日净值-实时行情（对应 akshare [`akshare.fund_etf_category_ths`]）。
///
/// `symbol`: `"股票型"/"债券型"/"混合型"/"ETF"/"LOF"/"QDII"/"保本型"/"指数型"/""`（"" 表示全部）；
/// `date`: `YYYYMMDD`，空字符串表示最新。
///
/// 数据源为 `fund.10jqka.com.cn` 的 JSONP 接口（jsonp 解包 → 对象转表 → 重排）。
///
/// # 返回列
/// `序号, 基金代码, 基金名称, 当前-单位净值, 当前-累计净值, 前一日-单位净值,
/// 前一日-累计净值, 增长值, 增长率, 赎回状态, 申购状态, 最新-交易日,
/// 最新-单位净值, 最新-累计净值, 基金类型, 查询日期`
pub fn fund_etf_category_ths(symbol: &str, date: &str) -> Result<Df> {
    let inner_symbol = match symbol {
        "股票型" => "gpx",
        "债券型" => "zqx",
        "混合型" => "hhx",
        "ETF" => "ETF",
        "LOF" => "LOF",
        "QDII" => "QDII",
        "保本型" => "bbx",
        "指数型" => "zsx",
        "" => "all",
        _ => "ETF",
    };
    let inner_date = if date.is_empty() {
        "0".to_string()
    } else if date.len() == 8 {
        format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])
    } else {
        return Err(crate::core::error::AkshareError::Param(format!(
            "无效日期: {date}（应为 YYYYMMDD 或空字符串）"
        )));
    };
    let url = format!(
        "https://fund.10jqka.com.cn/data/Net/info/{inner_symbol}_rate_desc_{inner_date}_0_1_9999_0_0_0_jsonp_g.html"
    );
    let data: Value = crate::sources::ths::fetch_ths_jsonp(&url)?;
    let rows = data
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(Value::as_object)
        .ok_or_else(|| crate::core::error::AkshareError::Empty("ths 响应缺少 data.data".into()))?;

    // 对象转行数组（对应 pandas DataFrame(data_json["data"]["data"]).T）
    let mut out_rows: Vec<Value> = Vec::with_capacity(rows.len());
    for v in rows.values() {
        out_rows.push(v.clone());
    }
    let mut df = Df::from_json_rows(&out_rows)?;

    // 序号列：1..n（对应 reset_index + index+1）
    let n = df.height();
    let seq: Vec<Option<String>> = (1..=n).map(|i| Some(i.to_string())).collect();
    df.with_column("index", &seq)?;

    // 重命名
    let rename = [
        ("index", "序号"),
        ("code", "基金代码"),
        ("typename", "基金类型"),
        ("net", "当前-单位净值"),
        ("name", "基金名称"),
        ("totalnet", "当前-累计净值"),
        ("newnet", "最新-单位净值"),
        ("newtotalnet", "最新-累计净值"),
        ("newdate", "最新-交易日"),
        ("net1", "前一日-单位净值"),
        ("totalnet1", "前一日-累计净值"),
        ("ranges", "增长值"),
        ("rate", "增长率"),
        ("shstat", "赎回状态"),
        ("sgstat", "申购状态"),
    ];
    let cur = df.column_names();
    let renamed: Vec<String> = cur
        .iter()
        .map(|c| {
            rename
                .iter()
                .find(|(k, _)| *k == c)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| c.clone())
        })
        .collect();
    let refs: Vec<&str> = renamed.iter().map(String::as_str).collect();
    df.rename_columns(&refs)?;

    // 重排到 akshare 输出列序
    let selected = df.select(&[
        "序号",
        "基金代码",
        "基金名称",
        "当前-单位净值",
        "当前-累计净值",
        "前一日-单位净值",
        "前一日-累计净值",
        "增长值",
        "增长率",
        "赎回状态",
        "申购状态",
        "最新-交易日",
        "最新-单位净值",
        "最新-累计净值",
        "基金类型",
    ])?;
    let mut out = selected;

    // 查询日期：date 非空则用入参，否则用首行最新-交易日
    let query_date = if date.is_empty() {
        out.inner()
            .column("最新-交易日")
            .ok()
            .and_then(|c| c.str().ok())
            .and_then(|s| s.get(0))
            .unwrap_or("")
            .to_string()
    } else {
        date.to_string()
    };
    let qd = crate::core::df::normalize_date(&query_date);
    let qd_col: Vec<Option<String>> = (0..out.height()).map(|_| qd.clone()).collect();
    out.with_column("查询日期", &qd_col)?;

    out.cast_date(&["最新-交易日", "查询日期"])?;
    out.cast_numeric(&[
        "序号",
        "当前-单位净值",
        "当前-累计净值",
        "前一日-单位净值",
        "前一日-累计净值",
        "增长值",
        "增长率",
        "最新-单位净值",
        "最新-累计净值",
    ])?;
    Ok(out)
}

/// 同花顺理财-基金数据-ETF 实时行情（对应 akshare [`akshare.fund_etf_spot_ths`]）。
pub fn fund_etf_spot_ths(date: &str) -> Result<Df> {
    fund_etf_category_ths("ETF", date)
}

// === BATCH37-F 东财基金 K 线（fund_lof_hist_em / fund_etf_hist_min_em）===
//
// 对应 akshare `fund/fund_lof_em.py` / `fund/fund_etf_em.py`。复用
// `fetch_kline`/`fetch_kline_min`/`fetch_trends`（push2his），与股票 K 线同构。
// 市场标识简化：`5`/`6` 开头沪市 `1`，其余深市 `0`。

/// 东财-LOF 历史行情（对应 akshare [`akshare.fund_lof_hist_em`]）。
///
/// - `symbol`: LOF 代码，如 `"166009"`
/// - `period`: `"daily"` / `"weekly"` / `"monthly"`
/// - `start_date`/`end_date`: `YYYYMMDD`；`adjust`: `""` / `"qfq"` / `"hfq"`
///
/// # 返回列
/// `日期, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 振幅, 涨跌幅, 涨跌额, 换手率`
pub fn fund_lof_hist_em(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
    adjust: &str,
) -> Result<Df> {
    let klt = match period {
        "daily" => "101",
        "weekly" => "102",
        "monthly" => "103",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 period: {other}，可选 daily/weekly/monthly"
            )))
        }
    };
    let fqt = match adjust {
        "" => "0",
        "qfq" => "1",
        "hfq" => "2",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 adjust: {other}，可选 qfq/hfq/空"
            )))
        }
    };
    let market = if symbol.starts_with('5') || symbol.starts_with('6') {
        "1"
    } else {
        "0"
    };
    let secid = format!("{market}.{symbol}");
    let http = HttpClient::default();
    let klines = fetch_kline(&http, &secid, klt, fqt, start_date, end_date)?;
    let df = kline_to_df(&KLINE_COLS, &klines, None)?;
    let mut df = df;
    df.cast_numeric(&[
        "日期",
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
    ])?;
    Ok(df)
}

/// 东财-ETF 分钟行情（对应 akshare [`akshare.fund_etf_hist_min_em`]）。
///
/// - `symbol`: ETF 代码，如 `"159707"`
/// - `start_date`/`end_date`: `YYYY-MM-DD HH:MM:SS`
/// - `period`: `"1"`（分时）/ `"5"` / `"15"` / `"30"` / `"60"`；`adjust`: `""` / `"qfq"` / `"hfq"`
///
/// # 返回列
/// period=1：`时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 均价`；
/// 其余：`时间, 开盘, 收盘, 最高, 最低, 成交量, 成交额, 振幅, 涨跌幅, 涨跌额, 换手率`
pub fn fund_etf_hist_min_em(
    symbol: &str,
    start_date: &str,
    end_date: &str,
    period: &str,
    adjust: &str,
) -> Result<Df> {
    let fqt = match adjust {
        "" => "0",
        "qfq" => "1",
        "hfq" => "2",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 adjust: {other}，可选 qfq/hfq/空"
            )))
        }
    };
    let market = if symbol.starts_with('5') || symbol.starts_with('6') {
        "1"
    } else {
        "0"
    };
    let secid = format!("{market}.{symbol}");
    let http = HttpClient::default();
    if period == "1" {
        let trends = fetch_trends(&http, &secid, "5", "0")?;
        let rows: Vec<Vec<Option<String>>> = trends
            .iter()
            .map(|t| t.iter().map(|s| Some(s.clone())).collect())
            .collect();
        let mut df = Df::from_string_rows(
            &[
                "时间",
                "开盘",
                "收盘",
                "最高",
                "最低",
                "成交量",
                "成交额",
                "均价",
            ],
            &rows,
        )?;
        df.cast_numeric(&["开盘", "收盘", "最高", "最低", "成交量", "成交额", "均价"])?;
        let _ = (start_date, end_date);
        Ok(df)
    } else {
        match period {
            "5" | "15" | "30" | "60" => {}
            other => {
                return Err(AkshareError::Param(format!(
                    "无效 period: {other}，可选 1/5/15/30/60"
                )))
            }
        }
        let klines = fetch_kline_min(&http, &secid, period, fqt)?;
        let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
        for k in &klines {
            let pick = |i: usize| k.get(i).map(|s| Some(s.clone())).unwrap_or(None);
            rows.push(vec![
                pick(0),
                pick(1),
                pick(2),
                pick(3),
                pick(4),
                pick(5),
                pick(6),
                pick(7),
                pick(8),
                pick(9),
                pick(10),
            ]);
        }
        const COLS: [&str; 11] = [
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
        let mut df = Df::from_string_rows(&COLS, &rows)?;
        df.cast_numeric(&COLS[1..])?;
        let _ = (start_date, end_date);
        Ok(df)
    }
}

// === BATCH38-A 东财基金排行（rankhandler.aspx，op=ph + dt 分类型）===
//
// 对应 akshare `fund/fund_rank_em.py`。响应 `var rankData = {datas:[...],...}`，
// `datas` 每行为逗号分隔字符串（24 字段），位置式列名后 select 18 列。

/// 东财-开放基金排行（对应 akshare [`akshare.fund_open_fund_rank_em`]）。
///
/// - `symbol`: `"全部"` / `"股票型"` / `"混合型"` / `"债券型"` / `"指数型"` /
///   `"QDII"` / `"LOF"` / `"FOF"`
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 日期, 单位净值, 累计净值, 日增长率, 近1周, 近1月,
/// 近3月, 近6月, 近1年, 近2年, 近3年, 今年来, 成立来, 自定义, 手续费`
pub fn fund_open_fund_rank_em(symbol: &str) -> Result<Df> {
    let (ft, sc) = match symbol {
        "全部" => ("all", "1nzf"),
        "股票型" => ("gp", "1nzf"),
        "混合型" => ("hh", "1nzf"),
        "债券型" => ("zq", "1nzf"),
        "指数型" => ("zs", "1nzf"),
        "QDII" => ("qdii", "1nzf"),
        "LOF" => ("lof", "1nzf"),
        "FOF" => ("fof", "1nzf"),
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全部/股票型/混合型/债券型/指数型/QDII/LOF/FOF"
            )))
        }
    };
    let now = chrono::Local::now();
    let ed = now.format("%Y-%m-%d").to_string();
    let sd = (now - chrono::Duration::days(366))
        .format("%Y-%m-%d")
        .to_string();
    let params = json!({
        "op": "ph",
        "dt": "kf",
        "ft": ft,
        "rs": "",
        "gs": "0",
        "sc": sc,
        "st": "desc",
        "sd": sd,
        "ed": ed,
        "qdii": "",
        "tabSubtype": ",,,,,",
        "pi": "1",
        "pn": "30000",
        "dx": "1",
        "v": "0.1591891419018292",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let text = http.get_text_with_headers(
        "https://fund.eastmoney.com/data/rankhandler.aspx",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36"),
            ("Referer", "https://fund.eastmoney.com/fundguzhi.html"),
        ],
        None,
    )?;
    // `var rankData = {...};` → 取首 `{` 至末尾
    let start = text
        .find('{')
        .ok_or_else(|| AkshareError::Empty("基金排行响应缺少对象".into()))?;
    let value: Value = serde_json::from_str(&text[start..])
        .map_err(|e| AkshareError::json("fund_open_fund_rank_em", e.to_string()))?;
    let datas = value
        .get("datas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 每行逗号分隔 24 字段 → 位置式列名 select 18 列
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(datas.len());
    for (i, d) in datas.iter().enumerate() {
        let line = d.as_str().unwrap_or("");
        let f: Vec<&str> = line.split(',').collect();
        let pick = |idx: usize| {
            f.get(idx)
                .map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or(None)
        };
        out.push(vec![
            Some((i + 1).to_string()), // 序号
            pick(1),                   // 基金代码
            pick(2),                   // 基金简称
            pick(4),                   // 日期
            pick(5),                   // 单位净值
            pick(6),                   // 累计净值
            pick(7),                   // 日增长率
            pick(8),                   // 近1周
            pick(9),                   // 近1月
            pick(10),                  // 近3月
            pick(11),                  // 近6月
            pick(12),                  // 近1年
            pick(13),                  // 近2年
            pick(14),                  // 近3年
            pick(15),                  // 今年来
            pick(16),                  // 成立来
            pick(19),                  // 自定义
            pick(21),                  // 手续费
        ]);
    }
    const COLS: [&str; 18] = [
        "序号",
        "基金代码",
        "基金简称",
        "日期",
        "单位净值",
        "累计净值",
        "日增长率",
        "近1周",
        "近1月",
        "近3月",
        "近6月",
        "近1年",
        "近2年",
        "近3年",
        "今年来",
        "成立来",
        "自定义",
        "手续费",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&[
        "单位净值",
        "累计净值",
        "日增长率",
        "近1周",
        "近1月",
        "近3月",
        "近6月",
        "近1年",
        "近2年",
        "近3年",
        "今年来",
        "成立来",
        "自定义",
    ])?;
    Ok(df)
}

/// 东财-场内交易基金排行（对应 akshare [`akshare.fund_exchange_rank_em`]）。
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 类型, 日期, 单位净值, 累计净值, 近1周, 近1月,
/// 近3月, 近6月, 近1年, 近2年, 近3年, 今年来, 成立来, 成立日期`
pub fn fund_exchange_rank_em() -> Result<Df> {
    let params = json!({
        "op": "ph",
        "dt": "fb",
        "ft": "ct",
        "rs": "",
        "gs": "0",
        "sc": "1nzf",
        "st": "desc",
        "pi": "1",
        "pn": "30000",
        "v": "0.1591891419018292",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let text = http.get_text_with_headers(
        "https://fund.eastmoney.com/data/rankhandler.aspx",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36"),
            ("Referer", "https://fund.eastmoney.com/fundguzhi.html"),
        ],
        None,
    )?;
    let start = text
        .find('{')
        .ok_or_else(|| AkshareError::Empty("基金排行响应缺少对象".into()))?;
    let value: Value = serde_json::from_str(&text[start..])
        .map_err(|e| AkshareError::json("fund_exchange_rank_em", e.to_string()))?;
    let datas = value
        .get("datas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 24 字段位置式：序号,基金代码,基金简称,_,日期,单位净值,累计净值,近1周,近1月,
    // 近3月,近6月,近1年,近2年,近3年,今年来,成立来,成立日期,_,_,_,_,_,类型,_
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(datas.len());
    for (i, d) in datas.iter().enumerate() {
        let line = d.as_str().unwrap_or("");
        let f: Vec<&str> = line.split(',').collect();
        let pick = |idx: usize| {
            f.get(idx)
                .map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or(None)
        };
        out.push(vec![
            Some((i + 1).to_string()),
            pick(1),
            pick(2),
            pick(22),
            pick(4),
            pick(5),
            pick(6),
            pick(7),
            pick(8),
            pick(9),
            pick(10),
            pick(11),
            pick(12),
            pick(13),
            pick(14),
            pick(15),
            pick(16),
        ]);
    }
    const COLS: [&str; 17] = [
        "序号",
        "基金代码",
        "基金简称",
        "类型",
        "日期",
        "单位净值",
        "累计净值",
        "近1周",
        "近1月",
        "近3月",
        "近6月",
        "近1年",
        "近2年",
        "近3年",
        "今年来",
        "成立来",
        "成立日期",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期", "成立日期"])?;
    df.cast_numeric(&[
        "单位净值",
        "累计净值",
        "近1周",
        "近1月",
        "近3月",
        "近6月",
        "近1年",
        "近2年",
        "近3年",
        "今年来",
        "成立来",
    ])?;
    Ok(df)
}

/// 东财-货币型基金排行（对应 akshare [`akshare.fund_money_rank_em`]）。
///
/// `api.fund.eastmoney.com/FundRank/GetHbRankList` JSON（`Data` 数组，
/// 28 字段位置式），select 18 列。
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 日期, 万份收益, 年化收益率7日, 年化收益率14日,
/// 年化收益率28日, 近1月, 近3月, 近6月, 近1年, 近2年, 近3年, 近5年, 今年来, 成立来, 手续费`
pub fn fund_money_rank_em() -> Result<Df> {
    let params = json!({
        "intCompany": "0",
        "MinsgType": "",
        "IsSale": "1",
        "strSortCol": "SYL_1N",
        "orderType": "desc",
        "pageIndex": "1",
        "pageSize": "10000",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://api.fund.eastmoney.com/FundRank/GetHbRankList",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36"),
            ("Referer", "https://fund.eastmoney.com/fundguzhi.html"),
        ],
        None,
    )?;
    let rows = value
        .get("Data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 28 字段位置式：序号,近1年,近2年,近3年,近5年,_,_,基金代码,基金简称,日期,万份收益,
    // 年化7日,_,年化14日,年化28日,近1月,近3月,近6月,今年来,成立来,_,手续费,_,_,_,_,_,_
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let obj = r.as_object().cloned().unwrap_or_default();
        let values: Vec<Option<String>> = obj
            .values()
            .take(28)
            .map(|v| match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect();
        let pick = |idx: usize| values.get(idx).cloned().flatten();
        out.push(vec![
            Some((i + 1).to_string()),
            pick(7),
            pick(8),
            pick(9),
            pick(10),
            pick(11),
            pick(13),
            pick(14),
            pick(15),
            pick(16),
            pick(17),
            pick(1),
            pick(2),
            pick(3),
            pick(4),
            pick(18),
            pick(19),
            pick(21),
        ]);
    }
    const COLS: [&str; 18] = [
        "序号",
        "基金代码",
        "基金简称",
        "日期",
        "万份收益",
        "年化收益率7日",
        "年化收益率14日",
        "年化收益率28日",
        "近1月",
        "近3月",
        "近6月",
        "近1年",
        "近2年",
        "近3年",
        "近5年",
        "今年来",
        "成立来",
        "手续费",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&COLS[4..])?;
    Ok(df)
}

/// 东财-理财基金排行（对应 akshare [`akshare.fund_lcx_rank_em`]）。
///
/// `api.fund.eastmoney.com/FundRank/GetLcRankList` JSON（`Data` 数组，
/// 23 字段位置式），select 16 列。注：akshare 注释该接口可能暂无数据。
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 日期, 万份收益, 年化收益率-7日, 年化收益率-14日,
/// 年化收益率-28日, 近1周, 近1月, 近3月, 近6月, 今年来, 成立来, 可购买, 手续费`
pub fn fund_lcx_rank_em() -> Result<Df> {
    let params = json!({
        "intCompany": "0",
        "MinsgType": "undefined",
        "IsSale": "1",
        "strSortCol": "SYL_Z",
        "orderType": "desc",
        "pageIndex": "1",
        "pageSize": "50",
        "FBQ": "",
        "callback": "jQuery18303264654966943197_1603867158043",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://api.fund.eastmoney.com/FundRank/GetLcRankList",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36"),
            ("Referer", "https://fund.eastmoney.com/fundguzhi.html"),
        ],
        None,
    )?;
    let rows = value
        .get("Data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 23 字段位置式：序号,近1周,基金代码,基金简称,日期,万份收益,年化7日,_,年化14日,
    // 年化28日,近1月,近3月,近6月,今年来,成立来,可购买,手续费,_,_,_,_,_,_
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let obj = r.as_object().cloned().unwrap_or_default();
        let values: Vec<Option<String>> = obj
            .values()
            .take(23)
            .map(|v| match v {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect();
        let pick = |idx: usize| values.get(idx).cloned().flatten();
        out.push(vec![
            Some((i + 1).to_string()),
            pick(2),
            pick(3),
            pick(4),
            pick(5),
            pick(6),
            pick(8),
            pick(9),
            pick(1),
            pick(10),
            pick(11),
            pick(12),
            pick(13),
            pick(14),
            pick(15),
            pick(16),
        ]);
    }
    const COLS: [&str; 16] = [
        "序号",
        "基金代码",
        "基金简称",
        "日期",
        "万份收益",
        "年化收益率-7日",
        "年化收益率-14日",
        "年化收益率-28日",
        "近1周",
        "近1月",
        "近3月",
        "近6月",
        "今年来",
        "成立来",
        "可购买",
        "手续费",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&COLS[4..])?;
    Ok(df)
}

// === BATCH39-A 东财基金净值列表（Data/Fund_JJJZ_Data.aspx，datas + showday）===
//
// 对应 akshare `fund/fund_em.py` 的 `fund_open_fund_daily_em` 等。响应
// `var db={datas:[[...]],showday:[d1,d2],...}`，datas 每行 21 字段位置式，
// 前两日为 `showday[0]`（今日）与 `showday[1]`（昨日）。

/// 东财-开放式基金净值（对应 akshare [`akshare.fund_open_fund_daily_em`]）。
///
/// # 返回列
/// `基金代码, 基金简称, {今日}-单位净值, {今日}-累计净值, {昨日}-单位净值,
/// {昨日}-累计净值, 日增长值, 日增长率, 申购状态, 赎回状态, 手续费`
pub fn fund_open_fund_daily_em() -> Result<Df> {
    fund_jjz_daily("1", "1")
}

/// 东财-货币型基金净值（对应 akshare [`akshare.fund_money_fund_daily_em`]）。
///
/// # 返回列
/// `基金代码, 基金简称, {今日}-每万份收益, {今日}-7日年化, {昨日}-每万份收益,
/// {昨日}-7日年化, 申购状态, 赎回状态, 手续费`
pub fn fund_money_fund_daily_em() -> Result<Df> {
    fund_jjz_daily("2", "2")
}

/// 东财-净值列表公共实现（Data/Fund_JJJZ_Data.aspx）。
fn fund_jjz_daily(t: &str, lx: &str) -> Result<Df> {
    let params = json!({
        "t": t,
        "lx": lx,
        "letter": "",
        "gsid": "",
        "text": "",
        "sort": "zdf,desc",
        "page": "1,50000",
        "dt": "1580914040623",
        "atfc": "",
        "onlySale": "0",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let text = http.get_text_with_headers(
        "https://fund.eastmoney.com/Data/Fund_JJJZ_Data.aspx",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/81.0.4044.138 Safari/537.36"),
            ("Referer", "https://fund.eastmoney.com/fund.html"),
        ],
        None,
    )?;
    let body = text
        .trim()
        .strip_prefix("var db=")
        .ok_or_else(|| AkshareError::Empty("基金净值响应缺少 var db= 前缀".into()))?;
    let value: Value = serde_json::from_str(body)
        .map_err(|e| AkshareError::json("fund_jjz_daily", e.to_string()))?;
    let datas = value
        .get("datas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let showday: Vec<String> = value
        .get("showday")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let d0 = showday.first().cloned().unwrap_or_default();
    let d1 = showday.get(1).cloned().unwrap_or_default();
    // 21 字段位置式：代码,简称,_,今日单位,今日累计,昨日单位,昨日累计,增长值,增长率,申购,赎回,_,_,_,_,_,_,手续费,_,_,_
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(datas.len());
    for d in &datas {
        let row = d.as_array().cloned().unwrap_or_default();
        let f = |idx: usize| {
            row.get(idx)
                .and_then(Value::as_str)
                .map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or(None)
        };
        out.push(vec![
            f(0),
            f(1),
            f(3),
            f(4),
            f(5),
            f(6),
            f(7),
            f(8),
            f(9),
            f(10),
            f(17),
        ]);
    }
    let cols = [
        "基金代码",
        "基金简称",
        &format!("{d0}-单位净值"),
        &format!("{d0}-累计净值"),
        &format!("{d1}-单位净值"),
        &format!("{d1}-累计净值"),
        "日增长值",
        "日增长率",
        "申购状态",
        "赎回状态",
        "手续费",
    ];
    let col_refs: Vec<&str> = cols.to_vec();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_numeric(&[cols[2], cols[3], cols[4], cols[5], "日增长值", "日增长率"])?;
    Ok(df)
}

/// 东财-理财型基金收益（对应 akshare [`akshare.fund_financial_fund_daily_em`]）。
///
/// `api.fund.eastmoney.com/FundNetValue/GetLCJJJZ` JSON（`Data.List` + `showday`）。
/// 注：akshare 注释该接口暂无数据。
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 上一期年化收益率, {今日}-万份收益, {今日}-7日年华,
/// {昨日}-万份收益, {昨日}-7日年华, 封闭期, 申购状态`
pub fn fund_financial_fund_daily_em() -> Result<Df> {
    let params = json!({
        "letter": "",
        "jjgsid": "0",
        "searchtext": "",
        "sort": "ljjz,desc",
        "page": "1,100",
        "AttentionCodes": "",
        "cycle": "",
        "OnlySale": "1",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let value = http.get_json_with_headers(
        "https://api.fund.eastmoney.com/FundNetValue/GetLCJJJZ",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.149 Safari/537.36"),
            ("Referer", "https://fund.eastmoney.com/lcjj.html"),
        ],
        None,
    )?;
    let data = value
        .get("Data")
        .ok_or_else(|| AkshareError::Empty("理财基金收益无 Data".into()))?;
    let rows = data
        .get("List")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let showday: Vec<String> = data
        .get("showday")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let d0 = showday.first().cloned().unwrap_or_default();
    let d1 = showday.get(1).cloned().unwrap_or_default();
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()),
            f("fcode"),
            f("shortname"),
            f("actualsyi"),
            f("mui"),
            f("syi"),
            f("zrmui"),
            f("zrsyi"),
            f("cycle"),
            f("kfr"),
        ]);
    }
    let cols = [
        "序号",
        "基金代码",
        "基金简称",
        "上一期年化收益率",
        &format!("{d0}-万份收益"),
        &format!("{d0}-7日年华"),
        &format!("{d1}-万份收益"),
        &format!("{d1}-7日年华"),
        "封闭期",
        "申购状态",
    ];
    let col_refs: Vec<&str> = cols.to_vec();
    let mut df = Df::from_string_rows(&col_refs, &out)?;
    df.cast_numeric(&[cols[3], cols[4], cols[5], cols[6], cols[7]])?;
    Ok(df)
}

/// 东财-所有基金名称和类型（对应 akshare [`akshare.fund_name_em`]）。
///
/// `fund.eastmoney.com/js/fundcode_search.js`，响应 `var r = [[...]];`，
/// 取 `var r = ` 前缀后到末尾分号前的二维数组。
///
/// # 返回列
/// `基金代码, 拼音缩写, 基金简称, 基金类型, 拼音全称`
pub fn fund_name_em() -> Result<Df> {
    let http = HttpClient::default();
    let text = http.get_text(
        "https://fund.eastmoney.com/js/fundcode_search.js",
        &Map::new(),
        None,
    )?;
    let body = text
        .trim()
        .strip_prefix("var r = ")
        .ok_or_else(|| AkshareError::Empty("基金名称响应缺少 var r = 前缀".into()))?;
    let body = body.strip_suffix(';').unwrap_or(body);
    let rows: Vec<Value> = serde_json::from_str(body)
        .map_err(|e| AkshareError::json("fund_name_em", e.to_string()))?;
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let row = r.as_array().cloned().unwrap_or_default();
        let f = |idx: usize| {
            row.get(idx)
                .and_then(Value::as_str)
                .map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or(None)
        };
        out.push(vec![f(0), f(1), f(2), f(3), f(4)]);
    }
    Df::from_string_rows(
        &["基金代码", "拼音缩写", "基金简称", "基金类型", "拼音全称"],
        &out,
    )
}

// === BATCH43-A 天天基金网-规模份额（FundDataPortfolio_Interface.aspx dt=9/11）===
//
// 对应 akshare `fund/fund_scale_em.py`。响应 `var hypzDetail={data:[...],pages:"57"}`，
// `data` 每行 6 字段位置式，分页直到 pages。

/// 天天基金网-规模变动（对应 akshare [`akshare.fund_scale_change_em`]）。
///
/// # 返回列
/// `序号, 截止日期, 基金家数, 期间申购, 期间赎回, 期末总份额, 期末净资产`
pub fn fund_scale_change_em() -> Result<Df> {
    fund_hypz_base("9", false)
}

/// 天天基金网-持有人结构（对应 akshare [`akshare.fund_hold_structure_em`]）。
///
/// # 返回列
/// `序号, 截止日期, 基金家数, 机构持有比列, 个人持有比列, 内部持有比列, 总份额`
pub fn fund_hold_structure_em() -> Result<Df> {
    fund_hypz_base("11", true)
}

/// 规模份额公共实现（FundDataPortfolio_Interface.aspx，dt 分接口）。
fn fund_hypz_base(dt: &str, is_hold: bool) -> Result<Df> {
    let url = "https://fund.eastmoney.com/data/FundDataPortfolio_Interface.aspx";
    let http = HttpClient::default();
    let mut params: Map<String, Value> = json!({
        "dt": dt,
        "pi": "1",
        "pn": "50",
        "mc": "hypzDetail",
        "st": "desc",
        "sc": "reportdate",
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    let parse = |text: &str| -> Result<Value> {
        let start = text
            .find('{')
            .ok_or_else(|| AkshareError::Empty("规模份额响应缺少对象".into()))?;
        let end = text.rfind('}').unwrap_or(text.len()).saturating_add(1);
        serde_json::from_str(&text[start..end]).map_err(|e| AkshareError::json(url, e.to_string()))
    };
    let first_text = http.get_text(url, &params, None)?;
    let first = parse(&first_text)?;
    let total_page: i64 = first
        .get("pages")
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v.get("data").and_then(Value::as_array) {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for page in 2..=total_page {
        params = json!({
            "dt": dt,
            "pi": page.to_string(),
            "pn": "50",
            "mc": "hypzDetail",
            "st": "desc",
            "sc": "reportdate",
        })
        .as_object()
        .cloned()
        .unwrap_or_default();
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(v) = parse(&t) {
                    append(&v, &mut rows);
                }
            }
            Err(_) => break,
        }
    }
    // data 每行 6 字段位置式
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let row = r.as_array().cloned().unwrap_or_default();
        let f = |idx: usize| {
            row.get(idx)
                .and_then(Value::as_str)
                .map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or(None)
        };
        out.push(vec![
            Some((i + 1).to_string()),
            f(0),
            f(1),
            f(2),
            f(3),
            f(4),
            f(5),
        ]);
    }
    if !is_hold {
        const COLS: [&str; 7] = [
            "序号",
            "截止日期",
            "基金家数",
            "期间申购",
            "期间赎回",
            "期末总份额",
            "期末净资产",
        ];
        let mut df = Df::from_string_rows(&COLS, &out)?;
        df.cast_date(&["截止日期"])?;
        df.cast_numeric(&[
            "基金家数",
            "期间申购",
            "期间赎回",
            "期末总份额",
            "期末净资产",
        ])?;
        Ok(df)
    } else {
        const COLS: [&str; 7] = [
            "序号",
            "截止日期",
            "基金家数",
            "机构持有比列",
            "个人持有比列",
            "内部持有比列",
            "总份额",
        ];
        let mut df = Df::from_string_rows(&COLS, &out)?;
        df.cast_date(&["截止日期"])?;
        df.cast_numeric(&["基金家数", "机构持有比列", "个人持有比列", "内部持有比列"])?;
        df.strip_commas(&["总份额"])?;
        df.cast_numeric(&["总份额"])?;
        Ok(df)
    }
}

// === BATCH44-A 天天基金网-投资组合（api.fund.eastmoney.com/f10/HYPZ/）===
//
// 对应 akshare `fund/fund_portfolio_em.py` 的 `fund_portfolio_industry_allocation_em`。
// 响应 `Data.QuarterInfos[].HYPZInfo[]`，展开为 5 列。

/// 天天基金网-投资组合-行业配置（对应 akshare [`akshare.fund_portfolio_industry_allocation_em`]）。
///
/// - `symbol`: 基金代码，如 `"000001"`；`date`: 查询年份，如 `"2023"`
///
/// # 返回列
/// `序号, 行业类别, 占净值比例, 市值, 截止时间`
pub fn fund_portfolio_industry_allocation_em(symbol: &str, date: &str) -> Result<Df> {
    let params = json!({
        "fundCode": symbol,
        "year": date,
        "callback": "jQuery183006997159478989867_1648016188499",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let text = http.get_text_with_headers(
        "https://api.fund.eastmoney.com/f10/HYPZ/",
        &params,
        &[
            ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.82 Safari/537.36"),
            ("Referer", "https://fundf10.eastmoney.com/"),
        ],
        None,
    )?;
    let body = text
        .trim()
        .strip_prefix("jQuery183006997159478989867_1648016188499(")
        .map(|s| s.strip_suffix(')').unwrap_or(s))
        .unwrap_or(text.trim());
    let value: Value = serde_json::from_str(body)
        .map_err(|e| AkshareError::json("fund_portfolio_industry_allocation_em", e.to_string()))?;
    let mut rows: Vec<Value> = Vec::new();
    if let Some(quarters) = value
        .get("Data")
        .and_then(|d| d.get("QuarterInfos"))
        .and_then(Value::as_array)
    {
        for q in quarters {
            if let Some(infos) = q.get("HYPZInfo").and_then(Value::as_array) {
                rows.extend(infos.iter().cloned());
            }
        }
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let f = |k: &str| r.get(k).and_then(json_value_to_string);
        out.push(vec![
            Some((i + 1).to_string()),
            f("HYMC"),
            f("ZJZBL"),
            f("SZ"),
            f("FSRQ"),
        ]);
    }
    const COLS: [&str; 5] = ["序号", "行业类别", "占净值比例", "市值", "截止时间"];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_numeric(&["占净值比例", "市值"])?;
    Ok(df)
}

// === BATCH45-A 天天基金网-新发基金（FundNewIssue.aspx，var newfunddata=）===
//
// 对应 akshare `fund/fund_init_em.py` 的 `fund_new_found_em`。响应
// `var newfunddata={datas:[...]}`，datas 每行 19 字段位置式。

/// 天天基金网-新成立基金（对应 akshare [`akshare.fund_new_found_em`]）。
///
/// # 返回列
/// `基金代码, 基金简称, 发行公司, 基金类型, 集中认购期, 募集份额, 成立日期,
/// 成立来涨幅, 基金经理, 申购状态, 优惠费率`
pub fn fund_new_found_em() -> Result<Df> {
    let params = json!({
        "t": "xcln",
        "sort": "jzrgq,desc",
        "y": "",
        "page": "1,50000",
        "isbuy": "1",
        "v": "0.4069919776543214",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    let http = HttpClient::default();
    let text = http.get_text(
        "https://fund.eastmoney.com/data/FundNewIssue.aspx",
        &params,
        None,
    )?;
    let body = text
        .trim()
        .strip_prefix("var newfunddata=")
        .ok_or_else(|| AkshareError::Empty("新发基金响应缺少 var newfunddata= 前缀".into()))?;
    let value: Value = serde_json::from_str(body)
        .map_err(|e| AkshareError::json("fund_new_found_em", e.to_string()))?;
    let rows = value
        .get("datas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // 19 字段位置式：代码,简称,发行公司,_,类型,募集份额,成立日期,成立来涨幅,基金经理,申购状态,集中认购期,_,_,_,_,_,_,_,优惠费率
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let row = r.as_array().cloned().unwrap_or_default();
        let f = |idx: usize| {
            row.get(idx)
                .and_then(Value::as_str)
                .map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or(None)
        };
        out.push(vec![
            f(0),
            f(1),
            f(2),
            f(4),
            f(10),
            f(5),
            f(6),
            f(7),
            f(8),
            f(9),
            f(18),
        ]);
    }
    const COLS: [&str; 11] = [
        "基金代码",
        "基金简称",
        "发行公司",
        "基金类型",
        "集中认购期",
        "募集份额",
        "成立日期",
        "成立来涨幅",
        "基金经理",
        "申购状态",
        "优惠费率",
    ];
    let mut df = Df::from_string_rows(&COLS, &out)?;
    df.cast_date(&["成立日期"])?;
    df.cast_numeric(&["募集份额"])?;
    df.strip_commas(&["成立来涨幅"])?;
    df.cast_numeric(&["成立来涨幅"])?;
    df.strip_suffix(&["优惠费率"], "%")?;
    df.cast_numeric(&["优惠费率"])?;
    Ok(df)
}

// === BATCH39-B 天天基金网分红送配（funddataIndex_Interface.aspx，dt=8/9）===
//
// 对应 akshare `fund/fund_fhsp_em.py` 的 `fund_fh_em`（dt=8 分红）与
// `fund_cf_em`（dt=9 拆分）。响应形如 `[[...],[...]];var jjfh_jjgs=...`，
// 取 `[[` 至 `;var` 之间的二维数组（eval），分页直到 total_page。

/// 天天基金网-基金分红（对应 akshare [`akshare.fund_fh_em`]）。
///
/// - `year`: 查询年份；`typ`: 基金类型（空串=全部）；`rank`: 排序字段
///   （`BZDM`/`ABBNAME`/`DJR`/`FSRQ`/`FHFCZ`/`FFR`）；`sort`: `"asc"`/`"desc"`；
///   `page`: `-1` 表示全部页
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 权益登记日, 除息日期, 分红, 分红发放日`
pub fn fund_fh_em(year: &str, typ: &str, rank: &str, sort: &str, page: i64) -> Result<Df> {
    fund_fhsp_base("8", year, typ, rank, sort, page)
}

/// 天天基金网-基金拆分（对应 akshare [`akshare.fund_cf_em`]）。
///
/// - `year`: 查询年份；`typ`: 基金类型（空串=全部）；`rank`: 排序字段
///   （`BZDM`/`ABBNAME`/`FSRQ`/`FHFCZ`）；`sort`: `"asc"`/`"desc"`；
///   `page`: `-1` 表示全部页
///
/// # 返回列
/// `序号, 基金代码, 基金简称, 拆分折算日, 拆分折算`
pub fn fund_cf_em(year: &str, typ: &str, rank: &str, sort: &str, page: i64) -> Result<Df> {
    fund_fhsp_base("9", year, typ, rank, sort, page)
}

/// 分红/拆分公共实现：分页拉取 `[[...]]` 二维数组。
fn fund_fhsp_base(
    dt: &str,
    year: &str,
    typ: &str,
    rank: &str,
    sort: &str,
    page: i64,
) -> Result<Df> {
    let url = "https://fund.eastmoney.com/Data/funddataIndex_Interface.aspx";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("dt".into(), Value::String(dt.into()));
    params.insert(
        "page".into(),
        Value::String(if page == -1 {
            "1".into()
        } else {
            page.to_string()
        }),
    );
    params.insert("rank".into(), Value::String(rank.into()));
    params.insert("sort".into(), Value::String(sort.into()));
    params.insert("gs".into(), Value::String("".into()));
    params.insert("ftype".into(), Value::String(typ.into()));
    params.insert("year".into(), Value::String(year.into()));

    let parse = |text: &str| -> Result<Value> {
        let start = text
            .find("[[")
            .ok_or_else(|| AkshareError::Empty("分红接口响应缺少 [[ 前缀".into()))?;
        let end = text.find(";var ").unwrap_or(text.len());
        serde_json::from_str(&text[start..end]).map_err(|e| AkshareError::json(url, e.to_string()))
    };

    let first_text = http.get_text(url, &params, None)?;
    let first = parse(&first_text)?;
    let total_page: i64 = if page == -1 {
        // `total_page=xx;var ...` → 取等号后分号前数字
        first_text
            .split('=')
            .nth(1)
            .and_then(|s| s.split(';').next())
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(1)
    } else {
        page
    };
    let mut rows: Vec<Value> = Vec::new();
    let append = |v: &Value, rows: &mut Vec<Value>| {
        if let Some(arr) = v.as_array() {
            rows.extend(arr.iter().cloned());
        }
    };
    append(&first, &mut rows);
    for p in 2..=total_page {
        params.insert("page".into(), Value::String(p.to_string()));
        let delay: f64 = rand::random_range(0.5..1.5);
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
        match http.get_text(url, &params, None) {
            Ok(t) => {
                if let Ok(v) = parse(&t) {
                    append(&v, &mut rows);
                }
            }
            Err(_) => break,
        }
    }
    // 行内字段数组 → Vec<Option<String>>
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let row = r.as_array().cloned().unwrap_or_default();
        let f = |idx: usize| {
            row.get(idx).and_then(|v| match v {
                Value::String(s) => {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                }
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
        };
        if dt == "8" {
            out.push(vec![
                Some((i + 1).to_string()),
                f(1),
                f(2),
                f(3),
                f(4),
                f(5),
                f(6),
            ]);
        } else {
            out.push(vec![Some((i + 1).to_string()), f(1), f(2), f(3), f(4)]);
        }
    }
    if dt == "8" {
        const COLS: [&str; 7] = [
            "序号",
            "基金代码",
            "基金简称",
            "权益登记日",
            "除息日期",
            "分红",
            "分红发放日",
        ];
        let mut df = Df::from_string_rows(&COLS, &out)?;
        df.cast_date(&["权益登记日", "除息日期", "分红发放日"])?;
        df.cast_numeric(&["分红"])?;
        Ok(df)
    } else {
        const COLS: [&str; 5] = ["序号", "基金代码", "基金简称", "拆分折算日", "拆分折算"];
        let mut df = Df::from_string_rows(&COLS, &out)?;
        df.cast_date(&["拆分折算日"])?;
        df.cast_numeric(&["拆分折算"])?;
        Ok(df)
    }
}

#[cfg(test)]
mod ths_tests {
    use super::*;

    #[test]
    fn jsonp_unwrap_ok() {
        let raw = r#"g({"data":{"info":{},"data":{"f1":{"code":"1"}}}})"#;
        let t = raw
            .trim()
            .strip_prefix("g(")
            .and_then(|t| t.strip_suffix(')'))
            .unwrap();
        let v: Value = serde_json::from_str(t).unwrap();
        assert!(v.get("data").is_some());
    }

    #[test]
    fn date_validation() {
        assert!(fund_etf_category_ths("ETF", "2024062").is_err());
    }
}
