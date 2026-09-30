//! energy 能源分类模块（碳排放 / 油价）。
//!
//! 实现覆盖 akshare `energy` 分类的公开函数：
//! - 碳排放交易（广州 / 湖北 / 碳交易网 / 北京 / 深圳国内 / 深圳国际，`carbon` 源）
//! - 中国油价（历史调价 / 各地区油价，`oil` 源）
//!
//! 列名与 akshare 逐字一致（含 `energy_oil_detail` 的位置重命名映射）。

pub use crate::sources::carbon::{
    energy_carbon_bj, energy_carbon_domestic, energy_carbon_eu, energy_carbon_gz, energy_carbon_hb,
    energy_carbon_sz,
};
pub use crate::sources::oil::{energy_oil_detail, energy_oil_hist};
