//! 按数据源划分的抓取实现。
//!
//! 每个数据源一个模块（对应 akshare 按站点组织的模块），
//! 打通一个源后，同源接口按模板批量实现。

pub mod carbon;
pub mod currency_boc;
pub mod eastmoney;
pub mod hurun;
pub mod jin10;
pub mod news_baidu;
pub mod oil;
pub mod news_cctv;
pub mod sge;
pub mod soozhu;
pub mod spot_goods;
pub mod spot_qh;
pub mod ths;
