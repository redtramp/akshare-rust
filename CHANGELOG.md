# 更新日志 (Changelog)

本文件记录 akshare-rust 的主要变更。完整迁移路线图与逐接口状态见 [`PLAN.md`](PLAN.md)。

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

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

## 已知未完成（路线图见 PLAN.md §1.2.1）

- 反爬/加密类暂未实现：`movie`（jm.js 解密）、`air`（crypto.js）、大商所期权 `option_hist_dce`（412）、国家统计局（签名）、雪球登录类（`AuthRequired`）、Excel 源债券（缺 calamine）。
- 余下 28 个长尾大类（futures_derivative/qdii/reits/forex/crypto 等）覆盖率为 0，待批量推进。
