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

    /// 从 JSON 对象数组构建数据表，并按 JSON 值类型推断列 dtype。
    ///
    /// 对应 akshare `pd.DataFrame(records)` 的类型推断：
    /// - 含浮点数的列 → `Float64`
    /// - 仅整数且无空值 → `Int64`
    /// - 仅整数但含空值 → `Float64`（pandas 把含空整数列提升为 float64）
    /// - 含字符串/布尔/其他 → `Utf8`(str)
    ///
    /// 与 [`Df::from_json_rows`]（全部按字符串）不同，本方法保留数值列的数值类型，
    /// 用于需要原样输出接口字段类型（如 `bond_zh_cov_info` 按字段键直接建表）的场景。
    pub fn from_json_rows_typed(rows: &[Value]) -> Result<Self> {
        if rows.is_empty() {
            return Ok(Self {
                inner: DataFrame::empty(),
            });
        }
        let Some(first) = rows.first().and_then(Value::as_object) else {
            return Err(AkshareError::Empty("首行不是 JSON 对象".into()));
        };
        let col_names: Vec<&str> = first.keys().map(String::as_str).collect();

        let mut columns: Vec<Column> = Vec::with_capacity(col_names.len());
        for name in &col_names {
            let mut nulls = 0usize;
            let mut ints = 0usize;
            let mut floats = 0usize;
            let mut others = 0usize;
            let mut int_vals: Vec<Option<i64>> = Vec::with_capacity(rows.len());
            let mut float_vals: Vec<Option<f64>> = Vec::with_capacity(rows.len());
            let mut str_vals: Vec<Option<String>> = Vec::with_capacity(rows.len());
            for r in rows {
                match r.get(*name) {
                    None | Some(Value::Null) => {
                        nulls += 1;
                        int_vals.push(None);
                        float_vals.push(None);
                        str_vals.push(None);
                    }
                    Some(Value::Number(n)) => {
                        if n.is_i64() || n.is_u64() {
                            let v = n.as_i64().or_else(|| n.as_u64().map(|v| v as i64));
                            ints += 1;
                            int_vals.push(v);
                            float_vals.push(v.map(|v| v as f64));
                            str_vals.push(v.map(|v| v.to_string()));
                        } else {
                            let v = n.as_f64();
                            floats += 1;
                            int_vals.push(v.map(|v| v as i64));
                            float_vals.push(v);
                            str_vals.push(v.map(|v| v.to_string()));
                        }
                    }
                    Some(Value::Bool(b)) => {
                        others += 1;
                        int_vals.push(None);
                        float_vals.push(None);
                        str_vals.push(Some(b.to_string()));
                    }
                    Some(Value::String(s)) => {
                        others += 1;
                        int_vals.push(None);
                        float_vals.push(None);
                        str_vals.push(Some(s.clone()));
                    }
                    Some(_) => {
                        others += 1;
                        int_vals.push(None);
                        float_vals.push(None);
                        str_vals.push(Some(String::new()));
                    }
                }
            }
            let series: Column = if floats > 0 {
                Float64Chunked::from_iter_options(
                    PlSmallStr::from_str(name),
                    float_vals.iter().copied(),
                )
                .into_series()
                .into()
            } else if others > 0 {
                StringChunked::from_iter_options(
                    PlSmallStr::from_str(name),
                    str_vals.iter().map(|v| v.as_deref()),
                )
                .into_series()
                .into()
            } else if ints > 0 && nulls > 0 {
                // pandas：含空值的整数列会被提升为 float64
                Float64Chunked::from_iter_options(
                    PlSmallStr::from_str(name),
                    float_vals.iter().copied(),
                )
                .into_series()
                .into()
            } else if ints > 0 {
                Int64Chunked::from_iter_options(
                    PlSmallStr::from_str(name),
                    int_vals.iter().copied(),
                )
                .into_series()
                .into()
            } else {
                // 全空列：pandas 推断为 object(str)，而非 float64
                StringChunked::from_iter_options(
                    PlSmallStr::from_str(name),
                    str_vals.iter().map(|v| v.as_deref()),
                )
                .into_series()
                .into()
            };
            columns.push(series);
        }
        let inner = DataFrame::new(rows.len(), columns)
            .map_err(|e| AkshareError::Empty(format!("构建 DataFrame 失败: {e}")))?;
        Ok(Self { inner })
    }
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

    /// 移除指定列的千分位逗号（对应 akshare `str.replace(",", "")`）。
    pub fn strip_commas(&mut self, cols: &[&str]) -> Result<&mut Self> {
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
                        .map(|i| s.get(i).map(|v| v.replace(',', "")))
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

    /// 移除指定列的尾部子串（对应 akshare `str.strip(suffix)`）。
    ///
    /// 同花顺排名表常见 `1.11%` 需要剥掉 `%` 后再数值化；列不存在时忽略。
    pub fn strip_suffix(&mut self, cols: &[&str], suffix: &str) -> Result<&mut Self> {
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
                        .map(|i| {
                            s.get(i).map(|v| {
                                v.strip_suffix(suffix)
                                    .map(str::to_string)
                                    .unwrap_or_else(|| v.to_string())
                            })
                        })
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

    /// 指定列左补零至固定宽度（对应 akshare `astype(str).str.zfill(n)`）。
    ///
    /// 同花顺排名表的 `股票代码` 以 `zfill(6)` 保证 6 位；数值不受影响。
    pub fn zfill_col(&mut self, col: &str, width: usize) -> Result<&mut Self> {
        let series = match self.inner.column(col) {
            Ok(s) => s.clone(),
            Err(_) => return Ok(self),
        };
        let values: Vec<Option<String>> = series
            .str()
            .ok()
            .map(|s| {
                (0..series.len())
                    .map(|i| s.get(i).map(|v| format!("{v:0>width$}")))
                    .collect()
            })
            .unwrap_or_default();
        let chunked = StringChunked::from_iter_options(
            PlSmallStr::from_str(col),
            values.iter().map(|v| v.as_deref()),
        );
        let new_col: Column = chunked.into_series().into();
        let _ = self.inner.replace(col, new_col);
        Ok(self)
    }

    /// 指定列按小数位四舍五入（对应 akshare `round(col, n)`，作用于已数值化的列）。
    ///
    /// 列先转 f64，逐元素四舍五入后写回；列不存在或非数值时忽略。
    pub fn round_column(&mut self, col: &str, decimals: u32) -> Result<&mut Self> {
        let series = match self.inner.column(col) {
            Ok(s) => s.clone(),
            Err(_) => return Ok(self),
        };
        let f64s = match series.cast(&DataType::Float64) {
            Ok(c) => c,
            Err(_) => return Ok(self),
        };
        let factor = 10f64.powi(decimals as i32);
        let rounded: Column = match f64s.f64() {
            Ok(ca) => ca
                .apply_values(|v| (v * factor).round() / factor)
                .into_series()
                .into(),
            Err(_) => f64s.into_column(),
        };
        let _ = self.inner.replace(col, rounded);
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

    /// 按列自动推断数值类型（对应 pandas `read_html` / `read_csv` 的 `dtype` 推断）。
    ///
    /// 对每列字符串：若其**全部**非空单元格都能解析为浮点数，则整体转 `Float64`；
    /// 否则保持字符串（`object`）。空单元格（null 或纯空白串）不参与判定（对应
    /// pandas `pd.to_numeric(errors="coerce")` 把缺失/非数值统一转 `NaN` 后整列
    /// 推断为浮点）。这与 akshare 上游 `pd.read_html` 的列类型推断语义一致，是
    /// loose 差分对账 dtype 对齐的关键。
    ///
    /// 注意：本工程 `read_html_tables` 默认产出全字符串列，需显式调用本方法才能得到
    /// 与 akshare 相同的数值列 dtype。
    pub fn infer_numeric(&mut self) -> Result<&mut Self> {
        let names: Vec<String> = self
            .inner
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        for name in &names {
            if let Ok(series) = self.inner.column(name) {
                if let Ok(ca) = series.str() {
                    let n = ca.len();
                    let mut all_numeric = true;
                    for i in 0..n {
                        match ca.get(i) {
                            // 纯空白串视为缺失，不参与整列数值判定
                            Some(s) if s.trim().is_empty() => {}
                            Some(s) if s.trim().parse::<f64>().is_ok() => {}
                            Some(_) => {
                                all_numeric = false;
                                break;
                            }
                            None => {}
                        }
                    }
                    if all_numeric {
                        if let Ok(casted) = series.cast(&DataType::Float64) {
                            let _ = self.inner.replace(name, casted);
                        }
                    }
                }
            }
        }
        Ok(self)
    }

    /// 指定列除以缩放因子（对应 akshare `df[col] = df[col] / N`）。
    ///
    /// 列先转 f64，逐元素除以 `factor`；列不存在或非数值时忽略。
    pub fn scale(&mut self, col: &str, factor: f64) -> Result<&mut Self> {
        let series = match self.inner.column(col) {
            Ok(s) => s.clone(),
            Err(_) => return Ok(self),
        };
        let f64s = match series.cast(&DataType::Float64) {
            Ok(c) => c,
            Err(_) => return Ok(self),
        };
        let scaled: Column = match f64s.f64() {
            Ok(ca) => ca.apply_values(|v| v / factor).into_series().into(),
            Err(_) => f64s.into_column(),
        };
        let _ = self.inner.replace(col, scaled);
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

    /// 从已有 polars DataFrame 构建（高级用法，保留列 dtype）。
    ///
    /// 用于需要按源列 dtype 原样复制列的场景（如期货结算统一入口
    /// 把 float64 原始列映射到统一列名）。
    pub fn from_inner(inner: DataFrame) -> Self {
        Self { inner }
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
pub(crate) fn normalize_date(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // YYYY-MM-DD / YYYY/MM/DD / YYYY.MM.DD / YYYYMMDD
    if s.len() >= 8
        && s.len() <= 10
        && s.bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'/' || b == b'.')
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
