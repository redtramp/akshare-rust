//! fx 外汇分类模块（批次 5 长尾 · 中国外汇交易中心 chinamoney）。
//!
//! 实现覆盖 akshare `fx` 分类下「网络可达」的公开函数：
//! - 中国外汇交易中心（chinamoney）即期/掉期/货币对行情（`fx_chinamoney` 源）
//!
//! 注意：`fx_quote_baidu` 需要百度 `acs-token`（返回 ResultCode 403），本批次跳过。
//! 列名与 akshare 逐字一致。

pub use crate::sources::fx_chinamoney::{
    fx_c_swap_cm, fx_pair_quote, fx_spot_quote, fx_swap_quote,
};
