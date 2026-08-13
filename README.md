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

> 截至当前共 **364** 个数据接口，覆盖 **19 / 47** 个功能大类、整体覆盖率 **≈ 33.1%**
> （对标 akshare 公开 API 共 1099 个）。全部接口与 Python akshare 同名函数对齐
> （列名/列序/值逐项差分验证）。

**按大类分布（已实现 / akshare 总数 / 覆盖率）：**

| 大类 | 已实现 | akshare | 覆盖率 |
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
| currency | 2 | ~数十 | 长尾 |
| energy | 17 | ~数十 | 长尾 |
| news | 5 | ~数十 | 长尾 |
| fortune | 1 | ~10 | 长尾 |
| spot | 3 | ~数十 | 长尾 |
| cninfo | 10 | — | 巨潮系 |
| sina | 2 | — | 新浪系 |
| legu | 14 | — | 乐咕系 |
| xueqiu | 2 | — | 雪球系 |
| exchange | 3 | — | 交易所系 |

> 以下按数据源列出主要接口；每个大类完整函数清单见 `src/` 对应模块。

### 股票（东方财富行情 / K线 / 资金 / 板块 / 龙虎榜 / 沪深港通）

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

> 股票特色（`stock_feature`）共 95 个，覆盖行情快照、股东分析、龙虎榜、沪深港通、财务报表、千股千评、技术选股等；完整清单见 `src/stock_feature/mod.rs`。

### 指数 / 基金

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `index_zh_a_hist` / `index_zh_a_hist_min_em` / `index_code_id_map_em` | `ak.index_*` | 指数 K 线/分钟线/代码映射 |
| `fund_etf_hist_em` / `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_*` | ETF/LOF K 线/行情 |
| `fund_etf_category_ths` / `fund_etf_spot_ths` | `ak.fund_*_ths` | ETF 分类/实时行情（JS 加密） |

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

> 期权共 46 个，覆盖中金所/上交所/深交所/东财/商品/期货期权历史；完整清单见 `src/option/mod.rs`。

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

> 债券共 29 个，覆盖可转债/现券/国债/回购/发行/货币网；完整清单见 `src/bond/mod.rs`。

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

> 宏观共 48 个（金十 + 东财 datacenter-web + 香港 + 多口径）；完整清单见 `src/economic/mod.rs` 与 `src/sources/jin10.rs`。

### 能源与商品（energy）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `energy_oil_hist` / `energy_oil_detail` | `ak.energy_oil_*` | 汽柴油历史调价/详情 |
| `spot_symbol_table_sge` / `spot_golden_benchmark_sge` / `spot_silver_benchmark_sge` / `spot_hist_sge` / `spot_quotations_sge` | `ak.spot_*_sge` | 上海黄金交易所行情 |
| `energy_carbon_gz` / `energy_carbon_hb` | `ak.energy_carbon_*` | 广州/湖北碳排放行情 |
| `spot_hog_soozhu` / `spot_hog_year_trend_soozhu` / `spot_hog_lean_price_soozhu` / `spot_hog_three_way_soozhu` / `spot_hog_crossbred_soozhu` / `spot_corn_price_soozhu` / `spot_soybean_price_soozhu` / `spot_mixed_feed_soozhu` | `ak.spot_hog_*` | 生猪/玉米/豆粕/混合饲料（搜猪） |

### 新闻（news）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `news_economic_baidu` / `news_trade_notify_suspend_baidu` / `news_trade_notify_dividend_baidu` / `news_report_time_baidu` | `ak.news_*` | 百度财经新闻/停牌/分红/财报预约 |
| `news_cctv` | `ak.news_cctv` | 央视新闻 |

### 财富榜单（fortune）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `hurun_rank` | `ak.hurun_rank` | 胡润百富榜 |

### 现货（spot）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `spot_goods` | `ak.spot_goods` | 商品现货 |
| `spot_price_table_qh` / `spot_price_qh` | `ak.spot_price_*_qh` | 99 期货期现价格 |

### 外汇（currency）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `currency_boc_safe` / `currency_boc_sina` | `ak.currency_boc_*` | 外汇局/新浪人民币中间价 |

### 期货交易所（结算参数 + 合约详情）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `futures_settle_cffex` / `futures_settle_czce` / `futures_settle_gfex` / `futures_settle_shfe` / `futures_settle_ine` | `ak.futures_settle_*` | 五家交易所结算参数 |
| `futures_settle` | `ak.futures_settle` | 结算参数统一入口（20 列规范化，`market` 分派） |
| `futures_contract_detail` | `ak.futures_contract_detail` | 新浪期货合约详情（GB2312 页面） |

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
├── economic/       # 宏观：金十中国宏观 + 东财 datacenter-web 宏观 + 香港/多口径（共 48 个）
├── futures/        # 期货：五家交易所结算参数 + 统一入口 + 新浪合约详情
├── option/         # 期权：中金所/上交所/深交所/东财/商品/期货期权历史（共 46 个）
├── bond/           # 债券：可转债/现券/国债/回购/发行/货币网（共 29 个）
├── cninfo/         # 巨潮资讯：datacenter 查询 + 内置 JS 加密
├── legu/           # 乐咕乐股：md5 token + 会话 cookie + csrf 两步流
├── sina/           # 新浪财经：港股现货分页 / 分钟线 JSONP
├── exchange/       # 交易所：上交所/深交所融资融券
├── xueqiu/         # 雪球：会话 cookie + 热度榜分页
├── stock/          # 股票接口（对应 akshare stock_* 函数）
├── stock_feature/  # 股票特色接口（东财 datacenter 龙虎榜/沪深港通 + 同花顺板块/新股等，95 个）
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
