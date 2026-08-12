//! currency 外汇/人民币汇率分类模块（批次 5 长尾 · 新浪 / 外汇局）。
//!
//! 实现覆盖 akshare `currency` 分类下「网络可达」的公开函数：
//! - 新浪财经中行人民币牌价历史（`currency_boc_sina`）
//! - 外汇局人民币汇率中间价（`currency_boc_safe`，近期在线查询部分）
//!
//! 其余 currency 函数为 currencyscoop / investing.com 系（需 API key 或已被封），跳过。
//! 列名与 akshare 逐字一致。

pub use crate::sources::currency_boc::{currency_boc_safe, currency_boc_sina};
