//! 命令行演示：验证已实现函数（对应 akshare 同名接口）。
//!
//! 每节独立容错：单节失败打印错误后继续，便于在部分节点限流时观察其余接口。

use akshare_rust::fund::{fund_etf_spot_em, fund_lof_spot_em};
use akshare_rust::index::{index_zh_a_hist, index_zh_a_hist_min_em};
use akshare_rust::stock::{
    stock_bid_ask_em, stock_board_concept_hist_em, stock_board_concept_name_em,
    stock_board_industry_cons_em, stock_board_industry_hist_em, stock_board_industry_name_em,
    stock_gpzy_profile_em, stock_hsgt_fund_flow_summary_em, stock_individual_fund_flow,
    stock_individual_info_em, stock_lhb_detail_em, stock_zh_a_hist, stock_zh_a_hist_min_em,
    stock_zh_a_spot_em, stock_zt_pool_em,
};

type BoxErr = Box<dyn std::error::Error>;

fn section(name: &str, f: impl FnOnce() -> Result<(), BoxErr>) {
    println!("\n=== {name} ===");
    match f() {
        Ok(()) => {}
        Err(e) => println!("Error: {e}"),
    }
}

fn main() {
    section("stock_zh_a_hist(000001, daily, 20240101~20240131)", || {
        let df = stock_zh_a_hist("000001", "daily", "20240101", "20240131", "")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_zh_a_hist_min_em(000001, period=5)", || {
        let df = stock_zh_a_hist_min_em(
            "000001",
            "2026-01-01 09:00:00",
            "2026-12-31 15:00:00",
            "5",
            "",
        )?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_individual_info_em(000001)", || {
        let df = stock_individual_info_em("000001")?;
        println!("{}", df);
        Ok(())
    });

    section("stock_bid_ask_em(000001)", || {
        let df = stock_bid_ask_em("000001")?;
        println!("{}", df);
        Ok(())
    });

    section("stock_zh_a_spot_em()", || {
        let df = stock_zh_a_spot_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("index_zh_a_hist(000001, daily)", || {
        let df = index_zh_a_hist("000001", "daily", "20240101", "20240131")?;
        println!("共 {} 行", df.height());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("index_zh_a_hist_min_em(399006, period=5)", || {
        let df =
            index_zh_a_hist_min_em("399006", "5", "2026-01-01 09:00:00", "2026-12-31 15:00:00")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("fund_etf_spot_em()", || {
        let df = fund_etf_spot_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("fund_lof_spot_em()", || {
        let df = fund_lof_spot_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_board_industry_name_em()", || {
        let df = stock_board_industry_name_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_board_concept_name_em()", || {
        let df = stock_board_concept_name_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_board_industry_cons_em(小金属)", || {
        let df = stock_board_industry_cons_em("小金属")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_board_industry_hist_em(BK1027, 日k)", || {
        let df = stock_board_industry_hist_em("BK1027", "20240101", "20240131", "日k", "")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_board_concept_hist_em(BK0715, daily)", || {
        let df = stock_board_concept_hist_em("BK0715", "20240101", "20240131", "daily", "")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_zt_pool_em(20260807)", || {
        let df = stock_zt_pool_em("20260807")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_individual_fund_flow(600094, sh)", || {
        let df = stock_individual_fund_flow("600094", "sh")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_lhb_detail_em(20260801~20260807)", || {
        let df = stock_lhb_detail_em("20260801", "20260807")?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_hsgt_fund_flow_summary_em()", || {
        let df = stock_hsgt_fund_flow_summary_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });

    section("stock_gpzy_profile_em()", || {
        let df = stock_gpzy_profile_em()?;
        println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
        println!("{}", df.head_preview(3));
        Ok(())
    });
}
