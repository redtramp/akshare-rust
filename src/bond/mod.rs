//! bond 分类模块（批次4 · 债券全部 44 个公开函数）。
//!
//! 按数据源拆分实现到子模块，集中在此 re-export：
//! - `g_cm`：外汇交易中心 chinamoney（`src/sources/chinamoney.rs`）
//! - `g_jsl`：集思录 jisilu（`src/sources/jisilu.rs`）
//! - `g_em`：东方财富（复用 `src/sources/eastmoney.rs`）
//! - `g_sina`：新浪（复用 `src/sina/mod.rs`）
//! - `g_exchange`：上交所（复用 `src/exchange/mod.rs`）
//! - `g_calc`：纯计算/索引类（chinabond、nafmii、中债指数等）

pub mod g_calc;
pub mod g_cm;
pub mod g_em;
pub mod g_exchange;
pub mod g_jsl;
pub mod g_sina;
pub(crate) mod util;

// 各子模块按实现阶段逐步 re-export；当前阶段1（chinamoney）+ 阶段2（jisilu）就绪，em/sina 等后续阶段追加。
pub use g_cm::*;
pub use g_em::*;
pub use g_jsl::*;
pub use g_sina::*;
