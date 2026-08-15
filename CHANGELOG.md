# 更新日志 (Changelog)

本文件记录 akshare-rust 的主要变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

## [2026-08-15] 批次 29-D · futures 东财期货行情（子组 D）

- **新增公开函数**：**515 → 518**（净 +3）。`src/futures/em.rs`（对应 akshare `futures/futures_hist_em.py`）落地 3 个东财期货行情函数：`futures_hist_table_em`（交易所品种对照表，`futsse-static.eastmoney.com/redis` 多级 `msgid` 展开）、`futures_hist_em`（期货行情 kline，`push2his.eastmoney.com/api/qt/stock/kline/get`，symbol→secid 经四张品种映射表解析，14 字段 kline CSV 取 10 列 `时间/开盘/最高/最低/收盘/涨跌/涨跌幅/成交量/成交额/持仓量`，按 `start_date`/`end_date` 区间过滤并数值化）、`futures_settlement_price_sgx`（新加坡交易所历史结算价，`links.sgx.com` `FUTURE.zip` ZIP 解析，序号经 `push2his` 的 `100.STI` kline 末行索引 +791 推算）。
- **实现覆盖率**：**≈ 47.3%**（518 / 1094 公开 API）；golden 差分验证 **443 fixture / ≈428 去重函数 ≈ 39.1%**，parity 注册用例 503 / 495 唯一函数；futures 大类 **57.1% → 61.4%**（40 → 43 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(222) 全绿；新增 `zip = "0.6"` 依赖（SGX ZIP 解析）。
- **parity 验证（子组 D 3 用例）**：**1 通过 / 2 跳过 / 0 失败**。`futures_hist_table_em` loose 比对通过（3 列 × 1061 行）。`futures_hist_em` 与 `futures_settlement_price_sgx` 均依赖 `push2his.eastmoney.com`（当前环境 TCP 层断连，直连 akshare 同错，属 §1.2.1 #10 EM push2 阻断），无法生成 golden，`--check` 自动跳过，非回归。

## [2026-08-15] 批次 29-C · futures 交易所官方数据（子组 C）

- **新增公开函数**：**497 → 515**（净 +18）。`src/futures/exchange.rs` 落地 18 个交易所官方数据函数，分三组：① 合约信息 `futures_contract_info_*`（中金所 `cffex` / 郑商所 `czce` / 大商所 `dce` / 广期所 `gfex` / 上期能源 `ine` / 上期所 `shfe`，6）——中金所/郑商所用内置扁平 XML 提取器（`xml_records`/`unescape_xml`/`fmt_ymd`）逐 `product` 切片合并，大商所/广期所走 JSON（`dce` 反爬 412 不可实时校验），上期能源/上期所用西甲所 `dailystat` 数据；② 仓单日报 `futures_warehouse_receipt_*`（`czce`/`dce`）与 `futures_shfe_warehouse_receipt`/`futures_gfex_warehouse_receipt`（4）——郑商所/广期所用 `calamine` 解析 `.xls`（BIFF8）/ form POST，上期所用 `dailystock.dat` 的 `o_cursor` 按 `品种` 列合并；③ 交割/期转现/历史行情 `futures_to_spot_shfe`/`futures_delivery_dce`/`futures_to_spot_dce`/`futures_delivery_match_dce`/`futures_to_spot_czce`/`futures_delivery_czce`/`futures_delivery_shfe`/`futures_hist_daily_cffex`（8）——大商所 `publicweb` 走 `read_html_tables` 取首表按列名定位并过滤「小计/总计」，郑商所用 calamine 解析 `.xls`（`skiprows=1`），中金所用 GBK 解码 CSV 按位置映射 12 列。
- **实现覆盖率**：**≈ 47.1%**（515 / 1094 公开 API）；golden 差分验证 **442 fixture / ≈427 去重函数 ≈ 39.0%**，parity 注册用例 500 / 492 唯一函数；futures 大类 **31.4% → 57.1%**（22 → 40 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(218) 全绿；`cargo test --lib` 218 passed。
- **parity 验证（子组 C 18 用例）**：**8 PASS / 10 SKIP / 0 FAIL**。`--check` 比对「列名（有序）+ dtype 类」（不比行数/head 值）：8 个有 golden 者全部通过（contract_info 5 个 `cffex`/`czce`/`gfex`/`ine`/`shfe` + `to_spot_czce` 2列×1行 + `delivery_czce` 3列×7行 + `hist_daily_cffex` 12列×28行）；10 个无 golden 者 `--check` 自动跳过（不计入失败）——其中 5 个因大商所 `publicweb` 反爬（412：`warehouse_receipt_dce`/`delivery_dce`/`to_spot_dce`/`delivery_match_dce` 及 `contract_info_dce`）、2 个因 `tsite.shfe.com.cn` 域名本环境无法解析 DNS（`to_spot_shfe`/`delivery_shfe`）、3 个因上游返回 `dict`（非 DataFrame，按「品种」分节纵向合并为带 `品种` 列的单一 `Df`，同 sub-group B `foreign_commodity_subscribe_exchange_symbol` 先例，不强制 parity：`warehouse_receipt_czce`/`shfe_warehouse_receipt`/`gfex_warehouse_receipt`）。
- **基础设施（影响全工程）**：① `Cargo.toml` 引入 `calamine = "0.26.1"`（`0.27` 因依赖 yanked `zip ~2.5.0` 构建失败，降级锁定）；用于郑商所/广期所 `.xls` 解析（`Xls::new(Cursor)`→`worksheet_range_at(0)`→`Data` 枚举转字符串，Float 整数去尾零）。② `src/core/http.rs` 新增 `get_bytes_with_headers`（返回原始字节，带重试，4xx 立即 Err），供二进制 `.xls`/CSV 端点使用。③ 广期所 `contract_info`/`warehouse_receipt` 由 `post_json`（query 参数无 body，被服务端拒 411）改为 `post_form`（表单体带 `Content-Length`，复用已工作的 gfex settle 端点标头 `GFEX_INFO_HEADERS`）。④ 修复 `regex_first_alpha` 字节切片越界 panic：原用 `chars().enumerate()`（字符索引）却以 `s[s0..end]`（字节索引）切片，中文前缀（如「品种：白糖SR」）会使 `s0` 落在多字节字符内部；改用 `char_indices()` 提供字节偏移，`end = i + c.len_utf8()`（`czce` 仓单日报现可正常输出 10列×1338行，此前 panic）。

## [2026-08-15] 批次 29-A · futures 国际/指数（子组 A）

- **新增公开函数**：**484 → 487**（净 +3）。`src/futures/em_global.rs` 落地 3 个国际期货/商品指数函数：中证商品指数 `futures_index_ccidx`（CCIDX `getDateLine`，全 24 列仅 6 字段中文化、余原样，三字符串列保留 str）+ 东财国际期货实时 `futures_global_spot_em`（`futsseapi.eastmoney.com/list`，复用 `option_current_em` 模板，14 列，`序号` 1 基数值化）+ 东财国际期货历史 `futures_global_hist_em`（push2his kline 日线，`日增` 还原 2^32 回卷）。
- **实现覆盖率**：**≈ 44.5%**（487 / 1094 公开 API）；golden 差分验证 **425 fixture / ≈410 去重函数 ≈ 37.5%**，parity 注册用例 473 / 465 唯一函数；futures 大类 **10.0% → 17.1%**（7 → 12 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(218) 全绿。
- **parity 验证**：`futures_index_ccidx`（24 列×970 行，2 个 symbol 用例）、`futures_global_spot_em`（14 列×620 行）loose 比对（列名+dtype）全部通过；`futures_global_hist_em` 因东财 push2his TCP 断连（直连 akshare 同错，属 §1.2.1 #10 EM push2 阻断）暂无 golden，`--check` 自动跳过，非回归。
- **范围调整**：子组 A 原规划 4 函数（含 `futures_rule_em`），经 `dir(ak)` 确认 `futures_rule_em` 非公开 API（`akshare` 仅含 `futures_rule` 国泰君安 HTML 表），已移除，实落 3 函数。

## [2026-08-15] 批次 29-B · futures 新浪集群（子组 B）

- **新增公开函数**：**487 → 497**（净 +10）。新建 `src/futures/sina.rs`（对应 akshare `futures_zh_sina.py` / `futures_hq_sina.py` / `futures_foreign.py`）落地 10 个新浪期货集群函数：`futures_symbol_mark`（品种↔市场码映射，解析 `qihuohangqing.js` 的 `ARRFUTURESNODES` 对象）+ `futures_zh_realtime`（品种实时合约，`Market_Center.getHQFuturesData`）+ `futures_zh_spot`（实时行情，`hq.sinajs.cn` `nf_` 前缀）+ `futures_zh_daily_sina`（日线 kline，JSONP 短键 `d/o/h/l/c/v/p/s`→标准名）+ `futures_zh_minute_sina`（分钟线 kline）+ `futures_hq_subscribe_exchange_symbol`（外盘品种字典）+ `futures_foreign_commodity_realtime`（外盘实时，人民币报价=最新价×乘数×美元人民币）+ `futures_foreign_commodity_subscribe_exchange_symbol`（外盘可订阅代码，`hf.html` `oHF_1`）+ `futures_foreign_detail`（外盘合约详情，`read_html` 第 7 表 label/value 网格）+ `futures_foreign_hist`（外盘历史日线）。
- **实现覆盖率**：**≈ 45.4%**（497 / 1094 公开 API）；golden 差分验证 **434 fixture / ≈419 去重函数 ≈ 38.3%**，parity 注册用例 482 / 474 唯一函数；futures 大类 **17.1% → 31.4%**（12 → 22 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(218) 全绿；9 个新函数全部 `parity --check` 通过（loose 9/9：symbol_mark 3×86、zh_realtime 23×12、zh_spot 15×1、zh_daily 8×4222、zh_minute 7×1023、hq_subscribe 2×30、foreign_commodity_realtime 14×2、foreign_detail 6×4、foreign_hist 8×2538）；`futures_foreign_commodity_subscribe_exchange_symbol` 上游返回 `list`（非 DataFrame）不入 parity 用例。
- **基础设施修复（影响全工程 HTTP 层）**：① `src/core/http.rs` 新增 gzip/deflate 手动解压——reqwest 0.12 的 `gzip` 特性仅对 **async** 客户端透明解压（经 `tower-http`），**blocking** 客户端不处理，而本工程统一用 blocking 客户端，故 `stock2.finance.sina.com.cn` 等 gzip 端点此前返回乱码；新增 `response_text`/`response_bytes` 按 `Content-Encoding` 解压（引入 `flate2`）。② `src/core/js_engine.rs::js_literal_to_json` 修复双重括号 `({ {...} })` 语法错误（`demjson` 等价还原会吃掉调用方已含的 `{}`），新增字符串感知的 `//` 行注释与 `/* */` 块注释剥离（新浪 `qihuohangqing.js` 尾部带 `// bohai:` 注释）。③ `futures_symbol_mark` 用括号配平提取 `ARRFUTURESNODES = { ... }` 对象（避免 `find('{')`/`rfind('}')` 一路截到文件末尾 JS 函数体）。④ 新浪 JSONP 端点包裹形如 `=([...]);`，`strip_jsonp` 改取首个 `[` 到最后一个 `]`（原 `rfind("];")` 因中间夹 `)` 失败）。⑤ 日线/分钟线短键 `d/o/h/l/c/v/p/s` → 标准列名 `date/open/high/low/close/volume/hold/settle`（akshare 重命名）。⑥ `futures_foreign_detail` 改用 `read_html_tables` 取原始二维表、不把首行当表头（对应 pandas `read_html(header=None)`，6 列整数列名全部 str）。

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
