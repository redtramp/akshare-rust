//! # akshare-rust
//!
//! Rust 版 akshare：纯 HTTP + 内置 JS 引擎的财经数据获取库。
//!
//! 与 Python akshare 的技术实现方式一致（v1.0 不使用浏览器）：
//! - 纯 HTTP 请求（`reqwest` blocking）＋ UA/Referer 伪装 ＋ 指数退避重试
//! - 内置 JS 引擎（`rquickjs`/QuickJS）执行网站下发的加密脚本，
//!   等价于 akshare 用 `py_mini_racer`（V8）执行同一份 JS
//! - 数据返回为 `Df`（polars DataFrame），列名与 akshare 逐字对齐
//!
//! ## 示例
//! ```no_run
//! use akshare_rust::stock::stock_zh_a_hist;
//!
//! let df = stock_zh_a_hist(
//!     "000001", "daily", "20240101", "20240131", "",
//! ).expect("获取历史行情失败");
//! println!("{}", df);
//! ```

pub mod bond;
pub mod cninfo;
pub mod core;
pub mod currency;
pub mod economic;
pub mod energy;
pub mod exchange;
pub mod fortune;
pub mod futures;
pub mod fund;
pub mod fx;
pub mod index;
pub mod interest_rate;
pub mod legu;
pub mod news;
pub mod option;
pub mod sina;
pub mod sources;
pub mod spot;
pub mod stock;
pub mod stock_feature;
pub mod stock_fund_flow;
pub mod stock_fundamental;
pub mod xueqiu;

/// 库版本（与 Cargo.toml 保持一致）
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
