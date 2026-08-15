# 更新日志 (Changelog)

本文件记录 akshare-rust 的主要变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

## [2026-08-15] 批次 29-A · futures 国际/指数（子组 A）

- **新增公开函数**：**484 → 487**（净 +3）。`src/futures/em_global.rs` 落地 3 个国际期货/商品指数函数：中证商品指数 `futures_index_ccidx`（CCIDX `getDateLine`，全 24 列仅 6 字段中文化、余原样，三字符串列保留 str）+ 东财国际期货实时 `futures_global_spot_em`（`futsseapi.eastmoney.com/list`，复用 `option_current_em` 模板，14 列，`序号` 1 基数值化）+ 东财国际期货历史 `futures_global_hist_em`（push2his kline 日线，`日增` 还原 2^32 回卷）。
- **实现覆盖率**：**≈ 44.5%**（487 / 1094 公开 API）；golden 差分验证 **425 fixture / ≈410 去重函数 ≈ 37.5%**，parity 注册用例 473 / 465 唯一函数；futures 大类 **10.0% → 17.1%**（7 → 12 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(218) 全绿。
- **parity 验证**：`futures_index_ccidx`（24 列×970 行，2 个 symbol 用例）、`futures_global_spot_em`（14 列×620 行）loose 比对（列名+dtype）全部通过；`futures_global_hist_em` 因东财 push2his TCP 断连（直连 akshare 同错，属 §1.2.1 #10 EM push2 阻断）暂无 golden，`--check` 自动跳过，非回归。
- **范围调整**：子组 A 原规划 4 函数（含 `futures_rule_em`），经 `dir(ak)` 确认 `futures_rule_em` 非公开 API（`akshare` 仅含 `futures_rule` 国泰君安 HTML 表），已移除，实落 3 函数。

## [2026-08-15] 批次 28 · bond g_calc 中债指数/同花顺可转债/国债收益率

- **新增公开函数**：**477 → 484**（净 +7）。`src/bond/g_calc.rs` 落地 7 个纯计算/索引类债券函数：中债指数族系 6（`bond_available_index_cbond`、`bond_index_general_cbond`、`bond_treasury_index_cbond`、`bond_new_composite_index_cbond`、`bond_composite_index_cbond`、`bond_china_yield`）+ 同花顺可转债 1（`bond_zh_cov_info_ths`）。
- **实现覆盖率**：**≈ 44.2%**（484 / 1094 公开 API）；其中 **≈407** 个函数经 golden 差分验证（**≈ 37.2%**），parity 注册用例 470 / 462 唯一函数；bond 大类 **63.0% → 78.3%**（29 → 36 / 46）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(218) 全绿；7 个新函数全部 `parity --check` 通过（loose 7/7）。
- **映射生成**：313 项 `INDEX_MAPPING` / 13 项 `PERIOD_MAPPING` / 17 项 `INDICATOR_MAPPING` / 13 项 `TREASURY_INDEX_ID` 由 Python 脚本直读 akshare 常量生成字面量（零转录错误）；中债指数 UTC 毫秒时间戳经 `+8h` 偏移 + Howard Hinnant 历法算法换算上海日期（无 chrono 依赖）。
- **跳过项**：`bond_debt_nafmii`（nafmii 源）已确认结构性源侧失效（`zhuce.nafmii.org.cn` 返回 403 WAF，连 akshare 原版都 `JSONDecodeError`），不实现、不入 parity 用例（见 PLAN §1.2.1 #12，与 `stock_esg_rate_sina` 同类）。

## [2026-08-14] 批次 6–27 · 实现覆盖率 ~43.6%（477 函数）/ golden 验证 ~36.6%

- **新增公开函数**：**364 → 477**（净 +113，跳过批次 14）。覆盖批次 6–13（海外宏观澳洲/加拿大/德国/日本/瑞士/英国共 51、`stock_register_*` 注册制 IPO/首发申报/盈利预测/行业对比/港股 F10/估值对比）+ 批次 15–27（`stock_gsrl_gsdt_em`/`stock_repurchase_em`/`stock_report_fund_hold*`/`stock_restricted_release_queue_sina`/`futures_comex_inventory`/`rate_interbank`/`stock_register_db`/`stock_hot_*`(7)/`stock_zt_pool_*`(6)/`stock_esg_*_sina`(5)/`stock_fund_flow_*`(4)/`stock_financial_*_analysis_indicator_em`/`stock_sy_em`/`stock_zh_a_gbjg_em`/`stock_*_notice_report`/`stock_zh_kcb_report_em`/`stock_zygc_em`）。
- **实现覆盖率**：**≈ 43.6%**（477 / 1094 公开 API）；其中 **≈400** 个函数经 golden 差分验证（**≈ 36.6%**），parity 注册用例 463 / 455 唯一函数。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(218) 全绿。
- **golden 回填（2026-08-15）**：补齐批次 22/23/24 缺失的 9 个 golden fixture（`stock_hot_keyword_em`/`stock_hot_rank_detail_em`/`stock_hot_rank_detail_realtime_em`/`stock_hot_rank_latest_em`/`stock_hot_up_em`/`stock_zt_pool_previous_em`/`stock_zt_pool_strong_em`/`stock_zt_pool_sub_new_em`/`stock_zt_pool_zbgc_em`），其中 8 个经 `--check` 通过；`stock_esg_rate_sina` 因 akshare 上游返回非 JSON 未生成、`stock_hot_up_em` 因 EM push2 瞬时失败 `--check` 待环境恢复复验。
- **parity 模式修正**：`stock_zt_pool_previous_em` 因源 `getYesterdayZTPool` 返回活体「前一交易日」数据（date 参数不被源采纳、跨调用漂移）由 strict 降级 loose（同 `spot_price_qh`）。
- **探查工件**：提交 `tests/golden_probe/`（批次 26 探查 `batch26_spec.json` / `consts_gen.rs` 等）。

## [2026-08-12] 批次 2–5 集成 · 覆盖率 33.1%

- **集成合并**：将 5 个 worktree 分支（`batch2-option`、`batch3-stockfund`、`batch3-economic-cn`、`batch4-bond`、`batch5-longtail`）经 `git merge --no-ff` 逐一合入 `main`（安全标签 `integrate-base` 指向 `a8c1ae6`）。
- **新增公开函数**：**195 → 364**（净 +169），整体覆盖率 **17.8% → 33.1%**，覆盖功能大类 **5 → 19 / 47**。
  - 期权 `option`（46）：中金所/上交所/深交所/东财/商品/期货期权历史全量。
  - 债券 `bond`（29）：可转债/现券/国债/回购/发行/中国货币网。
  - 宏观 `economic`（48）：金十 + 东财 datacenter-web + 香港 + 多口径。
  - 股票基本面 `stock_fundamental`（25）：限售股解禁 + 同花顺财务/公司大事。
  - 长尾：`currency` / `energy`（原油/上金所/碳排放/生猪）/ `news` / `fortune`(胡润) / `spot`。
- **补合债券尾巴**：`batch4-bond` 在首次合并后又推进 1 提交（`e27266f`，新浪债券补充 6 + `bond_info_cm_query`，+7），此前漏在 worktree，本次合入（提交 `5a08acb`）。冲突 `core/html.rs`（union 保留 `read_html_tables` 与 `read_html` 两个 API）、`core/http.rs`（保留 `get_json_allow_status` 与 `random_delay`）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(175) 全绿；新增 7 个债券函数 `parity --only` 全部通过。
- **文档**：刷新 [`README.md`](README.md) 覆盖率快照与接口清单；新增 [`README.en.md`](README.en.md) 中英互链。

## [2026-08-11] 批次 1 完成 · 公开函数 195

- **股票特色 `stock_feature`（95）**：龙虎榜全系、沪深港通持股/历史/榜单、财务报表（资产负债/利润/现金流）、千股千评、技术选股（同花顺 `stock_rank_*_ths`）、新股申购/分析师。
- **期货 `futures`（批次 2a/2b）**：五家交易所结算参数（CFFEX/CZCE/GFEX/SHFE/INE）+ 统一入口 `futures_settle` + 新浪合约详情。
- **宏观 `economic`（批次 3c/3f）**：金十中国宏观 14 + 东财 datacenter-web 香港/多口径 11+。
- **股票基本面 `stock_fundamental`（批次 3a/3b）**：限售股解禁 4 + 同花顺财务 8（旧/新系列）。
- **乐咕 `legu`（批次 3e）**：市盈率/市净率/拥挤度/巴菲特指标/股债利差/基金仓位 14 个。
- **同花顺板块/新股/公司大事（批次 3d）**：板块名册/新股/分红/盈利预测/高管持股变动 10 个。
- **基础设施**：`eastmoney` 源层（clist 多节点容灾 / datacenter 报表）、`ths` JS 引擎、HTML 解析（`read_html_tables`）。

## [2026-08-10] 批次 1 启动 · 基础设施与股票基线

- 建立核心管线：`core/http.rs`（指数退避重试 + 多节点容灾 + 反爬特征检测）、`core/df.rs`（`Df` 封装）、`core/js_engine.rs`（rquickjs 执行 akshare 原版加密 JS）。
- **股票/基金/指数基线**：东财行情快照、K 线、资金流、板块、股权质押、机构调研、分红送配、业绩报表等约 100 个接口。
- 差分测试框架：`tools/parity_runner.py` + `src/bin/parity.rs`，对比 Rust 输出与 Python akshare golden fixture（strict/loose 双模式）。
