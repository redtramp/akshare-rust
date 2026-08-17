//! 巨潮资讯（cninfo）数据源。
//!
//! 对应 akshare `stock/stock_profile_cninfo.py` 等模块：
//! - 所有接口走 `https://webapi.cninfo.com.cn/api/sysapi/p_sysapiXXXX` POST
//! - 请求头需携带 `Accept-Enckey`：由内置 `cninfo.js` 的 `getResCode1()`
//!   （AES-CBC(时间戳) → base64）生成，对应 akshare 的 `py_mini_racer` 执行
//! - 响应形如 `{count, records: [...]}`，列序 = records 首对象键序

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::core::js_engine::cninfo_get_res_code;
use serde_json::{json, Map, Value};

/// cninfo 公共请求头（对应 akshare 各 cninfo 函数的 headers 字典）。
fn cninfo_headers(mcode: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Accept", "*/*".to_string()),
        ("Accept-Enckey", mcode.to_string()),
        ("Accept-Encoding", "gzip, deflate".to_string()),
        ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8".to_string()),
        ("Cache-Control", "no-cache".to_string()),
        ("Content-Length", "0".to_string()),
        ("Host", "webapi.cninfo.com.cn".to_string()),
        ("Origin", "https://webapi.cninfo.com.cn".to_string()),
        ("Pragma", "no-cache".to_string()),
        ("Proxy-Connection", "keep-alive".to_string()),
        ("Referer", "https://webapi.cninfo.com.cn/".to_string()),
        ("X-Requested-With", "XMLHttpRequest".to_string()),
    ]
}

/// POST sysapi 端点并返回 `records` 数组。
///
/// 对应 akshare 每个 cninfo 函数中重复的
/// `js_code.call("getResCode1") → requests.post(url, params, headers)` 流程。
fn post_sysapi(endpoint: &str, params: &Map<String, Value>) -> Result<Vec<Value>> {
    let mcode = cninfo_get_res_code()?;
    let headers: Vec<(&str, String)> = cninfo_headers(&mcode);
    let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let url = format!("https://webapi.cninfo.com.cn/api/sysapi/{endpoint}");
    let http = HttpClient::default();
    let data = http.post_json(&url, params, &header_refs)?;
    let records = data
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(records)
}

/// 单条记录转 DataFrame：按 `records[0]` 键序取前 `n` 个值，列名映射为 `columns`。
///
/// 对应 akshare 的 `pd.Series(records_json).to_frame().T` + 列名覆盖。
/// 键序由 JSON `preserve_order` 保证与响应一致。
fn single_record_df(records: &[Value], columns: &[&str]) -> Result<Df> {
    let Some(record) = records.first() else {
        return Df::from_string_rows(columns, &[]);
    };
    let obj = record
        .as_object()
        .ok_or_else(|| AkshareError::Empty("records[0] 不是 JSON 对象".into()))?;
    let keys: Vec<&String> = obj.keys().collect();
    let n = columns.len().min(keys.len());
    let row: Vec<Option<String>> = keys[..n]
        .iter()
        .map(|k| match obj.get(*k) {
            Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(other) => Some(other.to_string()),
            None => None,
        })
        .collect();
    Df::from_string_rows(columns, &[row])
}

/// 多记录 DataFrame：records 数组转表，列名覆盖为 `columns`。
///
/// 对应 akshare `pd.DataFrame(records)` + 列名覆盖。
/// 注意：多条记录可能字段不一致，akshare 以首条键序为准。
fn multi_record_df(records: &[Value], columns: &[&str]) -> Result<Df> {
    if records.is_empty() {
        return Df::from_string_rows(columns, &[]);
    }
    let Some(first) = records.first().and_then(Value::as_object) else {
        return Err(AkshareError::Empty("records[0] 不是 JSON 对象".into()));
    };
    let keys: Vec<&String> = first.keys().collect();
    let n = columns.len().min(keys.len());
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(records.len());
    for r in records {
        let obj = r
            .as_object()
            .ok_or_else(|| AkshareError::Empty("records 元素不是 JSON 对象".into()))?;
        let row: Vec<Option<String>> = keys[..n]
            .iter()
            .map(|k| match obj.get(*k) {
                Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(other) => Some(other.to_string()),
                None => None,
            })
            .collect();
        rows.push(row);
    }
    Df::from_string_rows(columns, &rows)
}

/// 列重命名（基于新列名集合构建子表，对应 akshare `df[cols]` 重排）。
fn reorder_df(df: &Df, columns: &[&str]) -> Result<Df> {
    df.select(columns)
}

/// 巨潮资讯-个股-公司概况（对应 akshare [`akshare.stock_profile_cninfo`]）。
///
/// # 返回列
/// `公司名称, 英文名称, 曾用简称, A股代码, A股简称, B股代码, B股简称,
/// H股代码, H股简称, 入选指数, 所属市场, 所属行业, 法人代表, 注册资金,
/// 成立日期, 上市日期, 官方网站, 电子邮箱, 联系电话, 传真, 注册地址,
/// 办公地址, 邮政编码, 主营业务, 经营范围, 机构简介`
pub fn stock_profile_cninfo(symbol: &str) -> Result<Df> {
    let params = json!({ "scode": symbol });
    let records = post_sysapi("p_sysapi1133", params.as_object().expect("scode 参数"))?;
    const COLUMNS: [&str; 26] = [
        "公司名称",
        "英文名称",
        "曾用简称",
        "A股代码",
        "A股简称",
        "B股代码",
        "B股简称",
        "H股代码",
        "H股简称",
        "入选指数",
        "所属市场",
        "所属行业",
        "法人代表",
        "注册资金",
        "成立日期",
        "上市日期",
        "官方网站",
        "电子邮箱",
        "联系电话",
        "传真",
        "注册地址",
        "办公地址",
        "邮政编码",
        "主营业务",
        "经营范围",
        "机构简介",
    ];
    single_record_df(&records, &COLUMNS)
}

/// 巨潮资讯-个股-上市相关（对应 akshare [`akshare.stock_ipo_summary_cninfo`]）。
///
/// 日期列保留原字符串（akshare 转 `date` 类型，字符串形式一致）。
///
/// # 返回列
/// `股票代码, 招股公告日期, 中签率公告日, 每股面值, 总发行数量,
/// 发行前每股净资产, 摊薄发行市盈率, 募集资金净额, 上网发行日期,
/// 上市日期, 发行价格, 发行费用总额, 发行后每股净资产, 上网发行中签率, 主承销商`
pub fn stock_ipo_summary_cninfo(symbol: &str) -> Result<Df> {
    let params = json!({ "scode": symbol });
    let records = post_sysapi("p_sysapi1134", params.as_object().expect("scode 参数"))?;
    const COLUMNS: [&str; 15] = [
        "股票代码",
        "招股公告日期",
        "中签率公告日",
        "每股面值",
        "总发行数量",
        "发行前每股净资产",
        "摊薄发行市盈率",
        "募集资金净额",
        "上网发行日期",
        "上市日期",
        "发行价格",
        "发行费用总额",
        "发行后每股净资产",
        "上网发行中签率",
        "主承销商",
    ];
    let mut df = single_record_df(&records, &COLUMNS)?;
    df.cast_date(&["招股公告日期", "中签率公告日", "上网发行日期", "上市日期"])?;
    df.cast_numeric(&[
        "每股面值",
        "总发行数量",
        "发行前每股净资产",
        "摊薄发行市盈率",
        "募集资金净额",
        "发行价格",
        "发行费用总额",
        "发行后每股净资产",
        "上网发行中签率",
    ])?;
    Ok(df)
}

/// 巨潮资讯-个股-历史分红（对应 akshare [`akshare.stock_dividend_cninfo`]）。
///
/// 多记录：列重命名 + 数值化 + 按实施方案公告日期排序（升序）。
///
/// # 返回列
/// `实施方案公告日期, 分红类型, 送股比例, 转增比例, 派息比例, 股权登记日,
/// 除权日, 派息日, 股份到账日, 实施方案分红说明, 报告时间`
pub fn stock_dividend_cninfo(symbol: &str) -> Result<Df> {
    let params = json!({ "scode": symbol });
    let records = post_sysapi("p_sysapi1139", params.as_object().expect("scode 参数"))?;

    // 对应 akshare：`pd.DataFrame(records)`（列序=响应键序）→ `rename`（位置不变）
    // → 日期/数值转换 → 排序 → 重排到输出列序。
    // 注意：响应键序可能 ≠ RENAME 数组序，必须保持响应键序再按名重排。
    const RENAME: [(&str, &str); 11] = [
        ("F006D", "实施方案公告日期"),
        ("F044V", "分红类型"),
        ("F011N", "转增比例"),
        ("F010N", "送股比例"),
        ("F012N", "派息比例"),
        ("F018D", "股权登记日"),
        ("F020D", "除权日"),
        ("F023D", "派息日"),
        ("F025D", "股份到账日"),
        ("F007V", "实施方案分红说明"),
        ("F001V", "报告时间"),
    ];
    // 1) 保持响应键序构建（对应 pd.DataFrame(records)）
    let mut df = Df::from_json_rows(&records)?;
    // 2) 按名重命名（仅重命名存在的键，位置不变，对应 rename）
    let cur_names = df.column_names();
    let renamed: Vec<String> = cur_names
        .iter()
        .map(|n| {
            RENAME
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| n.clone())
        })
        .collect();
    let refs: Vec<&str> = renamed.iter().map(String::as_str).collect();
    df.rename_columns(&refs)?;
    // 3) 日期/数值转换
    df.cast_date(&["实施方案公告日期", "股权登记日", "除权日", "派息日"])?;
    df.cast_numeric(&["送股比例", "转增比例", "派息比例"])?;
    // 4) 按日期升序排序（ISO 字符串序 = 时间序）
    let sorted = df.sort_by("实施方案公告日期", true, false)?;
    // 5) 重排到 akshare 输出列序
    reorder_df(
        &sorted,
        &[
            "实施方案公告日期",
            "分红类型",
            "送股比例",
            "转增比例",
            "派息比例",
            "股权登记日",
            "除权日",
            "派息日",
            "股份到账日",
            "实施方案分红说明",
            "报告时间",
        ],
    )
}

/// 巨潮资讯-数据中心-新股数据-新股发行（对应 akshare [`akshare.stock_new_ipo_cninfo`]）。
///
/// # 返回列
/// `证劵代码, 证券简称, 上市日期, 申购日期, 发行价, 总发行数量, 发行市盈率,
/// 上网发行中签率, 摇号结果公告日, 中签公告日, 中签缴款日, 网上申购上限, 上网发行数量`
pub fn stock_new_ipo_cninfo() -> Result<Df> {
    let params = json!({ "timetype": "36", "market": "ALL" });
    let records = post_sysapi("p_sysapi1097", params.as_object().expect("timetype 参数"))?;
    const COLUMNS: [&str; 13] = [
        "摇号结果公告日",
        "中签公告日",
        "证券简称",
        "上市日期",
        "中签缴款日",
        "申购日期",
        "发行价",
        "证劵代码",
        "上网发行中签率",
        "总发行数量",
        "发行市盈率",
        "上网发行数量",
        "网上申购上限",
    ];
    let mut df = multi_record_df(&records, &COLUMNS)?;
    df.cast_date(&[
        "摇号结果公告日",
        "中签公告日",
        "上市日期",
        "中签缴款日",
        "申购日期",
    ])?;
    df.cast_numeric(&[
        "发行价",
        "上网发行中签率",
        "总发行数量",
        "发行市盈率",
        "上网发行数量",
        "网上申购上限",
    ])?;
    reorder_df(
        &df,
        &[
            "证劵代码",
            "证券简称",
            "上市日期",
            "申购日期",
            "发行价",
            "总发行数量",
            "发行市盈率",
            "上网发行中签率",
            "摇号结果公告日",
            "中签公告日",
            "中签缴款日",
            "网上申购上限",
            "上网发行数量",
        ],
    )
}

/// 巨潮资讯-数据中心-新股数据-新股过会（对应 akshare [`akshare.stock_new_gh_cninfo`]）。
///
/// # 返回列
/// `公司名称, 上会日期, 审核类型, 审议内容, 审核结果, 审核公告日`
pub fn stock_new_gh_cninfo() -> Result<Df> {
    let records = post_sysapi("p_sysapi1098", &Map::new())?;
    const COLUMNS: [&str; 6] = [
        "公司名称",
        "上会日期",
        "审核类型",
        "审议内容",
        "审核结果",
        "审核公告日",
    ];
    let mut df = multi_record_df(&records, &COLUMNS)?;
    df.cast_date(&["上会日期", "审核公告日"])?;
    Ok(df)
}

/// 从 cninfo 记录中按 `(原始键, 输出列名)` 对直接抽取目标列（顺序=输出列序）。
///
/// 对应 akshare `pd.DataFrame(records).rename(columns=...)[cols]`：
/// 只选取重命名映射里的键，忽略响应中可能存在的其余字段，并直接以中文名输出，
/// 从而避免 `rename_columns` 对列数严格相等的要求（响应键集不固定）。
fn extract_df(records: &[Value], picks: &[(&str, &str)]) -> Result<Df> {
    let col_names: Vec<&str> = picks.iter().map(|(_, cn)| *cn).collect();
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(records.len());
    for r in records {
        let obj = r
            .as_object()
            .ok_or_else(|| AkshareError::Empty("records 元素不是 JSON 对象".into()))?;
        let row: Vec<Option<String>> = picks
            .iter()
            .map(|(k, _)| match obj.get(*k) {
                Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(other) => Some(other.to_string()),
                None => None,
            })
            .collect();
        rows.push(row);
    }
    Df::from_string_rows(&col_names, &rows)
}

/// 将 `YYYYMMDD` 转为 `YYYY-MM-DD`（cninfo 日期范围参数格式）。
fn fmt_cninfo_date(d: &str) -> String {
    if d.len() == 8 {
        format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
    } else {
        d.to_string()
    }
}

/// 债券发行类函数公共流程：POST sysapi + 抽取列 + 日期/数值化。
fn bond_issue_cninfo(
    endpoint: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    picks: &[(&str, &str)],
    date_cols: &[&str],
    num_cols: &[&str],
) -> Result<Df> {
    let mut map = Map::new();
    if let (Some(s), Some(e)) = (start_date, end_date) {
        map.insert("sdate".into(), Value::String(fmt_cninfo_date(s)));
        map.insert("edate".into(), Value::String(fmt_cninfo_date(e)));
    }
    let records = post_sysapi(endpoint, &map)?;
    let mut df = extract_df(&records, picks)?;
    df.cast_date(date_cols)?;
    df.cast_numeric(num_cols)?;
    Ok(df)
}

/// 巨潮资讯-债券报表-国债发行（对应 akshare [`akshare.bond_treasure_issue_cninfo`]）。
///
/// # 返回列
/// `债券代码, 债券简称, 发行起始日, 发行终止日, 计划发行总量, 实际发行总量,
/// 发行价格, 单位面值, 缴款日, 增发次数, 交易市场, 发行方式, 发行对象,
/// 公告日期, 债券名称`
pub fn bond_treasure_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    const PICKS: [(&str, &str); 15] = [
        ("SECCODE", "债券代码"),
        ("SECNAME", "债券简称"),
        ("F004D", "发行起始日"),
        ("F003D", "发行终止日"),
        ("F006N", "计划发行总量"),
        ("F005N", "实际发行总量"),
        ("F007N", "发行价格"),
        ("F008N", "单位面值"),
        ("F009D", "缴款日"),
        ("F028N", "增发次数"),
        ("F002V", "交易市场"),
        ("F013V", "发行方式"),
        ("F014V", "发行对象"),
        ("DECLAREDATE", "公告日期"),
        ("BONDNAME", "债券名称"),
    ];
    bond_issue_cninfo(
        "p_sysapi1120",
        Some(start_date),
        Some(end_date),
        &PICKS,
        &["发行起始日", "发行终止日", "缴款日", "公告日期"],
        &[
            "计划发行总量",
            "实际发行总量",
            "发行价格",
            "单位面值",
            "增发次数",
        ],
    )
}

/// 巨潮资讯-债券报表-地方债发行（对应 akshare [`akshare.bond_local_government_issue_cninfo`]）。
///
/// 列契约与国债发行一致（`p_sysapi1121`）。
pub fn bond_local_government_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    const PICKS: [(&str, &str); 15] = [
        ("SECCODE", "债券代码"),
        ("SECNAME", "债券简称"),
        ("F004D", "发行起始日"),
        ("F003D", "发行终止日"),
        ("F006N", "计划发行总量"),
        ("F005N", "实际发行总量"),
        ("F007N", "发行价格"),
        ("F008N", "单位面值"),
        ("F009D", "缴款日"),
        ("F028N", "增发次数"),
        ("F002V", "交易市场"),
        ("F013V", "发行方式"),
        ("F014V", "发行对象"),
        ("DECLAREDATE", "公告日期"),
        ("BONDNAME", "债券名称"),
    ];
    bond_issue_cninfo(
        "p_sysapi1121",
        Some(start_date),
        Some(end_date),
        &PICKS,
        &["发行起始日", "发行终止日", "缴款日", "公告日期"],
        &[
            "计划发行总量",
            "实际发行总量",
            "发行价格",
            "单位面值",
            "增发次数",
        ],
    )
}

/// 巨潮资讯-债券报表-企业债发行（对应 akshare [`akshare.bond_corporate_issue_cninfo`]）。
///
/// # 返回列
/// `债券代码, 债券简称, 公告日期, 交易所网上发行起始日, 交易所网上发行终止日,
/// 计划发行总量, 实际发行总量, 发行面值, 发行价格, 发行方式, 发行对象, 发行范围,
/// 承销方式, 最小认购单位, 募资用途说明, 最低认购额, 债券名称`
pub fn bond_corporate_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    const PICKS: [(&str, &str); 17] = [
        ("SECCODE", "债券代码"),
        ("SECNAME", "债券简称"),
        ("DECLAREDATE", "公告日期"),
        ("F003D", "交易所网上发行起始日"),
        ("F004D", "交易所网上发行终止日"),
        ("F005N", "计划发行总量"),
        ("F006N", "实际发行总量"),
        ("F008N", "发行面值"),
        ("F007N", "发行价格"),
        ("F013V", "发行方式"),
        ("F014V", "发行对象"),
        ("F015V", "发行范围"),
        ("F017V", "承销方式"),
        ("F022N", "最小认购单位"),
        ("F023V", "募资用途说明"),
        ("F052N", "最低认购额"),
        ("BONDNAME", "债券名称"),
    ];
    bond_issue_cninfo(
        "p_sysapi1122",
        Some(start_date),
        Some(end_date),
        &PICKS,
        &["公告日期", "交易所网上发行起始日", "交易所网上发行终止日"],
        &[
            "计划发行总量",
            "实际发行总量",
            "发行面值",
            "发行价格",
            "最小认购单位",
            "最低认购额",
        ],
    )
}

/// 巨潮资讯-债券报表-可转债发行（对应 akshare [`akshare.bond_cov_issue_cninfo`]）。
///
/// # 返回列（31 列）
/// `债券代码, 债券简称, 公告日期, 发行起始日, 发行终止日, 计划发行总量,
/// 实际发行总量, 发行面值, 发行价格, 发行方式, 发行对象, 发行范围, 承销方式,
/// 募资用途说明, 初始转股价格, 转股开始日期, 转股终止日期, 网上申购日期,
/// 网上申购代码, 网上申购简称, 网上申购数量上限, 网上申购数量下限, 网上申购单位,
/// 网上申购中签结果公告日及退款日, 优先申购日, 配售价格, 债权登记日,
/// 优先申购缴款日, 转股代码, 交易市场, 债券名称`
pub fn bond_cov_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    const PICKS: [(&str, &str); 31] = [
        ("SECCODE", "债券代码"),
        ("SECNAME", "债券简称"),
        ("DECLAREDATE", "公告日期"),
        ("F029D", "发行起始日"),
        ("F003D", "发行终止日"),
        ("F005N", "计划发行总量"),
        ("F006N", "实际发行总量"),
        ("F007N", "发行面值"),
        ("F052N", "发行价格"),
        ("F013V", "发行方式"),
        ("F014V", "发行对象"),
        ("F015V", "发行范围"),
        ("F017V", "承销方式"),
        ("F021V", "募资用途说明"),
        ("F026N", "初始转股价格"),
        ("F027D", "转股开始日期"),
        ("F053D", "转股终止日期"),
        ("F051D", "网上申购日期"),
        ("F031V", "网上申购代码"),
        ("F032V", "网上申购简称"),
        ("F008N", "网上申购数量上限"),
        ("F066N", "网上申购数量下限"),
        ("F067N", "网上申购单位"),
        ("F068D", "网上申购中签结果公告日及退款日"),
        ("F004D", "优先申购日"),
        ("F065N", "配售价格"),
        ("F028D", "债权登记日"),
        ("F054D", "优先申购缴款日"),
        ("F086V", "转股代码"),
        ("F002V", "交易市场"),
        ("BONDNAME", "债券名称"),
    ];
    bond_issue_cninfo(
        "p_sysapi1123",
        Some(start_date),
        Some(end_date),
        &PICKS,
        &[
            "公告日期",
            "发行起始日",
            "发行终止日",
            "转股开始日期",
            "转股终止日期",
            "网上申购日期",
            "网上申购中签结果公告日及退款日",
            "债权登记日",
            "优先申购日",
            "优先申购缴款日",
        ],
        &[
            "计划发行总量",
            "实际发行总量",
            "发行面值",
            "发行价格",
            "初始转股价格",
            "网上申购数量上限",
            "网上申购数量下限",
            "网上申购单位",
            "配售价格",
        ],
    )
}

/// 巨潮资讯-债券报表-可转债转股（对应 akshare [`akshare.bond_cov_stock_issue_cninfo`]）。
///
/// 无日期范围参数（`p_sysapi1124`）。
///
/// # 返回列
/// `债券代码, 债券简称, 公告日期, 转股代码, 转股简称, 转股价格,
/// 自愿转换期起始日, 自愿转换期终止日, 标的股票, 债券名称`
pub fn bond_cov_stock_issue_cninfo() -> Result<Df> {
    const PICKS: [(&str, &str); 10] = [
        ("SECCODE", "债券代码"),
        ("SECNAME", "债券简称"),
        ("DECLAREDATE", "公告日期"),
        ("F001V", "转股代码"),
        ("F002V", "转股简称"),
        ("F003N", "转股价格"),
        ("F004D", "自愿转换期起始日"),
        ("F005D", "自愿转换期终止日"),
        ("F017V", "标的股票"),
        ("BONDNAME", "债券名称"),
    ];
    bond_issue_cninfo(
        "p_sysapi1124",
        None,
        None,
        &PICKS,
        &["公告日期", "自愿转换期起始日", "自愿转换期终止日"],
        &["转股价格"],
    )
}

// === BATCH36-J 行业分类/股本变动/研报预测（p_public0002 / p_stock2110 / p_sysapi1087 / p_sysapi1089 / p_stock2215）===
//
// 对应 akshare `stock/stock_industry_cninfo.py`、`stock/stock_industry_pe_cninfo.py`、
// `stock/stock_rank_forecast.py`、`stock/stock_share_changes_cninfo.py`。
// 其中 p_sysapi1087/1089 走 sysapi 前缀（复用 [`post_sysapi`]），其余走
// `/api/stock/` 前缀（GET/POST + cninfo headers）。

/// `/api/stock/` 前缀 GET 请求（带 cninfo headers）。
fn stock_api_get(url: &str, params: &Map<String, Value>) -> Result<Vec<Value>> {
    let mcode = cninfo_get_res_code()?;
    let headers = cninfo_headers(&mcode);
    let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let http = HttpClient::default();
    let data = http.get_json_with_headers(url, params, &header_refs, None)?;
    Ok(data
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// `/api/stock/` 前缀 POST 请求（带 cninfo headers）。
fn stock_api_post(url: &str, params: &Map<String, Value>) -> Result<Vec<Value>> {
    let mcode = cninfo_get_res_code()?;
    let headers = cninfo_headers(&mcode);
    let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let http = HttpClient::default();
    let data = http.post_json(url, params, &header_refs)?;
    Ok(data
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// 巨潮资讯-行业分类数据（对应 akshare [`akshare.stock_industry_category_cninfo`]）。
///
/// - `symbol`: `"证监会行业分类标准"` / `"巨潮行业分类标准"` / `"申银万国行业分类标准"` /
///   `"新财富行业分类标准"` / `"国资委行业分类标准"` / `"巨潮产业细分标准"` /
///   `"天相行业分类标准"` / `"全球行业分类标准"`
///
/// # 返回列
/// `父类编码, 类目编码, 类目名称, 类目名称英文, 终止日期, 行业类型编码, 行业类型, 分级`
pub fn stock_industry_category_cninfo(symbol: &str) -> Result<Df> {
    let indtype = match symbol {
        "证监会行业分类标准" => "008001",
        "巨潮行业分类标准" => "008002",
        "申银万国行业分类标准" => "008003",
        "新财富行业分类标准" => "008004",
        "国资委行业分类标准" => "008005",
        "巨潮产业细分标准" => "008006",
        "天相行业分类标准" => "008007",
        "全球行业分类标准" => "008008",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 证监会/巨潮/申银万国/新财富/国资委/巨潮产业细分/天相/全球 行业分类标准"
            )))
        }
    };
    let mut params = Map::new();
    params.insert("indcode".into(), Value::String("".into()));
    params.insert("indtype".into(), Value::String(indtype.into()));
    params.insert("format".into(), Value::String("json".into()));
    let records = stock_api_get(
        "https://webapi.cninfo.com.cn/api/stock/p_public0002",
        &params,
    )?;
    let mut df = multi_record_df(
        &records,
        &[
            "父类编码",
            "类目编码",
            "类目名称",
            "类目名称英文",
            "终止日期",
            "行业类型编码",
            "行业类型",
        ],
    )?;
    // 分级：按 类目编码 长度排序，长度升序 → 级别 0..n
    let mut lens: Vec<usize> = records
        .iter()
        .filter_map(|r| r.get("SORTCODE").and_then(Value::as_str))
        .map(str::len)
        .collect();
    lens.sort_unstable();
    lens.dedup();
    let level_col: Vec<Option<String>> = records
        .iter()
        .map(|r| {
            r.get("SORTCODE").and_then(Value::as_str).and_then(|s| {
                lens.iter()
                    .position(|l| *l == s.len())
                    .map(|i| i.to_string())
            })
        })
        .collect();
    df.with_column("分级", &level_col)?;
    df.cast_date(&["终止日期"])?;
    Ok(df)
}

/// 巨潮资讯-上市公司行业归属变动（对应 akshare [`akshare.stock_industry_change_cninfo`]）。
///
/// - `symbol`: 股票代码，如 `"002594"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `机构名称, 证券代码, 新证券简称, 变更日期, 分类标准编码, 分类标准, 行业编码,
/// 行业门类, 行业次类, 行业大类, 行业中类`
pub fn stock_industry_change_cninfo(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("scode".into(), Value::String(symbol.into()));
    params.insert("sdate".into(), Value::String(cninfo_fmt_date(start_date)));
    params.insert("edate".into(), Value::String(cninfo_fmt_date(end_date)));
    let records = stock_api_post(
        "https://webapi.cninfo.com.cn/api/stock/p_stock2110",
        &params,
    )?;
    let mut df = multi_record_df(
        &records,
        &[
            "机构名称",
            "证券代码",
            "新证券简称",
            "变更日期",
            "分类标准编码",
            "分类标准",
            "行业编码",
            "行业门类",
            "行业次类",
            "行业大类",
            "行业中类",
        ],
    )?;
    df.cast_date(&["变更日期"])?;
    Ok(df)
}

/// 巨潮资讯-行业市盈率（对应 akshare [`akshare.stock_industry_pe_ratio_cninfo`]）。
///
/// - `date`: 变动日期 `YYYYMMDD`
///
/// # 返回列
/// `变动日期, 行业分类, 行业层级, 行业编码, 行业名称, 公司数量, 纳入计算公司数量,
/// 总市值-静态, 净利润-静态, 静态市盈率-加权平均, 静态市盈率-中位数, 静态市盈率-算术平均`
pub fn stock_industry_pe_ratio_cninfo(date: &str) -> Result<Df> {
    let params = json!({ "tdate": cninfo_fmt_date(date) });
    let records = post_sysapi("p_sysapi1087", params.as_object().expect("tdate 参数"))?;
    let df = multi_record_df(
        &records,
        &[
            "行业层级",
            "静态市盈率-算术平均",
            "静态市盈率-中位数",
            "静态市盈率-加权平均",
            "净利润-静态",
            "行业名称",
            "行业编码",
            "行业分类",
            "总市值-静态",
            "纳入计算公司数量",
            "变动日期",
            "公司数量",
        ],
    )?;
    let mut df = reorder_df(
        &df,
        &[
            "变动日期",
            "行业分类",
            "行业层级",
            "行业编码",
            "行业名称",
            "公司数量",
            "纳入计算公司数量",
            "总市值-静态",
            "净利润-静态",
            "静态市盈率-加权平均",
            "静态市盈率-中位数",
            "静态市盈率-算术平均",
        ],
    )?;
    df.cast_date(&["变动日期"])?;
    df.cast_numeric(&[
        "公司数量",
        "纳入计算公司数量",
        "总市值-静态",
        "净利润-静态",
        "静态市盈率-加权平均",
        "静态市盈率-中位数",
        "静态市盈率-算术平均",
    ])?;
    Ok(df)
}

/// 巨潮资讯-个股研报-盈利预测（对应 akshare [`akshare.stock_rank_forecast_cninfo`]）。
///
/// - `date`: 发布日期 `YYYYMMDD`
///
/// # 返回列
/// `证券代码, 证券简称, 发布日期, 研究机构简称, 研究员名称, 投资评级, 是否首次评级,
/// 评级变化, 前一次投资评级, 目标价格-下限, 目标价格-上限`
pub fn stock_rank_forecast_cninfo(date: &str) -> Result<Df> {
    let params = json!({ "tdate": cninfo_fmt_date(date) });
    let records = post_sysapi("p_sysapi1089", params.as_object().expect("tdate 参数"))?;
    let df = multi_record_df(
        &records,
        &[
            "证券简称",
            "发布日期",
            "前一次投资评级",
            "评级变化",
            "目标价格-上限",
            "是否首次评级",
            "投资评级",
            "研究员名称",
            "研究机构简称",
            "目标价格-下限",
            "证券代码",
        ],
    )?;
    let mut df = reorder_df(
        &df,
        &[
            "证券代码",
            "证券简称",
            "发布日期",
            "研究机构简称",
            "研究员名称",
            "投资评级",
            "是否首次评级",
            "评级变化",
            "前一次投资评级",
            "目标价格-下限",
            "目标价格-上限",
        ],
    )?;
    df.cast_date(&["发布日期"])?;
    df.cast_numeric(&["目标价格-下限", "目标价格-上限"])?;
    Ok(df)
}

/// 巨潮资讯-公司股本变动（对应 akshare [`akshare.stock_share_change_cninfo`]）。
///
/// - `symbol`: 股票代码，如 `"002594"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `证券代码, 证券简称, 机构名称, 公告日期, 变动日期, 变动原因编码, 变动原因,
/// 总股本, 未流通股份, 已流通股份, 人民币普通股, 境内上市外资股-B股, 境外上市外资股-H股,
/// 高管股, 其他流通股, 流通受限股份, ...`（原始字段键）
pub fn stock_share_change_cninfo(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("scode".into(), Value::String(symbol.into()));
    params.insert("sdate".into(), Value::String(cninfo_fmt_date(start_date)));
    params.insert("edate".into(), Value::String(cninfo_fmt_date(end_date)));
    let records = stock_api_post(
        "https://webapi.cninfo.com.cn/api/stock/p_stock2215",
        &params,
    )?;
    let mut df = multi_record_df(
        &records,
        &[
            "证券代码",
            "证券简称",
            "机构名称",
            "公告日期",
            "变动日期",
            "变动原因编码",
            "变动原因",
            "总股本",
            "未流通股份",
            "发起人股份",
            "国家持股",
            "国有法人持股",
            "境内法人持股",
            "境外法人持股",
            "自然人持股",
            "募集法人股",
            "内部职工股",
            "转配股",
            "其他流通受限股份",
            "优先股",
            "其他未流通股",
            "已流通股份",
            "人民币普通股",
            "境内上市外资股-B股",
            "境外上市外资股-H股",
            "高管股",
            "其他流通股",
            "流通受限股份",
        ],
    )?;
    df.cast_date(&["公告日期", "变动日期"])?;
    df.cast_numeric(&[
        "总股本",
        "未流通股份",
        "已流通股份",
        "人民币普通股",
        "境内上市外资股-B股",
        "境外上市外资股-H股",
        "高管股",
        "其他流通股",
        "流通受限股份",
    ])?;
    Ok(df)
}
//
// 对应 akshare `stock/stock_hold_num_cninfo.py`、`stock/stock_cg_*.py`、
// `stock/stock_hold_control_cninfo.py`、`stock/stock_allotment_cninfo.py`。
// 全部走 cninfo sysapi POST（复用 [`post_sysapi`]），列名/列序与 akshare 逐字对齐。
// 注意 akshare 对这些接口**不做排序**，直接 `pd.DataFrame(records)` + 位置式列名。

/// 日期归一：`"20180630"` → `"2018-06-30"`（对应 akshare `"-".join([date[:4], date[4:6], date[6:]])`）。
fn cninfo_fmt_date(s: &str) -> String {
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
    } else {
        s.to_string()
    }
}

/// 巨潮资讯-专题统计-股东股本-股东人数及持股集中度（对应 akshare [`akshare.stock_hold_num_cninfo`]）。
///
/// - `date`: 统计日期，如 `"20210630"`。
///
/// # 返回列
/// `证券代码, 证券简称, 变动日期, 本期股东人数, 上期股东人数, 股东人数增幅,
/// 本期人均持股数量, 上期人均持股数量, 人均持股数量增幅`
pub fn stock_hold_num_cninfo(date: &str) -> Result<Df> {
    let params = json!({ "rdate": date });
    let records = post_sysapi("p_sysapi1034", params.as_object().expect("rdate 参数"))?;
    const COLS: [&str; 9] = [
        "本期人均持股数量",
        "股东人数增幅",
        "上期股东人数",
        "本期股东人数",
        "证券简称",
        "证券代码",
        "人均持股数量增幅",
        "变动日期",
        "上期人均持股数量",
    ];
    const SELECT: [&str; 9] = [
        "证券代码",
        "证券简称",
        "变动日期",
        "本期股东人数",
        "上期股东人数",
        "股东人数增幅",
        "本期人均持股数量",
        "上期人均持股数量",
        "人均持股数量增幅",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_date(&["变动日期"])?;
    df.cast_numeric(&[
        "本期股东人数",
        "上期股东人数",
        "股东人数增幅",
        "本期人均持股数量",
        "上期人均持股数量",
        "人均持股数量增幅",
    ])?;
    Ok(df)
}

/// 巨潮资讯-专题统计-公司治理-股权质押（对应 akshare [`akshare.stock_cg_equity_mortgage_cninfo`]）。
///
/// - `date`: 统计日期，如 `"20210930"`。
///
/// # 返回列
/// `股票代码, 股票简称, 公告日期, 出质人, 质权人, 质押数量, 占总股本比例,
/// 质押解除数量, 质押事项, 累计质押占总股本比例`
pub fn stock_cg_equity_mortgage_cninfo(date: &str) -> Result<Df> {
    let params = json!({ "tdate": cninfo_fmt_date(date) });
    let records = post_sysapi("p_sysapi1094", params.as_object().expect("tdate 参数"))?;
    const COLS: [&str; 10] = [
        "质押解除数量",
        "股票简称",
        "公告日期",
        "质押事项",
        "质权人",
        "出质人",
        "股票代码",
        "占总股本比例",
        "累计质押占总股本比例",
        "质押数量",
    ];
    const SELECT: [&str; 10] = [
        "股票代码",
        "股票简称",
        "公告日期",
        "出质人",
        "质权人",
        "质押数量",
        "占总股本比例",
        "质押解除数量",
        "质押事项",
        "累计质押占总股本比例",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_date(&["公告日期"])?;
    df.cast_numeric(&[
        "质押数量",
        "占总股本比例",
        "质押解除数量",
        "累计质押占总股本比例",
    ])?;
    Ok(df)
}

/// 巨潮资讯-专题统计-公司治理-对外担保（对应 akshare [`akshare.stock_cg_guarantee_cninfo`]）。
///
/// - `symbol`: `"全部"` / `"深市主板"` / `"沪市"` / `"创业板"` / `"科创板"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `证券代码, 证券简称, 公告统计区间, 担保笔数, 担保金额, 归属于母公司所有者权益, 担保金融占净资产比例`
pub fn stock_cg_guarantee_cninfo(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let market = match symbol {
        "全部" => "",
        "深市主板" => "012002",
        "沪市" => "012001",
        "创业板" => "012015",
        "科创板" => "012029",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全部/深市主板/沪市/创业板/科创板"
            )))
        }
    };
    let params = json!({
        "sdate": cninfo_fmt_date(start_date),
        "edate": cninfo_fmt_date(end_date),
        "market": market,
    });
    let records = post_sysapi("p_sysapi1054", params.as_object().expect("sdate 参数"))?;
    const COLS: [&str; 7] = [
        "公告统计区间",
        "担保金融占净资产比例",
        "担保金额",
        "担保笔数",
        "证券简称",
        "证券代码",
        "归属于母公司所有者权益",
    ];
    const SELECT: [&str; 7] = [
        "证券代码",
        "证券简称",
        "公告统计区间",
        "担保笔数",
        "担保金额",
        "归属于母公司所有者权益",
        "担保金融占净资产比例",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_numeric(&[
        "担保笔数",
        "担保金额",
        "归属于母公司所有者权益",
        "担保金融占净资产比例",
    ])?;
    Ok(df)
}

/// 巨潮资讯-专题统计-公司治理-公司诉讼（对应 akshare [`akshare.stock_cg_lawsuit_cninfo`]）。
///
/// - `symbol`: `"全部"` / `"深市主板"` / `"沪市"` / `"创业板"` / `"科创板"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// `证券代码, 证券简称, 公告统计区间, 诉讼次数, 诉讼金额`
pub fn stock_cg_lawsuit_cninfo(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let market = match symbol {
        "全部" => "",
        "深市主板" => "012002",
        "沪市" => "012001",
        "创业板" => "012015",
        "科创板" => "012029",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 全部/深市主板/沪市/创业板/科创板"
            )))
        }
    };
    let params = json!({
        "sdate": cninfo_fmt_date(start_date),
        "edate": cninfo_fmt_date(end_date),
        "market": market,
    });
    let records = post_sysapi("p_sysapi1055", params.as_object().expect("sdate 参数"))?;
    const COLS: [&str; 5] = [
        "公告统计区间",
        "诉讼金额",
        "诉讼次数",
        "证券简称",
        "证券代码",
    ];
    const SELECT: [&str; 5] = [
        "证券代码",
        "证券简称",
        "公告统计区间",
        "诉讼次数",
        "诉讼金额",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_numeric(&["诉讼次数", "诉讼金额"])?;
    Ok(df)
}

/// 巨潮资讯-专题统计-股东股本-公司控制权（对应 akshare [`akshare.stock_hold_control_cninfo`]）。
///
/// - `symbol`: `"单独控制"` / `"实际控制人"` / `"一致行动人"` / `"家族控制"` / `"全部"`
///
/// # 返回列
/// `证券代码, 证券简称, 变动日期, 实际控制人名称, 控股数量, 控股比例, 直接控制人名称, 控制类型`
pub fn stock_hold_control_cninfo(symbol: &str) -> Result<Df> {
    let ctype = match symbol {
        "单独控制" => "069001",
        "实际控制人" => "069002",
        "一致行动人" => "069003",
        "家族控制" => "069004",
        "全部" => "",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 单独控制/实际控制人/一致行动人/家族控制/全部"
            )))
        }
    };
    let params = json!({ "ctype": ctype });
    let records = post_sysapi("p_sysapi1033", params.as_object().expect("ctype 参数"))?;
    const COLS: [&str; 8] = [
        "控股比例",
        "控股数量",
        "证券简称",
        "实际控制人名称",
        "直接控制人名称",
        "控制类型",
        "证券代码",
        "变动日期",
    ];
    const SELECT: [&str; 8] = [
        "证券代码",
        "证券简称",
        "变动日期",
        "实际控制人名称",
        "控股数量",
        "控股比例",
        "直接控制人名称",
        "控制类型",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_date(&["变动日期"])?;
    df.cast_numeric(&["控股数量", "控股比例"])?;
    Ok(df)
}

/// 巨潮资讯-专题统计-股东股本-股本变动（对应 akshare [`akshare.stock_hold_change_cninfo`]）。
///
/// - `symbol`: `"深市主板"` / `"沪市"` / `"创业板"` / `"科创板"` / `"北交所"` / `"全部"`
///
/// # 返回列
/// `证券代码, 证券简称, 交易市场, 公告日期, 变动日期, 变动原因, 总股本, 已流通股份, 已流通比例, 流通受限股份`
pub fn stock_hold_change_cninfo(symbol: &str) -> Result<Df> {
    let market = match symbol {
        "深市主板" => "012002",
        "沪市" => "012001",
        "创业板" => "012015",
        "科创板" => "012029",
        "北交所" => "012046",
        "全部" => "",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 深市主板/沪市/创业板/科创板/北交所/全部"
            )))
        }
    };
    let params = json!({ "market": market });
    let records = post_sysapi("p_sysapi1033", params.as_object().expect("market 参数"))?;
    const COLS: [&str; 10] = [
        "已流通股份",
        "总股本",
        "交易市场",
        "证券简称",
        "公告日期",
        "变动原因",
        "证券代码",
        "变动日期",
        "流通受限股份",
        "已流通比例",
    ];
    const SELECT: [&str; 10] = [
        "证券代码",
        "证券简称",
        "交易市场",
        "公告日期",
        "变动日期",
        "变动原因",
        "总股本",
        "已流通股份",
        "已流通比例",
        "流通受限股份",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_date(&["公告日期", "变动日期"])?;
    df.cast_numeric(&["总股本", "已流通股份", "已流通比例", "流通受限股份"])?;
    Ok(df)
}

/// 巨潮资讯-专题统计-股东股本-董监高持股变动明细（对应 akshare [`akshare.stock_hold_management_detail_cninfo`]）。
///
/// - `symbol`: `"增持"` / `"减持"`
///
/// # 返回列
/// `证券代码, 证券简称, 截止日期, 公告日期, 高管姓名, 董监高姓名, 董监高职务,
/// 变动人与董监高关系, 期初持股数量, 期末持股数量, 变动数量, 变动比例, 成交均价,
/// 期末市值, 持股变动原因, 数据来源`
pub fn stock_hold_management_detail_cninfo(symbol: &str) -> Result<Df> {
    let varytype = match symbol {
        "增持" => "B",
        "减持" => "S",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}，可选 增持/减持"
            )))
        }
    };
    // sdate = 去年同日，edate = 今天（对应 akshare datetime.now().date().isoformat()）
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let year = today
        .get(..4)
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(2025);
    let sdate = format!("{}{}", year - 1, &today[4..]);
    let params = json!({
        "sdate": sdate,
        "edate": today,
        "varytype": varytype,
    });
    let records = post_sysapi("p_sysapi1033", params.as_object().expect("varytype 参数"))?;
    const COLS: [&str; 16] = [
        "证券简称",
        "公告日期",
        "高管姓名",
        "期末市值",
        "成交均价",
        "证券代码",
        "变动比例",
        "变动数量",
        "截止日期",
        "期末持股数量",
        "期初持股数量",
        "变动人与董监高关系",
        "董监高职务",
        "董监高姓名",
        "数据来源",
        "持股变动原因",
    ];
    const SELECT: [&str; 16] = [
        "证券代码",
        "证券简称",
        "截止日期",
        "公告日期",
        "高管姓名",
        "董监高姓名",
        "董监高职务",
        "变动人与董监高关系",
        "期初持股数量",
        "期末持股数量",
        "变动数量",
        "变动比例",
        "成交均价",
        "期末市值",
        "持股变动原因",
        "数据来源",
    ];
    let df = multi_record_df(&records, &COLS)?;
    let mut df = reorder_df(&df, &SELECT)?;
    df.cast_date(&["截止日期", "公告日期"])?;
    df.cast_numeric(&[
        "期初持股数量",
        "期末持股数量",
        "变动数量",
        "变动比例",
        "成交均价",
        "期末市值",
    ])?;
    Ok(df)
}

/// 巨潮资讯-个股-配股实施方案（对应 akshare [`akshare.stock_allotment_cninfo`]）。
///
/// - `symbol`: 股票代码，如 `"600030"`
/// - `start_date`/`end_date`: `YYYYMMDD`
///
/// # 返回列
/// 配股实施方案全部字段（约 56 列，按 akshare 位置式列名，日期列归一、数值列数值化）。
pub fn stock_allotment_cninfo(symbol: &str, start_date: &str, end_date: &str) -> Result<Df> {
    let params = json!({
        "scode": symbol,
        "sdate": cninfo_fmt_date(start_date),
        "edate": cninfo_fmt_date(end_date),
    });
    // 端点非 sysapi 前缀（/api/stock/p_stock2232），单独请求
    let mcode = cninfo_get_res_code()?;
    let headers = cninfo_headers(&mcode);
    let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let url = "https://webapi.cninfo.com.cn/api/stock/p_stock2232";
    let http = HttpClient::default();
    let params_map = params.as_object().expect("scode 参数");
    let data = http.post_json(url, params_map, &header_refs)?;
    let records = data
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    const COLS: [&str; 56] = [
        "记录标识",
        "证券简称",
        "停牌起始日",
        "上市公告日期",
        "配股缴款起始日",
        "可转配股数量",
        "停牌截止日",
        "实际配股数量",
        "配股价格",
        "配股比例",
        "配股前总股本",
        "每股配权转让费(元)",
        "法人股实配数量",
        "实际募资净额",
        "大股东认购方式",
        "其他配售简称",
        "发行方式",
        "配股失败，退还申购款日期",
        "除权基准日",
        "预计发行费用",
        "配股发行结果公告日",
        "证券代码",
        "配股权证交易截止日",
        "其他股份实配数量",
        "国家股实配数量",
        "委托单位",
        "公众获转配数量",
        "其他配售代码",
        "配售对象",
        "配股权证交易起始日",
        "资金到账日",
        "机构名称",
        "股权登记日",
        "实际募资总额",
        "预计募集资金",
        "大股东认购数量",
        "公众股实配数量",
        "转配股实配数量",
        "承销费用",
        "法人获转配数量",
        "配股后流通股本",
        "股票类别",
        "公众配售简称",
        "发行方式编码",
        "承销方式",
        "公告日期",
        "配股上市日",
        "配股缴款截止日",
        "承销余额(股)",
        "预计配股数量",
        "配股后总股本",
        "职工股实配数量",
        "承销方式编码",
        "发行费用总额",
        "配股前流通股本",
        "股票类别编码",
    ];
    let mut df = multi_record_df(&records, &COLS)?;
    df.cast_date(&[
        "停牌起始日",
        "上市公告日期",
        "配股缴款起始日",
        "停牌截止日",
        "除权基准日",
        "配股发行结果公告日",
        "配股权证交易截止日",
        "配股权证交易起始日",
        "资金到账日",
        "股权登记日",
        "公告日期",
        "配股上市日",
        "配股缴款截止日",
        "配股失败，退还申购款日期",
    ])?;
    df.cast_numeric(&[
        "可转配股数量",
        "实际配股数量",
        "配股价格",
        "配股比例",
        "配股前总股本",
        "每股配权转让费(元)",
        "法人股实配数量",
        "实际募资净额",
        "预计发行费用",
        "其他股份实配数量",
        "国家股实配数量",
        "公众获转配数量",
        "实际募资总额",
        "预计募集资金",
        "大股东认购数量",
        "公众股实配数量",
        "转配股实配数量",
        "承销费用",
        "法人获转配数量",
        "配股后流通股本",
        "承销余额(股)",
        "预计配股数量",
        "配股后总股本",
        "职工股实配数量",
        "发行费用总额",
        "配股前流通股本",
    ])?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_record_picks_first_n_keys() {
        let records = vec![json!({
            "K1": "v1", "K2": "v2", "K3": "v3", "K4": "v4",
        })];
        let df = single_record_df(&records, &["A", "B"]).unwrap();
        assert_eq!(df.column_names(), vec!["A", "B"]);
        assert_eq!(df.height(), 1);
        assert!(!df.head_preview(1).is_empty());
    }

    #[test]
    fn single_record_empty_gives_empty_df() {
        let df = single_record_df(&[], &["A", "B"]).unwrap();
        assert_eq!(df.height(), 0);
        assert_eq!(df.column_names(), vec!["A", "B"]);
    }

    #[test]
    fn multi_record_key_order_from_first() {
        let records = vec![
            json!({"F006D": "2024-01-01", "F044V": "派息"}),
            json!({"F006D": "2023-01-01", "F044V": "转增"}),
        ];
        let df = multi_record_df(&records, &["日期", "类型"]).unwrap();
        assert_eq!(df.column_names(), vec!["日期", "类型"]);
        assert_eq!(df.height(), 2);
    }

    #[test]
    fn reorder_keeps_given_order() {
        let df = Df::from_string_rows(
            &["b", "a"],
            &[vec![Some("1".into())], vec![Some("2".into())]],
        )
        .unwrap();
        let out = reorder_df(&df, &["a", "b"]).unwrap();
        assert_eq!(out.column_names(), vec!["a", "b"]);
    }
}
