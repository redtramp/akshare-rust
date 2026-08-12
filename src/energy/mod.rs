//! energy 能源分类模块（批次 5 长尾 · 碳排放 / 油价）。
//!
//! 实现覆盖 akshare `energy` 分类下「网络可达」的公开函数：
//! - 碳排放交易（广州 / 湖北，`carbon` 源）
//! - 中国油价（历史调价 / 各地区油价，`oil` 源）
//!
//! 列名与 akshare 逐字一致（含 `energy_oil_detail` 的位置重命名映射）。

pub use crate::sources::carbon::{energy_carbon_gz, energy_carbon_hb};
pub use crate::sources::oil::{energy_oil_detail, energy_oil_hist};
