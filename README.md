# akshare-rust

Rust 版 [akshare](https://github.com/akfamily/akshare)：纯 HTTP + 内置 JS 引擎的财经数据获取库。

> 🤖 本项目由 **AI 开发**，每个接口均与 Python akshare 同名函数逐项差分对账验证。

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

> 截至当前共 117 个数据接口，全部与 Python akshare 同名函数对齐（列名/列序/值逐项差分验证）。

### 东方财富（行情/K线/资金/板块）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_zh_a_hist` | `ak.stock_zh_a_hist` | A 股日/周/月 K 线（前复权/后复权/不复权） |
| `stock_zh_a_hist_min_em` | `ak.stock_zh_a_hist_min_em` | 分钟 K 线/分时 |
| `stock_zh_a_spot_em` / `stock_sh_a_spot_em` / `stock_sz_a_spot_em` / `stock_bj_a_spot_em` | `ak.stock_*_spot_em` | 沪深京实时行情 |
| `stock_zh_a_st_em` | `ak.stock_zh_a_st_em` | ST 风险警示板 |
| `stock_zh_a_new_em` | `ak.stock_zh_a_new_em` | 新股板块 |
| `stock_hk_spot_em` | `ak.stock_hk_spot_em` | 港股实时行情 |
| `stock_individual_info_em` | `ak.stock_individual_info_em` | 个股信息 |
| `stock_bid_ask_em` | `ak.stock_bid_ask_em` | 五档盘口 |
| `stock_individual_fund_flow` | `ak.stock_individual_fund_flow` | 个股资金流向 |
| `stock_hsgt_fund_flow_summary_em` | `ak.stock_hsgt_fund_flow_summary_em` | 沪深港通资金流向 |
| `stock_lhb_detail_em` | `ak.stock_lhb_detail_em` | 龙虎榜详情 |
| `stock_zt_pool_em` | `ak.stock_zt_pool_em` | 涨停股池 |
| `stock_gpzy_profile_em` | `ak.stock_gpzy_profile_em` | 股权质押 |
| `stock_board_industry_name_em` / `stock_board_industry_cons_em` / `stock_board_industry_hist_em` | `ak.stock_board_industry_*_em` | 行业板块 |
| `stock_board_concept_name_em` / `stock_board_concept_cons_em` / `stock_board_concept_hist_em` | `ak.stock_board_concept_*_em` | 概念板块 |
| `index_zh_a_hist` | `ak.index_zh_a_hist` | 指数 K 线 |
| `index_zh_a_hist_min_em` | `ak.index_zh_a_hist_min_em` | 指数分钟 K 线/分时 |
| `index_code_id_map_em` | `ak.index_code_id_map_em` | 指数代码映射 |
| `fund_etf_hist_em` | `ak.fund_etf_hist_em` | ETF K 线 |
| `fund_etf_spot_em` / `fund_lof_spot_em` | `ak.fund_etf_spot_em` / `ak.fund_lof_spot_em` | ETF/LOF 行情列表 |

### 巨潮资讯（cninfo）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_profile_cninfo` | `ak.stock_profile_cninfo` | 公司概况 |
| `stock_dividend_cninfo` | `ak.stock_dividend_cninfo` | 分红送配 |
| `stock_ipo_summary_cninfo` | `ak.stock_ipo_summary_cninfo` | IPO 明细 |
| `stock_new_ipo_cninfo` | `ak.stock_new_ipo_cninfo` | 新股申购 |
| `stock_new_gh_cninfo` | `ak.stock_new_gh_cninfo` | 新股过会 |

### 乐咕乐股（legulegu，两步流：md5 token + 会话 cookie + csrf）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_a_gxl_lg` | `ak.stock_a_gxl_lg` | A 股股息率 |
| `stock_hk_gxl_lg` | `ak.stock_hk_gxl_lg` | 港股股息率 |
| `stock_a_ttm_lyr` | `ak.stock_a_ttm_lyr` | A 股 TTM 市盈率 |
| `stock_market_pe_lg` / `stock_index_pe_lg` | `ak.stock_market_pe_lg` / `ak.stock_index_pe_lg` | 主板/指数市盈率 |
| `stock_market_pb_lg` / `stock_index_pb_lg` | `ak.stock_market_pb_lg` / `ak.stock_index_pb_lg` | 主板/指数市净率 |
| `stock_a_congestion_lg` | `ak.stock_a_congestion_lg` | 大盘拥挤度 |
| `stock_buffett_index_lg` | `ak.stock_buffett_index_lg` | 巴菲特指标 |
| `stock_ebs_lg` | `ak.stock_ebs_lg` | 股债利差 |
| `fund_stock_position_lg` / `fund_balance_position_lg` / `fund_linghuo_position_lg` | `ak.fund_*_position_lg` | 股票型/平衡混合/灵活配置基金仓位 |
| `get_token_lg` | （akshare 内部） | md5 本地日期 token |

### 新浪财经

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_hk_spot` | `ak.stock_hk_spot` | 港股实时行情（分页） |
| `stock_zh_a_minute` | `ak.stock_zh_a_minute` | A 股分钟线（JSONP） |

### 交易所（上交所/深交所）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_margin_sse` | `ak.stock_margin_sse` | 上交所融资融券汇总 |
| `stock_margin_detail_sse` | `ak.stock_margin_detail_sse` | 上交所融资融券明细 |
| `stock_margin_szse` | `ak.stock_margin_szse` | 深交所融资融券汇总 |

### 雪球（会话 cookie 两步流）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_hot_follow_xq` | `ak.stock_hot_follow_xq` | 关注热度榜 |
| `stock_hot_tweet_xq` | `ak.stock_hot_tweet_xq` | 讨论热度榜 |

### 同花顺

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `fund_etf_category_ths` | `ak.fund_etf_category_ths` | ETF 分类 |
| `fund_etf_spot_ths` | `ak.fund_etf_spot_ths` | ETF 实时行情（JS 加密） |
| `stock_rank_cxg_ths` / `stock_rank_cxd_ths` | `ak.stock_rank_cxg_ths` / `ak.stock_rank_cxd_ths` | 创月新高/新低 |
| `stock_rank_lxsz_ths` / `stock_rank_lxxd_ths` | `ak.stock_rank_lxsz_ths` / `ak.stock_rank_lxxd_ths` | 连续上涨/下跌 |
| `stock_rank_cxfl_ths` / `stock_rank_cxsl_ths` | `ak.stock_rank_cxfl_ths` / `ak.stock_rank_cxsl_ths` | 持续放量/缩量 |
| `stock_rank_ljqd_ths` / `stock_rank_ljqs_ths` | `ak.stock_rank_ljqd_ths` / `ak.stock_rank_ljqs_ths` | 量价齐跌/齐升 |
| `stock_rank_xstp_ths` / `stock_rank_xxtp_ths` | `ak.stock_rank_xstp_ths` / `ak.stock_rank_xxtp_ths` | 向上/向下突破 |
| `stock_rank_xzjp_ths` | `ak.stock_rank_xzjp_ths` | 险资举牌 |
| `stock_board_industry_name_ths` / `stock_board_industry_info_ths` | `ak.stock_board_industry_*_ths` | 行业板块名称/简介 |
| `stock_board_concept_name_ths` / `stock_board_concept_info_ths` | `ak.stock_board_concept_*_ths` | 概念板块名称/简介 |
| `stock_ipo_ths` / `stock_ipo_hk_ths` | `ak.stock_ipo_ths` / `ak.stock_ipo_hk_ths` | 新股申购（A 股/港股） |
| `stock_fhps_detail_ths` | `ak.stock_fhps_detail_ths` | 分红详情（GBK 页） |

### 同花顺财务/公司大事（stock_fundamental）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `stock_financial_abstract_ths` | `ak.stock_financial_abstract_ths` | 主要指标（旧系列，HTML 内嵌 JSON） |
| `stock_financial_debt_ths` / `stock_financial_benefit_ths` / `stock_financial_cash_ths` | `ak.stock_financial_*_ths` | 资产负债/利润/现金流量表（旧系列，flashData 双重 JSON） |
| `stock_financial_abstract_new_ths` | `ak.stock_financial_abstract_new_ths` | 重要指标（新系列，app_data 报表） |
| `stock_financial_debt_new_ths` / `stock_financial_benefit_new_ths` / `stock_financial_cash_new_ths` | `ak.stock_financial_*_new_ths` | 资产负债/利润/现金流量表（新系列） |
| `stock_profit_forecast_ths` | `ak.stock_profit_forecast_ths` | 盈利预测（两级表头展开） |
| `stock_management_change_ths` / `stock_shareholder_change_ths` | `ak.stock_management_change_ths` / `ak.stock_shareholder_change_ths` | 高管/股东持股变动 |

### 金十数据中心（中国宏观）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `macro_china_gdp_yearly` | `ak.macro_china_gdp_yearly` | 中国 GDP 年率 |
| `macro_china_cpi_yearly` / `macro_china_cpi_monthly` | `ak.macro_china_cpi_*` | 中国 CPI 年率/月率 |
| `macro_china_ppi_yearly` | `ak.macro_china_ppi_yearly` | 中国 PPI 年率 |
| `macro_china_exports_yoy` / `macro_china_imports_yoy` / `macro_china_trade_balance` | `ak.macro_china_*` | 出口/进口/贸易帐 |
| `macro_china_industrial_production_yoy` | `ak.macro_china_industrial_production_yoy` | 规模以上工业增加值 |
| `macro_china_pmi_yearly` / `macro_china_cx_pmi_yearly` / `macro_china_cx_services_pmi_yearly` / `macro_china_non_man_pmi` | `ak.macro_china_*_pmi*` | 官方/财新制造业/服务业/非制造业 PMI |
| `macro_china_fx_reserves_yearly` | `ak.macro_china_fx_reserves_yearly` | 外汇储备 |
| `macro_china_m2_yearly` | `ak.macro_china_m2_yearly` | M2 货币供应年率 |

> 以上 14 个金十宏观函数输出统一 5 列：`商品, 日期, 今值, 预测值, 前值`（日期升序）。

### 东方财富（宏观·datacenter-web）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `macro_china_hk_cpi` / `macro_china_hk_cpi_ratio` | `ak.macro_china_hk_cpi*` | 香港 CPI（当月/同比） |
| `macro_china_hk_rate_of_unemployment` | `ak.macro_china_hk_rate_of_unemployment` | 香港失业率 |
| `macro_china_hk_gbp` / `macro_china_hk_gbp_ratio` | `ak.macro_china_hk_gbp*` | 香港 GDP（值/同比） |
| `macro_china_hk_building_volume` / `macro_china_hk_building_amount` | `ak.macro_china_hk_building_*` | 香港楼宇买卖（宗数/金额） |
| `macro_china_hk_trade_diff_ratio` | `ak.macro_china_hk_trade_diff_ratio` | 香港进出口贸易差额同比 |
| `macro_china_hk_ppi` | `ak.macro_china_hk_ppi` | 香港 PPI |
| `macro_china_qyspjg` | `ak.macro_china_qyspjg` | 企业商品价格指数 |
| `macro_china_fdi` | `ak.macro_china_fdi` | 外商直接投资 |

> 以上 11 个东财宏观函数统一走 `datacenter-web.eastmoney.com`（reportName 查询 + `finalize_report` 管线）。

### 期货交易所（结算参数 + 合约详情）

| 函数 | 对应 akshare | 说明 |
|---|---|---|
| `futures_settle_cffex` | `ak.futures_settle_cffex` | 中金所结算参数（CSV） |
| `futures_settle_czce` | `ak.futures_settle_czce` | 郑商所结算参数 |
| `futures_settle_gfex` | `ak.futures_settle_gfex` | 广期所结算参数（POST 表单） |
| `futures_settle_shfe` | `ak.futures_settle_shfe` | 上期所结算参数 |
| `futures_settle_ine` | `ak.futures_settle_ine` | 上能中心结算参数 |
| `futures_settle` | `ak.futures_settle` | 结算参数统一入口（20 列规范化，`market` 分派） |
| `futures_contract_detail` | `ak.futures_contract_detail` | 新浪期货合约详情（GB2312 页面） |

> 完整实施计划见 [`PLAN.md`](PLAN.md)（1099 个函数 / 33 个分类的迁移路线图）。

## 架构

```
src/
├── core/           # 基础设施
│   ├── error.rs    # AkshareError 统一错误类型（Empty/Js/Blocked/AuthRequired/Status/Http...）
│   ├── config.rs   # 全局配置（UA/超时/重试/代理）
│   ├── http.rs     # reqwest 封装：指数退避+抖动重试、多节点容灾、字符集解码、反爬特征检测
│   ├── df.rs       # Df（polars DataFrame 封装）：JSON 建表/排序/列转换，列序对齐 pandas
│   └── js_engine.rs# rquickjs 封装：eval 加密 JS + 浏览器全局 shim 注入
├── sources/        # 数据源层（一个源一个模块）
│   ├── eastmoney.rs# 东财：clist 分页（多节点故障转移）/ K 线 / 市场判定 / datacenter 报表
│   ├── ths.rs      # 同花顺：v token（JS）+ HTML 表格/板块/公司大事解析
│   └── jin10.rs    # 金十：数据中心报表翻页（max_date 游标）
├── economic/       # 宏观：金十中国宏观 14 个 + 东财 datacenter-web 宏观 11 个（共 25 个）
├── futures/        # 期货：五家交易所结算参数 + 统一入口 + 新浪合约详情
├── cninfo/         # 巨潮资讯：datacenter 查询 + 内置 JS 加密
├── legu/           # 乐咕乐股：md5 token + 会话 cookie + csrf 两步流
├── sina/           # 新浪财经：港股现货分页 / 分钟线 JSONP
├── exchange/       # 交易所：上交所/深交所融资融券
├── xueqiu/         # 雪球：会话 cookie + 热度榜分页
├── stock/          # 股票接口（对应 akshare stock_* 函数）
├── stock_feature/  # 股票特色接口（东财 datacenter 龙虎榜/沪深港通 + 同花顺板块/新股等）
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
