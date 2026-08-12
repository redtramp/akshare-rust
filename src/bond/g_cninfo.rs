//! 巨潮 cninfo 债券发行类函数（批次4 · 阶段3）。
//!
//! 复用 `crate::cninfo::post_sysapi` 完成加密头 POST，响应 `records` 数组按
//! 「源字段名 → 目标列名」定列构建（见 `crate::bond::util::df_by_keys`），
//! 再按 akshare 原顺序 cast_date / cast_numeric。
//!
//! 对应 akshare `bond/bond_corporate_issue_cninfo.py` 等 5 个函数：
//! - `bond_corporate_issue_cninfo`  → p_sysapi1122  企业债发行
//! - `bond_cov_issue_cninfo`        → p_sysapi1123  可转债发行
//! - `bond_cov_stock_issue_cninfo`  → p_sysapi1124  可转债转股（无参数）
//! - `bond_local_government_issue_cninfo` → p_sysapi1121  地方债发行
//! - `bond_treasure_issue_cninfo`   → p_sysapi1120  国债发行

use crate::bond::util::df_by_keys;
use crate::cninfo::post_sysapi;
use crate::core::df::Df;
use crate::core::error::Result;
use serde_json::{json, Map};

/// 将 `YYYYMMDD` 格式化为 cninfo 接口需要的 `YYYY-MM-DD`。
fn fmt_ymd(s: &str) -> String {
    if s.len() == 8 {
        format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
    } else {
        s.to_string()
    }
}

/// 企业债发行（巨潮 p_sysapi1122）。
///
/// # 参数
/// - `start_date` / `end_date`：`YYYYMMDD` 区间
///
/// # 返回列
/// `债券代码, 债券简称, 公告日期, 交易所网上发行起始日, 交易所网上发行终止日,
/// 计划发行总量, 实际发行总量, 发行面值, 发行价格, 发行方式, 发行对象, 发行范围,
/// 承销方式, 最小认购单位, 募资用途说明, 最低认购额, 债券名称`
pub fn bond_corporate_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("sdate".into(), json!(fmt_ymd(start_date)));
    params.insert("edate".into(), json!(fmt_ymd(end_date)));
    let records = post_sysapi("p_sysapi1122", &params)?;
    let mapping: &[(&str, &str)] = &[
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
    let mut df = df_by_keys(&records, mapping)?;
    df.cast_date(&[
        "公告日期",
        "交易所网上发行起始日",
        "交易所网上发行终止日",
    ])?;
    df.cast_numeric(&[
        "计划发行总量",
        "实际发行总量",
        "发行面值",
        "发行价格",
        "最小认购单位",
        "最低认购额",
    ])?;
    Ok(df)
}

/// 可转债发行（巨潮 p_sysapi1123）。
///
/// # 参数
/// - `start_date` / `end_date`：`YYYYMMDD` 区间
///
/// # 返回列
/// `债券代码, 债券简称, 公告日期, 发行起始日, 发行终止日, 计划发行总量, 实际发行总量,
/// 发行面值, 发行价格, 发行方式, 发行对象, 发行范围, 承销方式, 募资用途说明, 初始转股价格,
/// 转股开始日期, 转股终止日期, 网上申购日期, 网上申购代码, 网上申购简称, 网上申购数量上限,
/// 网上申购数量下限, 网上申购单位, 网上申购中签结果公告日及退款日, 优先申购日, 配售价格,
/// 债权登记日, 优先申购缴款日, 转股代码, 交易市场, 债券名称`
pub fn bond_cov_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("sdate".into(), json!(fmt_ymd(start_date)));
    params.insert("edate".into(), json!(fmt_ymd(end_date)));
    let records = post_sysapi("p_sysapi1123", &params)?;
    let mapping: &[(&str, &str)] = &[
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
    let mut df = df_by_keys(&records, mapping)?;
    df.cast_date(&[
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
    ])?;
    df.cast_numeric(&[
        "计划发行总量",
        "实际发行总量",
        "发行面值",
        "发行价格",
        "初始转股价格",
        "网上申购数量上限",
        "网上申购数量下限",
        "网上申购单位",
        "配售价格",
    ])?;
    Ok(df)
}

/// 可转债转股（巨潮 p_sysapi1124，无参数）。
///
/// # 返回列
/// `债券代码, 债券简称, 公告日期, 转股代码, 转股简称, 转股价格,
/// 自愿转换期起始日, 自愿转换期终止日, 标的股票, 债券名称`
pub fn bond_cov_stock_issue_cninfo() -> Result<Df> {
    let records = post_sysapi("p_sysapi1124", &Map::new())?;
    let mapping: &[(&str, &str)] = &[
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
    let mut df = df_by_keys(&records, mapping)?;
    df.cast_date(&[
        "公告日期",
        "自愿转换期起始日",
        "自愿转换期终止日",
    ])?;
    df.cast_numeric(&["转股价格"])?;
    Ok(df)
}

/// 地方债发行（巨潮 p_sysapi1121）。
///
/// # 参数
/// - `start_date` / `end_date`：`YYYYMMDD` 区间
///
/// # 返回列
/// `债券代码, 债券简称, 发行起始日, 发行终止日, 计划发行总量, 实际发行总量,
/// 发行价格, 单位面值, 缴款日, 增发次数, 交易市场, 发行方式, 发行对象,
/// 公告日期, 债券名称`
pub fn bond_local_government_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("sdate".into(), json!(fmt_ymd(start_date)));
    params.insert("edate".into(), json!(fmt_ymd(end_date)));
    let records = post_sysapi("p_sysapi1121", &params)?;
    let mapping: &[(&str, &str)] = &[
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
    let mut df = df_by_keys(&records, mapping)?;
    df.cast_date(&["发行起始日", "发行终止日", "缴款日", "公告日期"])?;
    df.cast_numeric(&[
        "计划发行总量",
        "实际发行总量",
        "发行价格",
        "单位面值",
        "增发次数",
    ])?;
    Ok(df)
}

/// 国债发行（巨潮 p_sysapi1120）。
///
/// # 参数
/// - `start_date` / `end_date`：`YYYYMMDD` 区间
///
/// # 返回列
/// `债券代码, 债券简称, 发行起始日, 发行终止日, 计划发行总量, 实际发行总量,
/// 发行价格, 单位面值, 缴款日, 增发次数, 交易市场, 发行方式, 发行对象,
/// 公告日期, 债券名称`
pub fn bond_treasure_issue_cninfo(start_date: &str, end_date: &str) -> Result<Df> {
    let mut params = Map::new();
    params.insert("sdate".into(), json!(fmt_ymd(start_date)));
    params.insert("edate".into(), json!(fmt_ymd(end_date)));
    let records = post_sysapi("p_sysapi1120", &params)?;
    let mapping: &[(&str, &str)] = &[
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
    let mut df = df_by_keys(&records, mapping)?;
    df.cast_date(&["发行起始日", "发行终止日", "缴款日", "公告日期"])?;
    df.cast_numeric(&[
        "计划发行总量",
        "实际发行总量",
        "发行价格",
        "单位面值",
        "增发次数",
    ])?;
    Ok(df)
}
