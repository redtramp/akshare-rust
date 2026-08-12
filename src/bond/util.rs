//! bond 子模块（批次4）共享工具。
//!
//! 各数据源子模块（g_cm / g_jsl / g_em / g_sina / g_exchange / g_calc）
//! 共用的小工具：JSON 单元格规整、按「源字段名 → 目标列名」构建定列 DataFrame。

use crate::core::df::Df;
use crate::core::error::Result;
use serde_json::Value;

/// 将单个 JSON 单元格规整为 `Option<String>`（对应 akshare 的字符串化读取）。
pub(crate) fn cell_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// 按「源字段名 → 目标列名」映射，从记录数组构建定列 DataFrame。
///
/// 比位置重命名更稳健：只抽取需要的列，避免响应字段增删导致宽度错位；
/// 列顺序严格遵循 `mapping` 给定的目标列顺序（需与 akshare 的 `select` 一致）。
pub(crate) fn df_by_keys(records: &[Value], mapping: &[(&str, &str)]) -> Result<Df> {
    let names: Vec<&str> = mapping.iter().map(|(_, n)| *n).collect();
    let rows: Vec<Vec<Option<String>>> = records
        .iter()
        .map(|r| {
            mapping
                .iter()
                .map(|(k, _)| r.get(*k).and_then(cell_string))
                .collect()
        })
        .collect();
    Df::from_string_rows(&names, &rows)
}
