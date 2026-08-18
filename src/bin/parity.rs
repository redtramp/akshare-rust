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

use akshare_rust::bond::{
    bond_available_index_cbond, bond_buy_back_hist_em, bond_cb_adj_logs_jsl, bond_cb_index_jsl,
    bond_cb_jsl, bond_cb_profile_sina, bond_cb_redeem_jsl, bond_cb_summary_sina,
    bond_china_close_return, bond_china_close_return_map, bond_china_yield,
    bond_composite_index_cbond, bond_cov_comparison, bond_gb_us_sina, bond_gb_zh_sina,
    bond_index_general_cbond, bond_info_cm, bond_info_cm_query, bond_info_detail_cm,
    bond_new_composite_index_cbond, bond_sh_buy_back_em, bond_spot_deal, bond_spot_quote,
    bond_sz_buy_back_em, bond_treasury_index_cbond, bond_zh_cov, bond_zh_cov_info,
    bond_zh_cov_info_ths, bond_zh_cov_value_analysis, bond_zh_hs_cov_daily, bond_zh_hs_cov_min,
    bond_zh_hs_cov_pre_min, bond_zh_hs_cov_spot, bond_zh_hs_daily, bond_zh_hs_spot,
    bond_zh_us_rate,
};
use akshare_rust::cninfo::{
    bond_corporate_issue_cninfo, bond_cov_issue_cninfo, bond_cov_stock_issue_cninfo,
    bond_local_government_issue_cninfo, bond_treasure_issue_cninfo, stock_allotment_cninfo,
    stock_cg_equity_mortgage_cninfo, stock_cg_guarantee_cninfo, stock_cg_lawsuit_cninfo,
    stock_dividend_cninfo, stock_hold_change_cninfo, stock_hold_control_cninfo,
    stock_hold_management_detail_cninfo, stock_hold_num_cninfo, stock_industry_category_cninfo,
    stock_industry_change_cninfo, stock_industry_pe_ratio_cninfo, stock_ipo_summary_cninfo,
    stock_new_gh_cninfo, stock_new_ipo_cninfo, stock_profile_cninfo, stock_rank_forecast_cninfo,
    stock_share_change_cninfo,
};
use akshare_rust::core::df::Df;
use akshare_rust::currency::{currency_boc_safe, currency_boc_sina};
use akshare_rust::economic::{
    // === BATCH6 海外宏观（RPT_ECONOMICVALUE_* 系列） ===
    macro_australia_bank_rate,
    macro_australia_cpi_quarterly,
    macro_australia_cpi_yearly,
    macro_australia_ppi_quarterly,
    macro_australia_retail_rate_monthly,
    macro_australia_trade,
    macro_australia_unemployment_rate,
    macro_canada_bank_rate,
    macro_canada_core_cpi_monthly,
    macro_canada_core_cpi_yearly,
    macro_canada_cpi_monthly,
    macro_canada_cpi_yearly,
    macro_canada_gdp_monthly,
    macro_canada_new_house_rate,
    macro_canada_retail_rate_monthly,
    macro_canada_trade,
    macro_canada_unemployment_rate,
    // === BATCH3 ECONOMIC REMAINING: 东财 datacenter 36 个 ===
    macro_china_agricultural_index,
    macro_china_agricultural_product,
    // === BATCH3 ECONOMIC REMAINING: 金十 cdn 7 个 ===
    macro_china_au_report,
    macro_china_bank_financing,
    macro_china_bdti_index,
    macro_china_bond_public,
    macro_china_bsi_index,
    // === BATCH37-A 新浪宏观（MacPage_Service.get_pagedata） ===
    macro_china_central_bank_balance,
    macro_china_commodity_price_index,
    macro_china_construction_index,
    macro_china_construction_price_index,
    macro_china_consumer_goods_retail,
    macro_china_cpi,
    macro_china_cpi_monthly,
    macro_china_cpi_yearly,
    macro_china_cx_pmi_yearly,
    macro_china_cx_services_pmi_yearly,
    macro_china_czsr,
    macro_china_daily_energy,
    macro_china_energy_index,
    macro_china_enterprise_boom_index,
    macro_china_exports_yoy,
    macro_china_fdi,
    macro_china_foreign_exchange_gold,
    macro_china_freight_index,
    macro_china_fx_gold,
    macro_china_fx_reserves_yearly,
    macro_china_gdp,
    macro_china_gdp_yearly,
    macro_china_gdzctz,
    macro_china_gyzjz,
    macro_china_hgjck,
    macro_china_hk_building_amount,
    macro_china_hk_building_volume,
    macro_china_hk_cpi,
    macro_china_hk_cpi_ratio,
    macro_china_hk_gbp,
    macro_china_hk_gbp_ratio,
    macro_china_hk_market_info,
    macro_china_hk_ppi,
    macro_china_hk_rate_of_unemployment,
    macro_china_hk_trade_diff_ratio,
    macro_china_imports_yoy,
    macro_china_industrial_production_yoy,
    macro_china_insurance,
    macro_china_insurance_income,
    macro_china_international_tourism_fx,
    macro_china_lpi_index,
    macro_china_lpr,
    macro_china_m2_yearly,
    macro_china_market_margin_sh,
    macro_china_market_margin_sz,
    macro_china_mobile_number,
    macro_china_money_supply,
    macro_china_national_tax_receipts,
    macro_china_new_financial_credit,
    macro_china_new_house_price,
    macro_china_non_man_pmi,
    macro_china_passenger_load_factor,
    macro_china_pmi,
    macro_china_pmi_yearly,
    macro_china_postal_telecommunicational,
    macro_china_ppi,
    macro_china_ppi_yearly,
    macro_china_qyspjg,
    macro_china_real_estate,
    macro_china_reserve_requirement_ratio,
    macro_china_retail_price_index,
    macro_china_rmb,
    macro_china_shibor_all,
    // === BATCH37-B 商务数据中心/chinamoney 宏观 ===
    macro_china_shrzgm,
    macro_china_society_electricity,
    macro_china_society_traffic_volume,
    macro_china_stock_market_cap,
    macro_china_supply_of_money,
    macro_china_swap_rate,
    macro_china_trade_balance,
    macro_china_vegetable_basket,
    macro_china_wbck,
    macro_china_whxd,
    macro_china_xfzxx,
    macro_china_yw_electronic_index,
    // === BATCH42-A/B 欧元区宏观 16 个 ===
    macro_euro_cpi_mom,
    macro_euro_cpi_yoy,
    macro_euro_current_account_mom,
    macro_euro_employment_change_qoq,
    macro_euro_gdp_yoy,
    macro_euro_industrial_production_mom,
    macro_euro_lme_holding,
    macro_euro_lme_stock,
    macro_euro_manufacturing_pmi,
    macro_euro_ppi_mom,
    macro_euro_retail_sales_mom,
    macro_euro_sentix_investor_confidence,
    macro_euro_services_pmi,
    macro_euro_trade_balance,
    macro_euro_unemployment_rate_mom,
    macro_euro_zew_economic_sentiment,
    macro_germany_cpi_monthly,
    macro_germany_cpi_yearly,
    macro_germany_gdp,
    // === BATCH7a 海外宏观（RPT_ECONOMICVALUE_GER/JPAN/CH 系列） ===
    macro_germany_ifo,
    macro_germany_retail_sale_monthly,
    macro_germany_retail_sale_yearly,
    macro_germany_trade_adjusted,
    macro_germany_zew,
    macro_japan_bank_rate,
    macro_japan_core_cpi_yearly,
    macro_japan_cpi_yearly,
    macro_japan_head_indicator,
    macro_japan_unemployment_rate,
    macro_swiss_cpi_yearly,
    macro_swiss_gbd_bank_rate,
    macro_swiss_gbd_yearly,
    macro_swiss_gdp_quarterly,
    macro_swiss_svme,
    macro_swiss_trade,
    macro_uk_bank_rate,
    macro_uk_core_cpi_monthly,
    macro_uk_core_cpi_yearly,
    macro_uk_cpi_monthly,
    macro_uk_cpi_yearly,
    macro_uk_gdp_quarterly,
    macro_uk_gdp_yearly,
    // === BATCH7b 海外宏观（RPT_ECONOMICVALUE_BRITAIN 系列） ===
    macro_uk_halifax_monthly,
    macro_uk_halifax_yearly,
    macro_uk_retail_monthly,
    macro_uk_retail_yearly,
    macro_uk_rightmove_monthly,
    macro_uk_rightmove_yearly,
    macro_uk_trade,
    macro_uk_unemployment_rate,
};
use akshare_rust::energy::{
    energy_carbon_gz, energy_carbon_hb, energy_oil_detail, energy_oil_hist,
};
use akshare_rust::exchange::{stock_margin_detail_sse, stock_margin_sse, stock_margin_szse};
use akshare_rust::forex::{forex_hist_em, forex_spot_em};
use akshare_rust::fortune::hurun_rank;
use akshare_rust::fund::{
    fund_announcement_dividend_em, fund_announcement_personnel_em, fund_announcement_report_em,
    fund_cf_em, fund_etf_category_ths, fund_etf_fund_info_em, fund_etf_hist_min_em,
    fund_etf_spot_em, fund_etf_spot_ths, fund_exchange_rank_em, fund_fh_em,
    fund_financial_fund_daily_em, fund_graded_fund_info_em, fund_hold_structure_em,
    fund_lcx_rank_em, fund_lof_hist_em, fund_lof_spot_em, fund_money_fund_daily_em,
    fund_money_fund_info_em, fund_money_rank_em, fund_name_em, fund_new_found_em,
    fund_new_found_ths, fund_open_fund_daily_em, fund_open_fund_rank_em,
    fund_portfolio_bond_hold_em, fund_portfolio_hold_em, fund_portfolio_industry_allocation_em,
    fund_rating_all, fund_rating_ja, fund_rating_sh, fund_rating_zs, fund_scale_change_em,
};
use akshare_rust::futures::{
    futures_comex_inventory, futures_comm_info, futures_comm_js, futures_contract_detail,
    futures_contract_detail_em, futures_contract_info_cffex, futures_contract_info_czce,
    futures_contract_info_dce, futures_contract_info_gfex, futures_contract_info_ine,
    futures_contract_info_shfe, futures_delivery_czce, futures_delivery_dce,
    futures_delivery_match_dce, futures_delivery_shfe, futures_display_main_sina,
    futures_fees_info, futures_foreign_commodity_realtime,
    futures_foreign_commodity_subscribe_exchange_symbol, futures_foreign_detail,
    futures_foreign_hist, futures_gfex_warehouse_receipt, futures_global_hist_em,
    futures_global_spot_em, futures_hist_daily_cffex, futures_hist_em, futures_hist_table_em,
    futures_hold_pos_sina, futures_hq_subscribe_exchange_symbol, futures_index_ccidx,
    futures_inventory_99, futures_inventory_em, futures_main_sina, futures_news_shmet,
    futures_rule, futures_settle, futures_settle_cffex, futures_settle_czce, futures_settle_gfex,
    futures_settle_ine, futures_settle_shfe, futures_settlement_price_sgx,
    futures_shfe_warehouse_receipt, futures_spot_stock, futures_spot_sys, futures_stock_shfe_js,
    futures_symbol_mark, futures_to_spot_czce, futures_to_spot_dce, futures_to_spot_shfe,
    futures_warehouse_receipt_czce, futures_warehouse_receipt_dce, futures_zh_daily_sina,
    futures_zh_minute_sina, futures_zh_realtime, futures_zh_spot,
};
use akshare_rust::fx::{fx_c_swap_cm, fx_pair_quote, fx_quote_baidu, fx_spot_quote, fx_swap_quote};
use akshare_rust::index::{
    index_ai_cx, index_all_cni, index_analysis_daily_sw, index_analysis_monthly_sw,
    index_analysis_week_month_sw, index_analysis_weekly_sw, index_awpr_cx, index_bei_cx,
    index_bi_cx, index_cci_cx, index_ci_cx, index_component_sw, index_csindex_all, index_dei_cx,
    index_detail_cni, index_detail_hist_adjust_cni, index_detail_hist_cni, index_fi_cx,
    index_global_hist_em, index_global_hist_sina, index_global_name_table, index_global_spot_em,
    index_hist_cni, index_hist_fund_sw, index_hist_sw, index_ii_cx, index_inner_quote_sugar_msweet,
    index_li_cx, index_min_sw, index_neaw_cx, index_neei_cx, index_nei_cx,
    index_outer_quote_sugar_msweet, index_pmi_com_cx, index_pmi_man_cx, index_pmi_ser_cx,
    index_price_cflp, index_qli_cx, index_realtime_sw, index_si_cx, index_stock_cons,
    index_stock_cons_csindex, index_stock_cons_sina, index_stock_cons_weight_csindex,
    index_stock_info, index_sugar_msweet, index_ti_cx, index_volume_cflp, index_zh_a_hist,
    index_zh_a_hist_min_em,
};
use akshare_rust::interest_rate::rate_interbank;
use akshare_rust::legu::{
    fund_balance_position_lg, fund_linghuo_position_lg, fund_stock_position_lg,
    stock_a_congestion_lg, stock_buffett_index_lg, stock_ebs_lg, stock_index_pb_lg,
    stock_index_pe_lg, stock_market_pb_lg, stock_market_pe_lg,
};
use akshare_rust::news::{
    news_cctv, news_economic_baidu, news_report_time_baidu, news_trade_notify_dividend_baidu,
    news_trade_notify_suspend_baidu, stock_news_em,
};
use akshare_rust::option::{
    option_cffex_hs300_daily_sina, option_cffex_hs300_spot_sina, option_cffex_sz50_daily_sina,
    option_cffex_sz50_spot_sina, option_cffex_zz1000_daily_sina, option_cffex_zz1000_spot_sina,
    option_commodity_hist_sina, option_contract_info_ctp, option_current_day_sse,
    option_current_em, option_daily_stats_sse, option_daily_stats_szse, option_finance_board,
    option_finance_minute_sina, option_finance_sse_underlying, option_hist_czce, option_hist_dce,
    option_hist_gfex, option_hist_shfe, option_hist_yearly_czce, option_lhb_em, option_minute_em,
    option_premium_analysis_em, option_risk_analysis_em, option_risk_indicator_sse,
    option_sse_codes_sina, option_sse_daily_sina, option_sse_greeks_sina, option_sse_minute_sina,
    option_sse_spot_price_sina, option_sse_underlying_spot_price_sina, option_value_analysis_em,
    option_vol_gfex, option_vol_shfe,
};
use akshare_rust::reits::{reits_hist_em, reits_hist_min_em, reits_realtime_em};
use akshare_rust::sina::{stock_hk_spot, stock_zh_a_minute};
use akshare_rust::spot::{
    spot_corn_price_soozhu, spot_golden_benchmark_sge, spot_goods, spot_hist_sge,
    spot_hog_crossbred_soozhu, spot_hog_lean_price_soozhu, spot_hog_soozhu,
    spot_hog_three_way_soozhu, spot_hog_year_trend_soozhu, spot_mixed_feed_soozhu, spot_price_qh,
    spot_price_table_qh, spot_quotations_sge, spot_silver_benchmark_sge, spot_soybean_price_soozhu,
    spot_symbol_table_sge,
};
use akshare_rust::stock::{
    fund_etf_hist_em,
    stock_bid_ask_em,
    stock_board_concept_cons_em,
    stock_board_concept_hist_em,
    stock_board_concept_hist_min_em,
    stock_board_concept_name_em,
    stock_board_concept_spot_em,
    stock_board_industry_cons_em,
    stock_board_industry_hist_em,
    stock_board_industry_hist_min_em,
    stock_board_industry_name_em,
    stock_board_industry_spot_em,
    stock_concept_fund_flow_hist,
    // === BATCH15 东财数据中心：股市日历/高管持股/股票回购（RPT_ORGOP_ALL / RPT_EXECUTIVE_HOLD_DETAILS / RPTA_WEB_GETHGLIST_NEW） ===
    stock_gsrl_gsdt_em,
    stock_hk_company_profile_em,
    stock_hk_dividend_payout_em,
    stock_hk_financial_indicator_em,
    stock_hk_growth_comparison_em,
    stock_hk_scale_comparison_em,
    // === BATCH12 港股 F10（RPT_HKF10_* / RPT_CUSTOM_HKF10_*，securities datacenter） ===
    stock_hk_security_profile_em,
    stock_hk_valuation_comparison_em,
    stock_hold_management_detail_em,
    stock_hold_management_person_em,
    stock_hsgt_fund_flow_summary_em,
    stock_individual_fund_flow,
    stock_individual_fund_flow_rank,
    stock_individual_info_em,
    stock_main_fund_flow,
    stock_market_fund_flow,
    // === BATCH17 东财数据中心：基金持仓（dataapi host，位置式列映射→键 rename） ===
    stock_report_fund_hold,
    // === BATCH16 东财数据中心：基金持仓明细（RPT_MAINDATA_MAIN_POSITIONDETAILS） ===
    stock_report_fund_hold_detail,
    stock_repurchase_em,
    stock_sector_fund_flow_hist,
    stock_sector_fund_flow_rank,
    stock_sector_fund_flow_summary,
    stock_sh_a_spot_em,
    stock_sz_a_spot_em,
    stock_zh_a_hist,
    stock_zh_a_hist_min_em,
    stock_zh_a_spot_em,
    stock_zh_dupont_comparison_em,
    // === BATCH11 同行比较（RPT_PCF10_INDUSTRY_*，securities datacenter） ===
    stock_zh_growth_comparison_em,
    // === BATCH27 科创板报告（np-anotice-stock，ann_type=KCB） ===
    stock_zh_kcb_report_em,
    stock_zh_scale_comparison_em,
    // === BATCH13 估值对比（RPT_PCF10_INDUSTRY_CVALUE / RPT_PCF10_INDUSTRY_HKCVALUE） ===
    stock_zh_valuation_comparison_em,
    stock_zt_pool_dtgc_em,
    stock_zt_pool_em,
    stock_zt_pool_previous_em,
    stock_zt_pool_strong_em,
    stock_zt_pool_sub_new_em,
    stock_zt_pool_zbgc_em,
};
use akshare_rust::stock::{
    get_us_stock_name, stock_hk_daily, stock_hk_famous_spot_em, stock_hk_fhpx_detail_ths,
    stock_hot_search_baidu, stock_hsgt_sh_hk_spot_em, stock_info_a_code_name,
    stock_info_bj_name_code, stock_info_change_name, stock_info_sh_delist, stock_info_sh_name_code,
    stock_info_sz_change_name, stock_info_sz_delist, stock_info_sz_name_code, stock_intraday_em,
    stock_intraday_sina, stock_irm_ans_cninfo, stock_irm_cninfo, stock_js_weibo_nlp_time,
    stock_js_weibo_report, stock_news_main_cx, stock_price_js, stock_report_disclosure,
    stock_sector_detail, stock_sector_spot, stock_share_hold_change_bse,
    stock_share_hold_change_sse, stock_share_hold_change_szse, stock_sse_deal_daily,
    stock_sse_summary, stock_staq_net_stop, stock_szse_area_summary, stock_szse_summary,
    stock_us_daily, stock_us_famous_spot_em, stock_us_pink_spot_em, stock_us_spot,
    stock_zh_a_cdr_daily, stock_zh_a_daily, stock_zh_a_disclosure_relation_cninfo,
    stock_zh_a_disclosure_report_cninfo, stock_zh_a_new, stock_zh_a_spot, stock_zh_a_spot_tx,
    stock_zh_a_stop_em, stock_zh_a_tick_tx_js, stock_zh_ah_daily, stock_zh_ah_name,
    stock_zh_ah_spot, stock_zh_ah_spot_em, stock_zh_b_daily, stock_zh_b_minute, stock_zh_b_spot,
    stock_zh_kcb_daily, stock_zh_kcb_spot,
};
use akshare_rust::stock::{
    stock_balance_sheet_by_report_delisted_em, stock_balance_sheet_by_report_em,
    stock_balance_sheet_by_yearly_em, stock_cash_flow_sheet_by_quarterly_em,
    stock_cash_flow_sheet_by_report_delisted_em, stock_cash_flow_sheet_by_report_em,
    stock_cash_flow_sheet_by_yearly_em, stock_hk_spot_em, stock_profit_sheet_by_quarterly_em,
    stock_profit_sheet_by_report_delisted_em, stock_profit_sheet_by_report_em,
    stock_profit_sheet_by_yearly_em, stock_zh_a_new_em, stock_zh_a_st_em,
};
use akshare_rust::stock_feature::{
    stock_account_statistics_em, stock_analyst_detail_em, stock_analyst_rank_em,
    stock_board_concept_info_ths, stock_board_concept_name_ths, stock_board_industry_info_ths,
    stock_board_industry_name_ths, stock_comment_detail_scrd_desire_em,
    stock_comment_detail_scrd_focus_em, stock_comment_detail_zhpj_lspf_em,
    stock_comment_detail_zlkp_jgcyd_em, stock_comment_em, stock_cy_a_spot_em, stock_dxsyl_em,
    stock_esg_hz_sina, stock_esg_msci_sina, stock_esg_rate_sina, stock_esg_rft_sina,
    stock_esg_zd_sina, stock_fhps_detail_em, stock_fhps_detail_ths, stock_fhps_em, stock_gddh_em,
    stock_gdfx_free_holding_analyse_em, stock_gdfx_free_holding_change_em,
    stock_gdfx_free_holding_detail_em, stock_gdfx_free_holding_statistics_em,
    stock_gdfx_free_holding_teamwork_em, stock_gdfx_free_top_10_em, stock_gdfx_holding_analyse_em,
    stock_gdfx_holding_change_em, stock_gdfx_holding_detail_em, stock_gdfx_holding_statistics_em,
    stock_gdfx_holding_teamwork_em, stock_gdfx_top_10_em, stock_ggcg_em,
    stock_gpzy_distribute_statistics_bank_em, stock_gpzy_distribute_statistics_company_em,
    stock_gpzy_individual_pledge_ratio_detail_em, stock_gpzy_industry_data_em,
    stock_gpzy_pledge_ratio_detail_em, stock_gpzy_pledge_ratio_em, stock_gpzy_profile_em,
    stock_hk_ggt_components_em, stock_hk_hot_rank_detail_em, stock_hk_hot_rank_detail_realtime_em,
    stock_hk_hot_rank_em, stock_hk_hot_rank_latest_em, stock_hk_main_board_spot_em,
    stock_hot_keyword_em, stock_hot_rank_detail_em, stock_hot_rank_detail_realtime_em,
    stock_hot_rank_em, stock_hot_rank_latest_em, stock_hot_rank_relate_em, stock_hot_up_em,
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
use akshare_rust::stock_fund_flow::{
    stock_fund_flow_big_deal, stock_fund_flow_concept, stock_fund_flow_individual,
    stock_fund_flow_industry,
};
use akshare_rust::stock_fundamental::{
    stock_a_gxl_lg,
    stock_dzjy_hygtj,
    stock_dzjy_hyyybtj,
    stock_dzjy_mrmx,
    stock_dzjy_mrtj,
    stock_dzjy_sctj,
    stock_dzjy_yybph,
    stock_financial_abstract_new_ths,
    stock_financial_abstract_ths,
    // === BATCH26 东财 F10 股本结构/商誉/财务分析主要指标 ===
    stock_financial_analysis_indicator_em,
    stock_financial_benefit_new_ths,
    stock_financial_benefit_ths,
    stock_financial_cash_new_ths,
    stock_financial_cash_ths,
    stock_financial_debt_new_ths,
    stock_financial_debt_ths,
    stock_financial_hk_analysis_indicator_em,
    stock_financial_hk_report_em,
    stock_financial_us_analysis_indicator_em,
    stock_financial_us_report_em,
    stock_individual_basic_info_hk_xq,
    stock_individual_basic_info_us_xq,
    stock_individual_basic_info_xq,
    // === BATCH27 东财公告大全 / 主营构成（emweb F10 + np-anotice-stock） ===
    stock_individual_notice_report,
    // === BATCH9 首发申报/上会/辅导备案（RPT_IPO_DECORGNEWEST / RPT_IPO_REVIEW / RPT_IPO_TUTRECORD） ===
    stock_ipo_declare_em,
    stock_ipo_review_em,
    stock_ipo_tutor_em,
    stock_management_change_ths,
    stock_notice_report,
    // === BATCH10 盈利预测（RPT_WEB_RESPREDICT，动态 YEAR 列头） ===
    stock_profit_forecast_em,
    stock_profit_forecast_ths,
    // === BATCH8 注册制 IPO 审核信息（RPT_IPO_INFOALLNEW 系列） ===
    stock_register_all_em,
    stock_register_bj,
    stock_register_cyb,
    stock_register_db,
    stock_register_kcb,
    stock_register_sh,
    stock_register_sz,
    stock_restricted_release_detail_em,
    stock_restricted_release_queue_em,
    stock_restricted_release_queue_sina,
    stock_restricted_release_stockholder_em,
    stock_restricted_release_summary_em,
    stock_shareholder_change_ths,
    stock_sy_em,
    stock_zh_a_gbjg_em,
    stock_zygc_em,
};
use akshare_rust::tool::tool_trade_date_hist_sina;
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
        "stock_board_concept_spot_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_board_concept_spot_em(s)?)
        }
        "stock_board_industry_spot_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_board_industry_spot_em(s)?)
        }
        "stock_board_concept_hist_min_em" => {
            let [s, p] = take2(func, args)?;
            Ok(stock_board_concept_hist_min_em(s, p)?)
        }
        "stock_board_industry_hist_min_em" => {
            let [s, p] = take2(func, args)?;
            Ok(stock_board_industry_hist_min_em(s, p)?)
        }
        "stock_zt_pool_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_em(d)?)
        }
        "stock_zt_pool_previous_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_previous_em(d)?)
        }
        "stock_zt_pool_strong_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_strong_em(d)?)
        }
        "stock_zt_pool_sub_new_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_sub_new_em(d)?)
        }
        "stock_zt_pool_zbgc_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_zbgc_em(d)?)
        }
        // === BATCH27 科创板报告（np-anotice-stock，ann_type=KCB） ===
        "stock_zh_kcb_report_em" => {
            let [f, t] = take2(func, args)?;
            Ok(stock_zh_kcb_report_em(f, t)?)
        }
        "stock_zt_pool_dtgc_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_zt_pool_dtgc_em(d)?)
        }
        // === BATCH11 同行比较（RPT_PCF10_INDUSTRY_*，securities datacenter） ===
        "stock_zh_growth_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_growth_comparison_em(s)?)
        }
        "stock_zh_dupont_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_dupont_comparison_em(s)?)
        }
        "stock_zh_scale_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_scale_comparison_em(s)?)
        }
        "stock_hk_growth_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_growth_comparison_em(s)?)
        }
        "stock_hk_scale_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_scale_comparison_em(s)?)
        }
        "stock_hk_security_profile_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_security_profile_em(s)?)
        }
        "stock_hk_company_profile_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_company_profile_em(s)?)
        }
        "stock_hk_financial_indicator_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_financial_indicator_em(s)?)
        }
        "stock_hk_dividend_payout_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_dividend_payout_em(s)?)
        }
        "stock_zh_valuation_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_valuation_comparison_em(s)?)
        }
        "stock_hk_valuation_comparison_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_valuation_comparison_em(s)?)
        }
        // === BATCH15 东财数据中心：股市日历/高管持股/股票回购 ===
        "stock_gsrl_gsdt_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_gsrl_gsdt_em(d)?)
        }
        "stock_hold_management_detail_em" => Ok(stock_hold_management_detail_em()?),
        "stock_hold_management_person_em" => {
            let [s, n] = take2(func, args)?;
            Ok(stock_hold_management_person_em(s, n)?)
        }
        "stock_repurchase_em" => Ok(stock_repurchase_em()?),
        "stock_report_fund_hold_detail" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_report_fund_hold_detail(s, d)?)
        }
        "stock_report_fund_hold" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_report_fund_hold(s, d)?)
        }
        "stock_individual_fund_flow" => {
            let [s, m] = take2(func, args)?;
            Ok(stock_individual_fund_flow(s, m)?)
        }
        "stock_individual_fund_flow_rank" => {
            let [i] = take1(func, args)?;
            Ok(stock_individual_fund_flow_rank(i)?)
        }
        "stock_market_fund_flow" => Ok(stock_market_fund_flow()?),
        "stock_main_fund_flow" => {
            let [s] = take1(func, args)?;
            Ok(stock_main_fund_flow(s)?)
        }
        "stock_sector_fund_flow_rank" => {
            let [i, t] = take2(func, args)?;
            Ok(stock_sector_fund_flow_rank(i, t)?)
        }
        "stock_sector_fund_flow_hist" => {
            let [s] = take1(func, args)?;
            Ok(stock_sector_fund_flow_hist(s)?)
        }
        "stock_sector_fund_flow_summary" => {
            let [s, i] = take2(func, args)?;
            Ok(stock_sector_fund_flow_summary(s, i)?)
        }
        "stock_concept_fund_flow_hist" => {
            let [s] = take1(func, args)?;
            Ok(stock_concept_fund_flow_hist(s)?)
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
        "index_global_spot_em" => Ok(index_global_spot_em()?),
        "index_global_hist_em" => {
            let [s] = take1(func, args)?;
            Ok(index_global_hist_em(s)?)
        }
        "index_stock_cons_sina" => {
            let [s] = take1(func, args)?;
            Ok(index_stock_cons_sina(s)?)
        }
        "index_stock_info" => Ok(index_stock_info()?),
        "index_stock_cons_csindex" => {
            let [s] = take1(func, args)?;
            Ok(index_stock_cons_csindex(s)?)
        }
        "index_stock_cons_weight_csindex" => {
            let [s] = take1(func, args)?;
            Ok(index_stock_cons_weight_csindex(s)?)
        }
        "index_global_name_table" => Ok(index_global_name_table()?),
        "index_global_hist_sina" => {
            let [s] = take1(func, args)?;
            Ok(index_global_hist_sina(s)?)
        }
        "index_all_cni" => Ok(index_all_cni()?),
        "index_hist_cni" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(index_hist_cni(s, d0, d1)?)
        }
        "index_detail_cni" => {
            let [s] = take1(func, args)?;
            Ok(index_detail_cni(s)?)
        }
        "index_detail_hist_cni" => {
            let [s] = take1(func, args)?;
            Ok(index_detail_hist_cni(s)?)
        }
        "index_detail_hist_adjust_cni" => {
            let [s] = take1(func, args)?;
            Ok(index_detail_hist_adjust_cni(s)?)
        }
        "index_hist_sw" => {
            let [s, p] = take2(func, args)?;
            Ok(index_hist_sw(s, p)?)
        }
        "index_hist_fund_sw" => {
            let [s, p] = take2(func, args)?;
            Ok(index_hist_fund_sw(s, p)?)
        }
        "index_component_sw" => {
            let [s] = take1(func, args)?;
            Ok(index_component_sw(s)?)
        }
        "index_min_sw" => {
            let [s] = take1(func, args)?;
            Ok(index_min_sw(s)?)
        }
        "index_realtime_sw" => {
            let [s] = take1(func, args)?;
            Ok(index_realtime_sw(s)?)
        }
        "index_analysis_daily_sw" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(index_analysis_daily_sw(s, d0, d1)?)
        }
        "index_analysis_weekly_sw" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(index_analysis_weekly_sw(s, d0, d1)?)
        }
        "index_analysis_monthly_sw" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(index_analysis_monthly_sw(s, d0, d1)?)
        }
        "index_analysis_week_month_sw" => {
            let [s] = take1(func, args)?;
            Ok(index_analysis_week_month_sw(s)?)
        }
        // === BATCH41-A 财新指数报告 19 个（无参） ===
        "index_pmi_com_cx" => Ok(index_pmi_com_cx()?),
        "index_pmi_man_cx" => Ok(index_pmi_man_cx()?),
        "index_pmi_ser_cx" => Ok(index_pmi_ser_cx()?),
        "index_dei_cx" => Ok(index_dei_cx()?),
        "index_ii_cx" => Ok(index_ii_cx()?),
        "index_si_cx" => Ok(index_si_cx()?),
        "index_fi_cx" => Ok(index_fi_cx()?),
        "index_bi_cx" => Ok(index_bi_cx()?),
        "index_nei_cx" => Ok(index_nei_cx()?),
        "index_li_cx" => Ok(index_li_cx()?),
        "index_ci_cx" => Ok(index_ci_cx()?),
        "index_ti_cx" => Ok(index_ti_cx()?),
        "index_neaw_cx" => Ok(index_neaw_cx()?),
        "index_awpr_cx" => Ok(index_awpr_cx()?),
        "index_cci_cx" => Ok(index_cci_cx()?),
        "index_qli_cx" => Ok(index_qli_cx()?),
        "index_ai_cx" => Ok(index_ai_cx()?),
        "index_bei_cx" => Ok(index_bei_cx()?),
        "index_neei_cx" => Ok(index_neei_cx()?),
        "index_csindex_all" => Ok(index_csindex_all()?),
        "index_sugar_msweet" => Ok(index_sugar_msweet()?),
        "index_price_cflp" => {
            let [s] = take1(func, args)?;
            Ok(index_price_cflp(s)?)
        }
        "index_volume_cflp" => {
            let [s] = take1(func, args)?;
            Ok(index_volume_cflp(s)?)
        }
        "index_inner_quote_sugar_msweet" => Ok(index_inner_quote_sugar_msweet()?),
        "index_outer_quote_sugar_msweet" => Ok(index_outer_quote_sugar_msweet()?),
        "index_stock_cons" => {
            let [s] = take1(func, args)?;
            Ok(index_stock_cons(s)?)
        }
        "fund_etf_spot_em" => Ok(fund_etf_spot_em()?),
        "fund_etf_hist_min_em" => {
            let [s, d0, d1, p, a] = take5(func, args)?;
            Ok(fund_etf_hist_min_em(s, d0, d1, p, a)?)
        }
        "fund_lof_hist_em" => {
            let [s, p, d0, d1, a] = take5(func, args)?;
            Ok(fund_lof_hist_em(s, p, d0, d1, a)?)
        }
        "fund_open_fund_rank_em" => {
            let [s] = take1(func, args)?;
            Ok(fund_open_fund_rank_em(s)?)
        }
        "fund_exchange_rank_em" => Ok(fund_exchange_rank_em()?),
        "fund_money_rank_em" => Ok(fund_money_rank_em()?),
        "fund_lcx_rank_em" => Ok(fund_lcx_rank_em()?),
        "fund_open_fund_daily_em" => Ok(fund_open_fund_daily_em()?),
        "fund_money_fund_daily_em" => Ok(fund_money_fund_daily_em()?),
        "fund_financial_fund_daily_em" => Ok(fund_financial_fund_daily_em()?),
        "fund_fh_em" => {
            let [y, t, r, s, p] = take5(func, args)?;
            Ok(fund_fh_em(y, t, r, s, p.parse().unwrap_or(-1))?)
        }
        "fund_cf_em" => {
            let [y, t, r, s, p] = take5(func, args)?;
            Ok(fund_cf_em(y, t, r, s, p.parse().unwrap_or(-1))?)
        }
        "fund_name_em" => Ok(fund_name_em()?),
        "fund_scale_change_em" => Ok(fund_scale_change_em()?),
        "fund_hold_structure_em" => Ok(fund_hold_structure_em()?),
        "fund_portfolio_industry_allocation_em" => {
            let [s, d] = take2(func, args)?;
            Ok(fund_portfolio_industry_allocation_em(s, d)?)
        }
        "fund_portfolio_hold_em" => {
            let [s, d] = take2(func, args)?;
            Ok(fund_portfolio_hold_em(s, d)?)
        }
        "fund_portfolio_bond_hold_em" => {
            let [s, d] = take2(func, args)?;
            Ok(fund_portfolio_bond_hold_em(s, d)?)
        }
        "fund_new_found_em" => Ok(fund_new_found_em()?),
        "fund_new_found_ths" => {
            let [s] = take1(func, args)?;
            Ok(fund_new_found_ths(s)?)
        }
        "fund_money_fund_info_em" => {
            let [s] = take1(func, args)?;
            Ok(fund_money_fund_info_em(s)?)
        }
        "fund_etf_fund_info_em" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(fund_etf_fund_info_em(s, d0, d1)?)
        }
        "fund_graded_fund_info_em" => {
            let [s] = take1(func, args)?;
            Ok(fund_graded_fund_info_em(s)?)
        }
        "fund_rating_all" => Ok(fund_rating_all()?),
        "fund_rating_sh" => {
            let [d] = take1(func, args)?;
            Ok(fund_rating_sh(d)?)
        }
        "fund_rating_zs" => {
            let [d] = take1(func, args)?;
            Ok(fund_rating_zs(d)?)
        }
        "fund_rating_ja" => {
            let [d] = take1(func, args)?;
            Ok(fund_rating_ja(d)?)
        }
        "fund_announcement_dividend_em" => {
            let [s] = take1(func, args)?;
            Ok(fund_announcement_dividend_em(s)?)
        }
        "fund_announcement_report_em" => {
            let [s] = take1(func, args)?;
            Ok(fund_announcement_report_em(s)?)
        }
        "fund_announcement_personnel_em" => {
            let [s] = take1(func, args)?;
            Ok(fund_announcement_personnel_em(s)?)
        }
        "fund_lof_spot_em" => Ok(fund_lof_spot_em()?),
        "stock_profile_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_profile_cninfo(s)?)
        }
        "stock_industry_category_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_industry_category_cninfo(s)?)
        }
        "stock_industry_change_cninfo" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_industry_change_cninfo(s, d0, d1)?)
        }
        "stock_industry_pe_ratio_cninfo" => {
            let [d] = take1(func, args)?;
            Ok(stock_industry_pe_ratio_cninfo(d)?)
        }
        "stock_rank_forecast_cninfo" => {
            let [d] = take1(func, args)?;
            Ok(stock_rank_forecast_cninfo(d)?)
        }
        "stock_share_change_cninfo" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_share_change_cninfo(s, d0, d1)?)
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
        // === BATCH36-B cninfo 专题统计（股东股本/公司治理/配股） ===
        "stock_hold_num_cninfo" => {
            let [d] = take1(func, args)?;
            Ok(stock_hold_num_cninfo(d)?)
        }
        "stock_cg_equity_mortgage_cninfo" => {
            let [d] = take1(func, args)?;
            Ok(stock_cg_equity_mortgage_cninfo(d)?)
        }
        "stock_cg_guarantee_cninfo" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_cg_guarantee_cninfo(s, d0, d1)?)
        }
        "stock_cg_lawsuit_cninfo" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_cg_lawsuit_cninfo(s, d0, d1)?)
        }
        "stock_hold_control_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_hold_control_cninfo(s)?)
        }
        "stock_hold_change_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_hold_change_cninfo(s)?)
        }
        "stock_hold_management_detail_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_hold_management_detail_cninfo(s)?)
        }
        "stock_allotment_cninfo" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_allotment_cninfo(s, d0, d1)?)
        }
        "bond_treasure_issue_cninfo" => {
            let [s, e] = take2(func, args)?;
            Ok(bond_treasure_issue_cninfo(s, e)?)
        }
        "bond_local_government_issue_cninfo" => {
            let [s, e] = take2(func, args)?;
            Ok(bond_local_government_issue_cninfo(s, e)?)
        }
        "bond_corporate_issue_cninfo" => {
            let [s, e] = take2(func, args)?;
            Ok(bond_corporate_issue_cninfo(s, e)?)
        }
        "bond_cov_issue_cninfo" => {
            let [s, e] = take2(func, args)?;
            Ok(bond_cov_issue_cninfo(s, e)?)
        }
        "bond_cov_stock_issue_cninfo" => Ok(bond_cov_stock_issue_cninfo()?),
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
        "stock_info_sh_name_code" => {
            let [s] = take1(func, args)?;
            Ok(stock_info_sh_name_code(s)?)
        }
        "stock_info_sh_delist" => {
            let [s] = take1(func, args)?;
            Ok(stock_info_sh_delist(s)?)
        }
        "stock_info_bj_name_code" => Ok(stock_info_bj_name_code()?),
        "stock_hk_famous_spot_em" => Ok(stock_hk_famous_spot_em()?),
        "stock_us_famous_spot_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_us_famous_spot_em(s)?)
        }
        "stock_zh_a_spot_tx" => Ok(stock_zh_a_spot_tx()?),
        "stock_zh_a_stop_em" => Ok(stock_zh_a_stop_em()?),
        "stock_zh_a_new" => Ok(stock_zh_a_new()?),
        "stock_zh_a_daily" => {
            let [s, d0, d1, a] = take4(func, args)?;
            Ok(stock_zh_a_daily(s, d0, d1, a)?)
        }
        "stock_us_pink_spot_em" => Ok(stock_us_pink_spot_em()?),
        "stock_zh_ah_spot_em" => Ok(stock_zh_ah_spot_em()?),
        "stock_zh_ah_spot" => Ok(stock_zh_ah_spot()?),
        "stock_zh_ah_name" => Ok(stock_zh_ah_name()?),
        "stock_zh_ah_daily" => {
            let [s, y0, y1, a] = take4(func, args)?;
            Ok(stock_zh_ah_daily(s, y0, y1, a)?)
        }
        "stock_hsgt_sh_hk_spot_em" => Ok(stock_hsgt_sh_hk_spot_em()?),
        "stock_zh_a_spot" => Ok(stock_zh_a_spot()?),
        "stock_irm_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_irm_cninfo(s)?)
        }
        "stock_irm_ans_cninfo" => {
            let [s] = take1(func, args)?;
            Ok(stock_irm_ans_cninfo(s)?)
        }
        "stock_info_sz_name_code" => {
            let [s] = take1(func, args)?;
            Ok(stock_info_sz_name_code(s)?)
        }
        "stock_info_sz_delist" => {
            let [s] = take1(func, args)?;
            Ok(stock_info_sz_delist(s)?)
        }
        "stock_info_sz_change_name" => {
            let [s] = take1(func, args)?;
            Ok(stock_info_sz_change_name(s)?)
        }
        "stock_info_change_name" => {
            let [s] = take1(func, args)?;
            Ok(stock_info_change_name(s)?)
        }
        "stock_info_a_code_name" => Ok(stock_info_a_code_name()?),
        "stock_report_disclosure" => {
            let [m, p] = take2(func, args)?;
            Ok(stock_report_disclosure(m, p)?)
        }
        "stock_zh_a_cdr_daily" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(stock_zh_a_cdr_daily(s, d0, d1)?)
        }
        "stock_szse_summary" => {
            let [d] = take1(func, args)?;
            Ok(stock_szse_summary(d)?)
        }
        "stock_szse_area_summary" => {
            let [d] = take1(func, args)?;
            Ok(stock_szse_area_summary(d)?)
        }
        "stock_sse_summary" => Ok(stock_sse_summary()?),
        "stock_sse_deal_daily" => {
            let [d] = take1(func, args)?;
            Ok(stock_sse_deal_daily(d)?)
        }
        "stock_zh_b_daily" => {
            let [s, d0, d1, a] = take4(func, args)?;
            Ok(stock_zh_b_daily(s, d0, d1, a)?)
        }
        "stock_zh_b_minute" => {
            let [s, p, a] = take3(func, args)?;
            Ok(stock_zh_b_minute(s, p, a)?)
        }
        "stock_hk_daily" => {
            let [s, a] = take2(func, args)?;
            Ok(stock_hk_daily(s, a)?)
        }
        "stock_sector_spot" => {
            let [i] = take1(func, args)?;
            Ok(stock_sector_spot(i)?)
        }
        "stock_sector_detail" => {
            let [s] = take1(func, args)?;
            Ok(stock_sector_detail(s)?)
        }
        "stock_zh_a_tick_tx_js" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_a_tick_tx_js(s)?)
        }
        "stock_intraday_sina" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_intraday_sina(s, d)?)
        }
        "stock_staq_net_stop" => Ok(stock_staq_net_stop()?),
        "stock_intraday_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_intraday_em(s)?)
        }
        "stock_news_main_cx" => Ok(stock_news_main_cx()?),
        "stock_zh_a_disclosure_report_cninfo" => {
            let [s, m, k, c, d0, d1] = take6(func, args)?;
            Ok(stock_zh_a_disclosure_report_cninfo(s, m, k, c, d0, d1)?)
        }
        "stock_zh_a_disclosure_relation_cninfo" => {
            let [s, m, d0, d1] = take4(func, args)?;
            Ok(stock_zh_a_disclosure_relation_cninfo(s, m, d0, d1)?)
        }
        "stock_hot_search_baidu" => {
            let [s, d, t] = take3(func, args)?;
            Ok(stock_hot_search_baidu(s, d, t)?)
        }
        "stock_js_weibo_report" => {
            let [t] = take1(func, args)?;
            Ok(stock_js_weibo_report(t)?)
        }
        "stock_js_weibo_nlp_time" => Ok(stock_js_weibo_nlp_time()?),
        "stock_price_js" => {
            let [s] = take1(func, args)?;
            Ok(stock_price_js(s)?)
        }
        "stock_hk_fhpx_detail_ths" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_fhpx_detail_ths(s)?)
        }
        "get_us_stock_name" => Ok(get_us_stock_name()?),
        "stock_us_spot" => Ok(stock_us_spot()?),
        "stock_us_daily" => {
            let [s, a] = take2(func, args)?;
            Ok(stock_us_daily(s, a)?)
        }
        "stock_zh_kcb_daily" => {
            let [s, a] = take2(func, args)?;
            Ok(stock_zh_kcb_daily(s, a)?)
        }
        "stock_zh_kcb_spot" => Ok(stock_zh_kcb_spot()?),
        "stock_zh_b_spot" => Ok(stock_zh_b_spot()?),
        "stock_share_hold_change_sse" => {
            let [s] = take1(func, args)?;
            Ok(stock_share_hold_change_sse(s)?)
        }
        "stock_share_hold_change_szse" => {
            let [s] = take1(func, args)?;
            Ok(stock_share_hold_change_szse(s)?)
        }
        "stock_share_hold_change_bse" => {
            let [s] = take1(func, args)?;
            Ok(stock_share_hold_change_bse(s)?)
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
        // BATCH24 新浪 ESG 评级中心（0 参数）
        "stock_esg_msci_sina" => Ok(stock_esg_msci_sina()?),
        "stock_esg_rft_sina" => Ok(stock_esg_rft_sina()?),
        "stock_esg_rate_sina" => Ok(stock_esg_rate_sina()?),
        "stock_esg_zd_sina" => Ok(stock_esg_zd_sina()?),
        "stock_esg_hz_sina" => Ok(stock_esg_hz_sina()?),
        // BATCH25 同花顺-资金流向（data.10jqka.com.cn/funds/*）
        "stock_fund_flow_individual" => {
            let [s] = take1(func, args)?;
            Ok(stock_fund_flow_individual(s)?)
        }
        "stock_fund_flow_concept" => {
            let [s] = take1(func, args)?;
            Ok(stock_fund_flow_concept(s)?)
        }
        "stock_fund_flow_industry" => {
            let [s] = take1(func, args)?;
            Ok(stock_fund_flow_industry(s)?)
        }
        "stock_fund_flow_big_deal" => Ok(stock_fund_flow_big_deal()?),
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
        "stock_gdfx_top_10_em" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_gdfx_top_10_em(s, d)?)
        }
        "stock_gdfx_free_top_10_em" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_gdfx_free_top_10_em(s, d)?)
        }
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
        "stock_hot_rank_em" => Ok(stock_hot_rank_em()?),
        "stock_hk_hot_rank_em" => Ok(stock_hk_hot_rank_em()?),
        "stock_hk_hot_rank_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_hot_rank_detail_em(s)?)
        }
        "stock_hk_hot_rank_detail_realtime_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_hot_rank_detail_realtime_em(s)?)
        }
        "stock_hk_hot_rank_latest_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hk_hot_rank_latest_em(s)?)
        }
        "stock_hot_up_em" => Ok(stock_hot_up_em()?),
        "stock_hot_rank_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_rank_detail_em(s)?)
        }
        "stock_hot_rank_detail_realtime_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_rank_detail_realtime_em(s)?)
        }
        "stock_hot_keyword_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_keyword_em(s)?)
        }
        "stock_hot_rank_latest_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_rank_latest_em(s)?)
        }
        "stock_hot_rank_relate_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_hot_rank_relate_em(s)?)
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
        "stock_balance_sheet_by_report_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_balance_sheet_by_report_em(s)?)
        }
        "stock_balance_sheet_by_yearly_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_balance_sheet_by_yearly_em(s)?)
        }
        "stock_profit_sheet_by_report_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_profit_sheet_by_report_em(s)?)
        }
        "stock_profit_sheet_by_yearly_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_profit_sheet_by_yearly_em(s)?)
        }
        "stock_cash_flow_sheet_by_report_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_cash_flow_sheet_by_report_em(s)?)
        }
        "stock_cash_flow_sheet_by_yearly_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_cash_flow_sheet_by_yearly_em(s)?)
        }
        "stock_profit_sheet_by_quarterly_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_profit_sheet_by_quarterly_em(s)?)
        }
        "stock_cash_flow_sheet_by_quarterly_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_cash_flow_sheet_by_quarterly_em(s)?)
        }
        "stock_balance_sheet_by_report_delisted_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_balance_sheet_by_report_delisted_em(s)?)
        }
        "stock_profit_sheet_by_report_delisted_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_profit_sheet_by_report_delisted_em(s)?)
        }
        "stock_cash_flow_sheet_by_report_delisted_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_cash_flow_sheet_by_report_delisted_em(s)?)
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
        "stock_restricted_release_queue_sina" => {
            let [s] = take1(func, args)?;
            Ok(stock_restricted_release_queue_sina(s)?)
        }
        "stock_restricted_release_stockholder_em" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_restricted_release_stockholder_em(s, d)?)
        }
        // BATCH26 东财 F10 股本结构/商誉/财务分析主要指标
        "stock_zh_a_gbjg_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zh_a_gbjg_em(s)?)
        }
        "stock_sy_em" => {
            let [d] = take1(func, args)?;
            Ok(stock_sy_em(d)?)
        }
        "stock_financial_analysis_indicator_em" => {
            let [s, ind] = take2(func, args)?;
            Ok(stock_financial_analysis_indicator_em(s, ind)?)
        }
        "stock_financial_hk_analysis_indicator_em" => {
            let [s, ind] = take2(func, args)?;
            Ok(stock_financial_hk_analysis_indicator_em(s, ind)?)
        }
        "stock_financial_us_analysis_indicator_em" => {
            let [s, ind] = take2(func, args)?;
            Ok(stock_financial_us_analysis_indicator_em(s, ind)?)
        }
        "stock_financial_hk_report_em" => {
            let [s, sym, ind] = take3(func, args)?;
            Ok(stock_financial_hk_report_em(s, sym, ind)?)
        }
        "stock_financial_us_report_em" => {
            let [s, sym, ind] = take3(func, args)?;
            Ok(stock_financial_us_report_em(s, sym, ind)?)
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
        "macro_china_central_bank_balance" => Ok(macro_china_central_bank_balance()?),
        "macro_china_foreign_exchange_gold" => Ok(macro_china_foreign_exchange_gold()?),
        "macro_china_insurance" => Ok(macro_china_insurance()?),
        "macro_china_international_tourism_fx" => Ok(macro_china_international_tourism_fx()?),
        "macro_china_passenger_load_factor" => Ok(macro_china_passenger_load_factor()?),
        "macro_china_postal_telecommunicational" => Ok(macro_china_postal_telecommunicational()?),
        "macro_china_retail_price_index" => Ok(macro_china_retail_price_index()?),
        "macro_china_society_electricity" => Ok(macro_china_society_electricity()?),
        "macro_china_society_traffic_volume" => Ok(macro_china_society_traffic_volume()?),
        "macro_china_supply_of_money" => Ok(macro_china_supply_of_money()?),
        "macro_china_freight_index" => Ok(macro_china_freight_index()?),
        "macro_china_shrzgm" => Ok(macro_china_shrzgm()?),
        "macro_china_bond_public" => Ok(macro_china_bond_public()?),
        "macro_china_swap_rate" => {
            let [d0, d1] = take2(func, args)?;
            Ok(macro_china_swap_rate(d0, d1)?)
        }
        // === BATCH42-A/B 欧元区宏观 16 个（无参） ===
        "macro_euro_gdp_yoy" => Ok(macro_euro_gdp_yoy()?),
        "macro_euro_cpi_mom" => Ok(macro_euro_cpi_mom()?),
        "macro_euro_cpi_yoy" => Ok(macro_euro_cpi_yoy()?),
        "macro_euro_ppi_mom" => Ok(macro_euro_ppi_mom()?),
        "macro_euro_retail_sales_mom" => Ok(macro_euro_retail_sales_mom()?),
        "macro_euro_employment_change_qoq" => Ok(macro_euro_employment_change_qoq()?),
        "macro_euro_unemployment_rate_mom" => Ok(macro_euro_unemployment_rate_mom()?),
        "macro_euro_trade_balance" => Ok(macro_euro_trade_balance()?),
        "macro_euro_current_account_mom" => Ok(macro_euro_current_account_mom()?),
        "macro_euro_industrial_production_mom" => Ok(macro_euro_industrial_production_mom()?),
        "macro_euro_manufacturing_pmi" => Ok(macro_euro_manufacturing_pmi()?),
        "macro_euro_services_pmi" => Ok(macro_euro_services_pmi()?),
        "macro_euro_zew_economic_sentiment" => Ok(macro_euro_zew_economic_sentiment()?),
        "macro_euro_sentix_investor_confidence" => Ok(macro_euro_sentix_investor_confidence()?),
        "macro_euro_lme_holding" => Ok(macro_euro_lme_holding()?),
        "macro_euro_lme_stock" => Ok(macro_euro_lme_stock()?),
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
        "futures_comex_inventory" => {
            let [s] = take1(func, args)?;
            Ok(futures_comex_inventory(s)?)
        }
        "futures_inventory_em" => {
            let [s] = take1(func, args)?;
            Ok(futures_inventory_em(s)?)
        }
        "futures_index_ccidx" => {
            let [s] = take1(func, args)?;
            Ok(futures_index_ccidx(s)?)
        }
        "futures_global_spot_em" => Ok(futures_global_spot_em()?),
        "futures_global_hist_em" => {
            let [s] = take1(func, args)?;
            Ok(futures_global_hist_em(s)?)
        }
        // 批次 29 子组 B：新浪期货集群
        "futures_symbol_mark" => Ok(futures_symbol_mark()?),
        "futures_zh_realtime" => {
            let [s] = take1(func, args)?;
            Ok(futures_zh_realtime(s)?)
        }
        "futures_zh_spot" => {
            let [s, m, a] = take3(func, args)?;
            Ok(futures_zh_spot(s, m, a)?)
        }
        "futures_zh_daily_sina" => {
            let [s] = take1(func, args)?;
            Ok(futures_zh_daily_sina(s)?)
        }
        "futures_zh_minute_sina" => {
            let [s, p] = take2(func, args)?;
            Ok(futures_zh_minute_sina(s, p)?)
        }
        "futures_hq_subscribe_exchange_symbol" => Ok(futures_hq_subscribe_exchange_symbol()?),
        "futures_foreign_commodity_realtime" => {
            let [s] = take1(func, args)?;
            Ok(futures_foreign_commodity_realtime(s)?)
        }
        "futures_foreign_commodity_subscribe_exchange_symbol" => {
            Ok(futures_foreign_commodity_subscribe_exchange_symbol()?)
        }
        "futures_foreign_detail" => {
            let [s] = take1(func, args)?;
            Ok(futures_foreign_detail(s)?)
        }
        "futures_foreign_hist" => {
            let [s] = take1(func, args)?;
            Ok(futures_foreign_hist(s)?)
        }
        // 批次 29 子组 F：新浪主力/连续/持仓
        "futures_display_main_sina" => Ok(futures_display_main_sina()?),
        "futures_main_sina" => {
            let [s, d0, d1] = take3(func, args)?;
            Ok(futures_main_sina(s, d0, d1)?)
        }
        "futures_hold_pos_sina" => {
            let [s, c, d] = take3(func, args)?;
            Ok(futures_hold_pos_sina(s, c, d)?)
        }
        // 批次 29 子组 C：交易所官方数据（合约信息）
        "futures_contract_info_cffex" => {
            let [s] = take1(func, args)?;
            Ok(futures_contract_info_cffex(s)?)
        }
        "futures_contract_info_czce" => {
            let [s] = take1(func, args)?;
            Ok(futures_contract_info_czce(s)?)
        }
        "futures_contract_info_dce" => Ok(futures_contract_info_dce()?),
        "futures_contract_info_gfex" => Ok(futures_contract_info_gfex()?),
        "futures_contract_info_ine" => {
            let [s] = take1(func, args)?;
            Ok(futures_contract_info_ine(s)?)
        }
        "futures_contract_info_shfe" => {
            let [s] = take1(func, args)?;
            Ok(futures_contract_info_shfe(s)?)
        }
        // 批次 29 子组 C：交易所官方数据-仓单 / 交割 / 期转现 / 历史行情
        "futures_warehouse_receipt_czce" => {
            let [s] = take1(func, args)?;
            Ok(futures_warehouse_receipt_czce(s)?)
        }
        "futures_warehouse_receipt_dce" => {
            let [s] = take1(func, args)?;
            Ok(futures_warehouse_receipt_dce(s)?)
        }
        "futures_shfe_warehouse_receipt" => {
            let [s] = take1(func, args)?;
            Ok(futures_shfe_warehouse_receipt(s)?)
        }
        "futures_gfex_warehouse_receipt" => {
            let [s] = take1(func, args)?;
            Ok(futures_gfex_warehouse_receipt(s)?)
        }
        "futures_to_spot_shfe" => {
            let [s] = take1(func, args)?;
            Ok(futures_to_spot_shfe(s)?)
        }
        "futures_delivery_dce" => {
            let [s] = take1(func, args)?;
            Ok(futures_delivery_dce(s)?)
        }
        "futures_to_spot_dce" => {
            let [s] = take1(func, args)?;
            Ok(futures_to_spot_dce(s)?)
        }
        "futures_delivery_match_dce" => {
            let [s] = take1(func, args)?;
            Ok(futures_delivery_match_dce(s)?)
        }
        "futures_to_spot_czce" => {
            let [s] = take1(func, args)?;
            Ok(futures_to_spot_czce(s)?)
        }
        "futures_delivery_czce" => {
            let [s] = take1(func, args)?;
            Ok(futures_delivery_czce(s)?)
        }
        "futures_delivery_shfe" => {
            let [s] = take1(func, args)?;
            Ok(futures_delivery_shfe(s)?)
        }
        "futures_hist_daily_cffex" => {
            let [s] = take1(func, args)?;
            Ok(futures_hist_daily_cffex(s)?)
        }
        "futures_hist_table_em" => Ok(futures_hist_table_em()?),
        "futures_hist_em" => {
            let [symbol, period, start, end] = take4(func, args)?;
            Ok(futures_hist_em(symbol, period, start, end)?)
        }
        "futures_settlement_price_sgx" => {
            let [s] = take1(func, args)?;
            Ok(futures_settlement_price_sgx(s)?)
        }
        // 批次 29 子组 E：期货杂项 / 独立数据源集群
        "futures_comm_info" => {
            let [s] = take1(func, args)?;
            Ok(futures_comm_info(s)?)
        }
        "futures_comm_js" => {
            let [s] = take1(func, args)?;
            Ok(futures_comm_js(s)?)
        }
        "futures_fees_info" => Ok(futures_fees_info()?),
        "futures_rule" => {
            let [s] = take1(func, args)?;
            Ok(futures_rule(s)?)
        }
        "futures_news_shmet" => {
            let [s] = take1(func, args)?;
            Ok(futures_news_shmet(s)?)
        }
        "futures_inventory_99" => {
            let [s] = take1(func, args)?;
            Ok(futures_inventory_99(s)?)
        }
        "futures_spot_stock" => {
            let [s] = take1(func, args)?;
            Ok(futures_spot_stock(s)?)
        }
        "futures_stock_shfe_js" => {
            let [s] = take1(func, args)?;
            Ok(futures_stock_shfe_js(s)?)
        }
        "futures_spot_sys" => {
            let [s1, s2] = take2(func, args)?;
            Ok(futures_spot_sys(s1, s2)?)
        }
        "futures_contract_detail_em" => {
            let [s] = take1(func, args)?;
            Ok(futures_contract_detail_em(s)?)
        }
        "rate_interbank" => {
            let [m, s, ind] = take3(func, args)?;
            Ok(rate_interbank(m, s, ind)?)
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
        // === BATCH8 注册制 IPO 审核信息（RPT_IPO_INFOALLNEW 系列） ===
        "stock_register_all_em" => Ok(stock_register_all_em()?),
        "stock_register_db" => Ok(stock_register_db()?),
        "stock_register_kcb" => Ok(stock_register_kcb()?),
        "stock_register_cyb" => Ok(stock_register_cyb()?),
        "stock_register_bj" => Ok(stock_register_bj()?),
        "stock_register_sh" => Ok(stock_register_sh()?),
        "stock_register_sz" => Ok(stock_register_sz()?),
        // === BATCH9 首发申报/上会/辅导备案（RPT_IPO_*） ===
        "stock_ipo_declare_em" => Ok(stock_ipo_declare_em()?),
        "stock_ipo_review_em" => Ok(stock_ipo_review_em()?),
        "stock_ipo_tutor_em" => Ok(stock_ipo_tutor_em()?),
        // === BATCH10 盈利预测（RPT_WEB_RESPREDICT，动态 YEAR 列头） ===
        "stock_profit_forecast_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_profit_forecast_em(s)?)
        }
        // === BATCH27 东财公告大全 / 主营构成（emweb F10 + np-anotice-stock） ===
        "stock_zygc_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_zygc_em(s)?)
        }
        "stock_notice_report" => {
            let [s, d] = take2(func, args)?;
            Ok(stock_notice_report(s, d)?)
        }
        "stock_individual_notice_report" => {
            let [sec, s, d0, d1] = take4(func, args)?;
            Ok(stock_individual_notice_report(sec, s, d0, d1)?)
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
        // === BATCH6 海外宏观（RPT_ECONOMICVALUE_* 系列） ===
        "macro_australia_bank_rate" => Ok(macro_australia_bank_rate()?),
        "macro_australia_cpi_quarterly" => Ok(macro_australia_cpi_quarterly()?),
        "macro_australia_cpi_yearly" => Ok(macro_australia_cpi_yearly()?),
        "macro_australia_ppi_quarterly" => Ok(macro_australia_ppi_quarterly()?),
        "macro_australia_retail_rate_monthly" => Ok(macro_australia_retail_rate_monthly()?),
        "macro_australia_trade" => Ok(macro_australia_trade()?),
        "macro_australia_unemployment_rate" => Ok(macro_australia_unemployment_rate()?),
        "macro_canada_bank_rate" => Ok(macro_canada_bank_rate()?),
        "macro_canada_core_cpi_monthly" => Ok(macro_canada_core_cpi_monthly()?),
        "macro_canada_core_cpi_yearly" => Ok(macro_canada_core_cpi_yearly()?),
        "macro_canada_cpi_monthly" => Ok(macro_canada_cpi_monthly()?),
        "macro_canada_cpi_yearly" => Ok(macro_canada_cpi_yearly()?),
        "macro_canada_gdp_monthly" => Ok(macro_canada_gdp_monthly()?),
        "macro_canada_new_house_rate" => Ok(macro_canada_new_house_rate()?),
        "macro_canada_retail_rate_monthly" => Ok(macro_canada_retail_rate_monthly()?),
        "macro_canada_trade" => Ok(macro_canada_trade()?),
        "macro_canada_unemployment_rate" => Ok(macro_canada_unemployment_rate()?),
        // === BATCH7a 海外宏观（RPT_ECONOMICVALUE_GER/JPAN/CH 系列） ===
        "macro_germany_ifo" => Ok(macro_germany_ifo()?),
        "macro_germany_cpi_monthly" => Ok(macro_germany_cpi_monthly()?),
        "macro_germany_cpi_yearly" => Ok(macro_germany_cpi_yearly()?),
        "macro_germany_trade_adjusted" => Ok(macro_germany_trade_adjusted()?),
        "macro_germany_gdp" => Ok(macro_germany_gdp()?),
        "macro_germany_retail_sale_monthly" => Ok(macro_germany_retail_sale_monthly()?),
        "macro_germany_retail_sale_yearly" => Ok(macro_germany_retail_sale_yearly()?),
        "macro_germany_zew" => Ok(macro_germany_zew()?),
        "macro_japan_bank_rate" => Ok(macro_japan_bank_rate()?),
        "macro_japan_cpi_yearly" => Ok(macro_japan_cpi_yearly()?),
        "macro_japan_core_cpi_yearly" => Ok(macro_japan_core_cpi_yearly()?),
        "macro_japan_unemployment_rate" => Ok(macro_japan_unemployment_rate()?),
        "macro_japan_head_indicator" => Ok(macro_japan_head_indicator()?),
        "macro_swiss_svme" => Ok(macro_swiss_svme()?),
        "macro_swiss_trade" => Ok(macro_swiss_trade()?),
        "macro_swiss_cpi_yearly" => Ok(macro_swiss_cpi_yearly()?),
        "macro_swiss_gdp_quarterly" => Ok(macro_swiss_gdp_quarterly()?),
        "macro_swiss_gbd_yearly" => Ok(macro_swiss_gbd_yearly()?),
        "macro_swiss_gbd_bank_rate" => Ok(macro_swiss_gbd_bank_rate()?),
        // === BATCH7b 海外宏观（RPT_ECONOMICVALUE_BRITAIN 系列） ===
        "macro_uk_halifax_monthly" => Ok(macro_uk_halifax_monthly()?),
        "macro_uk_halifax_yearly" => Ok(macro_uk_halifax_yearly()?),
        "macro_uk_trade" => Ok(macro_uk_trade()?),
        "macro_uk_bank_rate" => Ok(macro_uk_bank_rate()?),
        "macro_uk_core_cpi_yearly" => Ok(macro_uk_core_cpi_yearly()?),
        "macro_uk_core_cpi_monthly" => Ok(macro_uk_core_cpi_monthly()?),
        "macro_uk_cpi_yearly" => Ok(macro_uk_cpi_yearly()?),
        "macro_uk_cpi_monthly" => Ok(macro_uk_cpi_monthly()?),
        "macro_uk_retail_monthly" => Ok(macro_uk_retail_monthly()?),
        "macro_uk_retail_yearly" => Ok(macro_uk_retail_yearly()?),
        "macro_uk_rightmove_yearly" => Ok(macro_uk_rightmove_yearly()?),
        "macro_uk_rightmove_monthly" => Ok(macro_uk_rightmove_monthly()?),
        "macro_uk_gdp_quarterly" => Ok(macro_uk_gdp_quarterly()?),
        "macro_uk_gdp_yearly" => Ok(macro_uk_gdp_yearly()?),
        "macro_uk_unemployment_rate" => Ok(macro_uk_unemployment_rate()?),
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
        "option_cffex_sz50_spot_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_cffex_sz50_spot_sina(s)?)
        }
        "option_cffex_sz50_daily_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_cffex_sz50_daily_sina(s)?)
        }
        "option_cffex_hs300_spot_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_cffex_hs300_spot_sina(s)?)
        }
        "option_cffex_hs300_daily_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_cffex_hs300_daily_sina(s)?)
        }
        "option_cffex_zz1000_spot_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_cffex_zz1000_spot_sina(s)?)
        }
        "option_cffex_zz1000_daily_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_cffex_zz1000_daily_sina(s)?)
        }
        // === BATCH2 OPTION 续：新浪 SSE / 商品 / 交易所 / 东财 / 其他 ===
        "option_sse_spot_price_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_sse_spot_price_sina(s)?)
        }
        "option_sse_underlying_spot_price_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_sse_underlying_spot_price_sina(s)?)
        }
        "option_sse_greeks_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_sse_greeks_sina(s)?)
        }
        "option_sse_minute_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_sse_minute_sina(s)?)
        }
        "option_sse_daily_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_sse_daily_sina(s)?)
        }
        "option_finance_minute_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_finance_minute_sina(s)?)
        }
        "option_commodity_hist_sina" => {
            let [s] = take1(func, args)?;
            Ok(option_commodity_hist_sina(s)?)
        }
        "option_daily_stats_sse" => {
            let [s] = take1(func, args)?;
            Ok(option_daily_stats_sse(s)?)
        }
        "option_daily_stats_szse" => {
            let [s] = take1(func, args)?;
            Ok(option_daily_stats_szse(s)?)
        }
        "option_risk_indicator_sse" => {
            let [s] = take1(func, args)?;
            Ok(option_risk_indicator_sse(s)?)
        }
        "option_minute_em" => {
            let [s] = take1(func, args)?;
            Ok(option_minute_em(s)?)
        }
        "option_finance_sse_underlying" => {
            let [s] = take1(func, args)?;
            Ok(option_finance_sse_underlying(s)?)
        }
        "option_sse_codes_sina" => {
            let [s, d, u] = take3(func, args)?;
            Ok(option_sse_codes_sina(s, d, u)?)
        }
        "option_lhb_em" => {
            let [s, ind, d] = take3(func, args)?;
            Ok(option_lhb_em(s, ind, d)?)
        }
        "option_hist_czce" => {
            let [s, d] = take2(func, args)?;
            Ok(option_hist_czce(s, d)?)
        }
        "option_hist_yearly_czce" => {
            let [s, y] = take2(func, args)?;
            Ok(option_hist_yearly_czce(s, y)?)
        }
        "option_hist_dce" => {
            let [s, d] = take2(func, args)?;
            Ok(option_hist_dce(s, d)?)
        }
        "option_hist_gfex" => {
            let [s, d] = take2(func, args)?;
            Ok(option_hist_gfex(s, d)?)
        }
        "option_hist_shfe" => {
            let [s, d] = take2(func, args)?;
            Ok(option_hist_shfe(s, d)?)
        }
        "option_vol_shfe" => {
            let [s, d] = take2(func, args)?;
            Ok(option_vol_shfe(s, d)?)
        }
        "option_vol_gfex" => {
            let [s, d] = take2(func, args)?;
            Ok(option_vol_gfex(s, d)?)
        }
        "option_finance_board" => {
            let [s, m] = take2(func, args)?;
            Ok(option_finance_board(s, m)?)
        }
        "option_current_day_sse" => Ok(option_current_day_sse()?),
        "option_current_em" => Ok(option_current_em()?),
        "option_premium_analysis_em" => Ok(option_premium_analysis_em()?),
        "option_risk_analysis_em" => Ok(option_risk_analysis_em()?),
        "option_value_analysis_em" => Ok(option_value_analysis_em()?),
        "option_contract_info_ctp" => Ok(option_contract_info_ctp()?),
        // === BATCH3 ECONOMIC REMAINING (jin10/em datacenter) ===
        // 东财 datacenter-web 宏观 36 个
        "macro_china_agricultural_index" => Ok(macro_china_agricultural_index()?),
        "macro_china_agricultural_product" => Ok(macro_china_agricultural_product()?),
        "macro_china_bank_financing" => Ok(macro_china_bank_financing()?),
        "macro_china_bdti_index" => Ok(macro_china_bdti_index()?),
        "macro_china_bsi_index" => Ok(macro_china_bsi_index()?),
        "macro_china_commodity_price_index" => Ok(macro_china_commodity_price_index()?),
        "macro_china_construction_index" => Ok(macro_china_construction_index()?),
        "macro_china_construction_price_index" => Ok(macro_china_construction_price_index()?),
        "macro_china_energy_index" => Ok(macro_china_energy_index()?),
        "macro_china_insurance_income" => Ok(macro_china_insurance_income()?),
        "macro_china_lpi_index" => Ok(macro_china_lpi_index()?),
        "macro_china_mobile_number" => Ok(macro_china_mobile_number()?),
        "macro_china_real_estate" => Ok(macro_china_real_estate()?),
        "macro_china_vegetable_basket" => Ok(macro_china_vegetable_basket()?),
        "macro_china_yw_electronic_index" => Ok(macro_china_yw_electronic_index()?),
        "macro_china_consumer_goods_retail" => Ok(macro_china_consumer_goods_retail()?),
        "macro_china_cpi" => Ok(macro_china_cpi()?),
        "macro_china_czsr" => Ok(macro_china_czsr()?),
        "macro_china_enterprise_boom_index" => Ok(macro_china_enterprise_boom_index()?),
        "macro_china_fx_gold" => Ok(macro_china_fx_gold()?),
        "macro_china_gdp" => Ok(macro_china_gdp()?),
        "macro_china_gdzctz" => Ok(macro_china_gdzctz()?),
        "macro_china_gyzjz" => Ok(macro_china_gyzjz()?),
        "macro_china_hgjck" => Ok(macro_china_hgjck()?),
        "macro_china_lpr" => Ok(macro_china_lpr()?),
        "macro_china_money_supply" => Ok(macro_china_money_supply()?),
        "macro_china_national_tax_receipts" => Ok(macro_china_national_tax_receipts()?),
        "macro_china_new_financial_credit" => Ok(macro_china_new_financial_credit()?),
        "macro_china_new_house_price" => Ok(macro_china_new_house_price()?),
        "macro_china_pmi" => Ok(macro_china_pmi()?),
        "macro_china_ppi" => Ok(macro_china_ppi()?),
        "macro_china_reserve_requirement_ratio" => Ok(macro_china_reserve_requirement_ratio()?),
        "macro_china_stock_market_cap" => Ok(macro_china_stock_market_cap()?),
        "macro_china_wbck" => Ok(macro_china_wbck()?),
        "macro_china_whxd" => Ok(macro_china_whxd()?),
        "macro_china_xfzxx" => Ok(macro_china_xfzxx()?),
        // 金十 cdn 7 个
        "macro_china_au_report" => Ok(macro_china_au_report()?),
        "macro_china_rmb" => Ok(macro_china_rmb()?),
        "macro_china_shibor_all" => Ok(macro_china_shibor_all()?),
        "macro_china_hk_market_info" => Ok(macro_china_hk_market_info()?),
        "macro_china_market_margin_sh" => Ok(macro_china_market_margin_sh()?),
        "macro_china_market_margin_sz" => Ok(macro_china_market_margin_sz()?),
        "macro_china_daily_energy" => Ok(macro_china_daily_energy()?),
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
        "fx_spot_quote" => Ok(fx_spot_quote()?),
        "fx_swap_quote" => Ok(fx_swap_quote()?),
        "fx_pair_quote" => Ok(fx_pair_quote()?),
        "fx_c_swap_cm" => Ok(fx_c_swap_cm()?),
        "fx_quote_baidu" => {
            let [s, t] = take2(func, args)?;
            Ok(fx_quote_baidu(s, t)?)
        }
        // === 批次36 forex / reits / tool ===
        "forex_spot_em" => Ok(forex_spot_em()?),
        "forex_hist_em" => {
            let [s] = take1(func, args)?;
            Ok(forex_hist_em(s)?)
        }
        "reits_realtime_em" => Ok(reits_realtime_em()?),
        "reits_hist_em" => {
            let [s] = take1(func, args)?;
            Ok(reits_hist_em(s)?)
        }
        "reits_hist_min_em" => {
            let [s] = take1(func, args)?;
            Ok(reits_hist_min_em(s)?)
        }
        "tool_trade_date_hist_sina" => Ok(tool_trade_date_hist_sina()?),
        "stock_news_em" => {
            let [s] = take1(func, args)?;
            Ok(stock_news_em(s)?)
        }
        "bond_spot_deal" => Ok(bond_spot_deal()?),
        "bond_spot_quote" => Ok(bond_spot_quote()?),
        "bond_china_close_return_map" => Ok(bond_china_close_return_map()?),
        "bond_china_close_return" => {
            let [s, p, d0, d1] = take4(func, args)?;
            Ok(bond_china_close_return(s, p, d0, d1)?)
        }
        "bond_info_detail_cm" => {
            let [s] = take1(func, args)?;
            Ok(bond_info_detail_cm(s)?)
        }
        "bond_info_cm" => {
            let [a, b, c, d, e, f, g, h] = take8(func, args)?;
            Ok(bond_info_cm(a, b, c, d, e, f, g, h)?)
        }
        "bond_info_cm_query" => {
            let [s] = take1(func, args)?;
            Ok(bond_info_cm_query(s)?)
        }
        "bond_cb_jsl" => {
            let [c] = take1(func, args)?;
            Ok(bond_cb_jsl(c)?)
        }
        "bond_cb_redeem_jsl" => Ok(bond_cb_redeem_jsl()?),
        "bond_cb_index_jsl" => Ok(bond_cb_index_jsl()?),
        "bond_cb_adj_logs_jsl" => {
            let [s] = take1(func, args)?;
            Ok(bond_cb_adj_logs_jsl(s)?)
        }
        // 阶段4: 东方财富 eastmoney 债券
        "bond_buy_back_hist_em" => {
            let [s] = take1(func, args)?;
            Ok(bond_buy_back_hist_em(s)?)
        }
        "bond_sh_buy_back_em" => Ok(bond_sh_buy_back_em()?),
        "bond_sz_buy_back_em" => Ok(bond_sz_buy_back_em()?),
        "bond_zh_hs_cov_min" => {
            let [s, p, a, d0, d1] = take5(func, args)?;
            Ok(bond_zh_hs_cov_min(s, p, a, d0, d1)?)
        }
        "bond_zh_hs_cov_pre_min" => {
            let [s] = take1(func, args)?;
            Ok(bond_zh_hs_cov_pre_min(s)?)
        }
        "bond_zh_cov" => Ok(bond_zh_cov()?),
        "bond_zh_cov_info" => {
            let [s, i] = take2(func, args)?;
            Ok(bond_zh_cov_info(s, i)?)
        }
        "bond_zh_cov_value_analysis" => {
            let [s] = take1(func, args)?;
            Ok(bond_zh_cov_value_analysis(s)?)
        }
        "bond_cov_comparison" => Ok(bond_cov_comparison()?),
        "bond_zh_us_rate" => {
            let [s] = take1(func, args)?;
            Ok(bond_zh_us_rate(s)?)
        }
        // 阶段6: BATCH28 bond g_calc（chinabond 中债指数 / 同花顺可转债 / 国债收益率）
        "bond_available_index_cbond" => Ok(bond_available_index_cbond()?),
        "bond_zh_cov_info_ths" => Ok(bond_zh_cov_info_ths()?),
        "bond_china_yield" => {
            let [d0, d1] = take2(func, args)?;
            Ok(bond_china_yield(d0, d1)?)
        }
        "bond_index_general_cbond" => {
            let [a, b, c] = take3(func, args)?;
            Ok(bond_index_general_cbond(a, b, c)?)
        }
        "bond_treasury_index_cbond" => {
            let [a, b] = take2(func, args)?;
            Ok(bond_treasury_index_cbond(a, b)?)
        }
        "bond_new_composite_index_cbond" => {
            let [a, b] = take2(func, args)?;
            Ok(bond_new_composite_index_cbond(a, b)?)
        }
        "bond_composite_index_cbond" => {
            let [a, b] = take2(func, args)?;
            Ok(bond_composite_index_cbond(a, b)?)
        }
        // 阶段5: 新浪 sina 债券
        "bond_gb_zh_sina" => {
            let [s] = take1(func, args)?;
            Ok(bond_gb_zh_sina(s)?)
        }
        "bond_gb_us_sina" => {
            let [s] = take1(func, args)?;
            Ok(bond_gb_us_sina(s)?)
        }
        // 阶段5: 新浪 sina 债券（补充：日 K / 实时 / 可转债详情）
        "bond_zh_hs_daily" => {
            let [s] = take1(func, args)?;
            Ok(bond_zh_hs_daily(s)?)
        }
        "bond_zh_hs_cov_daily" => {
            let [s] = take1(func, args)?;
            Ok(bond_zh_hs_cov_daily(s)?)
        }
        "bond_zh_hs_spot" => {
            let [a, b] = take2(func, args)?;
            Ok(bond_zh_hs_spot(a, b)?)
        }
        "bond_zh_hs_cov_spot" => Ok(bond_zh_hs_cov_spot()?),
        "bond_cb_profile_sina" => {
            let [s] = take1(func, args)?;
            Ok(bond_cb_profile_sina(s)?)
        }
        "bond_cb_summary_sina" => {
            let [s] = take1(func, args)?;
            Ok(bond_cb_summary_sina(s)?)
        }
        // === BATCH5 LONGTAIL (spot/energy/currency/news/fx/fortune) ===
        // ---- spot (搜猪网 / 上海黄金交易所 / 99期货 / 新浪) ----
        "spot_hog_soozhu" => Ok(spot_hog_soozhu()?),
        "spot_hog_year_trend_soozhu" => Ok(spot_hog_year_trend_soozhu()?),
        "spot_hog_lean_price_soozhu" => Ok(spot_hog_lean_price_soozhu()?),
        "spot_hog_three_way_soozhu" => Ok(spot_hog_three_way_soozhu()?),
        "spot_hog_crossbred_soozhu" => Ok(spot_hog_crossbred_soozhu()?),
        "spot_corn_price_soozhu" => Ok(spot_corn_price_soozhu()?),
        "spot_soybean_price_soozhu" => Ok(spot_soybean_price_soozhu()?),
        "spot_mixed_feed_soozhu" => Ok(spot_mixed_feed_soozhu()?),
        "spot_goods" => {
            let [s] = take1(func, args)?;
            Ok(spot_goods(s)?)
        }
        "spot_symbol_table_sge" => Ok(spot_symbol_table_sge()?),
        "spot_golden_benchmark_sge" => Ok(spot_golden_benchmark_sge()?),
        "spot_silver_benchmark_sge" => Ok(spot_silver_benchmark_sge()?),
        "spot_hist_sge" => {
            let [s] = take1(func, args)?;
            Ok(spot_hist_sge(s)?)
        }
        "spot_quotations_sge" => {
            let [s] = take1(func, args)?;
            Ok(spot_quotations_sge(s)?)
        }
        "spot_price_table_qh" => Ok(spot_price_table_qh()?),
        "spot_price_qh" => {
            let [s] = take1(func, args)?;
            Ok(spot_price_qh(s)?)
        }
        // ---- energy (碳排放 / 油价) ----
        "energy_carbon_gz" => Ok(energy_carbon_gz()?),
        "energy_carbon_hb" => Ok(energy_carbon_hb()?),
        "energy_oil_hist" => Ok(energy_oil_hist()?),
        "energy_oil_detail" => {
            let [s] = take1(func, args)?;
            Ok(energy_oil_detail(s)?)
        }
        // ---- currency (新浪中行 / 外汇局) ----
        "currency_boc_sina" => {
            let [s, a, b] = take3(func, args)?;
            Ok(currency_boc_sina(s, a, b)?)
        }
        "currency_boc_safe" => Ok(currency_boc_safe()?),
        // ---- news (百度股市通 / 央视) ----
        "news_cctv" => {
            let [s] = take1(func, args)?;
            Ok(news_cctv(s)?)
        }
        "news_economic_baidu" => {
            let [s] = take1(func, args)?;
            Ok(news_economic_baidu(s)?)
        }
        "news_trade_notify_suspend_baidu" => {
            let [s] = take1(func, args)?;
            Ok(news_trade_notify_suspend_baidu(s)?)
        }
        "news_trade_notify_dividend_baidu" => {
            let [s] = take1(func, args)?;
            Ok(news_trade_notify_dividend_baidu(s)?)
        }
        "news_report_time_baidu" => {
            let [s] = take1(func, args)?;
            Ok(news_report_time_baidu(s)?)
        }
        // ---- fortune (胡润研究院) ----
        "hurun_rank" => {
            let [a, b] = take2(func, args)?;
            Ok(hurun_rank(a, b)?)
        }
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

// take6/take7 为批次4 后续 6/7 参数函数预留的分派辅助（当前阶段尚未用到）。
#[allow(dead_code)]
fn take6<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 6], BoxErr> {
    if args.len() != 6 {
        return Err(format!("{func} 需要 6 个参数, 实际 {}", args.len()).into());
    }
    Ok([
        args[0].as_str(),
        args[1].as_str(),
        args[2].as_str(),
        args[3].as_str(),
        args[4].as_str(),
        args[5].as_str(),
    ])
}

#[allow(dead_code)]
fn take7<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 7], BoxErr> {
    if args.len() != 7 {
        return Err(format!("{func} 需要 7 个参数, 实际 {}", args.len()).into());
    }
    Ok([
        args[0].as_str(),
        args[1].as_str(),
        args[2].as_str(),
        args[3].as_str(),
        args[4].as_str(),
        args[5].as_str(),
        args[6].as_str(),
    ])
}

fn take8<'a>(func: &str, args: &'a [String]) -> Result<[&'a str; 8], BoxErr> {
    if args.len() != 8 {
        return Err(format!("{func} 需要 8 个参数, 实际 {}", args.len()).into());
    }
    Ok([
        args[0].as_str(),
        args[1].as_str(),
        args[2].as_str(),
        args[3].as_str(),
        args[4].as_str(),
        args[5].as_str(),
        args[6].as_str(),
        args[7].as_str(),
    ])
}
