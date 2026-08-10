//! 差分对比导出工具（A3：parity 测试框架的 Rust 侧）。
//!
//! 调用库函数并输出规范化契约 JSON（列名/dtype/行数/前 N 行值），
//! 由 `tools/parity_runner.py` 与 Python akshare 的同名输出对比。
//!
//! 用法：
//! ```text
//! parity --func stock_zh_a_hist --args '["000001","daily","20240101","20240131",""]'
//! ```
//!
//! 输出（stdout 单行 JSON）：
//! ```json
//! {"function":"...","ok":true,"error":null,
//!  "columns":[{"name":"日期","dtype":"str"},...],
//!  "height":22,"head":[["2024-01-02","9.11",...],...]}
//! ```

use akshare_rust::cninfo::{
    stock_dividend_cninfo, stock_ipo_summary_cninfo, stock_new_gh_cninfo, stock_new_ipo_cninfo,
    stock_profile_cninfo,
};
use akshare_rust::core::df::Df;
use akshare_rust::exchange::{stock_margin_detail_sse, stock_margin_sse, stock_margin_szse};
use akshare_rust::fund::{
    fund_etf_category_ths, fund_etf_spot_em, fund_etf_spot_ths, fund_lof_spot_em,
};
use akshare_rust::index::{index_zh_a_hist, index_zh_a_hist_min_em};
use akshare_rust::sina::{stock_hk_spot, stock_zh_a_minute};
use akshare_rust::stock::{
    fund_etf_hist_em, stock_bid_ask_em, stock_board_concept_cons_em, stock_board_concept_hist_em,
    stock_board_concept_name_em, stock_board_industry_cons_em, stock_board_industry_hist_em,
    stock_board_industry_name_em, stock_hsgt_fund_flow_summary_em, stock_individual_fund_flow,
    stock_individual_info_em, stock_lhb_detail_em, stock_sh_a_spot_em, stock_sz_a_spot_em,
    stock_zh_a_hist, stock_zh_a_hist_min_em, stock_zh_a_spot_em, stock_zt_pool_em,
};
use akshare_rust::stock::{stock_hk_spot_em, stock_zh_a_new_em, stock_zh_a_st_em};
use akshare_rust::stock_feature::{
    stock_cy_a_spot_em, stock_dxsyl_em, stock_gddh_em, stock_gdfx_free_holding_analyse_em,
    stock_gdfx_free_holding_detail_em, stock_gdfx_holding_analyse_em, stock_gdfx_holding_detail_em,
    stock_ggcg_em, stock_gpzy_individual_pledge_ratio_detail_em, stock_gpzy_industry_data_em,
    stock_gpzy_pledge_ratio_detail_em, stock_gpzy_pledge_ratio_em, stock_gpzy_profile_em,
    stock_hk_ggt_components_em, stock_hk_main_board_spot_em, stock_kc_a_spot_em,
    stock_margin_account_info, stock_new_a_spot_em, stock_qsjy_em, stock_sy_profile_em,
    stock_value_em, stock_zdhtmx_em, stock_zh_a_gdhs, stock_zh_b_spot_em,
};
use akshare_rust::xueqiu::{stock_hot_follow_xq, stock_hot_tweet_xq};
use serde_json::json;

type BoxErr = Box<dyn std::error::Error>;

fn main() {
    let mut func = String::new();
    let mut args: Vec<String> = Vec::new();
    let mut head_n: usize = 5;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--func" => func = it.next().unwrap_or_default(),
            "--args" => {
                if let Some(raw) = it.next() {
                    args = serde_json::from_str(&raw).unwrap_or_default();
                }
            }
            "--head" => {
                if let Some(n) = it.next() {
                    head_n = n.parse().unwrap_or(5);
                }
            }
            other => eprintln!("忽略未知参数: {other}"),
        }
    }

    if func.is_empty() {
        eprintln!("用法: parity --func <name> --args '[...]' [--head N]");
        std::process::exit(2);
    }

    let out = match dispatch(&func, &args) {
        Ok(df) => {
            let mut v = df.export_parity(head_n);
            let obj = v.as_object_mut().expect("契约必须是对象");
            obj.insert("function".into(), json!(func));
            obj.insert("ok".into(), json!(true));
            obj.insert("error".into(), json!(null));
            v
        }
        Err(e) => json!({
            "function": func,
            "ok": false,
            "error": e.to_string(),
        }),
    };
    println!("{out}");
}

/// 函数分派表：函数名 → 参数个数校验 + 调用。
fn dispatch(func: &str, args: &[String]) -> Result<Df, BoxErr> {
    match func {
        "stock_zh_a_hist" => {
            let [s, p, d0, d1, a] = take5(func, args)?;
            Ok(stock_zh_a_hist(s, p, d0, d1, a)?)
        }
        "fund_etf_hist_em" => {
            let [s, p, d0, d1, a] = take5(func, args)?;
            Ok(fund_etf_hist_em(s, p, d0, d1, a)?)
        }
        "stock_zh_a_hist_min_em" => {
            let [s, d0, d1, p, a] = take5(func, args)?;
            Ok(stock_zh_a_hist_min_em(s, d0, d1, p, a)?)
        }
        "stock_individual_info_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_individual_info_em(s)?)
        }
        "stock_bid_ask_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_bid_ask_em(s)?)
        }
        "stock_board_industry_name_em" => Ok(stock_board_industry_name_em()?),
        "stock_board_concept_name_em" => Ok(stock_board_concept_name_em()?),
        "stock_board_industry_cons_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_board_industry_cons_em(s)?)
        }
        "stock_board_concept_cons_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_board_concept_cons_em(s)?)
        }
        "stock_board_industry_hist_em" => {
            let [s, d0, d1, p, a] = take5(func, args)?;
            Ok(stock_board_industry_hist_em(s, d0, d1, p, a)?)
        }
        "stock_board_concept_hist_em" => {
            let [s, p, d0, d1, a] = take5(func, args)?;
            Ok(stock_board_concept_hist_em(s, p, d0, d1, a)?)
        }
        "stock_zt_pool_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_em(d)?)
        }
        "stock_individual_fund_flow" => {
            let [s, m] = take2(func, args)?;
            Ok(stock_individual_fund_flow(s, m)?)
        }
        "stock_lhb_detail_em" => {
            let [d0, d1] = take2(func, args)?;
            Ok(stock_lhb_detail_em(d0, d1)?)
        }
        "stock_hsgt_fund_flow_summary_em" => Ok(stock_hsgt_fund_flow_summary_em()?),
        "stock_zh_a_spot_em" => Ok(stock_zh_a_spot_em()?),
        "stock_sh_a_spot_em" => Ok(stock_sh_a_spot_em()?),
        "stock_sz_a_spot_em" => Ok(stock_sz_a_spot_em()?),
        "index_zh_a_hist" => {
            let [s, p, d0, d1] = take4(func, args)?;
            Ok(index_zh_a_hist(s, p, d0, d1)?)
        }
        "index_zh_a_hist_min_em" => {
            let [s, p, d0, d1] = take4(func, args)?;
            Ok(index_zh_a_hist_min_em(s, p, d0, d1)?)
        }
        "fund_etf_spot_em" => Ok(fund_etf_spot_em()?),
        "fund_lof_spot_em" => Ok(fund_lof_spot_em()?),
        "stock_profile_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_profile_cninfo(s)?)
        }
        "stock_ipo_summary_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_ipo_summary_cninfo(s)?)
        }
        "stock_dividend_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_dividend_cninfo(s)?)
        }
        "stock_new_ipo_cninfo" => Ok(stock_new_ipo_cninfo()?),
        "stock_new_gh_cninfo" => Ok(stock_new_gh_cninfo()?),
        "fund_etf_category_ths" => {
            let [s, d] = take2(func, args)?;
            Ok(fund_etf_category_ths(s, d)?)
        }
        "fund_etf_spot_ths" => {
            let [d] = take1(func, args)?;
            Ok(fund_etf_spot_ths(d)?)
        }
        "stock_hk_spot" => Ok(stock_hk_spot()?),
        "stock_zh_a_minute" => {
            let [s, p, a] = take3(func, args)?;
            Ok(stock_zh_a_minute(s, p, a)?)
        }
        "stock_margin_sse" => {
            let [d0, d1] = take2(func, args)?;
            Ok(stock_margin_sse(d0, d1)?)
        }
        "stock_margin_detail_sse" => {
            let [d] = take1(func, args)?;
            Ok(stock_margin_detail_sse(d)?)
        }
        "stock_margin_szse" => {
            let [d] = take1(func, args)?;
            Ok(stock_margin_szse(d)?)
        }
        "stock_hot_follow_xq" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_follow_xq(s)?)
        }
        "stock_zh_a_st_em" => Ok(stock_zh_a_st_em()?),
        "stock_zh_a_new_em" => Ok(stock_zh_a_new_em()?),
        "stock_hk_spot_em" => Ok(stock_hk_spot_em()?),
        "stock_hot_tweet_xq" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_tweet_xq(s)?)
        }
        "stock_cy_a_spot_em" => Ok(stock_cy_a_spot_em()?),
        "stock_kc_a_spot_em" => Ok(stock_kc_a_spot_em()?),
        "stock_zh_b_spot_em" => Ok(stock_zh_b_spot_em()?),
        "stock_new_a_spot_em" => Ok(stock_new_a_spot_em()?),
        "stock_hk_main_board_spot_em" => Ok(stock_hk_main_board_spot_em()?),
        "stock_hk_ggt_components_em" => Ok(stock_hk_ggt_components_em()?),
        "stock_zh_a_gdhs" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_a_gdhs(s)?)
        }
        "stock_margin_account_info" => Ok(stock_margin_account_info()?),
        "stock_gdfx_free_holding_detail_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_free_holding_detail_em(d)?)
        }
        "stock_gdfx_holding_detail_em" => {
            let [d, ind, sym] = take3(func, args)?;
            Ok(stock_gdfx_holding_detail_em(d, ind, sym)?)
        }
        "stock_gdfx_free_holding_analyse_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_free_holding_analyse_em(d)?)
        }
        "stock_gdfx_holding_analyse_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_holding_analyse_em(d)?)
        }
        "stock_qsjy_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_qsjy_em(d)?)
        }
        "stock_gpzy_profile_em" => Ok(stock_gpzy_profile_em()?),
        "stock_gpzy_pledge_ratio_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gpzy_pledge_ratio_em(d)?)
        }
        "stock_gpzy_industry_data_em" => Ok(stock_gpzy_industry_data_em()?),
        "stock_gpzy_pledge_ratio_detail_em" => Ok(stock_gpzy_pledge_ratio_detail_em()?),
        "stock_gpzy_individual_pledge_ratio_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_gpzy_individual_pledge_ratio_detail_em(s)?)
        }
        "stock_ggcg_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_ggcg_em(s)?)
        }
        "stock_value_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_value_em(s)?)
        }
        "stock_gddh_em" => Ok(stock_gddh_em()?),
        "stock_zdhtmx_em" => {
            let [s, e] = take2(func, args)?;
            Ok(stock_zdhtmx_em(s, e)?)
        }
        "stock_dxsyl_em" => Ok(stock_dxsyl_em()?),
        "stock_sy_profile_em" => Ok(stock_sy_profile_em()?),
        _ => Err(format!("未知函数: {func}").into()),
    }
}

fn take1<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 1], BoxErr> {
    if args.len() != 1 {
        return Err(format!("{func} 需要 1 个参数, 实际 {}", args.len()).into());
    }
    Ok([args[0].as_str()])
}

fn take2<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 2], BoxErr> {
    if args.len() != 2 {
        return Err(format!("{func} 需要 2 个参数, 实际 {}", args.len()).into());
    }
    Ok([args[0].as_str(), args[1].as_str()])
}

fn take3<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 3], BoxErr> {
    if args.len() != 3 {
        return Err(format!("{func} 需要 3 个参数, 实际 {}", args.len()).into());
    }
    Ok([args[0].as_str(), args[1].as_str(), args[2].as_str()])
}

fn take4<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 4], BoxErr> {
    if args.len() != 4 {
        return Err(format!("{func} 需要 4 个参数, 实际 {}", args.len()).into());
    }
    Ok([
        args[0].as_str(),
        args[1].as_str(),
        args[2].as_str(),
        args[3].as_str(),
    ])
}

fn take5<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 5], BoxErr> {
    if args.len() != 5 {
        return Err(format!("{func} 需要 5 个参数, 实际 {}", args.len()).into());
    }
    Ok([
        args[0].as_str(),
        args[1].as_str(),
        args[2].as_str(),
        args[3].as_str(),
        args[4].as_str(),
    ])
}
