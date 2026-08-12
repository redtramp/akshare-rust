//! fortune 财富榜分类模块（批次 5 长尾 · 胡润研究院）。
//!
//! 实现覆盖 akshare `fortune` 分类下「网络可达」的公开函数：
//! - 胡润排行榜（`hurun` 源，`hurun_rank`）
//!
//! 其余 fortune 函数为财富媒体榜（Bloomberg / Forbes / 新财富 500 / fortune 500），
//! 上游页面结构已变或需反爬/订阅（`index_bloomberg_billionaires` 解析 NoneType、
//! `forbes_rank` 返回 HTML 错误页、`xincaifu_rank` 连接被拒），本批次不可达，跳过。
//! 列名与 akshare 逐字一致。

pub use crate::sources::hurun::hurun_rank;
