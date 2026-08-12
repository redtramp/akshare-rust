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
use akshare_rust::economic::{
    macro_china_cpi_monthly, macro_china_cpi_yearly, macro_china_cx_pmi_yearly,
    macro_china_cx_services_pmi_yearly, macro_china_exports_yoy, macro_china_fdi,
    macro_china_fx_reserves_yearly, macro_china_gdp_yearly, macro_china_hk_building_amount,
    macro_china_hk_building_volume, macro_china_hk_cpi, macro_china_hk_cpi_ratio,
    macro_china_hk_gbp, macro_china_hk_gbp_ratio, macro_china_hk_ppi,
    macro_china_hk_rate_of_unemployment, macro_china_hk_trade_diff_ratio,
    macro_china_imports_yoy, macro_china_industrial_production_yoy, macro_china_m2_yearly,
    macro_china_non_man_pmi, macro_china_pmi_yearly, macro_china_ppi_yearly,
    macro_china_qyspjg, macro_china_trade_balance,
};
use akshare_rust::core::df::Df;
use akshare_rust::exchange::{stock_margin_detail_sse, stock_margin_sse, stock_margin_szse};
use akshare_rust::fund::{
    fund_etf_category_ths, fund_etf_spot_em, fund_etf_spot_ths, fund_lof_spot_em,
};
use akshare_rust::futures::{
    futures_contract_detail, futures_settle, futures_settle_cffex, futures_settle_czce,
    futures_settle_gfex, futures_settle_ine, futures_settle_shfe,
};
use akshare_rust::index::{index_zh_a_hist, index_zh_a_hist_min_em};
use akshare_rust::legu::{
    fund_balance_position_lg, fund_linghuo_position_lg, fund_stock_position_lg,
    stock_a_congestion_lg, stock_buffett_index_lg, stock_ebs_lg, stock_index_pb_lg,
    stock_index_pe_lg, stock_market_pb_lg, stock_market_pe_lg,
};
use akshare_rust::sina::{stock_hk_spot, stock_zh_a_minute};
use akshare_rust::stock::{
    fund_etf_hist_em, stock_bid_ask_em, stock_board_concept_cons_em, stock_board_concept_hist_em,
    stock_board_concept_name_em, stock_board_industry_cons_em, stock_board_industry_hist_em,
    stock_board_industry_name_em, stock_hsgt_fund_flow_summary_em, stock_individual_fund_flow,
    stock_individual_info_em, stock_sh_a_spot_em, stock_sz_a_spot_em, stock_zh_a_hist,
    stock_zh_a_hist_min_em, stock_zh_a_spot_em, stock_zt_pool_em,
};
use akshare_rust::stock::{stock_hk_spot_em, stock_zh_a_new_em, stock_zh_a_st_em};
use akshare_rust::stock_feature::{
    stock_account_statistics_em, stock_analyst_detail_em, stock_analyst_rank_em,
    stock_rank_cxfl_ths, stock_rank_cxg_ths, stock_rank_cxd_ths, stock_rank_cxsl_ths,
    stock_rank_ljqd_ths, stock_rank_ljqs_ths, stock_rank_lxxd_ths, stock_rank_lxsz_ths,
    stock_rank_xstp_ths, stock_rank_xxtp_ths, stock_rank_xzjp_ths,
    stock_comment_detail_scrd_desire_em, stock_comment_detail_scrd_focus_em,
    stock_comment_detail_zhpj_lspf_em, stock_comment_detail_zlkp_jgcyd_em, stock_comment_em,
    stock_cy_a_spot_em, stock_dxsyl_em, stock_fhps_detail_em, stock_fhps_em, stock_gddh_em,
    stock_gdfx_free_holding_analyse_em, stock_gdfx_free_holding_change_em,
    stock_gdfx_free_holding_detail_em, stock_gdfx_free_holding_statistics_em,
    stock_gdfx_free_holding_teamwork_em, stock_gdfx_holding_analyse_em,
    stock_gdfx_holding_change_em, stock_gdfx_holding_detail_em, stock_gdfx_holding_statistics_em,
    stock_gdfx_holding_teamwork_em, stock_ggcg_em, stock_gpzy_distribute_statistics_bank_em,
    stock_gpzy_distribute_statistics_company_em, stock_gpzy_individual_pledge_ratio_detail_em,
    stock_gpzy_industry_data_em, stock_gpzy_pledge_ratio_detail_em, stock_gpzy_pledge_ratio_em,
    stock_gpzy_profile_em, stock_hk_ggt_components_em, stock_hk_main_board_spot_em,
    stock_hsgt_board_rank_em, stock_hsgt_hist_em, stock_hsgt_hold_stock_em,
    stock_hsgt_individual_detail_em, stock_hsgt_individual_em,
    stock_hsgt_institution_statistics_em, stock_hsgt_stock_statistics_em, stock_ipo_hk_ths,
    stock_ipo_ths, stock_jgdy_detail_em, stock_jgdy_tj_em, stock_kc_a_spot_em, stock_lhb_detail_em,
    stock_lhb_hyyyb_em, stock_lhb_jgmmtj_em, stock_lhb_jgstatistic_em,
    stock_lhb_stock_detail_date_em, stock_lhb_stock_detail_em, stock_lhb_stock_statistic_em,
    stock_lhb_traderstatistic_em, stock_lhb_yyb_detail_em, stock_lhb_yybph_em, stock_lrb_em,
    stock_margin_account_info, stock_new_a_spot_em, stock_pg_em, stock_qbzf_em, stock_qsjy_em,
    stock_rank_cxd_ths, stock_rank_cxfl_ths, stock_rank_cxg_ths, stock_rank_cxsl_ths,
    stock_rank_ljqd_ths, stock_rank_ljqs_ths, stock_rank_lxsz_ths, stock_rank_lxxd_ths,
    stock_rank_xstp_ths, stock_rank_xxtp_ths, stock_rank_xzjp_ths, stock_sy_hy_em, stock_sy_jz_em,
    stock_sy_profile_em, stock_sy_yq_em, stock_tfp_em, stock_value_em, stock_xgsglb_em,
    stock_xjll_em, stock_yjbb_em, stock_yjkb_em, stock_yjyg_em, stock_yysj_em, stock_zcfz_bj_em,
    stock_zcfz_em, stock_zdhtmx_em, stock_zh_a_gdhs, stock_zh_a_gdhs_detail_em, stock_zh_b_spot_em,
};
use akshare_rust::stock_fundamental::{
    stock_a_gxl_lg, stock_dzjy_hygtj, stock_dzjy_hyyybtj, stock_dzjy_mrmx, stock_dzjy_mrtj,
    stock_dzjy_sctj, stock_dzjy_yybph, stock_financial_abstract_new_ths,
    stock_financial_abstract_ths, stock_financial_benefit_new_ths, stock_financial_benefit_ths,
    stock_financial_cash_new_ths, stock_financial_cash_ths, stock_financial_debt_new_ths,
    stock_financial_debt_ths, stock_individual_basic_info_hk_xq, stock_individual_basic_info_us_xq,
    stock_individual_basic_info_xq, stock_management_change_ths, stock_profit_forecast_ths,
    stock_restricted_release_detail_em, stock_restricted_release_queue_em,
    stock_restricted_release_stockholder_em, stock_restricted_release_summary_em,
    stock_shareholder_change_ths,
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
        "stock_jgdy_tj_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_jgdy_tj_em(d)?)
        }
        "stock_jgdy_detail_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_jgdy_detail_em(d)?)
        }
        "stock_fhps_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_fhps_em(d)?)
        }
        "stock_fhps_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_fhps_detail_em(s)?)
        }
        "stock_tfp_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_tfp_em(d)?)
        }
        "stock_qbzf_em" => Ok(stock_qbzf_em()?),
        "stock_pg_em" => Ok(stock_pg_em()?),
        "stock_account_statistics_em" => Ok(stock_account_statistics_em()?),
        "stock_yjbb_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_yjbb_em(d)?)
        }
        "stock_yjkb_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_yjkb_em(d)?)
        }
        "stock_yjyg_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_yjyg_em(d)?)
        }
        "stock_yysj_em" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_yysj_em(s, d)?)
        }
        "stock_comment_em" => Ok(stock_comment_em()?),
        "stock_lhb_stock_statistic_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_lhb_stock_statistic_em(s)?)
        }
        "stock_lhb_jgmmtj_em" => {
            let [d0, d1] = take2(func, args)?;
            Ok(stock_lhb_jgmmtj_em(d0, d1)?)
        }
        "stock_lhb_jgstatistic_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_lhb_jgstatistic_em(s)?)
        }
        "stock_lhb_hyyyb_em" => {
            let [d0, d1] = take2(func, args)?;
            Ok(stock_lhb_hyyyb_em(d0, d1)?)
        }
        "stock_lhb_yybph_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_lhb_yybph_em(s)?)
        }
        "stock_lhb_traderstatistic_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_lhb_traderstatistic_em(s)?)
        }
        "stock_lhb_stock_detail_date_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_lhb_stock_detail_date_em(s)?)
        }
        "stock_lhb_stock_detail_em" => {
            let [s, d, f] = take3(func, args)?;
            Ok(stock_lhb_stock_detail_em(s, d, f)?)
        }
        "stock_lhb_yyb_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_lhb_yyb_detail_em(s)?)
        }
        "stock_gdfx_free_holding_statistics_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_free_holding_statistics_em(d)?)
        }
        "stock_gdfx_holding_statistics_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_holding_statistics_em(d)?)
        }
        "stock_gdfx_free_holding_change_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_free_holding_change_em(d)?)
        }
        "stock_gdfx_holding_change_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gdfx_holding_change_em(d)?)
        }
        "stock_comment_detail_zlkp_jgcyd_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_comment_detail_zlkp_jgcyd_em(s)?)
        }
        "stock_comment_detail_zhpj_lspf_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_comment_detail_zhpj_lspf_em(s)?)
        }
        "stock_hsgt_stock_statistics_em" => {
            let [s, e] = take2(func, args)?;
            Ok(stock_hsgt_stock_statistics_em(s, e)?)
        }
        "stock_hsgt_hold_stock_em" => {
            let [m, ind, d] = take3(func, args)?;
            Ok(stock_hsgt_hold_stock_em(m, ind, d)?)
        }
        "stock_hsgt_institution_statistics_em" => {
            let [m, s, e] = take3(func, args)?;
            Ok(stock_hsgt_institution_statistics_em(m, s, e)?)
        }
        "stock_hsgt_hist_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hsgt_hist_em(s)?)
        }
        "stock_hsgt_board_rank_em" => {
            let [s, ind, d] = take3(func, args)?;
            Ok(stock_hsgt_board_rank_em(s, ind, d)?)
        }
        "stock_hsgt_individual_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hsgt_individual_em(s)?)
        }
        "stock_hsgt_individual_detail_em" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_hsgt_individual_detail_em(s, d0, d1)?)
        }
        "stock_sy_yq_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_sy_yq_em(d)?)
        }
        "stock_sy_jz_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_sy_jz_em(d)?)
        }
        "stock_zcfz_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zcfz_em(d)?)
        }
        "stock_zcfz_bj_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zcfz_bj_em(d)?)
        }
        "stock_lrb_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_lrb_em(d)?)
        }
        "stock_xjll_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_xjll_em(d)?)
        }
        "stock_gpzy_distribute_statistics_company_em" => {
            Ok(stock_gpzy_distribute_statistics_company_em()?)
        }
        "stock_gpzy_distribute_statistics_bank_em" => {
            Ok(stock_gpzy_distribute_statistics_bank_em()?)
        }
        "stock_zh_a_gdhs_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_a_gdhs_detail_em(s)?)
        }
        "stock_gdfx_free_holding_teamwork_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_gdfx_free_holding_teamwork_em(s)?)
        }
        "stock_gdfx_holding_teamwork_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_gdfx_holding_teamwork_em(s)?)
        }
        "stock_comment_detail_scrd_focus_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_comment_detail_scrd_focus_em(s)?)
        }
        "stock_comment_detail_scrd_desire_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_comment_detail_scrd_desire_em(s)?)
        }
        "stock_sy_hy_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_sy_hy_em(d)?)
        }
        "stock_xgsglb_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_xgsglb_em(s)?)
        }
        "stock_analyst_rank_em" => {
            let [y] = take1(func, args)?;
            Ok(stock_analyst_rank_em(y)?)
        }
        "stock_analyst_detail_em" => {
            let [id, ind] = take2(func, args)?;
            Ok(stock_analyst_detail_em(id, ind)?)
        }
        "stock_restricted_release_summary_em" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_restricted_release_summary_em(s, d0, d1)?)
        }
        "stock_restricted_release_detail_em" => {
            let [d0, d1] = take2(func, args)?;
            Ok(stock_restricted_release_detail_em(d0, d1)?)
        }
        "stock_restricted_release_queue_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_restricted_release_queue_em(s)?)
        }
        "stock_restricted_release_stockholder_em" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_restricted_release_stockholder_em(s, d)?)
        }
        "stock_financial_abstract_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_abstract_ths(s, i)?)
        }
        "stock_financial_debt_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_debt_ths(s, i)?)
        }
        "stock_financial_benefit_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_benefit_ths(s, i)?)
        }
        "stock_financial_cash_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_cash_ths(s, i)?)
        }
        "stock_financial_abstract_new_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_abstract_new_ths(s, i)?)
        }
        "stock_financial_debt_new_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_debt_new_ths(s, i)?)
        }
        "stock_financial_benefit_new_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_benefit_new_ths(s, i)?)
        }
        "stock_financial_cash_new_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_financial_cash_new_ths(s, i)?)
        }
        // 批次3 金十宏观 14 个（无参）
        "macro_china_gdp_yearly" => Ok(macro_china_gdp_yearly()?),
        "macro_china_cpi_yearly" => Ok(macro_china_cpi_yearly()?),
        "macro_china_cpi_monthly" => Ok(macro_china_cpi_monthly()?),
        "macro_china_ppi_yearly" => Ok(macro_china_ppi_yearly()?),
        "macro_china_exports_yoy" => Ok(macro_china_exports_yoy()?),
        "macro_china_imports_yoy" => Ok(macro_china_imports_yoy()?),
        "macro_china_trade_balance" => Ok(macro_china_trade_balance()?),
        "macro_china_industrial_production_yoy" => Ok(macro_china_industrial_production_yoy()?),
        "macro_china_pmi_yearly" => Ok(macro_china_pmi_yearly()?),
        "macro_china_cx_pmi_yearly" => Ok(macro_china_cx_pmi_yearly()?),
        "macro_china_cx_services_pmi_yearly" => Ok(macro_china_cx_services_pmi_yearly()?),
        "macro_china_non_man_pmi" => Ok(macro_china_non_man_pmi()?),
        "macro_china_fx_reserves_yearly" => Ok(macro_china_fx_reserves_yearly()?),
        "macro_china_m2_yearly" => Ok(macro_china_m2_yearly()?),
        "stock_rank_cxg_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_rank_cxg_ths(s)?)
        }
        "stock_rank_cxd_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_rank_cxd_ths(s)?)
        }
        "stock_rank_lxsz_ths" => Ok(stock_rank_lxsz_ths()?),
        "stock_rank_lxxd_ths" => Ok(stock_rank_lxxd_ths()?),
        "stock_rank_cxfl_ths" => Ok(stock_rank_cxfl_ths()?),
        "stock_rank_cxsl_ths" => Ok(stock_rank_cxsl_ths()?),
        "stock_rank_xstp_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_rank_xstp_ths(s)?)
        }
        "stock_rank_xxtp_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_rank_xxtp_ths(s)?)
        }
        "stock_rank_ljqs_ths" => Ok(stock_rank_ljqs_ths()?),
        "stock_rank_ljqd_ths" => Ok(stock_rank_ljqd_ths()?),
        "stock_rank_xzjp_ths" => Ok(stock_rank_xzjp_ths()?),
        "futures_settle_cffex" => {
            let [d] = take1(func, args)?;
            Ok(futures_settle_cffex(d)?)
        }
        "futures_settle_czce" => {
            let [d] = take1(func, args)?;
            Ok(futures_settle_czce(d)?)
        }
        "futures_settle_gfex" => {
            let [d] = take1(func, args)?;
            Ok(futures_settle_gfex(d)?)
        }
        "futures_settle_shfe" => {
            let [d] = take1(func, args)?;
            Ok(futures_settle_shfe(d)?)
        }
        "futures_settle_ine" => {
            let [d] = take1(func, args)?;
            Ok(futures_settle_ine(d)?)
        }
        "futures_settle" => {
            let [d, m] = take2(func, args)?;
            Ok(futures_settle(d, m)?)
        }
        "futures_contract_detail" => {
            let [s] = take1(func, args)?;
            Ok(futures_contract_detail(s)?)
        }
        // 批次3 阶段3d 同花顺板块/新股/公司大事
        "stock_board_industry_name_ths" => Ok(stock_board_industry_name_ths()?),
        "stock_board_industry_info_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_board_industry_info_ths(s)?)
        }
        "stock_board_concept_name_ths" => Ok(stock_board_concept_name_ths()?),
        "stock_board_concept_info_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_board_concept_info_ths(s)?)
        }
        "stock_ipo_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_ipo_ths(s)?)
        }
        "stock_ipo_hk_ths" => Ok(stock_ipo_hk_ths()?),
        "stock_fhps_detail_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_fhps_detail_ths(s)?)
        }
        "stock_profit_forecast_ths" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_profit_forecast_ths(s, i)?)
        }
        "stock_management_change_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_management_change_ths(s)?)
        }
        "stock_shareholder_change_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_shareholder_change_ths(s)?)
        }
        // 批次3 阶段3f 东财 datacenter-web 宏观
        "macro_china_hk_cpi" => Ok(macro_china_hk_cpi()?),
        "macro_china_hk_cpi_ratio" => Ok(macro_china_hk_cpi_ratio()?),
        "macro_china_hk_rate_of_unemployment" => Ok(macro_china_hk_rate_of_unemployment()?),
        "macro_china_hk_gbp" => Ok(macro_china_hk_gbp()?),
        "macro_china_hk_gbp_ratio" => Ok(macro_china_hk_gbp_ratio()?),
        "macro_china_hk_building_volume" => Ok(macro_china_hk_building_volume()?),
        "macro_china_hk_building_amount" => Ok(macro_china_hk_building_amount()?),
        "macro_china_hk_trade_diff_ratio" => Ok(macro_china_hk_trade_diff_ratio()?),
        "macro_china_hk_ppi" => Ok(macro_china_hk_ppi()?),
        "macro_china_qyspjg" => Ok(macro_china_qyspjg()?),
        "macro_china_fdi" => Ok(macro_china_fdi()?),
        // 批次3 阶段3e 乐咕系
        "stock_market_pe_lg" => {
            let [s] = take1(func, args)?;
            Ok(stock_market_pe_lg(s)?)
        }
        "stock_index_pe_lg" => {
            let [s] = take1(func, args)?;
            Ok(stock_index_pe_lg(s)?)
        }
        "stock_market_pb_lg" => {
            let [s] = take1(func, args)?;
            Ok(stock_market_pb_lg(s)?)
        }
        "stock_index_pb_lg" => {
            let [s] = take1(func, args)?;
            Ok(stock_index_pb_lg(s)?)
        }
        "stock_a_congestion_lg" => Ok(stock_a_congestion_lg()?),
        "stock_buffett_index_lg" => Ok(stock_buffett_index_lg()?),
        "stock_ebs_lg" => Ok(stock_ebs_lg()?),
        "fund_stock_position_lg" => Ok(fund_stock_position_lg()?),
        "fund_balance_position_lg" => Ok(fund_balance_position_lg()?),
        "fund_linghuo_position_lg" => Ok(fund_linghuo_position_lg()?),
        // === BATCH2 OPTION (sina/exchange/em) ===
        // === BATCH3 ECONOMIC REMAINING (jin10/em datacenter) ===
        // === BATCH3 STOCK_FUNDAMENTAL REMAINING (ths/sina/em) ===
        "stock_a_gxl_lg" => {
            let [s] = take1(func, args)?;
            Ok(stock_a_gxl_lg(s)?)
        }
        "stock_dzjy_hygtj" => {
            let [s] = take1(func, args)?;
            Ok(stock_dzjy_hygtj(s)?)
        }
        "stock_dzjy_hyyybtj" => {
            let [s] = take1(func, args)?;
            Ok(stock_dzjy_hyyybtj(s)?)
        }
        "stock_dzjy_mrmx" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_dzjy_mrmx(s, d0, d1)?)
        }
        "stock_dzjy_mrtj" => {
            let [d0, d1] = take2(func, args)?;
            Ok(stock_dzjy_mrtj(d0, d1)?)
        }
        "stock_dzjy_sctj" => Ok(stock_dzjy_sctj()?),
        "stock_dzjy_yybph" => {
            let [s] = take1(func, args)?;
            Ok(stock_dzjy_yybph(s)?)
        }
        "stock_individual_basic_info_xq" => {
            let [s] = take1(func, args)?;
            Ok(stock_individual_basic_info_xq(s)?)
        }
        "stock_individual_basic_info_hk_xq" => {
            let [s] = take1(func, args)?;
            Ok(stock_individual_basic_info_hk_xq(s)?)
        }
        "stock_individual_basic_info_us_xq" => {
            let [s] = take1(func, args)?;
            Ok(stock_individual_basic_info_us_xq(s)?)
        }
        // === BATCH4 BOND (chinamoney/jisilu/cninfo) ===
        // === BATCH5 LONGTAIL (spot/energy/currency/news/fx/fortune) ===
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
