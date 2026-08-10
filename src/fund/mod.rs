//! 基金数据接口。
//!
//! 首批实现（对应 akshare `fund/fund_etf_em.py`、`fund/fund_lof_em.py`）：
//! - [`fund_etf_spot_em`]：ETF 实时行情
//! - [`fund_lof_spot_em`]：LOF 实时行情
//!
//! 说明：akshare 的 ETF 实时行情主用 `push2delay` 延迟节点，本实现走
//! [`push2_urls`] 多节点容灾（同簇数据，避免单节点限流）。

use crate::core::df::Df;
use crate::core::error::Result;
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{fetch_clist, push2_urls};
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
