//! 命令行演示：验证首批实现的函数（对应 akshare 同名接口）。

use akshare_rust::index::index_zh_a_hist;
use akshare_rust::stock::{stock_zh_a_hist, stock_zh_a_spot_em};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) A 股历史行情
    println!("=== stock_zh_a_hist(000001, daily, 20240101~20240131) ===");
    let df = stock_zh_a_hist("000001", "daily", "20240101", "20240131", "")?;
    println!("{}", df.head_preview(3));

    // 2) A 股实时行情（分页全量，可能较慢）
    println!("\n=== stock_zh_a_spot_em() ===");
    let df = stock_zh_a_spot_em()?;
    println!("共 {} 行, 列: {:?}", df.height(), df.column_names());
    println!("{}", df.head_preview(3));

    // 3) 指数历史行情
    println!("\n=== index_zh_a_hist(000001, daily) ===");
    let df = index_zh_a_hist("000001", "daily", "20240101", "20240131")?;
    println!("共 {} 行", df.height());
    println!("{}", df.head_preview(3));

    Ok(())
}
