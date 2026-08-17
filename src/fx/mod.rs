//! fx 分类模块：外汇市场行情（中国外汇交易中心 chinamoney / 百度股市通）。

pub mod fx_chinamoney;
pub mod fx_quote_baidu;

pub use fx_chinamoney::{fx_c_swap_cm, fx_pair_quote, fx_spot_quote, fx_swap_quote};
pub use fx_quote_baidu::fx_quote_baidu;
