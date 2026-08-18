# akshare-rust

Rust 版 [akshare](https://github.com/akfamily/akshare)：纯 HTTP + 内置 JS 引擎的财经数据获取库。

> 🤖 本项目由 **AI 开发**，每个接口均与 Python akshare 同名函数逐项差分对账验证。
> 📘 文档：[English](README.en.md) · [更新日志 CHANGELOG.md](CHANGELOG.md)

数据获取**完整参照 akshare 的技术实现方式**（v1.0 不使用浏览器）：

- 纯 HTTP 请求（`reqwest` blocking）＋ UA 伪装 ＋ 指数退避重试 ＋ 多节点容灾
- 内置 JS 引擎（`rquickjs`/QuickJS）执行网站下发的加密脚本，
  等价于 akshare 用 `py_mini_racer`（V8）执行同一份 JS（已实测输出逐字符一致）
- 数据返回为 `Df`（polars DataFrame），列名与 akshare 逐字对齐

## 快速开始

```bash
cargo build
cargo run --bin demo    # 真实网络冒烟测试
cargo test              # 离线单测（含 JS 引擎与数据管线）
```

```rust
use akshare_rust::stock::stock_zh_a_hist;

let df = stock_zh_a_hist("000001", "daily", "20240101", "20240131", "qfq")?;
println!("{}", df);
```

## 已实现接口

> 截至当前共 **805** 个数据接口，覆盖 **17 / 35** 个功能大类、整体覆盖率 **≈ 71.2%**
> （对标 akshare 公开 API 共 1131 个）。全部接口与 Python akshare 同名函数对齐
> （列名/列序/值逐项差分验证）。

**按大类分布（已实现 / akshare 总数 / 覆盖率）：**

| 大类 | 已实现 | akshare | 覆盖率 |
|---|---|---|---|
| stock | 313 | 407 | 76.9% |
| fund | 55 | 88 | 62.5% |
| index | 50 | 95 | 52.6% |
| economic | 222 | 226 | 98.2% |
| futures | 55 | 70 | 78.6% |
| option | 28 | 47 | 59.6% |
| bond | 41 | 46 | 89.1% |
| spot | 16 | ~数十 | 长尾 |
| news | 5 | ~数十 | 长尾 |
| energy | 4 | ~数十 | 长尾 |
| currency | 2 | ~数十 | 长尾 |
| fx | 5 | 6 | 83.3% |
| forex | 2 | 2 | 100.0% |
| reits | 3 | 3 | 100.0% |
| cninfo | 45 | — | 巨潮系 |
| legu | 8 | — | 乐咕系 |
| xueqiu | 6 | — | 雪球系 |
| exchange | 3 | — | 交易所系 |
| sina | 4 | — | 新浪系 |
| ths | 15 | — | 同花顺系 |
| interest_rate | 1 | 2 | 50.0% |
| fortune | 1 | ~10 | 长尾 |
| tool | 1 | — | 工具 |

> 注：`stock` 大类 313 个含 198 个特色功能（龙虎榜/沪深港通/资金流/板块/ESG 等）+ 62 个核心行情 + 51 个基本面 + 45 个巨潮 + 8 个新浪/乐咕/雪球。非 stock 类 418 个。

> 以下按数据源列出主要接口；每个大类完整函数清单见 `src/` 对应模块。

### 股票（东方财富行情 / K线 / 资金 / 板块 / 龙虎榜 / 沪深港通）

> 共 313 个接口，覆盖 akshare stock 分类约 77%。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_zh_a_hist` | `ak.stock_zh_a_hist` | A 股日/周/月 K 线（前复权/后复权/不复权） |
| `stock_zh_a_hist_min_em` | `ak.stock_zh_a_hist_min_em` | 分钟 K 线/分时 |
| `stock_zh_a_spot_em` / `stock_sh_a_spot_em` / `stock_sz_a_spot_em` / `stock_bj_a_spot_em` | `ak.stock_*_spot_em` | 沪深京实时行情 |
| `stock_cy_a_spot_em` / `stock_kc_a_spot_em` / `stock_zh_b_spot_em` / `stock_new_a_spot_em` | `ak.stock_*_spot_em` | 创业板/科创板/B股/新股实时行情 |
| `stock_hk_spot_em` / `stock_hk_main_board_spot_em` / `stock_hk_ggt_components_em` | `ak.stock_hk_*_spot_em` | 港股实时/主板/港股通成份 |
| `stock_zh_a_st_em` | `ak.stock_zh_a_st_em` | ST 风险警示板 |
| `stock_zh_a_new_em` | `ak.stock_zh_a_new_em` | 新股板块 |
| `stock_individual_info_em` / `stock_bid_ask_em` | `ak.stock_*` | 个股信息 / 五档盘口 |
| `stock_individual_fund_flow` / `stock_hsgt_fund_flow_summary_em` | `ak.stock_*` | 个股/沪深港通资金流向 |
| `stock_lhb_detail_em` 及龙虎榜系列（`stock_lhb_jgstatistic_em` / `stock_lhb_hyyyb_em` / `stock_lhb_yybph_em` / `stock_lhb_stock_detail_em` / …） | `ak.stock_lhb_*` | 龙虎榜详情/营业部/机构/个股 |
| `stock_zt_pool_em` | `ak.stock_zt_pool_em` | 涨停股池 |
| `stock_gpzy_profile_em` / `stock_gpzy_pledge_ratio_detail_em` / `stock_gpzy_individual_pledge_ratio_detail_em` | `ak.stock_gpzy_*` | 股权质押 |
| `stock_board_industry_name_em` / `_cons_em` / `_hist_em` | `ak.stock_board_industry_*_em` | 行业板块 |
| `stock_board_concept_name_em` / `_cons_em` / `_hist_em` | `ak.stock_board_concept_*_em` | 概念板块 |
| `stock_hsgt_hold_stock_em` / `_hist_em` / `_board_rank_em` / `_individual_em` / `_institution_statistics_em` | `ak.stock_hsgt_*` | 沪深港通持股/历史/榜单 |
| `stock_jgdy_tj_em` / `_detail_em` / `stock_fhps_em` / `stock_tfp_em` / `stock_pg_em` / `stock_account_statistics_em` | `ak.stock_*` | 机构调研/分红/停复牌/增发 |
| `stock_yjbb_em` / `_yjkb_em` / `_yjyg_em` / `_yysj_em` | `ak.stock_*` | 业绩报表/快报/预告/预约披露 |
| `stock_zcfz_em` / `_bj_em` / `stock_lrb_em` / `stock_xjll_em` | `ak.stock_*` | 财务报表（资产负债/利润/现金流） |
| `stock_comment_em` / `stock_comment_detail_*` / `stock_rank_*_ths` | `ak.stock_*` | 千股千评/技术选股 |
| `stock_xgsglb_em` / `stock_analyst_rank_em` / `stock_analyst_detail_em` | `ak.stock_*` | 新股申购/分析师指数 |
| `stock_hot_follow_xq` / `stock_hot_tweet_xq` | `ak.stock_hot_*_xq` | 雪球关注/讨论热度 |
| `stock_individual_basic_info_xq` / `_hk_xq` / `_us_xq` | `ak.stock_individual_basic_info_*` | 雪球个股基本信息 |
| `stock_hk_spot` / `stock_hk_daily` / `stock_hk_famous_spot_em` | `ak.stock_hk_*` | 港股行情/日K/知名股 |
| `stock_us_spot` / `stock_us_daily` / `stock_us_famous_spot_em` | `ak.stock_us_*` | 美股行情/日K/知名股 |
| `stock_ah_spot` / `stock_ah_name` / `stock_ah_daily` | `ak.stock_ah_*` | A/H 股对比 |
| `stock_bid_ask_em` | `ak.stock_bid_ask_em` | 五档盘口 |
| `stock_intraday_em` / `stock_intraday_sina` | `ak.stock_intraday_*` | 日内交易明细 |
| `stock_news_em` | `ak.stock_news_em` | 东财个股新闻 |
| `stock_news_main_cx` | `ak.stock_news_main_cx` | 财新网新闻 |
| `stock_hot_search_baidu` | `ak.stock_hot_search_baidu` | 百度股市通热搜 |
| `stock_esg_*_sina` 系列（hz/msci/rate/rft/zd） | `ak.stock_esg_*` | 新浪财经 ESG 评级 |
| `stock_gsrl_gsdt_em` | `ak.stock_gsrl_gsdt_em` | 公司大事 |
| `stock_qsjy_em` | `ak.stock_qsjy_em` | 期权套利 |

> 股票系列共 313 个接口，覆盖行情快照、K 线、港股/美股、板块、龙虎榜、资金流、千股千评、ESG、新闻等；完整清单见 `src/stock/mod.rs`。

### 指数 / 基金

> 指数共 50 个，基金共 55 个。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `index_zh_a_hist` / `index_zh_a_hist_min_em` / `index_code_id_map_em` | `ak.index_*` | A 股指数 K 线/分钟线/代码映射 |
| `index_all_cni` / `index_hist_cni` / `index_detail_cni` / `index_detail_hist_cni` / `index_detail_hist_adjust_cni` | `ak.index_*_cni` | 国证指数列表/历史/详情 |
| `index_hist_sw` / `index_min_sw` / `index_realtime_sw` / `index_analysis_*_sw` / `index_component_sw` / `index_hist_fund_sw` | `ak.index_*_sw` | 申万宏源指数（实时/分钟/分析/成分/基金） |
| `index_pmi_*_cx` / `index_dei_cx` / `index_ii_cx` / `index_si_cx` 等 19 个 | `ak.index_*_cx` | 财新 PMI/综合指数（19 类） |
| `index_price_cflp` / `index_volume_cflp` | `ak.index_price_*` / `ak.index_volume_*` | 公路物流运价/运量指数 |
| `index_sugar_msweet` / `index_inner_quote_sugar_msweet` / `index_outer_quote_sugar_msweet` | `ak.index_*_sugar_msweet` | 沐甜食糖指数 |
| `index_stock_cons` | `ak.index_stock_cons` | 新浪最新指数成份股 |
| `index_stock_cons_csindex` / `index_stock_cons_weight_csindex` | `ak.index_stock_cons_*` | 中证指数成份股/权重 |
| `index_stock_cons_sina` / `index_stock_info` / `index_global_*` 系列 | `ak.index_*` | 指数列表/全球指数 |
| `fund_etf_hist_em` / `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_*` | ETF/LOF K 线/行情 |
| `fund_etf_hist_min_em` / `fund_lof_hist_em` / `fund_lof_hist_min_em` | `ak.fund_*` | ETF/LOF 分钟 K 线 |
| `fund_etf_category_ths` / `fund_etf_spot_ths` | `ak.fund_*_ths` | ETF 分类/实时行情（JS 加密） |
| `fund_etf_category_sina` / `fund_etf_hist_sina` | `ak.fund_*_sina` | 新浪 ETF 分类/日 K 线 |
| `fund_open_fund_rank_em` / `fund_exchange_rank_em` / `fund_money_rank_em` / `fund_lcx_rank_em` | `ak.fund_*_rank_*` | 基金排行（开放式/交易所/货币/理财） |
| `fund_open_fund_daily_em` / `fund_money_fund_daily_em` / `fund_financial_fund_daily_em` | `ak.fund_*_daily_*` | 基金净值列表 |
| `fund_fh_em` / `fund_cf_em` | `ak.fund_fh_*` / `ak.fund_cf_*` | 基金分红/拆分 |
| `fund_fh_rank_em` | `ak.fund_fh_rank_em` | 基金分红排行 |
| `fund_purchase_em` | `ak.fund_purchase_em` | 基金申购状态 |
| `fund_scale_change_em` / `fund_hold_structure_em` | `ak.fund_scale_*` / `ak.fund_hold_*` | 基金规模变化/持仓结构 |
| `fund_scale_daily_szse` | `ak.fund_scale_daily_szse` | 深交所基金规模日频 |
| `fund_scale_open_sina` / `fund_scale_close_sina` / `fund_scale_structured_sina` | `ak.fund_scale_*_sina` | 新浪基金规模（开放/封闭/分级） |
| `fund_aum_em` / `fund_aum_hist_em` / `fund_aum_trend_em` | `ak.fund_aum_*` | 基金公司规模/历史/趋势 |
| `fund_etf_scale_sse` / `fund_etf_scale_szse` | `ak.fund_etf_scale_*` | 交易所 ETF 规模 |
| `fund_portfolio_hold_em` / `fund_portfolio_bond_hold_em` / `fund_portfolio_industry_allocation_em` | `ak.fund_portfolio_*` | 基金投资组合持仓 |
| `fund_money_fund_info_em` / `fund_etf_fund_info_em` / `fund_graded_fund_info_em` | `ak.fund_*_info_*` | 基金历史净值明细 |
| `fund_rating_all` / `fund_rating_sh` / `fund_rating_zs` / `fund_rating_ja` | `ak.fund_rating_*` | 基金评级 |
| `fund_new_found_em` / `fund_new_found_ths` | `ak.fund_new_found_*` | 新发基金（东财/同花顺） |
| `fund_announcement_dividend_em` / `fund_announcement_report_em` / `fund_announcement_personnel_em` | `ak.fund_announcement_*` | 基金公告（分红/定期报告/人事） |
| `fund_value_estimation_em` | `ak.fund_value_estimation_em` | 基金净值估算 |
| `fund_name_em` | `ak.fund_name_em` | 基金名称列表 |
| `fund_hk_rank_em` / `fund_hk_fund_hist_em` | `ak.fund_hk_*` | 香港基金排行/历史净值 |

### 巨潮资讯（cninfo）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_profile_cninfo` / `stock_dividend_cninfo` / `stock_ipo_summary_cninfo` / `stock_new_ipo_cninfo` / `stock_new_gh_cninfo` | `ak.stock_*` | 公司概况/分红/IPO/新股过会 |
| `bond_treasure_issue_cninfo` / `bond_local_government_issue_cninfo` / `bond_corporate_issue_cninfo` / `bond_cov_issue_cninfo` / `bond_cov_stock_issue_cninfo` | `ak.bond_*` | 国债/地方债/企业债/可转债发行 |

### 乐咕乐股（legulegu，两步流：md5 token + 会话 cookie + csrf）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_a_gxl_lg` / `stock_hk_gxl_lg` / `stock_a_ttm_lyr` | `ak.stock_*` | A/港股息率 / TTM 市盈率 |
| `stock_market_pe_lg` / `stock_index_pe_lg` / `stock_market_pb_lg` / `stock_index_pb_lg` | `ak.stock_*` | 主板/指数市盈率/市净率 |
| `stock_a_congestion_lg` / `stock_buffett_index_lg` / `stock_ebs_lg` | `ak.stock_*` | 大盘拥挤度/巴菲特指标/股债利差 |
| `fund_stock_position_lg` / `fund_balance_position_lg` / `fund_linghuo_position_lg` | `ak.fund_*` | 基金仓位 |
| `get_token_lg` | （akshare 内部） | md5 本地日期 token |

### 新浪财经

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_hk_spot` | `ak.stock_hk_spot` | 港股实时行情（分页） |
| `stock_zh_a_minute` | `ak.stock_zh_a_minute` | A 股分钟线（JSONP） |

### 交易所（上交所/深交所）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_margin_sse` / `stock_margin_detail_sse` / `stock_margin_szse` | `ak.stock_margin_*` | 融资融券汇总/明细 |

### 雪球（会话 cookie 两步流）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_hot_follow_xq` / `stock_hot_tweet_xq` | `ak.stock_hot_*` | 关注/讨论热度榜 |
| `stock_individual_basic_info_xq` / `_hk_xq` / `_us_xq` | `ak.stock_individual_basic_info_*` | 个股基本信息 |

### 同花顺

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_rank_cxg_ths` / `_cxd_ths` / `_lxsz_ths` / `_lxxd_ths` / `_cxfl_ths` / `_cxsl_ths` / `_xstp_ths` / `_xxtp_ths` / `_ljqs_ths` / `_ljqd_ths` / `_xzjp_ths` | `ak.stock_rank_*_ths` | 技术选股（创新高/低、连涨/跌、放量/缩量、突破、举牌） |
| `stock_board_industry_name_ths` / `_info_ths` / `stock_board_concept_name_ths` / `_info_ths` | `ak.stock_board_*_ths` | 行业/概念板块 |
| `stock_ipo_ths` / `stock_ipo_hk_ths` / `stock_fhps_detail_ths` | `ak.stock_*` | 新股申购/分红详情 |

### 同花顺财务 / 公司大事（stock_fundamental）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_restricted_release_summary_em` / `_detail_em` / `_queue_em` / `_stockholder_em` | `ak.stock_restricted_release_*` | 限售股解禁 |
| `stock_financial_abstract_ths` / `_debt_ths` / `_benefit_ths` / `_cash_ths` | `ak.stock_financial_*_ths` | 财务指标（旧系列） |
| `stock_financial_abstract_new_ths` / `_debt_new_ths` / `_benefit_new_ths` / `_cash_new_ths` | `ak.stock_financial_*_new_ths` | 财务指标（新系列） |
| `stock_profit_forecast_ths` / `stock_management_change_ths` / `stock_shareholder_change_ths` | `ak.stock_*` | 盈利预测/高管/股东持股变动 |
| `stock_dzjy_hygtj` / `_hyybtj` / `_mrmx` / `_mrtj` / `_sctj` / `_yybph` | `ak.stock_dzjy_*` | 大宗交易统计 |

### 期权（option）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `option_cffex_hs` / `_sz` / `_zz` | `ak.option_cffex_*` | 中金所期权（沪深300/中证500/中证1000） |
| `option_sse_list_sina` / `option_sse_codes_sina` / `option_sse_expire_day_sina` | `ak.option_sse_*` | 上交所期权列表/代码/到期 |
| `option_sse_spot_price_sina` / `option_sse_underlying_spot_price_sina` / `option_sse_greeks_sina` / `option_sse_minute_sina` / `option_sse_daily_sina` | `ak.option_sse_*` | 上交所期权实时/标的/希腊字母/分钟/日线 |
| `option_finance_sse_underlying` / `option_finance_board` | `ak.option_finance_*` | 上交所 ETF 期权标的/板块 |
| `option_current_day_sse` / `option_current_day_szse` / `option_daily_stats_sse` / `option_daily_stats_szse` / `option_risk_indicator_sse` | `ak.option_*` | 上/深交所期权当日/每日统计/风险指标 |
| `option_current_em` / `option_minute_em` / `option_premium_analysis_em` / `option_risk_analysis_em` / `option_value_analysis_em` / `option_lhb_em` | `ak.option_*_em` | 东财期权实时/分钟/溢价/风险/价值/龙虎榜 |
| `option_commodity_hist_sina` / `option_commodity_contract_sina` / `option_commodity_contract_table_sina` / `option_comm_info` / `option_comm_symbol` / `option_margin` / `option_margin_symbol` | `ak.option_commodity_*` | 商品期权历史/合约/保证金 |
| `option_hist_czce` / `option_hist_yearly_czce` / `option_hist_dce` / `option_hist_gfex` / `option_hist_shfe` / `option_vol_shfe` / `option_vol_gfex` | `ak.option_hist_*` | 期货期权历史（郑商所/大商所/广期所/上期所） |
| `option_contract_info_ctp` | `ak.option_contract_info_ctp` | CTP 期权合约信息 |

> 期权共 28 个，覆盖中金所/上交所/深交所/东财/商品/期货期权历史；完整清单见 `src/option/mod.rs`。

### 债券（bond）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `bond_cb_jsl` / `bond_cb_redeem_jsl` / `bond_cb_index_jsl` / `bond_cb_adj_logs_jsl` | `ak.bond_cb_*_jsl` | 集思录可转债列表/强赎/等权指数/转股价调整 |
| `bond_cb_profile_sina` / `bond_cb_summary_sina` | `ak.bond_cb_*_sina` | 可转债详情资料/概况（新浪） |
| `bond_spot_deal` / `bond_spot_quote` | `ak.bond_spot_*` | 现券成交/做市报价 |
| `bond_china_close_return` / `bond_china_close_return_map` | `ak.bond_china_close_return*` | 收盘收益率曲线 |
| `bond_zh_hs_daily` / `bond_zh_hs_spot` / `bond_zh_hs_cov_daily` / `bond_zh_hs_cov_spot` / `bond_zh_hs_cov_min` / `bond_zh_hs_cov_pre_min` | `ak.bond_zh_hs_*` | 沪深债券/可转债历史/实时/分钟 |
| `bond_zh_cov` / `bond_zh_cov_info` / `bond_zh_cov_value_analysis` / `bond_cov_comparison` | `ak.bond_zh_cov*` | 可转债数据/详情/价值分析/比价 |
| `bond_zh_us_rate` / `bond_gb_zh_sina` / `bond_gb_us_sina` | `ak.bond_*_rate` / `ak.bond_gb_*` | 中美国债收益率 |
| `bond_buy_back_hist_em` / `bond_sh_buy_back_em` / `bond_sz_buy_back_em` | `ak.bond_*_buy_back_*` | 质押式回购 |
| `bond_info_cm` / `bond_info_detail_cm` / `bond_info_cm_query` | `ak.bond_info_cm*` | 中国货币网债券查询 |

> 债券共 41 个，覆盖可转债/现券/国债/回购/发行/货币网；完整清单见 `src/bond/mod.rs`。

### 宏观（economic）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `macro_china_gdp` / `macro_china_gdp_yearly` / `macro_china_cpi` / `macro_china_cpi_yearly` / `macro_china_cpi_monthly` / `macro_china_ppi_yearly` | `ak.macro_china_*` | GDP/CPI/PPI |
| `macro_china_money_supply` / `macro_china_m2_yearly` / `macro_china_lpr` / `macro_china_reserve_requirement_ratio` / `macro_china_shibor_all` | `ak.macro_china_*` | 货币供应/M2/LPR/准备金/SHIBOR |
| `macro_china_pmi` / `macro_china_cx_pmi_yearly` / `macro_china_cx_services_pmi_yearly` / `macro_china_non_man_pmi` | `ak.macro_china_*_pmi*` | 官方/财新 PMI |
| `macro_china_fx_reserves_yearly` / `macro_china_fx_gold` / `macro_china_rmb` | `ak.macro_china_*` | 外汇储备/外汇占款/人民币 |
| `macro_china_exports_yoy` / `macro_china_imports_yoy` / `macro_china_trade_balance` / `macro_china_hgjck` | `ak.macro_china_*` | 进出口/贸易帐 |
| `macro_china_hk_cpi` / `macro_china_hk_rate_of_unemployment` / `macro_china_hk_gbp` / `macro_china_hk_ppi` / `macro_china_hk_market_info` | `ak.macro_china_hk_*` | 香港宏观 |
| `macro_china_qyspjg` / `macro_china_fdi` / `macro_china_new_house_price` / `macro_china_consumer_goods_retail` / `macro_china_stock_market_cap` / `macro_china_daily_energy` / `macro_china_au_report` | `ak.macro_china_*` | 企业商品价格/外商直接投资/房价/消费/市值/能源/黄金 |

> 宏观共 148 个（金十 + 东财 datacenter-web + 香港 + 多口径 + 欧元区 + 全球）；完整清单见 `src/economic/mod.rs` 与 `src/sources/jin10.rs`。

### 能源与商品（energy）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `energy_oil_hist` / `energy_oil_detail` | `ak.energy_oil_*` | 汽柴油历史调价/详情 |
| `spot_symbol_table_sge` / `spot_golden_benchmark_sge` / `spot_silver_benchmark_sge` / `spot_hist_sge` / `spot_quotations_sge` | `ak.spot_*_sge` | 上海黄金交易所行情 |
| `energy_carbon_gz` / `energy_carbon_hb` | `ak.energy_carbon_*` | 广州/湖北碳排放行情 |
| `spot_hog_soozhu` / `spot_hog_year_trend_soozhu` / `spot_hog_lean_price_soozhu` / `spot_hog_three_way_soozhu` / `spot_hog_crossbred_soozhu` / `spot_corn_price_soozhu` / `spot_soybean_price_soozhu` / `spot_mixed_feed_soozhu` | `ak.spot_hog_*` | 生猪/玉米/豆粕/混合饲料（搜猪） |

### 新闻（news）

> 共 5 个接口（新闻模块 5 个 + stock 模块 2 个股票相关新闻）。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `news_economic_baidu` / `news_trade_notify_suspend_baidu` / `news_trade_notify_dividend_baidu` / `news_report_time_baidu` | `ak.news_*` | 百度财经新闻/停牌/分红/财报预约 |
| `news_cctv` | `ak.news_cctv` | 央视新闻 |
| `stock_news_em` | `ak.stock_news_em` | 东财个股新闻（在 stock 模块） |
| `stock_news_main_cx` | `ak.stock_news_main_cx` | 财新网新闻（在 stock 模块） |

### 财富榜单（fortune）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `hurun_rank` | `ak.hurun_rank` | 胡润百富榜 |

### REITs（不动产投资信托基金）

> 共 3 个接口，全部实现（100%）。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `reits_realtime_em` | `ak.reits_realtime_em` | 沪深 REITs 实时行情 |
| `reits_hist_em` | `ak.reits_hist_em` | 沪深 REITs 日 K 线 |
| `reits_hist_min_em` | `ak.reits_hist_min_em` | 沪深 REITs 分钟 K 线 |

### 现货（spot）

> 共 16 个接口，覆盖上海黄金交易所、搜猪网、99期货等。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `spot_goods` | `ak.spot_goods` | 商品现货 |
| `spot_price_table_qh` / `spot_price_qh` | `ak.spot_price_*_qh` | 99 期货期现价格 |
| `spot_symbol_table_sge` / `spot_golden_benchmark_sge` / `spot_silver_benchmark_sge` | `ak.spot_*_sge` | 上海黄金交易所 |
| `spot_hist_sge` / `spot_quotations_sge` | `ak.spot_*_sge` | 上金所历史/报价 |
| `spot_hog_soozhu` / `spot_hog_year_trend_soozhu` / `spot_hog_lean_price_soozhu` | `ak.spot_hog_*` | 搜猪网生猪数据 |
| `spot_hog_three_way_soozhu` / `spot_hog_crossbred_soozhu` | `ak.spot_hog_*` | 搜猪网三元/杂交猪 |
| `spot_corn_price_soozhu` / `spot_soybean_price_soozhu` / `spot_mixed_feed_soozhu` | `ak.spot_*` | 搜猪网玉米/豆粕/饲料 |

### 外汇（currency / forex / fx）

> 共 9 个接口（人民币中间价 2 个 + 外汇牌价 2 个 + 外汇兑换 5 个）。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `currency_boc_safe` / `currency_boc_sina` | `ak.currency_boc_*` | 外汇局/新浪人民币中间价 |
| `forex_spot_em` / `forex_hist_em` | `ak.forex_*` | 东财外汇牌价/历史 |
| `fx_c_swap_cm` / `fx_quote_baidu` / `fx_spot_quote` / `fx_swap_quote` / `fx_pair_quote` | `ak.fx_*` | 中国货币网/百度外汇 |

### 期货（futures）

> 共 55 个接口，覆盖五家交易所结算参数、合约详情、历史行情、外盘期货、库存仓单等。

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `futures_settle_cffex` / `futures_settle_czce` / `futures_settle_gfex` / `futures_settle_shfe` / `futures_settle_ine` | `ak.futures_settle_*` | 五家交易所结算参数 |
| `futures_settle` | `ak.futures_settle` | 结算参数统一入口（20 列规范化，`market` 分派） |
| `futures_contract_detail` | `ak.futures_contract_detail` | 新浪期货合约详情（GB2312 页面） |
| `futures_contract_detail_em` | `ak.futures_contract_detail_em` | 东财期货合约详情 |
| `futures_contract_info_*` 系列（6 交易所） | `ak.futures_contract_info_*` | 各交易所合约信息 |
| `futures_warehouse_receipt_*` 系列（4 交易所） | `ak.futures_warehouse_receipt_*` | 各交易所仓单 |
| `futures_delivery_*` / `futures_to_spot_*` 系列（8 个） | `ak.futures_delivery_*` / `ak.futures_to_spot_*` | 交割/期转现 |
| `futures_hist_daily_cffex` | `ak.futures_hist_daily_cffex` | 中金所历史日线 |
| `futures_hist_em` / `futures_hist_table_em` | `ak.futures_hist_*` | 东财期货历史 K 线 |
| `futures_global_spot_em` / `futures_global_hist_em` | `ak.futures_global_*` | 东财国际期货实时/历史 |
| `futures_index_ccidx` | `ak.futures_index_ccidx` | 中证商品指数 CCIDX |
| `futures_symbol_mark` / `futures_zh_realtime` / `futures_zh_spot` | `ak.futures_*` | 新浪品种映射/实时行情 |
| `futures_zh_daily_sina` / `futures_zh_minute_sina` | `ak.futures_zh_*` | 新浪日/分钟 K 线 |
| `futures_foreign_commodity_realtime` / `futures_foreign_detail` / `futures_foreign_hist` | `ak.futures_foreign_*` | 外盘期货实时/详情/历史 |
| `futures_comm_info` / `futures_comm_js` / `futures_fees_info` | `ak.futures_*` | 手续费/费用表 |
| `futures_news_shmet` | `ak.futures_news_shmet` | 上海金属网快讯 |
| `futures_inventory_99` / `futures_spot_stock` / `futures_stock_shfe_js` | `ak.futures_*` | 库存/现货数据 |
| `futures_rule` | `ak.futures_rule` | 国泰君安交易日历 |
| `futures_hold_pos_sina` / `futures_main_sina` / `futures_display_main_sina` | `ak.futures_*` | 新浪持仓/主力合约 |

## 架构

```
src/
├── core/           # 基础设施
│   ├── error.rs    # AkshareError 统一错误类型（Empty/Js/Blocked/AuthRequired/Status/Http...）
│   ├── config.rs   # 全局配置（UA/超时/重试/代理）
│   ├── http.rs     # reqwest 封装：指数退避+抖动重试、多节点容灾、字符集解码、反爬特征检测
│   ├── df.rs       # Df（polars DataFrame 封装）：JSON 建表/排序/列转换，列序对齐 pandas
│   ├── html.rs     # HTML 表格解析（read_html_tables 二维字符串 / read_html 返回 Vec<Df>）
│   └── js_engine.rs# rquickjs 封装：eval 加密 JS + 浏览器全局 shim 注入
├── sources/        # 数据源层（一个源一个模块）
│   ├── eastmoney.rs# 东财：clist 分页（多节点故障转移）/ K 线 / 市场判定 / datacenter 报表
│   ├── ths.rs      # 同花顺：v token（JS）+ HTML 表格/板块/公司大事解析
│   ├── jin10.rs    # 金十：数据中心报表翻页（max_date 游标）
│   ├── currency_boc.rs # 外汇局/新浪人民币中间价
│   ├── oil.rs / sge.rs / carbon.rs # 能源：原油 / 上金所 / 碳排放
│   ├── news_baidu.rs / news_cctv.rs # 新闻
│   ├── hurun.rs    # 胡润榜单
│   ├── soozhu.rs   # 搜猪（生猪/饲料）
│   ├── spot_goods.rs / spot_qh.rs # 现货
│   ├── jisilu.rs / chinamoney.rs  # 集思录 / 中国货币网（债券）
│   └── ...         # 其它源模块
├── economic/       # 宏观：金十中国宏观 + 东财 datacenter-web 宏观 + 香港/欧元区/全球（共 148 个）
├── futures/        # 期货：五家交易所结算参数 + 合约详情 + 历史行情 + 外盘期货（共 55 个）
├── option/         # 期权：中金所/上交所/深交所/东财/商品/期货期权历史（共 28 个）
├── bond/           # 债券：可转债/现券/国债/回购/发行/货币网（共 41 个）
├── interest_rate/  # 利率：上海银行间同业拆放利率（SHIBOR）
├── cninfo/         # 巨潮资讯：datacenter 查询 + 内置 JS 加密
├── reits/          # REITs：沪深不动产投资信托基金实时/历史（共 3 个）
├── legu/           # 乐咕乐股：md5 token + 会话 cookie + csrf 两步流
├── sina/           # 新浪财经：港股现货分页 / 分钟线 JSONP
├── exchange/       # 交易所：上交所/深交所融资融券
├── xueqiu/         # 雪球：会话 cookie + 热度榜分页
├── stock/          # 股票接口（对应 akshare stock_* 函数）
├── stock_feature/  # 股票特色接口（东财 datacenter 龙虎榜/沪深港通 + 同花顺板块/新股等，~130 个）
├── stock_fundamental/ # 基本面接口（限售股解禁 / 同花顺财务指标 / 公司大事）
├── index/          # 指数接口（对应 akshare index_* 函数）
├── fund/           # 基金接口（对应 akshare fund_* 函数）
└── bin/
    ├── demo.rs     # 命令行冒烟演示
    └── parity.rs   # 差分对比 CLI（供 tools/parity_runner.py 调用）
```

### 关键设计

- **多节点容灾**：东财 push2 集群单节点可能被限流/故障，`fetch_paginated_diff_any` /
  `get_json_any` 第一轮每节点单次快速探测、失败立即切换，全部失败后再按完整重试策略兜底。
- **分钟级数据滚动窗口**：东财分钟 K 线/分时接口只返回最近约 8 个月的滚动数据，
  与 akshare 行为一致；请求较早日期的分钟数据会得到空表。
- **JS 加密**：一律用 rquickjs 执行 akshare 原版 JS，不在 Rust 手写算法；
  通过注入 `var BROWSER_LIST; var time;` 等浏览器全局 shim 兼容非严格模式写法。
- **会话两步流**：legulegu/雪球等需先访问页面建立 cookie + 提取 csrf/token 再请求 API，
  `get_text_allow_blocked` 用于会话建立（cookie 才是目的，不校验页面内容）。
- **反爬识别**：响应含 `_waf`/`Just a moment`/`challenge-platform` 判为 `Blocked`，
  含 `400016`/`xq_a_token` 等判为 `AuthRequired`，明确报错而非返回脏数据。
- **4xx 不重试**：客户端错误立即返回；仅 5xx 与连接错误进入退避重试
  （对应 akshare `raise_for_status` 语义）。

## 开发规范

- `cargo fmt` / `cargo clippy --all-targets -- -D warnings` 必须零告警
- 无 `unwrap`/`expect`（除 `Client::build` 等构造点外），错误统一走 `Result<AkshareError>`
- 公开函数必须带 `///` 文档注释（参数、返回列）
- 数据变换逻辑抽成纯函数并配离线单测（不依赖网络）

## 已知限制

- 东财 push2 集群对本机 IP 有临时限流（表现为 TLS close_notify 连接重置），
  Python akshare 同样受影响；容灾与重试会尽量规避，必要时稍后重试。
- legulegu（乐咕）当前对本机 IP 返回 403（nginx 封禁），接口已按 akshare 原逻辑实现
  并通过 token 交叉验证，待环境恢复后做真实验证。
- 东财 clist 系接口（st/new/hk_spot_em）在 push2 限流窗口内无法做真实验证，
  已通过键名映射（与已验证的 spot_em 同构）+ 离线单测保障正确性。
