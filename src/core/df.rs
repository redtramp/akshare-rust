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

    /// 批量重命名列（对应 akshare `df.rename(columns=...)`）。
    ///
    /// `new_names` 长度须与当前列数一致；名称与 pandas 相同，
    /// 重命名不改变列序。
    pub fn rename_columns(&mut self, new_names: &[&str]) -> Result<&mut Self> {
        if new_names.len() != self.inner.width() {
            return Err(AkshareError::Empty(format!(
                "重命名列数不匹配: 当前 {} 列, 给定 {}",
                self.inner.width(),
                new_names.len()
            )));
        }
        let mut names = Vec::with_capacity(new_names.len());
        for n in new_names {
            names.push(PlSmallStr::from_str(n));
        }
        self.inner
            .set_column_names(&names)
            .map_err(|e| AkshareError::Empty(format!("重命名失败: {e}")))?;
        Ok(self)
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
    ///
    /// `try_numeric=true` 时先把列转数值再排序（无法解析的行视为 NaN 排末尾）；
    /// `false` 时按字符串字典序（适合已归一化为 ISO 日期的列，字符串序=时间序）。
    pub fn sort_by(&self, col: &str, ascending: bool, try_numeric: bool) -> Result<Self> {
        // 先把列转数值（若可转），再排序；无法解析的行视为 NaN 排末尾
        let mut df = self.inner.clone();
        if try_numeric {
            if let Ok(casted) = df.column(col).and_then(|c| c.cast(&DataType::Float64)) {
                let _ = df.replace(col, casted);
            }
        }
        let opts = SortMultipleOptions::default()
            .with_order_descending(!ascending)
            .with_nulls_last(true);
        let inner = df
            .sort([col], opts)
            .map_err(|e| AkshareError::Empty(format!("排序失败: {e}")))?;
        Ok(Self { inner })
    }

    /// 指定列转日期字符串（对应 akshare `pd.to_datetime(errors="coerce").dt.date`）。
    ///
    /// 有效日期归一化为 `YYYY-MM-DD`；无法解析的值 → `None`（对应 `NaT`）。
    /// 支持 `YYYY-MM-DD`、`YYYY/MM/DD`、`YYYYMMDD`、带时间部分等常见格式。
    pub fn cast_date(&mut self, cols: &[&str]) -> Result<&mut Self> {
        for c in cols {
            let series = match self.inner.column(c) {
                Ok(s) => s.clone(),
                Err(_) => continue,
            };
            let values: Vec<Option<String>> = series
                .str()
                .ok()
                .map(|s| {
                    (0..series.len())
                        .map(|i| s.get(i).and_then(normalize_date))
                        .collect()
                })
                .unwrap_or_default();
            let chunked = StringChunked::from_iter_options(
                PlSmallStr::from_str(c),
                values.iter().map(|v| v.as_deref()),
            );
            let col: Column = chunked.into_series().into();
            let _ = self.inner.replace(c, col);
        }
        Ok(self)
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

    /// 导出差分对比契约（供 `tools/parity_runner.py` 与 Python akshare 对比）。
    ///
    /// 输出：`{ok, columns:[{name,dtype}], height, head:[[..]]}`，
    /// 其中 `head` 为前 `head_n` 行所有列字符串化后的值（`None` 表示空值），
    /// dtype 映射为 `str/int64/float64/bool/other` 五类，与 pandas dtype 简化一致。
    pub fn export_parity(&self, head_n: usize) -> Value {
        let columns: Vec<Value> = self
            .inner
            .columns()
            .iter()
            .map(|c| {
                let dtype = match c.dtype() {
                    DataType::String => "str",
                    DataType::Int64 => "int64",
                    DataType::Float64 => "float64",
                    DataType::Boolean => "bool",
                    DataType::Datetime(_, _) => "datetime",
                    _ => "other",
                };
                serde_json::json!({ "name": c.name().to_string(), "dtype": dtype })
            })
            .collect();
        let mut head_rows: Vec<Value> = Vec::new();
        for i in 0..self.inner.height().min(head_n) {
            let mut row: Vec<Value> = Vec::with_capacity(self.inner.width());
            for c in self.inner.columns() {
                row.push(cell_to_json(c, i));
            }
            head_rows.push(serde_json::Value::Array(row));
        }
        serde_json::json!({
            "ok": true,
            "columns": columns,
            "height": self.inner.height(),
            "head": head_rows,
        })
    }

    /// 内部 DataFrame 可变引用（高级用法）。
    pub fn inner_mut(&mut self) -> &mut DataFrame {
        &mut self.inner
    }
}

/// 归一化日期字符串为 `YYYY-MM-DD`；无法解析返回 `None`。
fn normalize_date(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // YYYY-MM-DD / YYYY/MM/DD / YYYYMMDD
    if s.len() >= 8
        && s.len() <= 10
        && s.bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'/')
    {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() == 8 {
            let (y, m, d) = (
                digits[0..4].parse::<u32>().ok()?,
                digits[4..6].parse::<u32>().ok()?,
                digits[6..8].parse::<u32>().ok()?,
            );
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return Some(format!("{y:04}-{m:02}-{d:02}"));
            }
            return None;
        }
        return None;
    }
    // 带时间部分：前 10 字节为 YYYY-MM-DD 时按日期解析
    if s.len() > 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' {
        return normalize_date(&s[..10]);
    }
    None
}

/// 将某列第 `i` 行的单元格转为 JSON（字符串化，`None` 表示空值）。
fn cell_to_json(c: &Column, i: usize) -> Value {
    match c.dtype() {
        DataType::String => c
            .str()
            .ok()
            .and_then(|s| s.get(i))
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
        DataType::Int64 => c
            .i64()
            .ok()
            .and_then(|s| s.get(i))
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),
        DataType::Float64 => c
            .f64()
            .ok()
            .and_then(|s| s.get(i))
            .map(|v| Value::String(format_float(v)))
            .unwrap_or(Value::Null),
        DataType::Boolean => c
            .bool()
            .ok()
            .and_then(|s| s.get(i))
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),
        DataType::Datetime(_, _) => c
            .get(i)
            .ok()
            .map(|any| Value::String(any.to_string()))
            .unwrap_or(Value::Null),
        _ => c
            .get(i)
            .ok()
            .map(|any| Value::String(any.to_string()))
            .unwrap_or(Value::Null),
    }
}

/// 浮点字符串化：与 pandas `str()` 对齐（整数时省略小数部分）。
fn format_float(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
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
    fn cast_date_normalizes() {
        let rows = vec![
            json!({"d1": "2024/01/05", "d2": "20240102", "d3": "2024-03-04 15:30:00", "bad": "不是日期"}),
            json!({"d1": null, "d2": "20240102", "d3": "", "bad": "abc"}),
        ];
        let mut df = Df::from_json_rows(&rows).unwrap();
        df.cast_date(&["d1", "d2", "d3", "bad"]).unwrap();
        let d1 = df.inner().column("d1").unwrap().str().unwrap();
        assert_eq!(d1.get(0), Some("2024-01-05"));
        assert_eq!(d1.get(1), None);
        let d2 = df.inner().column("d2").unwrap().str().unwrap();
        assert_eq!(d2.get(0), Some("2024-01-02"));
        let d3 = df.inner().column("d3").unwrap().str().unwrap();
        assert_eq!(d3.get(0), Some("2024-03-04"));
        assert_eq!(d3.get(1), None);
        let bad = df.inner().column("bad").unwrap().str().unwrap();
        assert_eq!(bad.get(0), None);
        assert_eq!(bad.get(1), None);
    }

    #[test]
    fn export_parity_contract() {
        let rows = vec![
            json!({"code": "000001", "name": "平安银行", "price": "10.5", "chg": "1.2"}),
            json!({"code": "600000", "name": "浦发银行", "price": "7.8", "chg": "-"}),
        ];
        let mut df = Df::from_json_rows(&rows).unwrap();
        df.cast_numeric(&["price", "chg"]).unwrap();
        let out = df.export_parity(2);
        let obj = out.as_object().unwrap();
        assert_eq!(obj["ok"], serde_json::json!(true));
        assert_eq!(obj["height"], serde_json::json!(2));
        let cols = obj["columns"].as_array().unwrap();
        let names: Vec<_> = cols.iter().map(|c| c["name"].as_str().unwrap()).collect();
        let dtypes: Vec<_> = cols.iter().map(|c| c["dtype"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["code", "name", "price", "chg"]);
        assert_eq!(dtypes, vec!["str", "str", "float64", "float64"]);
        // 空值(\"-\" 转数值失败)应为 null；浮点整数显示不带小数点
        let head = obj["head"].as_array().unwrap();
        assert_eq!(
            head[0],
            serde_json::json!(["000001", "平安银行", "10.5", "1.2"])
        );
        assert_eq!(
            head[1],
            serde_json::json!(["600000", "浦发银行", "7.8", null])
        );
    }

    #[test]
    fn sort_by_numeric() {
        let rows = vec![
            json!({"code": "a", "f3": "1.0"}),
            json!({"code": "b", "f3": "3.0"}),
            json!({"code": "c", "f3": "2.0"}),
        ];
        let df = Df::from_json_rows(&rows).unwrap();
        let sorted = df.sort_by("f3", false, true).unwrap();
        let codes = sorted.inner().column("code").unwrap().str().unwrap();
        assert_eq!(codes.get(0), Some("b"));
        assert_eq!(codes.get(1), Some("c"));
        assert_eq!(codes.get(2), Some("a"));
    }
}
