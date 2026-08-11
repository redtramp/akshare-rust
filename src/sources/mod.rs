//! 按数据源划分的抓取实现。
//!
//! 每个数据源一个模块（对应 akshare 按站点组织的模块），
//! 打通一个源后，同源接口按模板批量实现。

pub mod eastmoney;
pub mod jin10;
pub mod ths;
