//! news 新闻分类模块（批次 5 长尾 · 百度股市通 / 央视）。
//!
//! 实现覆盖 akshare `news` 分类下「网络可达」的公开函数：
//! - 百度股市通财经日历（经济数据 / 停复牌 / 分红派息 / 财报披露，`news_baidu` 源）
//! - 新闻联播文字稿（`news_cctv` 源）
//!
//! 列名与 akshare 逐字一致。

pub use crate::sources::news_baidu::{
    news_economic_baidu, news_report_time_baidu, news_trade_notify_dividend_baidu,
    news_trade_notify_suspend_baidu,
};
pub use crate::sources::news_cctv::news_cctv;
