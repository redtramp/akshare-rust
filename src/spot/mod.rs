//! spot 现货分类模块（批次 5 长尾 · 搜猪网/上海黄金交易所/99期货/新浪）。
//!
//! 实现覆盖 akshare `spot` 分类下「网络可达」的公开函数：
//! - 搜猪网生猪/饲料大数据（`soozhu` 源）
//! - 上海黄金交易所行情/基准价（`sge` 源）
//! - 99 期货期现（`spot_qh` 源）
//! - 新浪商品现货价格指数（`spot_goods` 源）
//!
//! 列名与 akshare 逐字一致；实时类数据（行情/排名）用 loose 模式对账。

pub use crate::sources::soozhu::{
    spot_corn_price_soozhu, spot_hog_crossbred_soozhu, spot_hog_lean_price_soozhu,
    spot_hog_soozhu, spot_hog_three_way_soozhu, spot_hog_year_trend_soozhu,
    spot_mixed_feed_soozhu, spot_soybean_price_soozhu,
};
pub use crate::sources::sge::{
    spot_golden_benchmark_sge, spot_hist_sge, spot_quotations_sge, spot_silver_benchmark_sge,
    spot_symbol_table_sge,
};
pub use crate::sources::spot_goods::spot_goods;
pub use crate::sources::spot_qh::{spot_price_qh, spot_price_table_qh};
