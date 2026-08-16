//! bond 子模块（批次4）· 外汇交易中心 chinamoney 源。
//!
//! 对应 akshare `bond/bond_china.py`、`bond/bond_china_money.py`、
//! `bond/bond_info_cm.py`：现券成交/做市报价、收盘收益率曲线、债券信息查询/详情。
//!
//! 实现要点（§9 生产标准）：
//! - 无 `unwrap`/`expect`/`panic`；错误统一 `?`-传播为 `AkshareError`。
//! - 翻页带随机延迟 + `HttpClient` 内置重试（应对反爬限流）。
//! - 列名逐字对齐 akshare 输出（含其位置映射对应的实际字段名）。
//! - 日期类字段统一归一化为 `YYYY-MM-DD`；数值列 `cast_numeric`。

use crate::bond::util::{cell_string, df_by_keys};
use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::sources::chinamoney as cm;
use serde_json::Value;

/// 把 `YYYYMMDD` 规整为 `YYYY-MM-DD`（akshare 拼 `startDate`/`endDate` 的格式）。
fn fmt_date(s: &str) -> Result<String> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 8 {
        return Err(AkshareError::Param(format!("日期格式应为 YYYYMMDD: {s}")));
    }
    Ok(format!(
        "{}-{}-{}",
        &digits[0..4],
        &digits[4..6],
        &digits[6..8]
    ))
}

/// 在记录数组中按 `key` 字段精确匹配 `label`，取同记录的 `code` 字段值。
fn resolve_code(records: &[Value], key: &str, label: &str, code_key: &str) -> Result<String> {
    records
        .iter()
        .find(|r| r.get(key).and_then(Value::as_str) == Some(label))
        .and_then(|r| r.get(code_key).and_then(Value::as_str).map(str::to_string))
        .ok_or_else(|| AkshareError::Param(format!("在筛选条件中未找到: {label}")))
}

/// 现券市场成交行情。
///
/// 对应 akshare `bond_spot_deal()`：POST `CbtPri`，取 `records`。
/// 列名对齐 akshare 的位置映射结果（实际字段见每项注释）。
pub fn bond_spot_deal() -> Result<Df> {
    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-md-bond/CbtPri";
    let payload = vec![
        ("flag".to_string(), "1".to_string()),
        ("lang".to_string(), "cn".to_string()),
        ("bondName".to_string(), String::new()),
    ];
    let data = cm::cm_post(url, &payload)?;
    let records = cm::records_of(&data);
    let mut df = df_by_keys(
        &records,
        &[
            // akshare 位置映射对应的实际字段（顺序对齐 akshare 的 select）
            ("abdAssetEncdShrtDesc", "债券简称"),       // 位置2
            ("dmiLatestRate", "成交净价"),              // 位置12
            ("dmiLatestContraRateLabel", "最新收益率"), // 位置15
            ("bpNum", "涨跌"),                          // 位置7
            ("dmiWghtdContraRate", "加权收益率"),       // 位置11
            ("dmiTtlTradedAmnt", "交易量"),             // 位置17
        ],
    )?;
    df.cast_numeric(&["成交净价", "最新收益率", "涨跌", "加权收益率", "交易量"])?;
    Ok(df)
}

/// 现券市场做市报价。
///
/// 对应 akshare `bond_spot_quote()`：POST `CbMktMakQuot`，取 `records`。
/// `买入/卖出净价`、`买入/卖出收益率` 形如 `"105.97 / 106.02"`，按 `/` 拆成两列。
pub fn bond_spot_quote() -> Result<Df> {
    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-md-bond/CbMktMakQuot";
    let payload = vec![
        ("flag".to_string(), "1".to_string()),
        ("lang".to_string(), "cn".to_string()),
    ];
    let data = cm::cm_post(url, &payload)?;
    let records = cm::records_of(&data);
    let mut df = df_by_keys(
        &records,
        &[
            ("emaEntyEncdShrtDesc", "报价机构"),  // 位置2
            ("abdAssetEncdShrtDesc", "债券简称"), // 位置6
            ("tradeAmnt", "买入/卖出净价"),       // 位置13
            ("contraRate", "买入/卖出收益率"),    // 位置11
        ],
    )?;
    // 拆列：与 akshare str.split("/").expand 等价（去空格后数值化）。
    split_slash(&mut df, "买入/卖出净价", "买入净价", "卖出净价")?;
    split_slash(&mut df, "买入/卖出收益率", "买入收益率", "卖出收益率")?;
    let mut df = df.select(&[
        "报价机构",
        "债券简称",
        "买入净价",
        "卖出净价",
        "买入收益率",
        "卖出收益率",
    ])?;
    df.cast_numeric(&["买入净价", "卖出净价", "买入收益率", "卖出收益率"])?;
    Ok(df)
}

/// 把 `src` 列按 `/` 拆分为 `left`/`right` 两列（与 akshare `str.split("/")` 一致）。
fn split_slash(df: &mut Df, src: &str, left: &str, right: &str) -> Result<()> {
    let series = df
        .inner()
        .column(src)
        .map_err(|e| AkshareError::Empty(format!("拆列缺失 {src}: {e}")))?
        .clone();
    let mut lvals: Vec<Option<String>> = Vec::with_capacity(series.len());
    let mut rvals: Vec<Option<String>> = Vec::with_capacity(series.len());
    let ca = series
        .str()
        .map_err(|_| AkshareError::Empty(format!("{src} 非字符串列")))?;
    for i in 0..series.len() {
        match ca.get(i) {
            Some(s) => {
                let parts: Vec<&str> = s.split('/').collect();
                lvals.push(parts.first().map(|v| v.trim().to_string()));
                rvals.push(parts.get(1).map(|v| v.trim().to_string()));
            }
            None => {
                lvals.push(None);
                rvals.push(None);
            }
        }
    }
    df.with_column(left, &lvals)?;
    df.with_column(right, &rvals)?;
    Ok(())
}

/// 收盘收益率曲线历史数据。
///
/// 对应 akshare `bond_china_close_return(symbol, period, start_date, end_date)`：
/// 先由 `bond_china_close_return_map()` 解析 `symbol→code`，再 GET `ClsYldCurvHis`。
pub fn bond_china_close_return(
    symbol: &str,
    period: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    let map = cm::close_return_map()?;
    let symbol_code = resolve_code(&map, "cnLabel", symbol, "value")?;
    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-bk-currency/ClsYldCurvHis";
    let params = vec![
        ("lang".to_string(), "CN".to_string()),
        ("reference".to_string(), "1,2,3".to_string()),
        ("bondType".to_string(), symbol_code),
        ("startDate".to_string(), fmt_date(start_date)?),
        ("endDate".to_string(), fmt_date(end_date)?),
        ("termId".to_string(), period.to_string()),
        ("pageNum".to_string(), "1".to_string()),
        ("pageSize".to_string(), "50".to_string()),
    ];
    let data = cm::cm_get(url, &params)?;
    let records = cm::records_of(&data);
    // akshare 先 del newDateValue（英文日期），再位置映射剩余 5 列。
    let mut df = Df::from_json_rows(&records)?;
    let keep: Vec<String> = df
        .column_names()
        .iter()
        .filter(|c| c.as_str() != "newDateValue")
        .cloned()
        .collect();
    let keep_refs: Vec<&str> = keep.iter().map(String::as_str).collect();
    df = df.select(&keep_refs)?;
    df.rename_columns(&["日期", "期限", "到期收益率", "即期收益率", "远期收益率"])?;
    let mut df = df.select(&["日期", "期限", "到期收益率", "即期收益率", "远期收益率"])?;
    df.cast_date(&["日期"])?;
    df.cast_numeric(&["期限", "到期收益率", "即期收益率", "远期收益率"])?;
    Ok(df)
}

/// 收盘收益率曲线映射表（symbol→code 解析用）。
///
/// 对应 akshare `bond_china_close_return_map()`：GET `ClsYldCurvCurvGO`，
/// 返回原始 `records`（含 `value`/`cnLabel`/`enLabel` 等键）。
pub fn bond_china_close_return_map() -> Result<Df> {
    let records = cm::close_return_map()?;
    Df::from_json_rows(&records)
}

/// 债券信息查询。
///
/// 对应 akshare `bond_info_cm(...)`：先按需要解析债券类型/息票类型/主承销商代码，
/// 再分页 POST `BondMarketInfoList2` 取 `data.resultList`。
#[allow(clippy::too_many_arguments)]
pub fn bond_info_cm(
    bond_name: &str,
    bond_code: &str,
    bond_issue: &str,
    bond_type: &str,
    coupon_type: &str,
    issue_year: &str,
    underwriter: &str,
    grade: &str,
) -> Result<Df> {
    let bt_val = if bond_type.is_empty() {
        String::new()
    } else {
        let df = cm::info_cm_query("债券类型")?;
        resolve_code(&df, "name", bond_type, "code")?
    };
    let ct_val = if coupon_type.is_empty() {
        String::new()
    } else {
        let df = cm::info_cm_query("息票类型")?;
        resolve_code(&df, "name", coupon_type, "code")?
    };
    let uw_val = if underwriter.is_empty() {
        String::new()
    } else {
        let df = cm::info_cm_query("主承销商")?;
        resolve_code(&df, "name", underwriter, "code")?
    };

    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-bond-md/BondMarketInfoList2";
    let mut all: Vec<Value> = Vec::new();
    let mut page: u64 = 1;
    loop {
        let payload = vec![
            ("pageNo".to_string(), page.to_string()),
            ("pageSize".to_string(), "15".to_string()),
            ("bondName".to_string(), bond_name.to_string()),
            ("bondCode".to_string(), bond_code.to_string()),
            ("issueEnty".to_string(), bond_issue.to_string()),
            ("bondType".to_string(), bt_val.clone()),
            ("bondSpclPrjctVrty".to_string(), String::new()),
            ("couponType".to_string(), ct_val.clone()),
            ("issueYear".to_string(), issue_year.to_string()),
            ("entyDefinedCode".to_string(), uw_val.clone()),
            ("rtngShrt".to_string(), grade.to_string()),
        ];
        let data = cm::cm_post(url, &payload)?;
        let rl = cm::result_list_of(&data);
        let total = cm::page_total_of(&data);
        if rl.is_empty() {
            break;
        }
        all.extend(rl);
        if page >= total {
            break;
        }
        page += 1;
        cm::random_delay();
    }

    df_by_keys(
        &all,
        &[
            ("bondName", "债券简称"),
            ("bondCode", "债券代码"),
            ("entyFullName", "发行人/受托机构"),
            ("bondType", "债券类型"),
            ("issueStartDate", "发行日期"),
            ("debtRtng", "最新债项评级"),
            ("bondDefinedCode", "查询代码"),
        ],
    )
}

/// 债券详情。
///
/// 对应 akshare `bond_info_detail_cm(symbol)`：先用 `bond_info_cm(bond_name=symbol)`
/// 取 `查询代码`，再 POST `BondDetailInfo` 取 `data.bondBaseInfo`，展开为 `name/value` 两列。
pub fn bond_info_detail_cm(symbol: &str) -> Result<Df> {
    let info = bond_info_cm(symbol, "", "", "", "", "", "", "")?;
    // 取首行「查询代码」作为 bondDefinedCode
    let bond_defined_code = info
        .inner()
        .column("查询代码")
        .ok()
        .and_then(|c| c.str().ok())
        .and_then(|s| s.get(0))
        .map(str::to_string)
        .ok_or_else(|| AkshareError::Empty(format!("未查到债券: {symbol}")))?;

    let url = "https://www.chinamoney.com.cn/ags/ms/cm-u-bond-md/BondDetailInfo";
    let payload = vec![("bondDefinedCode".to_string(), bond_defined_code)];
    let data = cm::cm_post(url, &payload)?;
    let dict = data
        .get("data")
        .and_then(|d| d.get("bondBaseInfo"))
        .and_then(Value::as_object)
        .ok_or_else(|| AkshareError::Empty("债券详情响应缺失 bondBaseInfo".into()))?;

    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    for (k, v) in dict {
        if k == "creditRateEntyList" || k == "exerciseInfoList" {
            continue;
        }
        rows.push(vec![Some(k.clone()), cell_string(v)]);
    }
    Df::from_string_rows(&["name", "value"], &rows)
}

/// 债券信息查询-筛选条件查询（对应 akshare `bond_info_cm_query(symbol)`）。
///
/// `symbol` ∈ {"主承销商", "债券类型", "息票类型", "发行年份", "评级等级"}。
/// 返回该筛选维度的「名称→代码」映射表，列名 `name, code`（均为字符串）。
///
/// 接口返回的是**数组的数组**，逐元素宽度不定（主承销商为 `[code, name]`；
/// 其余维度多为 `[name, code]`，个别为单元素 `[name]`）。按宽度归一化：
/// 宽度 ≥ 2 取 `[name, code] = [cells[0], cells[1]]`；宽度 == 1 取 `code = name = cells[0]`。
/// 主承销商分支额外交换首尾（接口为 `[code, name]`）。
pub fn bond_info_cm_query(symbol: &str) -> Result<Df> {
    let records = cm::info_cm_query(symbol)?;
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(records.len());
    for v in &records {
        let cells = arr_cells(v);
        let pair = if symbol == "主承销商" {
            // [code, name] → 交换为 [name, code]
            match cells.len() {
                0 => continue,
                1 => vec![cells[0].clone(), cells[0].clone()],
                _ => vec![cells[1].clone(), cells[0].clone()],
            }
        } else {
            match cells.len() {
                0 => continue,
                1 => vec![cells[0].clone(), cells[0].clone()],
                _ => vec![cells[0].clone(), cells[1].clone()],
            }
        };
        rows.push(pair);
    }
    Df::from_string_rows(&["name", "code"], &rows)
}

/// 把「数组的数组」中的单行（或单元素）转成字符串单元向量。
fn arr_cells(v: &Value) -> Vec<Option<String>> {
    match v {
        Value::Array(arr) => arr.iter().map(cell_string).collect(),
        Value::String(s) => vec![Some(s.clone())],
        other => vec![cell_string(other)],
    }
}
