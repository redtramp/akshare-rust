# akshare-rust

Rust port of [akshare](https://github.com/akfamily/akshare): a financial data library that fetches data via pure HTTP plus a built-in JS engine.

> 🤖 This project is **AI-developed**; every interface is verified against the identically-named Python akshare functions via item-by-item differential reconciliation.
> 📘 Docs: [中文](README.md) · [Changelog CHANGELOG.md](CHANGELOG.md)

Data fetching **fully mirrors akshare's technical implementation** (no browser in v1.0):

- Pure HTTP requests (`reqwest` blocking) + UA spoofing + exponential-backoff retries + multi-node failover
- A built-in JS engine (`rquickjs`/QuickJS) runs the encrypted scripts served by the websites,
  equivalent to akshare running the same JS with `py_mini_racer` (V8) (verified to produce byte-for-byte identical output)
- Data is returned as `Df` (polars DataFrame), with column names aligned character-by-character with akshare

## Quick Start

```bash
cargo build
cargo run --bin demo    # live network smoke test
cargo test              # offline unit tests (incl. JS engine and data pipeline)
```

```rust
use akshare_rust::stock::stock_zh_a_hist;

let df = stock_zh_a_hist("000001", "daily", "20240101", "20240131", "qfq")?;
println!("{}", df);
```

## Implemented Interfaces

> As of now, a total of **899** data interfaces are implemented, covering **21 / 35** functional categories, with an overall coverage of **≈ 83.2%**
> (benchmarked against the **1080** identically-named public functions in akshare **1.18.83**; earlier docs used a 1131 baseline).
> All interfaces align with the identically-named Python akshare functions (column names / column order / values verified differentially item by item).

**By category (implemented / akshare total / coverage):**

> Categories follow akshare's source modules (public callables from `dir(akshare)` classified via `inspect.getmodule`).

| Category | Implemented | akshare | Coverage |
|---|---|---|---|
| economic | 224 | 225 | 99.6% |
| stock_feature | 168 | 208 | 80.8% |
| stock | 122 | 125 | 97.6% |
| index | 78 | 94 | 83.0% |
| fund | 68 | 88 | 77.3% |
| futures | 45 | 69 | 65.2% |
| stock_fundamental | 43 | 57 | 75.4% |
| option | 44 | 44 | 100.0% |
| bond | 39 | 42 | 92.9% |
| spot | 15 | 15 | 100.0% |
| futures_derivative | 10 | 13 | 76.9% |
| movie | 8 | 12 | 66.7% |
| energy | 8 | 8 | 100.0% |
| other | 0 | 8 | 0.0% |
| qhkc_web | 0 | 8 | 0.0% |
| air | 0 | 7 | 0.0% |
| article | 0 | 7 | 0.0% |
| currency | 2 | 7 | 28.6% |
| fx | 5 | 6 | 83.3% |
| news | 6 | 6 | 100.0% |
| fortune | 1 | 5 | 20.0% |
| cal | 0 | 3 | 0.0% |
| qdii | 0 | 3 | 0.0% |
| reits | 3 | 3 | 100.0% |
| crypto | 0 | 2 | 0.0% |
| event | 0 | 2 | 0.0% |
| forex | 2 | 2 | 100.0% |
| nlp | 0 | 2 | 0.0% |
| rate | 0 | 2 | 0.0% |
| utils | 0 | 2 | 0.0% |
| bank | 0 | 1 | 0.0% |
| hf | 0 | 1 | 0.0% |
| interest_rate | 1 | 1 | 100.0% |
| pro | 0 | 1 | 0.0% |
| tool | 1 | 1 | 100.0% |

> Note: project groupings such as `cninfo` / `legu` / `xueqiu` / `exchange` / `sina` / `ths`
> are counted inside the matching akshare module above (e.g. CNINFO functions land in
> `stock`/`bond`; THS technical screening lands in `stock_feature`). `stock` (125) +
> `stock_feature` (208) + `stock_fundamental` (57) together form the former "stock" super-category.

> The interfaces below are listed by data source; the full function list for each category is in the corresponding `src/` module.

### Stock (Eastmoney quotes / K-line / funds / sectors / dragon-tiger list / Shanghai-Shenzhen-HK Connect)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_zh_a_hist` | `ak.stock_zh_a_hist` | A-share daily/weekly/monthly K-line (forward/backward/non-adjusted) |
| `stock_zh_a_hist_min_em` | `ak.stock_zh_a_hist_min_em` | Minute K-line / time-sharing |
| `stock_zh_a_spot_em` / `stock_sh_a_spot_em` / `stock_sz_a_spot_em` / `stock_bj_a_spot_em` | `ak.stock_*_spot_em` | Shanghai/Shenzhen/Beijing real-time quotes |
| `stock_cy_a_spot_em` / `stock_kc_a_spot_em` / `stock_zh_b_spot_em` / `stock_new_a_spot_em` | `ak.stock_*_spot_em` | ChiNext / STAR Market / B-share / new-stock real-time quotes |
| `stock_hk_spot_em` / `stock_hk_main_board_spot_em` / `stock_hk_ggt_components_em` | `ak.stock_hk_*_spot_em` | HK real-time / main board / Stock Connect constituents |
| `stock_zh_a_st_em` | `ak.stock_zh_a_st_em` | ST risk-warning board |
| `stock_zh_a_new_em` | `ak.stock_zh_a_new_em` | New-stock board |
| `stock_individual_info_em` / `stock_bid_ask_em` | `ak.stock_*` | Individual stock info / level-5 order book |
| `stock_individual_fund_flow` / `stock_hsgt_fund_flow_summary_em` | `ak.stock_*` | Individual stock / Shanghai-Shenzhen-HK Connect fund flows |
| `stock_lhb_detail_em` and dragon-tiger series (`stock_lhb_jgstatistic_em` / `stock_lhb_hyyyb_em` / `stock_lhb_yybph_em` / `stock_lhb_stock_detail_em` / …) | `ak.stock_lhb_*` | Dragon-tiger details / brokerages / institutions / individual stocks |
| `stock_zt_pool_em` | `ak.stock_zt_pool_em` | Limit-up stock pool |
| `stock_gpzy_profile_em` / `stock_gpzy_pledge_ratio_detail_em` / `stock_gpzy_individual_pledge_ratio_detail_em` | `ak.stock_gpzy_*` | Equity pledge |
| `stock_board_industry_name_em` / `_cons_em` / `_hist_em` | `ak.stock_board_industry_*_em` | Industry sectors |
| `stock_board_concept_name_em` / `_cons_em` / `_hist_em` | `ak.stock_board_concept_*_em` | Concept sectors |
| `stock_hsgt_hold_stock_em` / `_hist_em` / `_board_rank_em` / `_individual_em` / `_institution_statistics_em` | `ak.stock_hsgt_*` | Shanghai-Shenzhen-HK Connect holdings / history / rankings |
| `stock_jgdy_tj_em` / `_detail_em` / `stock_fhps_em` / `stock_tfp_em` / `stock_pg_em` / `stock_account_statistics_em` | `ak.stock_*` | Institutional research / dividends / trading suspension / rights issuance |
| `stock_yjbb_em` / `_yjkb_em` / `_yjyg_em` / `_yysj_em` | `ak.stock_*` | Earnings reports / express / forecasts / disclosure schedule |
| `stock_zcfz_em` / `_bj_em` / `stock_lrb_em` / `stock_xjll_em` | `ak.stock_*` | Financial statements (balance / income / cash flow) |
| `stock_comment_em` / `stock_comment_detail_*` / `stock_rank_*_ths` | `ak.stock_*` | Per-stock commentary / technical stock screening |
| `stock_xgsglb_em` / `stock_analyst_rank_em` / `stock_analyst_detail_em` | `ak.stock_*` | New-stock subscriptions / analyst indices |

> Stock features (`stock_feature`) total ~130, covering quote snapshots, shareholder analysis, dragon-tiger list, Shanghai-Shenzhen-HK Connect, financial statements, per-stock commentary, technical screening, etc.; full list in `src/stock_feature/mod.rs`.

### Index / Fund

> Index 50, fund 55.

| Function | Corresponding akshare | Description |
|---|---|---|
| `index_zh_a_hist` / `index_zh_a_hist_min_em` / `index_code_id_map_em` | `ak.index_*` | A-share index K-line / minute line / code mapping |
| `index_all_cni` / `index_hist_cni` / `index_detail_cni` / `index_detail_hist_cni` / `index_detail_hist_adjust_cni` | `ak.index_*_cni` | CSI index list / history / details |
| `index_hist_sw` / `index_min_sw` / `index_realtime_sw` / `index_analysis_*_sw` / `index_component_sw` / `index_hist_fund_sw` | `ak.index_*_sw` | SW (Shenwan Hongyuan) index |
| `sw_index_first_info` / `sw_index_second_info` / `sw_index_third_info` / `sw_index_third_cons` | `ak.sw_index_*` | Legulegu SW level-1/2/3 industry classification & constituents |
| `index_pmi_*_cx` / `index_dei_cx` / `index_ii_cx` / `index_si_cx` etc. (19 total) | `ak.index_*_cx` | Caixin PMI / composite index (19 types) |
| `index_price_cflp` / `index_volume_cflp` | `ak.index_price_*` / `ak.index_volume_*` | CFLP freight rate / volume index |
| `index_sugar_msweet` / `index_inner_quote_sugar_msweet` / `index_outer_quote_sugar_msweet` | `ak.index_*_sugar_msweet` | MuTian sugar index |
| `index_stock_cons` | `ak.index_stock_cons` | Sina latest index constituents |
| `index_stock_cons_csindex` / `index_stock_cons_weight_csindex` | `ak.index_stock_cons_*` | CSI constituents / weights |
| `index_stock_cons_sina` / `index_stock_info` / `index_global_*` series | `ak.index_*` | Index list / global indices |
| `fund_etf_hist_em` / `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_*` | ETF/LOF K-line / quotes |
| `fund_etf_hist_min_em` / `fund_lof_hist_em` / `fund_lof_hist_min_em` | `ak.fund_*` | ETF/LOF minute K-line |
| `fund_etf_category_ths` / `fund_etf_spot_ths` | `ak.fund_*_ths` | ETF categories / real-time (JS encryption) |
| `fund_etf_category_sina` / `fund_etf_hist_sina` | `ak.fund_*_sina` | Sina ETF category / daily K-line |
| `fund_open_fund_rank_em` / `fund_exchange_rank_em` / `fund_money_rank_em` / `fund_lcx_rank_em` | `ak.fund_*_rank_*` | Fund rankings (open/exchange/money/wealth) |
| `fund_open_fund_daily_em` / `fund_money_fund_daily_em` / `fund_financial_fund_daily_em` | `ak.fund_*_daily_*` | Fund NAV list |
| `fund_fh_em` / `fund_cf_em` | `ak.fund_fh_*` / `ak.fund_cf_*` | Fund dividends / splits |
| `fund_fh_rank_em` | `ak.fund_fh_rank_em` | Fund dividend ranking |
| `fund_purchase_em` | `ak.fund_purchase_em` | Fund subscription status |
| `fund_scale_change_em` / `fund_hold_structure_em` | `ak.fund_scale_*` / `ak.fund_hold_*` | Fund scale change / hold structure |
| `fund_scale_daily_szse` | `ak.fund_scale_daily_szse` | SZSE fund scale daily |
| `fund_scale_open_sina` / `fund_scale_close_sina` / `fund_scale_structured_sina` | `ak.fund_scale_*_sina` | Sina fund scale (open/closed/structured) |
| `fund_aum_em` / `fund_aum_hist_em` / `fund_aum_trend_em` | `ak.fund_aum_*` | Fund company AUM |
| `fund_etf_scale_sse` / `fund_etf_scale_szse` | `ak.fund_etf_scale_*` | Exchange ETF scale |
| `fund_portfolio_hold_em` / `fund_portfolio_bond_hold_em` / `fund_portfolio_industry_allocation_em` | `ak.fund_portfolio_*` | Fund portfolio holdings |
| `fund_money_fund_info_em` / `fund_etf_fund_info_em` / `fund_graded_fund_info_em` | `ak.fund_*_info_*` | Fund historical NAV details |
| `fund_rating_all` / `fund_rating_sh` / `fund_rating_zs` / `fund_rating_ja` | `ak.fund_rating_*` | Fund rating |
| `fund_new_found_em` / `fund_new_found_ths` | `ak.fund_new_found_*` | New fund launches |
| `fund_announcement_dividend_em` / `fund_announcement_report_em` / `fund_announcement_personnel_em` | `ak.fund_announcement_*` | Fund announcements |
| `fund_value_estimation_em` | `ak.fund_value_estimation_em` | Fund NAV estimation |
| `fund_name_em` | `ak.fund_name_em` | Fund name list |
| `fund_hk_rank_em` / `fund_hk_fund_hist_em` | `ak.fund_hk_*` | HK fund ranking / historical NAV |
| `fund_manager_em` / `fund_overview_em` / `fund_info_ths` / `fund_fee_em` / `fund_info_index_em` | `ak.fund_*` | Manager list / fund profile / THS info / purchase fee / index funds |
| `fund_report_stock_cninfo` / `fund_etf_fund_daily_em` / `fund_graded_fund_daily_em` / `fund_financial_fund_info_em` / `fund_etf_dividend_sina` | `ak.fund_*` | CNINFO heavy holdings / exchange NAV / graded NAV / financial-fund NAV / ETF dividend |

| Function | Corresponding akshare | Description |
|---|---|---|
| `index_zh_a_hist` / `index_zh_a_hist_min_em` / `index_code_id_map_em` | `ak.index_*` | Index K-line / minute line / code mapping |
| `fund_etf_hist_em` / `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_*` | ETF/LOF K-line / quotes |
| `fund_etf_category_ths` / `fund_etf_spot_ths` | `ak.fund_*_ths` | ETF categories / real-time quotes (JS encryption) |

### CNINFO (cninfo)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_profile_cninfo` / `stock_dividend_cninfo` / `stock_ipo_summary_cninfo` / `stock_new_ipo_cninfo` / `stock_new_gh_cninfo` | `ak.stock_*` | Company profile / dividends / IPO / new-stock approval |
| `bond_treasure_issue_cninfo` / `bond_local_government_issue_cninfo` / `bond_corporate_issue_cninfo` / `bond_cov_issue_cninfo` / `bond_cov_stock_issue_cninfo` | `ak.bond_*` | Treasury / local / corporate / convertible bond issuance |

### Legulegu (legulegu, two-step flow: md5 token + session cookie + csrf)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_a_gxl_lg` / `stock_hk_gxl_lg` / `stock_a_ttm_lyr` | `ak.stock_*` | A-share/HK dividend yield / TTM P/E |
| `stock_market_pe_lg` / `stock_index_pe_lg` / `stock_market_pb_lg` / `stock_index_pb_lg` | `ak.stock_*` | Main board / index P/E / P/B |
| `stock_a_congestion_lg` / `stock_buffett_index_lg` / `stock_ebs_lg` | `ak.stock_*` | Market congestion / Buffett indicator / equity-bond spread |
| `fund_stock_position_lg` / `fund_balance_position_lg` / `fund_linghuo_position_lg` | `ak.fund_*` | Fund positions |
| `get_token_lg` | (akshare internal) | md5 local-date token |

### Sina Finance

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_hk_spot` | `ak.stock_hk_spot` | HK real-time quotes (paginated) |
| `stock_zh_a_minute` | `ak.stock_zh_a_minute` | A-share minute line (JSONP) |

### Exchanges (SSE / SZSE)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_margin_sse` / `stock_margin_detail_sse` / `stock_margin_szse` | `ak.stock_margin_*` | Margin trading summary / detail |

### Xueqiu (session cookie two-step flow)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_hot_follow_xq` / `stock_hot_tweet_xq` | `ak.stock_hot_*` | Follow / discussion heat rankings |
| `stock_individual_basic_info_xq` / `_hk_xq` / `_us_xq` | `ak.stock_individual_basic_info_*` | Individual stock basic info |

### THS (Tonghuashun)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_rank_cxg_ths` / `_cxd_ths` / `_lxsz_ths` / `_lxxd_ths` / `_cxfl_ths` / `_cxsl_ths` / `_xstp_ths` / `_xxtp_ths` / `_ljqs_ths` / `_ljqd_ths` / `_xzjp_ths` | `ak.stock_rank_*_ths` | Technical screening (new high/low, consecutive up/down, volume up/down, breakout, takeover) |
| `stock_board_industry_name_ths` / `_info_ths` / `stock_board_concept_name_ths` / `_info_ths` | `ak.stock_board_*_ths` | Industry / concept sectors |
| `stock_ipo_ths` / `stock_ipo_hk_ths` / `stock_fhps_detail_ths` | `ak.stock_*` | New-stock subscriptions / dividend details |

### THS Financial / Company Events (stock_fundamental)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_restricted_release_summary_em` / `_detail_em` / `_queue_em` / `_stockholder_em` | `ak.stock_restricted_release_*` | Restricted-share unlocking |
| `stock_financial_abstract_ths` / `_debt_ths` / `_benefit_ths` / `_cash_ths` | `ak.stock_financial_*_ths` | Financial indicators (old series) |
| `stock_financial_abstract_new_ths` / `_debt_new_ths` / `_benefit_new_ths` / `_cash_new_ths` | `ak.stock_financial_*_new_ths` | Financial indicators (new series) |
| `stock_profit_forecast_ths` / `stock_management_change_ths` / `stock_shareholder_change_ths` | `ak.stock_*` | Profit forecast / executives / shareholder holding changes |
| `stock_dzjy_hygtj` / `_hyybtj` / `_mrmx` / `_mrtj` / `_sctj` / `_yybph` | `ak.stock_dzjy_*` | Block trade statistics |

### Options (option)

| Function | Corresponding akshare | Description |
|---|---|---|
| `option_cffex_hs` / `_sz` / `_zz` | `ak.option_cffex_*` | CFFEX options (CSI 300 / CSI 500 / CSI 1000) |
| `option_sse_list_sina` / `option_sse_codes_sina` / `option_sse_expire_day_sina` | `ak.option_sse_*` | SSE option list / codes / expiry |
| `option_sse_spot_price_sina` / `option_sse_underlying_spot_price_sina` / `option_sse_greeks_sina` / `option_sse_minute_sina` / `option_sse_daily_sina` | `ak.option_sse_*` | SSE option real-time / underlying / greeks / minute / daily |
| `option_finance_sse_underlying` / `option_finance_board` | `ak.option_finance_*` | SSE ETF option underlying / board |
| `option_current_day_sse` / `option_current_day_szse` / `option_daily_stats_sse` / `option_daily_stats_szse` / `option_risk_indicator_sse` | `ak.option_*` | SSE/SZSE option current-day / daily stats / risk indicators |
| `option_current_em` / `option_minute_em` / `option_premium_analysis_em` / `option_risk_analysis_em` / `option_value_analysis_em` / `option_lhb_em` | `ak.option_*_em` | Eastmoney option real-time / minute / premium / risk / value / dragon-tiger |
| `option_commodity_hist_sina` / `option_commodity_contract_sina` / `option_commodity_contract_table_sina` / `option_comm_info` / `option_comm_symbol` / `option_margin` / `option_margin_symbol` | `ak.option_commodity_*` | Commodity option history / contract / margin |
| `option_hist_czce` / `option_hist_yearly_czce` / `option_hist_dce` / `option_hist_gfex` / `option_hist_shfe` / `option_vol_shfe` / `option_vol_gfex` | `ak.option_hist_*` | Futures-option history (CZCE / DCE / GFEX / SHFE) |
| `option_contract_info_ctp` | `ak.option_contract_info_ctp` | CTP option contract info |

> Options total 28, covering CFFEX / SSE / SZSE / Eastmoney / commodity / futures-option history; full list in `src/option/mod.rs`.

### Bonds (bond)

| Function | Corresponding akshare | Description |
|---|---|---|
| `bond_cb_jsl` / `bond_cb_redeem_jsl` / `bond_cb_index_jsl` / `bond_cb_adj_logs_jsl` | `ak.bond_cb_*_jsl` | Jisilu convertible bond list / forced redemption / equal-weight index / conversion-price adjustment |
| `bond_cb_profile_sina` / `bond_cb_summary_sina` | `ak.bond_cb_*_sina` | Convertible bond details / profile (Sina) |
| `bond_spot_deal` / `bond_spot_quote` | `ak.bond_spot_*` | Spot bond trading / dealer quotes |
| `bond_china_close_return` / `bond_china_close_return_map` | `ak.bond_china_close_return*` | Closing yield curve |
| `bond_zh_hs_daily` / `bond_zh_hs_spot` / `bond_zh_hs_cov_daily` / `bond_zh_hs_cov_spot` / `bond_zh_hs_cov_min` / `bond_zh_hs_cov_pre_min` | `ak.bond_zh_hs_*` | Shanghai-Shenzhen bonds / convertible bonds history / real-time / minute |
| `bond_zh_cov` / `bond_zh_cov_info` / `bond_zh_cov_value_analysis` / `bond_cov_comparison` | `ak.bond_zh_cov*` | Convertible bond data / details / value analysis / comparison |
| `bond_zh_us_rate` / `bond_gb_zh_sina` / `bond_gb_us_sina` | `ak.bond_*_rate` / `ak.bond_gb_*` | China-US treasury yields |
| `bond_buy_back_hist_em` / `bond_sh_buy_back_em` / `bond_sz_buy_back_em` | `ak.bond_*_buy_back_*` | Pledged repo |
| `bond_info_cm` / `bond_info_detail_cm` / `bond_info_cm_query` | `ak.bond_info_cm*` | China Money bond query |

> Bonds total 41, covering convertible bonds / spot bonds / treasury / repo / issuance / China Money; full list in `src/bond/mod.rs`.

### Macro (economic)

| Function | Corresponding akshare | Description |
|---|---|---|
| `macro_china_gdp` / `macro_china_gdp_yearly` / `macro_china_cpi` / `macro_china_cpi_yearly` / `macro_china_cpi_monthly` / `macro_china_ppi_yearly` | `ak.macro_china_*` | GDP / CPI / PPI |
| `macro_china_money_supply` / `macro_china_m2_yearly` / `macro_china_lpr` / `macro_china_reserve_requirement_ratio` / `macro_china_shibor_all` | `ak.macro_china_*` | Money supply / M2 / LPR / reserve requirement / SHIBOR |
| `macro_china_pmi` / `macro_china_cx_pmi_yearly` / `macro_china_cx_services_pmi_yearly` / `macro_china_non_man_pmi` | `ak.macro_china_*_pmi*` | Official / Caixin PMI |
| `macro_china_fx_reserves_yearly` / `macro_china_fx_gold` / `macro_china_rmb` | `ak.macro_china_*` | FX reserves / FX position / RMB |
| `macro_china_exports_yoy` / `macro_china_imports_yoy` / `macro_china_trade_balance` / `macro_china_hgjck` | `ak.macro_china_*` | Exports / imports / trade balance |
| `macro_china_hk_cpi` / `macro_china_hk_rate_of_unemployment` / `macro_china_hk_gbp` / `macro_china_hk_ppi` / `macro_china_hk_market_info` | `ak.macro_china_hk_*` | Hong Kong macro |
| `macro_china_qyspjg` / `macro_china_fdi` / `macro_china_new_house_price` / `macro_china_consumer_goods_retail` / `macro_china_stock_market_cap` / `macro_china_daily_energy` / `macro_china_au_report` | `ak.macro_china_*` | Corporate goods price / FDI / house price / consumption / market cap / energy / gold |

> Macro totals 148 (Jin10 + Eastmoney datacenter-web + Hong Kong + multi-caliber + Eurozone + global); full list in `src/economic/mod.rs` and `src/sources/jin10.rs`.mic/mod.rs` and `src/sources/jin10.rs`.

### Energy & Commodities (energy)

| Function | Corresponding akshare | Description |
|---|---|---|
| `energy_oil_hist` / `energy_oil_detail` | `ak.energy_oil_*` | Gas/diesel historical price adjustments / details |
| `spot_symbol_table_sge` / `spot_golden_benchmark_sge` / `spot_silver_benchmark_sge` / `spot_hist_sge` / `spot_quotations_sge` | `ak.spot_*_sge` | Shanghai Gold Exchange quotes |
| `energy_carbon_gz` / `energy_carbon_hb` / `energy_carbon_domestic` / `energy_carbon_bj` / `energy_carbon_sz` / `energy_carbon_eu` | `ak.energy_carbon_*` | Guangzhou / Hubei / Tanjiaoyi (8 pilots) / Beijing / Shenzhen domestic / Shenzhen international carbon quotes |
| `spot_hog_soozhu` / `spot_hog_year_trend_soozhu` / `spot_hog_lean_price_soozhu` / `spot_hog_three_way_soozhu` / `spot_hog_crossbred_soozhu` / `spot_corn_price_soozhu` / `spot_soybean_price_soozhu` / `spot_mixed_feed_soozhu` | `ak.spot_hog_*` | Hogs / corn / soybean meal / mixed feed (Soozhu) |

### News (news)

> Total 5 interfaces (news module 5 + stock module 2 stock-related news).

| Function | Corresponding akshare | Description |
|---|---|---|
| `news_economic_baidu` / `news_trade_notify_suspend_baidu` / `news_trade_notify_dividend_baidu` / `news_report_time_baidu` | `ak.news_*` | Baidu finance news / suspension / dividends / earnings schedule |
| `news_cctv` | `ak.news_cctv` | CCTV news |
| `stock_news_em` | `ak.stock_news_em` | Eastmoney individual stock news (in stock module) |
| `stock_news_main_cx` | `ak.stock_news_main_cx` | Caixin news (in stock module) |

### Wealth Rankings (fortune)

| Function | Corresponding akshare | Description |
|---|---|---|
| `hurun_rank` | `ak.hurun_rank` | Hurun Rich List |

### REITs (real estate investment trusts)

> Total 3 interfaces, all implemented (100%).

| Function | Corresponding akshare | Description |
|---|---|---|
| `reits_realtime_em` | `ak.reits_realtime_em` | Shanghai/Shenzhen REITs real-time quotes |
| `reits_hist_em` | `ak.reits_hist_em` | Shanghai/Shenzhen REITs daily K-line |
| `reits_hist_min_em` | `ak.reits_hist_min_em` | Shanghai/Shenzhen REITs minute K-line |

### Spot (spot)

> Total 16 interfaces covering Shanghai Gold Exchange, Soozhu network, 99 futures, etc.

| Function | Corresponding akshare | Description |
|---|---|---|
| `spot_goods` | `ak.spot_goods` | Commodity spot |
| `spot_price_table_qh` / `spot_price_qh` | `ak.spot_price_*_qh` | 99 futures spot/futures prices |
| `spot_symbol_table_sge` / `spot_golden_benchmark_sge` / `spot_silver_benchmark_sge` | `ak.spot_*_sge` | Shanghai Gold Exchange |
| `spot_hist_sge` / `spot_quotations_sge` | `ak.spot_*_sge` | SGE history / quotes |
| `spot_hog_soozhu` / `spot_hog_year_trend_soozhu` / `spot_hog_lean_price_soozhu` | `ak.spot_hog_*` | Soozhu hog data |
| `spot_hog_three_way_soozhu` / `spot_hog_crossbred_soozhu` | `ak.spot_hog_*` | Soozhu three-way / crossbred |
| `spot_corn_price_soozhu` / `spot_soybean_price_soozhu` / `spot_mixed_feed_soozhu` | `ak.spot_*` | Soozhu corn / soybean meal / feed |

### FX (currency / forex / fx)

> Total 9 interfaces (RMB central parity 2 + forex quotes 2 + forex exchange 5).

| Function | Corresponding akshare | Description |
|---|---|---|
| `currency_boc_safe` / `currency_boc_sina` | `ak.currency_boc_*` | SAFE / Sina RMB central parity |
| `forex_spot_em` / `forex_hist_em` | `ak.forex_*` | Eastmoney forex quotes / history |
| `fx_c_swap_cm` / `fx_quote_baidu` / `fx_spot_quote` / `fx_swap_quote` / `fx_pair_quote` | `ak.fx_*` | China Money / Baidu FX |

### Futures (futures)

> Total 55 interfaces covering settlement parameters for five exchanges, contract details, historical quotes, global futures, and inventory data.

| Function | Corresponding akshare | Description |
|---|---|---|
| `futures_settle_cffex` / `futures_settle_czce` / `futures_settle_gfex` / `futures_settle_shfe` / `futures_settle_ine` | `ak.futures_settle_*` | Settlement parameters for the five exchanges |
| `futures_settle` | `ak.futures_settle` | Unified settlement-parameter entry (20-column normalization, `market` dispatch) |
| `futures_contract_detail` | `ak.futures_contract_detail` | Sina futures contract details (GB2312 page) |
| `futures_contract_detail_em` | `ak.futures_contract_detail_em` | Eastmoney futures contract details |
| `futures_contract_info_*` series (6 exchanges) | `ak.futures_contract_info_*` | Contract info for each exchange |
| `futures_warehouse_receipt_*` series (4 exchanges) | `ak.futures_warehouse_receipt_*` | Warehouse receipts for each exchange |
| `futures_delivery_*` / `futures_to_spot_*` series (8) | `ak.futures_delivery_*` / `ak.futures_to_spot_*` | Delivery / forward-to-spot |
| `futures_hist_daily_cffex` | `ak.futures_hist_daily_cffex` | CFFEX historical daily |
| `futures_hist_em` / `futures_hist_table_em` | `ak.futures_hist_*` | Eastmoney futures historical K-line |
| `futures_global_spot_em` / `futures_global_hist_em` | `ak.futures_global_*` | Eastmoney global futures real-time/history |
| `futures_index_ccidx` | `ak.futures_index_ccidx` | CSI Commodity Index CCIDX |
| `futures_symbol_mark` / `futures_zh_realtime` / `futures_zh_spot` | `ak.futures_*` | Sina variety mapping / real-time quotes |
| `futures_zh_daily_sina` / `futures_zh_minute_sina` | `ak.futures_zh_*` | Sina daily / minute K-line |
| `futures_foreign_commodity_realtime` / `futures_foreign_detail` / `futures_foreign_hist` | `ak.futures_foreign_*` | Global commodity futures real-time/detail/history |
| `futures_comm_info` / `futures_comm_js` / `futures_fees_info` | `ak.futures_*` | Commission / fee tables |
| `futures_news_shmet` | `ak.futures_news_shmet` | Shanghai Metals Market news |
| `futures_inventory_99` / `futures_spot_stock` / `futures_stock_shfe_js` | `ak.futures_*` | Inventory / spot data |
| `futures_rule` | `ak.futures_rule` | Guotai Junan trading calendar |
| `futures_hold_pos_sina` / `futures_main_sina` / `futures_display_main_sina` | `ak.futures_*` | Sina position / main contract |

## Architecture

```
src/
├── core/           # Infrastructure
│   ├── error.rs    # AkshareError unified error type (Empty/Js/Blocked/AuthRequired/Status/Http...)
│   ├── config.rs   # Global config (UA/timeout/retry/proxy)
│   ├── http.rs     # reqwest wrapper: exponential-backoff+jittered retry, multi-node failover, charset decoding, anti-scraping detection
│   ├── df.rs       # Df (polars DataFrame wrapper): JSON table build / sorting / column conversion, column order aligned to pandas
│   ├── html.rs     # HTML table parser (read_html_tables returns 2D strings / read_html returns Vec<Df>)
│   └── js_engine.rs# rquickjs wrapper: eval encrypted JS + inject browser global shims
├── sources/        # Data source layer (one source per module)
│   ├── eastmoney.rs# Eastmoney: clist pagination (multi-node failover) / K-line / market detection / datacenter reports
│   ├── ths.rs      # THS: v token (JS) + HTML table / sector / company-events parsing
│   ├── jin10.rs    # Jin10: datacenter report pagination (max_date cursor)
│   ├── currency_boc.rs # SAFE / Sina RMB central parity
│   ├── oil.rs / sge.rs / carbon.rs # Energy: crude oil / SGE / carbon emissions
│   ├── news_baidu.rs / news_cctv.rs # News
│   ├── hurun.rs    # Hurun rankings
│   ├── soozhu.rs   # Soozhu (hogs / feed)
│   ├── spot_goods.rs / spot_qh.rs # Spot
│   ├── jisilu.rs / chinamoney.rs  # Jisilu / China Money (bonds)
│   └── ...         # Other source modules
├── economic/       # Macro: Jin10 China macro + Eastmoney datacenter-web macro + HK/Eurozone/global (148 total)
├── futures/        # Futures: settlement parameters for five exchanges + contract details + history + global (55 total)
├── option/         # Options: CFFEX / SSE / SZSE / Eastmoney / commodity / futures-option history (28 total)
├── bond/           # Bonds: convertible / spot / treasury / repo / issuance / China Money (41 total)
├── interest_rate/  # Interest rate: SHIBOR (Shanghai Interbank Offered Rate)
├── cninfo/         # CNINFO: datacenter query + built-in JS encryption
├── legu/           # Legulegu: md5 token + session cookie + csrf two-step flow
├── sina/           # Sina Finance: HK spot pagination / minute line JSONP
├── reits/          # REITs: China REITs daily/weekly/monthly K-line / spot
├── exchange/       # Exchanges: SSE / SZSE margin trading
├── xueqiu/         # Xueqiu: session cookie + heat-ranking pagination
├── stock/          # Stock interfaces (corresponding to akshare stock_* functions)
├── stock_feature/  # Stock-feature interfaces (Eastmoney datacenter dragon-tiger / Shanghai-Shenzhen-HK Connect + THS sectors / new stocks, etc., 95 total)
├── stock_fundamental/ # Fundamental interfaces (restricted-share unlocking / THS financial indicators / company events)
├── index/          # Index interfaces (corresponding to akshare index_* functions)
├── fund/           # Fund interfaces (corresponding to akshare fund_* functions)
└── bin/
    ├── demo.rs     # CLI smoke-test demo
    └── parity.rs   # Differential-comparison CLI (invoked by tools/parity_runner.py)
```

### Key Design

- **Multi-node failover**: A single Eastmoney push2 node may be rate-limited / fail; `fetch_paginated_diff_any` /
  `get_json_any` does a single fast probe per node in the first round, switches immediately on failure, and falls back to
  the full retry strategy only after all nodes fail.
- **Minute-level rolling window**: Eastmoney minute K-line / time-sharing interfaces only return roughly the last 8 months
  of rolling data, consistent with akshare behavior; requesting minute data for earlier dates returns an empty table.
- **JS encryption**: Always run akshare's original JS via rquickjs; do not hand-write algorithms in Rust;
  inject browser globals such as `var BROWSER_LIST; var time;` to shim non-strict-mode code.
- **Session two-step flow**: legulegu / xueqiu etc. require visiting a page first to establish a cookie + extract csrf/token
  before calling the API; `get_text_allow_blocked` is used for session establishment (the cookie is the goal, page content is not validated).
- **Anti-scraping detection**: A response containing `_waf` / `Just a moment` / `challenge-platform` is classified as `Blocked`,
  containing `400016` / `xq_a_token` etc. is classified as `AuthRequired` — errors are reported clearly rather than returning dirty data.
- **No retry on 4xx**: Client errors return immediately; only 5xx and connection errors enter backoff retries
  (matching akshare's `raise_for_status` semantics).

## Development Guidelines

- `cargo fmt` / `cargo clippy --all-targets -- -D warnings` must be warning-free
- No `unwrap` / `expect` (except at construction points such as `Client::build`); errors go through `Result<AkshareError>` uniformly
- Public functions must carry `///` doc comments (parameters, returned columns)
- Data-transformation logic is extracted into pure functions with offline unit tests (no network dependency)

## Known Limitations

- The Eastmoney push2 cluster applies temporary rate-limiting to the local IP (manifested as TLS close_notify connection resets),
  which also affects Python akshare; failover and retries mitigate this as much as possible — retry later if necessary.
- legulegu (Legulegu) currently returns 403 (nginx block) for the local IP; the interfaces are implemented per akshare's original
  logic and verified via token cross-checking, pending a real-environment validation once access is restored.
- Eastmoney clist-family interfaces (st/new/hk_spot_em) cannot be validated live within the push2 rate-limiting window;
  correctness is ensured via key-name mapping (structurally identical to the already-verified spot_em) plus offline unit tests.
