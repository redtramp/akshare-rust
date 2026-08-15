# 更新日志 (Changelog)

本文件记录 akshare-rust 的主要变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

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
