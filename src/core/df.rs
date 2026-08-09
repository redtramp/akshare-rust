//! 数据表 `Df`：polars DataFrame 的轻量封装。
//!
//! 对应 akshare 的 pandas.DataFrame 语义，提供：
//! - 从 JSON 对象数组（`diff` 行）构建
//! - 列选择 / 重命名 / 追加
//! - 按列排序（对应 `sort_values`）
//! - 字符串列转数值（对应 `pd.to_numeric(errors="coerce")`）
//! - 调试打印（对应 `df.head()`）

use crate::core::error::{AkshareError, Result};
use polars::prelude::*;
use serde_json::Value;

/// 数据表：polars DataFrame 封装。
#[derive(Debug, Clone)]
pub struct Df {
    inner: DataFrame,
}

impl std::fmt::Display for Df {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl Df {
    /// 从 JSON 对象数组构建数据表。
    ///
    /// 列顺序 = 首行对象的键序（保持响应字段顺序，与 Python dict 一致）。
    /// 若 `rows` 为空，则返回零行零列的空表（对应 akshare 空 DataFrame）。
    pub fn from_json_rows(rows: &[Value]) -> Result<Self> {
        if rows.is_empty() {
            return Ok(Self {
                inner: DataFrame::empty(),
            });
        }
        // 列名集合 = 首行键序
        let Some(first) = rows.first().and_then(Value::as_object) else {
            return Err(AkshareError::Empty("首行不是 JSON 对象".into()));
        };
        let col_names: Vec<&str> = first.keys().map(String::as_str).collect();

        // 逐列构建 Series
        let mut columns: Vec<Column> = Vec::with_capacity(col_names.len());
        for name in &col_names {
            let values: Vec<Option<String>> = rows
                .iter()
                .map(|r| {
                    r.get(*name).and_then(|v| match v {
                        Value::Null => None,
                        Value::String(s) => Some(s.clone()),
                        other => Some(other.to_string()),
                    })
                })
                .collect();
            let chunked = StringChunked::from_iter_options(
                PlSmallStr::from_str(name),
                values.iter().map(|v| v.as_deref()),
            );
            columns.push(chunked.into_series().into());
        }

        let inner = DataFrame::new(rows.len(), columns)
            .map_err(|e| AkshareError::Empty(format!("构建 DataFrame 失败: {e}")))?;
        Ok(Self { inner })
    }

    /// 从字符串二维数组构建数据表（列名与每行值一一对应）。
    pub fn from_string_rows(col_names: &[&str], rows: &[Vec<Option<String>>]) -> Result<Self> {
        let mut columns: Vec<Column> = Vec::with_capacity(col_names.len());
        for (i, name) in col_names.iter().enumerate() {
            let values: Vec<Option<&str>> = rows
                .iter()
                .map(|r| r.get(i).and_then(|v| v.as_deref()))
                .collect();
            let chunked = StringChunked::from_iter_options(
                PlSmallStr::from_str(name),
                values.iter().copied(),
            );
            columns.push(chunked.into_series().into());
        }
        let inner = DataFrame::new(rows.len(), columns)
            .map_err(|e| AkshareError::Empty(format!("构建 DataFrame 失败: {e}")))?;
        Ok(Self { inner })
    }

    /// 行数。
    pub fn height(&self) -> usize {
        self.inner.height()
    }

    /// 列名。
    pub fn column_names(&self) -> Vec<String> {
        self.inner
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// 选择列子集（保持给定顺序，对应 akshare `df[cols]`）。
    pub fn select(&self, cols: &[&str]) -> Result<Self> {
        let inner = self
            .inner
            .select(cols)
            .map_err(|e| AkshareError::Empty(format!("选择列失败: {e}")))?;
        Ok(Self { inner })
    }

    /// 追加一列（对应 akshare `df[\"col\"] = value`）。
    pub fn with_column(&mut self, name: &str, values: &[Option<String>]) -> Result<&mut Self> {
        let chunked = StringChunked::from_iter_options(
            PlSmallStr::from_str(name),
            values.iter().map(|v| v.as_deref()),
        );
        let col: Column = chunked.into_series().into();
        self.inner
            .with_column(col)
            .map_err(|e| AkshareError::Empty(format!("追加列失败: {e}")))?;
        Ok(self)
    }

    /// 按列排序（对应 akshare `sort_values(by=col, ascending=...)`）。
    pub fn sort_by(&self, col: &str, ascending: bool) -> Result<Self> {
        // 先把列转数值（若可转），再排序；无法解析的行视为 NaN 排末尾
        let mut df = self.inner.clone();
        if let Ok(casted) = df.column(col).and_then(|c| c.cast(&DataType::Float64)) {
            let _ = df.replace(col, casted);
        }
        let opts = SortMultipleOptions::default()
            .with_order_descending(!ascending)
            .with_nulls_last(true);
        let inner = df
            .sort([col], opts)
            .map_err(|e| AkshareError::Empty(format!("排序失败: {e}")))?;
        Ok(Self { inner })
    }

    /// 指定列转 f64（对应 akshare `pd.to_numeric(errors="coerce")`）。
    pub fn cast_numeric(&mut self, cols: &[&str]) -> Result<&mut Self> {
        for c in cols {
            if let Ok(series) = self.inner.column(c) {
                if let Ok(casted) = series.cast(&DataType::Float64) {
                    let _ = self.inner.replace(c, casted);
                }
            }
        }
        Ok(self)
    }

    /// 前 n 行文本预览（对应 akshare `df.head(n)` 打印）。
    pub fn head_preview(&self, n: usize) -> String {
        self.inner.head(Some(n)).to_string()
    }

    /// 内部 DataFrame 引用（高级用法）。
    pub fn inner(&self) -> &DataFrame {
        &self.inner
    }

    /// 内部 DataFrame 可变引用（高级用法）。
    pub fn inner_mut(&mut self) -> &mut DataFrame {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_json_rows_preserves_key_order() {
        let rows = vec![
            json!({"f12": "000001", "f14": "平安银行", "f2": "10.5"}),
            json!({"f12": "600000", "f14": "浦发银行", "f2": "7.8"}),
        ];
        let df = Df::from_json_rows(&rows).unwrap();
        assert_eq!(df.height(), 2);
        assert_eq!(df.column_names(), vec!["f12", "f14", "f2"]);
    }

    #[test]
    fn empty_rows_gives_empty_df() {
        let df = Df::from_json_rows(&[]).unwrap();
        assert_eq!(df.height(), 0);
    }

    #[test]
    fn select_and_cast() {
        let rows = vec![
            json!({"f12": "000001", "f2": "10.5", "f3": "-"}),
            json!({"f12": "600000", "f2": "7.8", "f3": "1.2"}),
        ];
        let mut df = Df::from_json_rows(&rows).unwrap();
        df.cast_numeric(&["f2", "f3"]).unwrap();
        let sel = df.select(&["f12", "f2"]).unwrap();
        assert_eq!(sel.column_names(), vec!["f12", "f2"]);
        // f3 的 "-" 转数值后应为空值
        let f3 = df.inner().column("f3").unwrap().f64().unwrap();
        assert!(f3.get(0).is_none());
        assert_eq!(f3.get(1), Some(1.2));
    }

    #[test]
    fn sort_by_numeric() {
        let rows = vec![
            json!({"code": "a", "f3": "1.0"}),
            json!({"code": "b", "f3": "3.0"}),
            json!({"code": "c", "f3": "2.0"}),
        ];
        let df = Df::from_json_rows(&rows).unwrap();
        let sorted = df.sort_by("f3", false).unwrap();
        let codes = sorted.inner().column("code").unwrap().str().unwrap();
        assert_eq!(codes.get(0), Some("b"));
        assert_eq!(codes.get(1), Some("c"));
        assert_eq!(codes.get(2), Some("a"));
    }
}
