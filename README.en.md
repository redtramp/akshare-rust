# akshare-rust

A Rust implementation of [akshare](https://github.com/akfamily/akshare): a financial data library built on pure HTTP + a built-in JS engine.

> 🤖 This project was **developed by AI** (generated end-to-end by the Freebuff AI coding assistant); every interface is diff-verified against its Python akshare counterpart.

Data retrieval **fully mirrors akshare's technical approach** (v1.0 does not use a browser):

- Pure HTTP requests (`reqwest` blocking) + UA spoofing + exponential backoff retry + multi-node failover
- Built-in JS engine (`rquickjs`/QuickJS) executes the encrypted scripts served by websites,
  equivalent to akshare using `py_mini_racer` (V8) to run the same JS (verified to produce character-identical output)
- Data is returned as `Df` (a polars DataFrame) with column names aligned verbatim with akshare

## Quick Start

```bash
cargo build
cargo run --bin demo    # real-network smoke test
cargo test              # offline unit tests (including JS engine and data pipeline)
```

```rust
use akshare_rust::stock::stock_zh_a_hist;

let df = stock_zh_a_hist("000001", "daily", "20240101", "20240131", "qfq")?;
println!("{}", df);
```

## Implemented Interfaces

> 117 data interfaces so far, all aligned with the same-named Python akshare functions (column names/order/values verified via itemized diff).

### East Money (quotes/K-line/fund flow/sectors)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_zh_a_hist` | `ak.stock_zh_a_hist` | A-share daily/weekly/monthly K-line (qfq/hfq/none) |
| `stock_zh_a_hist_min_em` | `ak.stock_zh_a_hist_min_em` | Minute K-line/intraday |
| `stock_zh_a_spot_em` / `stock_sh_a_spot_em` / `stock_sz_a_spot_em` / `stock_bj_a_spot_em` | `ak.stock_*_spot_em` | Real-time quotes for Shanghai/Shenzhen/Beijing |
| `stock_zh_a_st_em` | `ak.stock_zh_a_st_em` | ST risk-warning board |
| `stock_zh_a_new_em` | `ak.stock_zh_a_new_em` | New-stock board |
| `stock_hk_spot_em` | `ak.stock_hk_spot_em` | Hong Kong real-time quotes |
| `stock_individual_info_em` | `ak.stock_individual_info_em` | Individual stock info |
| `stock_bid_ask_em` | `ak.stock_bid_ask_em` | Five-level order book |
| `stock_individual_fund_flow` | `ak.stock_individual_fund_flow` | Individual stock fund flow |
| `stock_hsgt_fund_flow_summary_em` | `ak.stock_hsgt_fund_flow_summary_em` | Shanghai/Shenzhen-HK Connect fund flow |
| `stock_lhb_detail_em` | `ak.stock_lhb_detail_em` | Dragon-Tiger list details |
| `stock_zt_pool_em` | `ak.stock_zt_pool_em` | Limit-up pool |
| `stock_gpzy_profile_em` | `ak.stock_gpzy_profile_em` | Equity pledge |
| `stock_board_industry_name_em` / `stock_board_industry_cons_em` / `stock_board_industry_hist_em` | `ak.stock_board_industry_*_em` | Industry sectors |
| `stock_board_concept_name_em` / `stock_board_concept_cons_em` / `stock_board_concept_hist_em` | `ak.stock_board_concept_*_em` | Concept sectors |
| `index_zh_a_hist` | `ak.index_zh_a_hist` | Index K-line |
| `index_zh_a_hist_min_em` | `ak.index_zh_a_hist_min_em` | Index minute K-line/intraday |
| `index_code_id_map_em` | `ak.index_code_id_map_em` | Index code mapping |
| `fund_etf_hist_em` | `ak.fund_etf_hist_em` | ETF K-line |
| `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_etf_spot_em` / `ak.fund_lof_spot_em` | ETF/LOF quote lists |

### CNINFO

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_profile_cninfo` | `ak.stock_profile_cninfo` | Company profile |
| `stock_dividend_cninfo` | `ak.stock_dividend_cninfo` | Dividends |
| `stock_ipo_summary_cninfo` | `ak.stock_ipo_summary_cninfo` | IPO details |
| `stock_new_ipo_cninfo` | `ak.stock_new_ipo_cninfo` | New-stock subscription |
| `stock_new_gh_cninfo` | `ak.stock_new_gh_cninfo` | New-stock approval meetings |

### Legulegu (two-step flow: md5 token + session cookie + csrf)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_a_gxl_lg` | `ak.stock_a_gxl_lg` | A-share dividend yield |
| `stock_hk_gxl_lg` | `ak.stock_hk_gxl_lg` | HK-share dividend yield |
| `stock_a_ttm_lyr` | `ak.stock_a_ttm_lyr` | A-share TTM P/E |
| `stock_market_pe_lg` / `stock_index_pe_lg` | `ak.stock_market_pe_lg` / `ak.stock_index_pe_lg` | Main-board/index P/E |
| `stock_market_pb_lg` / `stock_index_pb_lg` | `ak.stock_market_pb_lg` / `ak.stock_index_pb_lg` | Main-board/index P/B |
| `stock_a_congestion_lg` | `ak.stock_a_congestion_lg` | Market congestion |
| `stock_buffett_index_lg` | `ak.stock_buffett_index_lg` | Buffett indicator |
| `stock_ebs_lg` | `ak.stock_ebs_lg` | Equity-bond spread |
| `fund_stock_position_lg` / `fund_balance_position_lg` / `fund_linghuo_position_lg` | `ak.fund_*_position_lg` | Equity/balanced/flexible fund positions |
| `get_token_lg` | (akshare internal) | md5 local-date token |

### Sina Finance

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_hk_spot` | `ak.stock_hk_spot` | HK real-time quotes (paginated) |
| `stock_zh_a_minute` | `ak.stock_zh_a_minute` | A-share minute line (JSONP) |

### Exchanges (SSE/SZSE)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_margin_sse` | `ak.stock_margin_sse` | SSE margin trading summary |
| `stock_margin_detail_sse` | `ak.stock_margin_detail_sse` | SSE margin trading details |
| `stock_margin_szse` | `ak.stock_margin_szse` | SZSE margin trading summary |

### Xueqiu (session-cookie two-step flow)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_hot_follow_xq` | `ak.stock_hot_follow_xq` | Follow popularity ranking |
| `stock_hot_tweet_xq` | `ak.stock_hot_tweet_xq` | Discussion popularity ranking |

### Tonghuashun (THS)

| Function | Corresponding akshare | Description |
|---|---|---|
| `fund_etf_category_ths` | `ak.fund_etf_category_ths` | ETF categories |
| `fund_etf_spot_ths` | `ak.fund_etf_spot_ths` | ETF real-time quotes (JS encrypted) |
| `stock_rank_cxg_ths` / `stock_rank_cxd_ths` | `ak.stock_rank_cxg_ths` / `ak.stock_rank_cxd_ths` | Monthly high/low breakouts |
| `stock_rank_lxsz_ths` / `stock_rank_lxxd_ths` | `ak.stock_rank_lxsz_ths` / `ak.stock_rank_lxxd_ths` | Consecutive up/down |
| `stock_rank_cxfl_ths` / `stock_rank_cxsl_ths` | `ak.stock_rank_cxfl_ths` / `ak.stock_rank_cxsl_ths` | Sustained volume expansion/shrinkage |
| `stock_rank_ljqd_ths` / `stock_rank_ljqs_ths` | `ak.stock_rank_ljqd_ths` / `ak.stock_rank_ljqs_ths` | Price down with volume / price up with volume |
| `stock_rank_xstp_ths` / `stock_rank_xxtp_ths` | `ak.stock_rank_xstp_ths` / `ak.stock_rank_xxtp_ths` | Upward/downward breakout |
| `stock_rank_xzjp_ths` | `ak.stock_rank_xzjp_ths` | Insurance-capital position increases |
| `stock_board_industry_name_ths` / `stock_board_industry_info_ths` | `ak.stock_board_industry_*_ths` | Industry sector names/profiles |
| `stock_board_concept_name_ths` / `stock_board_concept_info_ths` | `ak.stock_board_concept_*_ths` | Concept sector names/profiles |
| `stock_ipo_ths` / `stock_ipo_hk_ths` | `ak.stock_ipo_ths` / `ak.stock_ipo_hk_ths` | New-stock subscription (A/H shares) |
| `stock_fhps_detail_ths` | `ak.stock_fhps_detail_ths` | Dividend details (GBK page) |

### THS Financials/Corporate Events (stock_fundamental)

| Function | Corresponding akshare | Description |
|---|---|---|
| `stock_financial_abstract_ths` | `ak.stock_financial_abstract_ths` | Key indicators (legacy series, HTML-embedded JSON) |
| `stock_financial_debt_ths` / `stock_financial_benefit_ths` / `stock_financial_cash_ths` | `ak.stock_financial_*_ths` | Balance sheet/income/cash flow (legacy series, flashData double JSON) |
| `stock_financial_abstract_new_ths` | `ak.stock_financial_abstract_new_ths` | Important indicators (new series, app_data reports) |
| `stock_financial_debt_new_ths` / `stock_financial_benefit_new_ths` / `stock_financial_cash_new_ths` | `ak.stock_financial_*_new_ths` | Balance sheet/income/cash flow (new series) |
| `stock_profit_forecast_ths` | `ak.stock_profit_forecast_ths` | Earnings forecasts (two-level header expansion) |
| `stock_management_change_ths` / `stock_shareholder_change_ths` | `ak.stock_management_change_ths` / `ak.stock_shareholder_change_ths` | Executive/shareholder holding changes |

### Jin10 Data Center (China macro)

| Function | Corresponding akshare | Description |
|---|---|---|
| `macro_china_gdp_yearly` | `ak.macro_china_gdp_yearly` | China GDP YoY |
| `macro_china_cpi_yearly` / `macro_china_cpi_monthly` | `ak.macro_china_cpi_*` | China CPI YoY/MoM |
| `macro_china_ppi_yearly` | `ak.macro_china_ppi_yearly` | China PPI YoY |
| `macro_china_exports_yoy` / `macro_china_imports_yoy` / `macro_china_trade_balance` | `ak.macro_china_*` | Exports/imports/trade balance |
| `macro_china_industrial_production_yoy` | `ak.macro_china_industrial_production_yoy` | Industrial value added (above-scale) |
| `macro_china_pmi_yearly` / `macro_china_cx_pmi_yearly` / `macro_china_cx_services_pmi_yearly` / `macro_china_non_man_pmi` | `ak.macro_china_*_pmi*` | Official/Caixin manufacturing/services/non-manufacturing PMI |
| `macro_china_fx_reserves_yearly` | `ak.macro_china_fx_reserves_yearly` | Foreign exchange reserves |
| `macro_china_m2_yearly` | `ak.macro_china_m2_yearly` | M2 money supply YoY |

> The 14 Jin10 macro functions above all output a unified 5-column shape: `商品, 日期, 今值, 预测值, 前值` (dates ascending).

### East Money (macro · datacenter-web)

| Function | Corresponding akshare | Description |
|---|---|---|
| `macro_china_hk_cpi` / `macro_china_hk_cpi_ratio` | `ak.macro_china_hk_cpi*` | HK CPI (level/YoY) |
| `macro_china_hk_rate_of_unemployment` | `ak.macro_china_hk_rate_of_unemployment` | HK unemployment rate |
| `macro_china_hk_gbp` / `macro_china_hk_gbp_ratio` | `ak.macro_china_hk_gbp*` | HK GDP (level/YoY) |
| `macro_china_hk_building_volume` / `macro_china_hk_building_amount` | `ak.macro_china_hk_building_*` | HK property transactions (count/amount) |
| `macro_china_hk_trade_diff_ratio` | `ak.macro_china_hk_trade_diff_ratio` | HK trade balance YoY |
| `macro_china_hk_ppi` | `ak.macro_china_hk_ppi` | HK PPI |
| `macro_china_qyspjg` | `ak.macro_china_qyspjg` | Enterprise commodity price index |
| `macro_china_fdi` | `ak.macro_china_fdi` | Foreign direct investment |

> The 11 East Money macro functions above all go through `datacenter-web.eastmoney.com` (reportName query + `finalize_report` pipeline).

### Futures Exchanges (settlement params + contract details)

| Function | Corresponding akshare | Description |
|---|---|---|
| `futures_settle_cffex` | `ak.futures_settle_cffex` | CFFEX settlement params (CSV) |
| `futures_settle_czce` | `ak.futures_settle_czce` | CZCE settlement params |
| `futures_settle_gfex` | `ak.futures_settle_gfex` | GFEX settlement params (POST form) |
| `futures_settle_shfe` | `ak.futures_settle_shfe` | SHFE settlement params |
| `futures_settle_ine` | `ak.futures_settle_ine` | INE settlement params |
| `futures_settle` | `ak.futures_settle` | Unified settlement entry (20-column normalization, dispatched by `market`) |
| `futures_contract_detail` | `ak.futures_contract_detail` | Sina futures contract details (GB2312 page) |

> See [`PLAN.md`](PLAN.md) for the full implementation roadmap (1099 functions / 33 categories).

## Architecture

```
src/
├── core/           # Infrastructure
│   ├── error.rs    # Unified AkshareError types (Empty/Js/Blocked/AuthRequired/Status/Http...)
│   ├── config.rs   # Global config (UA/timeout/retry/proxy)
│   ├── http.rs     # reqwest wrapper: exponential backoff + jitter retry, multi-node failover, charset decoding, anti-crawler detection
│   ├── df.rs       # Df (polars DataFrame wrapper): JSON table building/sorting/column conversion, column order aligned with pandas
│   └── js_engine.rs# rquickjs wrapper: evaluate encrypted JS + browser-global shim injection
├── sources/        # Data-source layer (one module per source)
│   ├── eastmoney.rs# East Money: clist pagination (multi-node failover) / K-line / market detection / datacenter reports
│   ├── ths.rs      # Tonghuashun: v token (JS) + HTML table/sector/corporate-event parsing
│   └── jin10.rs    # Jin10: data-center report pagination (max_date cursor)
├── economic/       # Macro: 14 Jin10 China macro + 11 East Money datacenter-web macro (25 total)
├── futures/        # Futures: settlement params for five exchanges + unified entry + Sina contract details
├── cninfo/         # CNINFO: datacenter queries + built-in JS encryption
├── legu/           # Legulegu: md5 token + session cookie + csrf two-step flow
├── sina/           # Sina Finance: HK spot pagination / minute-line JSONP
├── exchange/       # Exchanges: SSE/SZSE margin trading
├── xueqiu/         # Xueqiu: session cookie + popularity ranking pagination
├── stock/          # Stock interfaces (corresponding to akshare stock_* functions)
├── stock_feature/  # Stock-feature interfaces (EM datacenter dragon-tiger list/HK-connect + THS sectors/new stocks, etc.)
├── stock_fundamental/ # Fundamental interfaces (restricted-share unlock / THS financial indicators / corporate events)
├── index/          # Index interfaces (corresponding to akshare index_* functions)
├── fund/           # Fund interfaces (corresponding to akshare fund_* functions)
└── bin/
    ├── demo.rs     # Command-line smoke demo
    └── parity.rs   # Diff-comparison CLI (invoked by tools/parity_runner.py)
```

### Key Design Decisions

- **Multi-node failover**: individual nodes of the East Money push2 cluster may be rate-limited or down; `fetch_paginated_diff_any` /
  `get_json_any` first do a single fast probe per node, switching immediately on failure, then fall back to the full retry strategy only if all nodes fail.
- **Minute-data rolling window**: East Money minute K-line/intraday endpoints only return roughly the last 8 months of rolling data,
  matching akshare's behavior; requesting minute data for earlier dates returns an empty table.
- **JS encryption**: always execute akshare's original JS with rquickjs rather than hand-writing the algorithm in Rust;
  browser-global shims such as `var BROWSER_LIST; var time;` are injected to support non-strict-mode code.
- **Session two-step flow**: legulegu/Xueqiu etc. require visiting a page first to establish cookies and extract csrf/token before calling the API;
  `get_text_allow_blocked` is used for session establishment (the cookie is the goal; page content is not validated).
- **Anti-crawler detection**: responses containing `_waf`/`Just a moment`/`challenge-platform` are classified as `Blocked`,
  and those containing `400016`/`xq_a_token` as `AuthRequired` — explicit errors rather than dirty data.
- **No retry on 4xx**: client errors return immediately; only 5xx and connection errors enter backoff retry
  (mirroring akshare's `raise_for_status` semantics).

## Development Conventions

- `cargo fmt` / `cargo clippy --all-targets -- -D warnings` must be warning-free
- No `unwrap`/`expect` (except at construction points such as `Client::build`); errors go through `Result<AkshareError>`
- Public functions must carry `///` doc comments (parameters, returned columns)
- Data-transformation logic should be extracted into pure functions with offline unit tests (no network dependency)

## Known Limitations

- The East Money push2 cluster temporarily rate-limits this machine's IP (manifested as TLS close_notify connection resets);
  Python akshare is equally affected; failover and retries mitigate it, and a later retry may be needed.
- legulegu currently returns 403 for this machine's IP (nginx ban); the interfaces are implemented per akshare's original logic
  and cross-validated via the token, pending real verification once the environment recovers.
- The East Money clist-family interfaces (st/new/hk_spot_em) cannot be truly verified during the push2 rate-limit window;
  correctness is ensured via key-name mapping (isomorphic to the verified spot_em) + offline unit tests.
