//! 东方财富国际期货 + 中证商品指数 + 东财期货规则（批次 29 子组 A）。
//!
//! 对应 akshare：
//! - `futures/futures_hf_em.py`：`futures_global_spot_em` / `futures_global_hist_em`
//! - `futures/futures_index_ccidx.py`：`futures_index_ccidx`
//! - `futures/futures_rule_em.py`：`futures_rule_em`
//!
//! 列名/列序严格对齐 akshare；实时类接口（`global_spot_em`）数值随时间变化，
//! parity 用例使用 `loose` 仅校验列契约；历史/静态接口使用 `loose`（数据量大，
//! 放行浮点末位漂移）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::finalize_report;
use serde_json::{Map, Value};

/// 东财国际期货列表 token（与 `option_current_em` 同源）。
const FUTSSE_TOKEN: &str = "58b2fa8f54638b60b87d69b31969089c";

/// JSON 值 → `Option<String>`（null 映射为 None；其余 `to_string`）。
fn cell(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// 中证商品指数（CCIDX）：直接 JSON
// ---------------------------------------------------------------------------

/// 中证商品指数-日频率（对应 akshare [`akshare.futures_index_ccidx`]）。
///
/// `symbol`：可选 `中证商品期货指数` / `中证商品期货价格指数`。
/// 数据源 `http://www.ccidx.com/CCI-ZZZS/index/getDateLine`（GET `indexId`）。
///
/// # 返回列
/// `日期, 指数代码, 收盘点位, 结算点位, 涨跌, 涨跌幅`
/// （`日期` 归一化为 `YYYY-MM-DD`；四个点位/涨跌列转 float64）
pub fn futures_index_ccidx(symbol: &str) -> Result<Df> {
    let index_id = match symbol {
        "中证商品期货指数" => "100001.CCI",
        "中证商品期货价格指数" => "000001.CCI",
        other => {
            return Err(AkshareError::Param(format!(
                "未知 symbol: {other}（可选：中证商品期货指数/中证商品期货价格指数）"
            )))
        }
    };
    let url = "http://www.ccidx.com/CCI-ZZZS/index/getDateLine";
    let mut params = Map::new();
    params.insert("indexId".into(), Value::String(index_id.into()));
    let http = HttpClient::default();
    let v = http.get_json(url, &params, None)?;
    let arr = v
        .get("data")
        .and_then(|d| d.get("dateLineJson"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if arr.is_empty() {
        return Df::from_string_rows(&["日期", "指数代码", "收盘点位", "结算点位", "涨跌", "涨跌幅"], &[]);
    }
    // 列名/列序严格对齐 akshare：`pd.DataFrame(dateLineJson)` 取首项键序，
    // 仅对 6 个字段做中文重命名，其余 18 个英文字段原样保留（共 24 列）。
    let first_keys: Vec<&str> = arr[0]
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let rename: Vec<(&str, &str)> = first_keys
        .iter()
        .map(|k| {
            let c = match *k {
                "tradeDate" => "日期",
                "indexId" => "指数代码",
                "closingPrice" => "收盘点位",
                "settlePrice" => "结算点位",
                "dailyIncreaseAndDecrease" => "涨跌",
                "dailyIncreaseAndDecreasePercentage" => "涨跌幅",
                other => other,
            };
            (*k, c)
        })
        .collect();
    let select: Vec<&str> = rename.iter().map(|(_, c)| *c).collect();
    // akshare `pd.DataFrame(dateLineJson)` 将该 JSON 视为全 float64，仅 `日期`/
    // `createTime`/`指数代码` 三列为字符串。cast_numeric 为宽松转换（无法解析的
    // 列会写成 NaN 且 dtype 仍变 Float64），故必须显式排除这三列，仅对其余列转数值。
    let str_cols = ["日期", "createTime", "指数代码"];
    let numeric_cols: Vec<&str> = select
        .iter()
        .filter(|c| !str_cols.contains(c))
        .copied()
        .collect();
    let mut df = finalize_report(&arr, &rename, &select, &numeric_cols, None)?;
    // akshare：`sort_values(by=["日期"])`（升序）
    df = df.sort_by("日期", true, false)?;
    Ok(df)
}

// ---------------------------------------------------------------------------
// 东方财富-国际期货实时行情（futsseapi list，复用 option_current_em 模板）
// ---------------------------------------------------------------------------

/// 东方财富网-行情中心-期货市场-国际期货实时行情（对应 akshare
/// [`akshare.futures_global_spot_em`]）。
///
/// 数据源 `https://futsseapi.eastmoney.com/list/COMEX,NYMEX,COBOT,SGX,NYBOT,LME,MDEX,TOCOM,IPE`
/// （GET，token 固定；`pageSize=20000` 一次取全量，对应 akshare 分页 concat 后的全集）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨结, 成交量, 买盘, 卖盘, 持仓量`
/// （`序号` 1 起始；数值列转 float64）
pub fn futures_global_spot_em() -> Result<Df> {
    let url = "https://futsseapi.eastmoney.com/list/COMEX,NYMEX,COBOT,SGX,NYBOT,LME,MDEX,TOCOM,IPE";
    let mut params = Map::new();
    params.insert("orderBy".into(), Value::String("dm".into()));
    params.insert("sort".into(), Value::String("desc".into()));
    params.insert("pageSize".into(), Value::String("20000".into()));
    params.insert("pageIndex".into(), Value::String("0".into()));
    params.insert("token".into(), Value::String(FUTSSE_TOKEN.into()));
    params.insert(
        "field".into(),
        Value::String(
            "dm,sc,name,p,zsjd,zde,zdf,f152,o,h,l,zjsj,vol,wp,np,ccl".into(),
        ),
    );
    params.insert("blockName".into(), Value::String("callback".into()));
    let http = HttpClient::default();
    let v = http.get_json(url, &params, None)?;
    let list = v.get("list").and_then(Value::as_array).cloned().unwrap_or_default();
    let cols = [
        "序号",
        "代码",
        "名称",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "今开",
        "最高",
        "最低",
        "昨结",
        "成交量",
        "买盘",
        "卖盘",
        "持仓量",
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(list.len());
    for item in &list {
        let f = |k: &str| item.get(k).and_then(cell);
        rows.push(vec![
            None, // 序号，构建后填充
            f("dm"),
            f("name"),
            f("p"),
            f("zde"),
            f("zdf"),
            f("o"),
            f("h"),
            f("l"),
            f("zjsj"),
            f("vol"),
            f("wp"),
            f("np"),
            f("ccl"),
        ]);
    }
    let mut df = Df::from_string_rows(&cols, &rows)?;
    // 1 起始序号（对应 akshare `big_df["index"] = big_df["index"] + 1`）。
    let n = df.height();
    let idx: Vec<Option<String>> = (1..=n).map(|i| Some(i.to_string())).collect();
    df.with_column("序号", &idx)?;
    df.cast_numeric(&[
        "序号",
        "最新价",
        "涨跌额",
        "涨跌幅",
        "今开",
        "最高",
        "最低",
        "昨结",
        "成交量",
        "买盘",
        "卖盘",
        "持仓量",
    ])?;
    Ok(df)
}

// ---------------------------------------------------------------------------
// 东方财富-国际期货历史行情（push2his kline）
// ---------------------------------------------------------------------------

/// 国际期货品种代码 → 东财市场代码（对应 akshare `__futures_global_hist_market_code`）。
fn global_market_code(symbol: &str) -> Option<i64> {
    let base: String = symbol.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let metals = ["HG", "GC", "SI", "QI", "QO", "MGC", "LTH"];
    let energy = ["CL", "NG", "RB", "HO", "PA", "PL", "QM"];
    let agro = [
        "ZW", "ZM", "ZS", "ZC", "XC", "XK", "XW", "YM", "TY", "US", "EH", "ZL", "ZR", "ZO", "FV",
        "TU", "UL", "NQ", "ES",
    ];
    let china = ["TF", "RT", "CN"];
    let soft = ["SB", "CT", "SF"];
    let l_special = ["LCPT", "LZNT", "LALT", "LTNT", "LLDT", "LNKT"];
    if metals.contains(&base.as_str()) {
        return Some(101);
    }
    if energy.contains(&base.as_str()) {
        return Some(102);
    }
    if agro.contains(&base.as_str()) {
        return Some(103);
    }
    if china.contains(&base.as_str()) {
        return Some(104);
    }
    if soft.contains(&base.as_str()) {
        return Some(108);
    }
    if l_special.contains(&base.as_str()) {
        return Some(109);
    }
    if base == "MPM" {
        return Some(110);
    }
    if base.starts_with('J') {
        return Some(111);
    }
    if ["M", "B", "G"].contains(&base.as_str()) {
        return Some(112);
    }
    None
}

/// 东方财富网-行情中心-期货市场-国际期货历史行情（对应 akshare
/// [`akshare.futures_global_hist_em`]）。
///
/// `symbol`：品种代码（如 `HG00Y`）。数据源 `push2his.eastmoney.com/api/qt/stock/kline/get`
/// （`secid = {market}.{symbol}`，`klt=101` 日线，`lmt=6600`）。
///
/// # 返回列
/// `日期, 代码, 名称, 开盘, 最新价, 最高, 最低, 总量, 涨幅, 持仓, 日增`
/// （`日期` 为 `YYYY-MM-DD`；数值列转 float64；`日增` 还原为有符号 32 位整数）
pub fn futures_global_hist_em(symbol: &str) -> Result<Df> {
    let market = global_market_code(symbol).ok_or_else(|| {
        AkshareError::Param(format!("无法识别国际期货品种市场代码: {symbol}"))
    })?;
    let url = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
    let mut params = Map::new();
    params.insert("secid".into(), Value::String(format!("{market}.{symbol}")));
    params.insert("klt".into(), Value::String("101".into()));
    params.insert("fqt".into(), Value::String("1".into()));
    params.insert("lmt".into(), Value::String("6600".into()));
    params.insert("end".into(), Value::String("20500000".into()));
    params.insert("iscca".into(), Value::String("1".into()));
    params.insert(
        "fields1".into(),
        Value::String("f1,f2,f3,f4,f5,f6,f7,f8".into()),
    );
    params.insert(
        "fields2".into(),
        Value::String("f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64".into()),
    );
    params.insert("ut".into(), Value::String("f057cbcbce2a86e2866ab8877db1d059".into()));
    params.insert("forcect".into(), Value::String("1".into()));
    let http = HttpClient::default();
    let v = http.get_json(url, &params, None)?;
    let data = v.get("data").and_then(Value::as_object).cloned().unwrap_or_default();
    let code = data.get("code").and_then(cell).unwrap_or_default();
    let name = data.get("name").and_then(cell).unwrap_or_default();
    let klines = data
        .get("klines")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cols = [
        "日期",
        "代码",
        "名称",
        "开盘",
        "最新价",
        "最高",
        "最低",
        "总量",
        "涨幅",
        "持仓",
        "日增",
    ];
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(klines.len());
    for k in &klines {
        let line = match k.as_str() {
            Some(s) => s,
            None => continue,
        };
        let p: Vec<&str> = line.split(',').collect();
        if p.len() < 14 {
            continue;
        }
        // 字段索引对应 f51..f64：0 日期, 1 开, 2 收(最新价), 3 高, 4 低, 5 量(总量),
        // 8 涨幅(f59), 12 持仓(f63), 13 日增(f64)
        let mut daily_increase = p[13].parse::<f64>().unwrap_or(0.0);
        // 还原有符号 32 位整数（对应 akshare 的 2^32 回卷修复）
        if daily_increase > 2f64.powi(31) - 1.0 {
            daily_increase -= 2f64.powi(32) - 1.0 + 1.0;
        }
        rows.push(vec![
            Some(p[0].to_string()),
            Some(code.clone()),
            Some(name.clone()),
            Some(p[1].to_string()),
            Some(p[2].to_string()),
            Some(p[3].to_string()),
            Some(p[4].to_string()),
            Some(p[5].to_string()),
            Some(p[8].to_string()),
            Some(p[12].to_string()),
            Some(daily_increase.to_string()),
        ]);
    }
    let mut df = Df::from_string_rows(&cols, &rows)?;
    df.cast_numeric(&[
        "开盘",
        "最新价",
        "最高",
        "最低",
        "总量",
        "涨幅",
        "持仓",
        "日增",
    ])?;
    Ok(df)
}
