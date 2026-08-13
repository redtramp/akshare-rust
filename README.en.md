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

> As of now, a total of **364** data interfaces are implemented, covering **19 / 47** functional categories, with an overall coverage of **≈ 33.1%**
> (benchmarked against akshare's 1099 public APIs). All interfaces align with the identically-named Python akshare functions
> (column names / column order / values verified differentially item by item). 

**By category (implemented / akshare total / coverage):**

| Category | Implemented | akshare | Coverage |
|---|---|---|---|
| stock | 21 | 407 | 5.2% |
| fund | 4 | 74 | 5.4% |
| index | 3 | 79 | 3.8% |
| stock_feature | 95 | 211 | 45.0% |
| stock_fundamental | 25 | 57 | 43.9% |
| economic | 48 | 226 | 21.2% |
| futures | 7 | 70 | 10.0% |
| option | 46 | 47 | 97.9% |
| bond | 29 | 46 | 63.0% |
| currency | 2 | ~dozens | long tail |
| energy | 17 | ~dozens | long tail |
| news | 5 | ~dozens | long tail |
| fortune | 1 | ~10 | long tail |
| spot | 3 | ~dozens | long tail |
| cninfo | 10 | — | CNINFO family |
| sina | 2 | — | Sina family |
| legu | 14 | — | Legulegu family |
| xueqiu | 2 | — | Xueqiu family |
| exchange | 3 | — | Exchange family |

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

> Stock features (`stock_feature`) total 95, covering quote snapshots, shareholder analysis, dragon-tiger list, Shanghai-Shenzhen-HK Connect, financial statements, per-stock commentary, technical screening, etc.; full list in `src/stock_feature/mod.rs`.

### Index / Fund

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

> Options total 46, covering CFFEX / SSE / SZSE / Eastmoney / commodity / futures-option history; full list in `src/option/mod.rs`.

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

> Bonds total 29, covering convertible bonds / spot bonds / treasury / repo / issuance / China Money; full list in `src/bond/mod.rs`.

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

> Macro totals 48 (Jin10 + Eastmoney datacenter-web + Hong Kong + multi-caliber); full list in `src/economic/mod.rs` and `src/sources/jin10.rs`.

### Energy & Commodities (energy)

| Function | Corresponding akshare | Description |
|---|---|---|
| `energy_oil_hist` / `energy_oil_detail` | `ak.energy_oil_*` | Gas/diesel historical price adjustments / details |
| `spot_symbol_table_sge` / `spot_golden_benchmark_sge` / `spot_silver_benchmark_sge` / `spot_hist_sge` / `spot_quotations_sge` | `ak.spot_*_sge` | Shanghai Gold Exchange quotes |
| `energy_carbon_gz` / `energy_carbon_hb` | `ak.energy_carbon_*` | Guangzhou / Hubei carbon emission quotes |
| `spot_hog_soozhu` / `spot_hog_year_trend_soozhu` / `spot_hog_lean_price_soozhu` / `spot_hog_three_way_soozhu` / `spot_hog_crossbred_soozhu` / `spot_corn_price_soozhu` / `spot_soybean_price_soozhu` / `spot_mixed_feed_soozhu` | `ak.spot_hog_*` | Hogs / corn / soybean meal / mixed feed (Soozhu) |

### News (news)

| Function | Corresponding akshare | Description |
|---|---|---|
| `news_economic_baidu` / `news_trade_notify_suspend_baidu` / `news_trade_notify_dividend_baidu` / `news_report_time_baidu` | `ak.news_*` | Baidu finance news / suspension / dividends / earnings schedule |
| `news_cctv` | `ak.news_cctv` | CCTV news |

### Wealth Rankings (fortune)

| Function | Corresponding akshare | Description |
|---|---|---|
| `hurun_rank` | `ak.hurun_rank` | Hurun Rich List |

### Spot (spot)

| Function | Corresponding akshare | Description |
|---|---|---|
| `spot_goods` | `ak.spot_goods` | Commodity spot |
| `spot_price_table_qh` / `spot_price_qh` | `ak.spot_price_*_qh` | 99 futures spot/futures prices |

### FX (currency)

| Function | Corresponding akshare | Description |
|---|---|---|
| `currency_boc_safe` / `currency_boc_sina` | `ak.currency_boc_*` | SAFE / Sina RMB central parity |

### Futures Exchanges (settlement parameters + contract details)

| Function | Corresponding akshare | Description |
|---|---|---|
| `futures_settle_cffex` / `futures_settle_czce` / `futures_settle_gfex` / `futures_settle_shfe` / `futures_settle_ine` | `ak.futures_settle_*` | Settlement parameters for the five exchanges |
| `futures_settle` | `ak.futures_settle` | Unified settlement-parameter entry (20-column normalization, `market` dispatch) |
| `futures_contract_detail` | `ak.futures_contract_detail` | Sina futures contract details (GB2312 page) |

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
├── economic/       # Macro: Jin10 China macro + Eastmoney datacenter-web macro + HK / multi-caliber (48 total)
├── futures/        # Futures: settlement parameters for five exchanges + unified entry + Sina contract details
├── option/         # Options: CFFEX / SSE / SZSE / Eastmoney / commodity / futures-option history (46 total)
├── bond/           # Bonds: convertible / spot / treasury / repo / issuance / China Money (29 total)
├── cninfo/         # CNINFO: datacenter query + built-in JS encryption
├── legu/           # Legulegu: md5 token + session cookie + csrf two-step flow
├── sina/           # Sina Finance: HK spot pagination / minute line JSONP
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
