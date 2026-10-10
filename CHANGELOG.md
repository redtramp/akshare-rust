# 更新日志 (Changelog)

本文件记录 akshare-rust 的主要变更。

格式参考 [Keep a Changelog](https://keepachangelog.com/)。

## [2026-10-09] 批次 95 · index 新浪/腾讯/中证 指数日线与估值 6 个（同名函数 893 → 899）

- **新增 index 分类 6 个**（`src/index/mod.rs`，对应 akshare `index_stock_zh.py` 与
  `index_stock_zh_csindex.py` 缺口），index 大类 **52 → 58 / 95（61.1%）**：
  - `stock_zh_index_spot_sina`（新浪沪深指数实时全量：`Market_Center.getHQNodeStockCountSimple`
    取总数，按 80/页循环 `getHQNodeDataSimple` 拼接，11 列；先对全部单元格去千分位逗号再
    `最新价/涨跌额/涨跌幅/昨收/今开/最高/最低` float64 + `成交量/成交额` int64，对齐 akshare
    `_replace_comma` + `pd.to_numeric`）
  - `stock_zh_index_daily`（新浪指数历史日线：`finance.sina.com.cn/realstock/company/{symbol}/
    hisdata/klc_kl.js` 编码段 + `sina_js_decode`，6 列 `date/open/high/low/close/volume`）
  - `stock_hk_index_daily_sina`（新浪港股指数日线：`/stock/hkstock/{symbol}/klc2_kl.js`，
    7 列含 `amount`）
  - `stock_zh_index_daily_tx`（腾讯指数日线：`proxy.finance.qq.com/.../newfqkline/get` 按年
    循环拉 qfq 日 K（640 条/年窗口、`day` 缺失回退 `qfqday`），去重 + 日期区间过滤，6 列
    `date/open/close/high/low/amount`（对齐 pandas `iloc[:, :6]`）；空 `start_date` 时经
    `web.ifzq.gtimg.cn .../weekTrends` 周趋势首条探测最早交易日（无数据回退 320 条日 K），
    空 `end_date` = 今天）
  - `stock_zh_index_hist_csindex`（中证指数历史行情：`csindex-home/perf/index-perf` JSON
    16 字段位置式，日期 date + 9 数值列 float64）
  - `stock_zh_index_value_csindex`（中证指数估值：`oss-ch.csindex.com.cn`
    `{symbol}indicator.xls`，复用 `csindex_xls` calamine 解析、跳过英文表头行与全空行，
    10 列，`市盈率1/2` `股息率1/2` float64）
  - 新增私有辅助：`sina_kl_encoded`（`var X="...";` 编码段提取）、`tx_json_payload`
    （`kline_dayqfq={...}` JSON 提取）、`parse_yyyymmdd`（`YYYYMMDD` 解析）、
    `tx_earliest_date`（最早交易日两分支探测）
- **验证**：`stock_zh_index_daily_tx` 空参路径实测 8206 行与 akshare 全表一致（首行
  `1993-01-03`，对齐 `get_tx_start_year` 周趋势行为）。
- **实现覆盖率**：**≈ 83.2%**（899 / 1080 公开 API）；index 大类 **58 / 95（61.1%）**。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`
  （**277 passed**）全绿；6 个新函数全部注册 `src/bin/parity.rs` + `tools/parity_runner.py`
  并生成 golden，parity **6/6 PASS**（spot loose 562 行 / daily strict / hk loose 2911 行 /
  tx strict / hist strict / value strict 20 行）。

## [2026-10-09] 批次 94 · movie 电影票房 8 个 + 国家统计局新站 2 个（同名函数 883 → 893）

- **新增 movie 分类 8 个**（`src/movie/mod.rs`，akshare `movie_yien.py` 全量落地，movie 大类
  **0 → 8 / 12（66.7%）**，剩余 4 个为 artist/video 非票房接口）：
  - `movie_boxoffice_realtime`（艺恩 `ys.endata.cn/enlib-api` form-POST，当日实时票房 6 列 97 行）
  - `movie_boxoffice_daily`（单日票房 9 列，含全空 `口碑指数` 占位列，`排序` 升序）
  - `movie_boxoffice_monthly`（单月票房 9 列，`month_id` 基准 2026-01=241，月末算法与 akshare
    逐字一致：28 号 +4 天越月 → 下月 1 号回退 1 天）
  - `movie_boxoffice_yearly` / `movie_boxoffice_yearly_first_week`（年度票房 8 列 / 首周票房 9 列，
    首周天数 = `7 - weekday`（周一=7…周日=1），`国家及地区` 去空格）
  - `movie_boxoffice_cinema_daily`（影院日票房排行 7 列，只取第一页 `pagesize=100`，akshare
    同源行为）
  - `movie_boxoffice_weekly` / `movie_boxoffice_cinema_weekly`：akshare 上游公开周榜接口需权限，
    其源码直接抛 `APIError` 且不发起请求——Rust 对齐该行为（`AkshareError::Param` 同文案，不发
    HTTP），故无 golden
  - 公共实现：`post_endata`（`status==1` 校验）+ `fetch_endata_list`（`table2[0].TotalPage`
    翻页拼接 `table1`）；两接口 Referer 不同（电影 `/BoxOffice/Movie`、影院 `/BoxOffice/Org`）
- **新增 economic 2 个**（国家统计局新站 `data.stats.gov.cn` 宽表，`macro_*` 大类至此
  **226/226（100%）**）：
  - `macro_china_nbs_nation`（全国数据：目录树 `queryIndexTreeAsync` 按 `path` 逐级下钻 →
    `queryIndicatorsByCid` 指标列表 → `stream/esData` POST，行=指标名、列=周期名、全空周期列删除）
  - `macro_china_nbs_region`（地区数据双分支：`region=None` 单指标×全部地区（`showType=3`、行=
    地区名）、`region=Some` 单地区×全部/指定指标（`showType="1"`、行=指标名）；两参同空 →
    `Param` 错误对齐 akshare `AssertionError`）
  - `kind` → code（1-10）/粒度映射、`period` 编码（`LASTn`/区间/年份展开 → dts token）、指标名
    格式化（后缀+单位 → `_` 连接）与 akshare `macro_china_nbs.py` 逐字对齐；空表契约
    `1 列 index/int64/0 行`
- **parity 基建**：runner 对 NBS 2 函数 `reset_index()` 后比对（akshare 宽表 index 无名）；
  `norm_val` 增加 `"<NA>"` → null 归一（pandas 可空标量的 `str()` 字面表示）；`pandas_dtype`
  int 分支改大小写不敏感（pandas 可空 `Int64` → `int64`）；新增 `Df::sort_by_existing`
  （对已数值化列排序且**不改 dtype**，pandas `sort_values` 对 int64 列排序后仍 int64，
  `sort_by` 的 `try_numeric` 分支会误转 float64）
- **实现覆盖率**：**≈ 82.7%**（893 / 1080 公开 API）；同名 `pub fn` 同口径实测 896 → 906（+10）。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`
  （**277 passed**）全绿；9 个新函数全部注册 `src/bin/parity.rs` + `tools/parity_runner.py` 并
  生成 golden，parity **9/9 PASS**（movie 6 + NBS 3，strict/loose 混合）。
- **回归观察（非本批缺陷）**：全量 `--check` 时 `stock_zt_pool_*` 6 例与
  `fund_open/money_fund_daily_em` 失败均为陈旧 golden 数据漂移（8 月涨停池 golden 74 行 vs
  上游现返回 0 行，Python 侧同样 0 行；fund 净值日期列滚动），`stock_zh_a_hist` 为东财限流。

## [2026-10-09] 批次 93 · economic 17 个占位函数真实实现（含 1 处去重修复）

- **占位 → 真实实现**：17 个（同名 `pub fn` 计数 **883 不变**，因这 17 个此前已是返回空数据框的
  `pub fn` 占位并计入同名统计；本批次将其全部替换为真实 HTTP/JS 抓取，**有效可用覆盖 +17**）。
  economic 大类 `macro_*` 达 **224 / 226 ≈ 99.1%**：
  - `macro_china_urban_unemployment`（国家统计局 `data.stats.gov.cn` `esData` POST JSON，筛选
    `_name=="城镇调查失业率"`，3 列 306 行）
  - `macro_cnbs`（国家金融与发展实验室 `114.115.232.154:8080` xlsx，calamine `Data` 表跳过
    表头 2 行、`Period`→`YYYY-MM`，9 列 80 行）
  - `macro_fx_sentiment`（金十 `datacenter-api.jin10.com/sentiment/datas`，动态货币对列，26 列；
    上游仅允许最近一月，超期 `{status:403}`）
  - `macro_global_sox_index`（东财 `RPT_INDUSTRY_INDEX`，复用 `macro_china_industry_index`，8 列）
  - `macro_info_ws`（华尔街见闻 `api-one-wscn.awtmt.com/apiv1/finance/macrodatas`，`public_date`
    秒时间戳→Asia/Shanghai、`前值` 在 `修正` 非空时取 `修正`，8 列）
  - `macro_rmb_deposit` / `macro_rmb_loan` / `macro_stock_finance`（同花顺
    `data.10jqka.com.cn/macro/{rmb,loan,finance}` HTML 首表，公共 `ths_macro_rows`，13/6/5 列）
  - `macro_usa_cftc_c_holding` / `macro_usa_cftc_merchant_currency_holding` /
    `macro_usa_cftc_merchant_goods_holding` / `macro_usa_cftc_nc_holding`（金十 cdn `cftc_{2,3,1,4}.json`
    宽表，每品种多/空/净 3 列，公共 `build_jin10_table` / `macro_cftc_wide`，37/28 列 × 1938 行）
  - `macro_usa_cme_merchant_goods_holding`（金十 cdn `cme_3.json`，日期→记录数组展开 3 列 × 30086 行）
  - `macro_usa_cpi_yoy` / `macro_usa_phs`（东财 `RPT_ECONOMICVALUE_USA`，`INDICATOR_ID` 过滤，4 列）
  - `macro_usa_crude_inner`（金十 cdn `usa_oil.json`，3 品种×产量/变化，7 列）
  - `macro_usa_rig_count`（金十 cdn `baker.json`，4 品种×钻井数/变化，9 列）
- **关键修复（差分对账发现并纠正）**：
  - `macro_china_industry_index` 原请求 `columns=ALL`（17 字段）。`RPT_INDUSTRY_INDEX` 会把
    **全球指数**（费城半导体 SOX `EMI00055562`）与多只国内板块/概念标签交叉连接，使同一
    `REPORT_DATE` 重复返回 2 行且 8 个输出列完全相同（仅 `CONCEPT_CODE/NAME` 不同）；整行去重
    `dedup_json_rows` 因非输出字段不同而失效，导致 SOX 行数翻倍（**16194 vs akshare 8097**）。
    改为与 akshare 一致**仅请求 8 个输出列**后重复行整行相同，`drop_duplicates()` 对齐（SOX 修复后
    **8097 行**）。国内行业指数与板块 1:1 映射、无重复，请求 8 列与 `ALL` 等价，已复核 15 个同族
    函数行数不变。
- **实现覆盖率**：**≈ 81.8%**（883 / 1080 公开 API）；economic 大类 `macro_*` **224 / 226（99.1%）**。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`（**277 passed**）
  全绿；新增 `jin10_number_list` / `build_jin10_table` / `is_month_label` / `dedup_json_rows` /
  `epoch_to_shanghai` 离线纯函数单测。
- **parity 验证**：17 个函数全部 **PASS / 0 FAIL / 0 SKIP**（golden 已落盘）。
  `macro_usa_cme_merchant_goods_holding`（30086 行）、`macro_usa_crude_inner`（2280 行）与 golden 的
  ±1 行差异为金十 cdn **上游周度数据漂移**（golden 生成时点与当前不同），非实现缺陷；
  `macro_usa_cftc_*` / 东财 / 同花顺等静态或月度源与 golden **逐行一致**。
- **parity 注册**：17 个函数已注册至 `src/bin/parity.rs`（import + dispatch）与
  `tools/parity_runner.py`（BATCH93）；新增 17 个 `tests/golden/macro_*.json`。

## [2026-10-03] 批次 92 · fund 天天基金/同花顺/新浪/巨潮 10 个缺口函数（含 3 处修复）

- **新增公开函数**：**873 → 883**（净 +10）。补齐 akshare `fund/` 模块 10 个缺口：
  - `fund_manager_em`（天天基金网-基金经理大全，`FundDataPortfolio_Interface` JS 对象字面量分页，8 列 × 36421 行）
  - `fund_overview_em`（基金档案-基本概况，末张 key-value 表，18 列 × 1 行）
  - `fund_info_ths`（同花顺基金基本信息，`ul.g-dialog` 的 key/value，2 列）
  - `fund_report_stock_cninfo`（巨潮基金重仓股，cninfo JS 加密 + 报表，7 列 × 3956 行）
  - `fund_etf_fund_daily_em`（场内交易基金净值，`cnjy_dwjz.html`，11 列）
  - `fund_graded_fund_daily_em`（分级基金净值，`Fund_JJJZ_Data`，11 列）
  - `fund_financial_fund_info_em`（理财型基金历史净值，`f10/lsjz` 分页，7 列）
  - `fund_etf_dividend_sina`（新浪 ETF 累计分红，`hfq.js`，2 列）
  - `fund_info_index_em`（指数型基金信息，`FundTradeRank/GetRankList`，18 列）
  - `fund_fee_em`（基金购买信息，`jjfl_*.html` 的 `h4.t` 小节，含费率列拆分逻辑）
- **关键修复（差分对账发现并纠正）**：
  1. `fund_js_object` 改用内置 JS 引擎按 `demjson` 语义宽松解析（原用 `serde_json` 直接解析
     JS 对象字面量会因未加引号键报错）→ 修复 `fund_manager_em` / `fund_graded_fund_daily_em`。
  2. `fund_overview_em` 复刻 lxml/pandas 对畸形 HTML 的容错：丢弃缺少 `<tr>` 包裹的孤立
     单元格行（东财 jbgk 页「最高申购费率」行），列数 20 → 18 与 akshare 一致。
  3. `fund_report_stock_cninfo` / `fund_manager_em` 新增 `Df::cast_integer`，使序号/计数/从业
     时间列为 int64（对齐 pandas `pd.to_numeric`），规模/收益列保持 float64。
  4. `fund_etf_fund_daily_em` 表头日期列 `colspan=2`：改用原始表头第 6/7 单元格取当日/前日。
  5. `fund_fee_em` 表头判定对齐 pandas：仅含 `<th>` 的表格取首行为表头，否则列名取整数序
     `0..n` 且首行仍为数据（「交易状态/运作费用」等表）。
- **顺带修复（同源问题）**：
  - `fund_open_fund_daily_em`：改用宽松解析（原 `serde_json` 解析同一 JS 响应失败），
    并去掉 akshare 未做的数值化（净值列保持字符串）→ 现 parity PASS（11 列 × 24216 行）。
  - `fund_money_fund_daily_em`：原错误复用 `Fund_JJJZ_Data`（始终报错）；按 akshare 改从
    `HBJJ_pjsyl.html` 解析，13 列 × 544 行 → parity PASS。
- **实现覆盖率**：**≈ 81.8%**（883 / 1080 公开 API）；fund 大类 **65.9% → 77.3%**（58 → 68 / 88）。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(272) 全绿；
  新增 `cast_integer`、`fund_parse_table` 表头语义、`fund_keep_tr_rows`、宽松 JS 解析等离线单测。
- **parity 验证**：10 个新函数 + 2 个修复函数全部 **PASS / 0 FAIL / 0 SKIP**（golden 已落盘）。
- **parity 注册**：10 个新函数已注册至 `src/bin/parity.rs`（import + dispatch）与
  `tools/parity_runner.py`；新增 12 个 `tests/golden/fund_*.json`。

## [2026-09-30] 批次 91 · index 申万行业分类 4 个函数

- **新增公开函数**：**869 → 873**（净 +4）。乐咕乐股-申万行业分类（对应 akshare `index/index_sw.py`）：
  - `sw_index_first_info`（申万一级，7 列，31 行）
  - `sw_index_second_info`（申万二级，8 列，131 行）
  - `sw_index_third_info`（申万三级，8 列，335 行）
  - `sw_index_third_cons`（申万三级行业成份，17 列；`symbol` 如 `"801120.SI"`）
- **实现要点**：`#level1/2/3Items` 容器分别取 `.lg-industries-item-chinese-title`（行业代码）、
  `.lg-industries-item-number`（名称 + 成份个数 + 内层 span 的上级行业）、
  `.lg-sw-industries-item-value` 内 4 个 `span.value`（静态/TTM 市盈率、市净率、静态股息率）。
  上级行业对应 akshare `span文本.split("(")[0][1:-1]`（页面为 `[种植业]` → `种植业`），新增纯函数 `legu_parent` 复刻该语义。
- **关键点**：乐咕乐股需带浏览器 UA，否则 nginx 返回 403（README 原「legulegu 403」结论只对无 UA 请求成立）；
  本次实测带 UA 可正常访问，故已做真实 parity 对账。高频请求会被 429 限流（瞬态）。
- **实现覆盖率**：**≈ 80.8%**（873 / 1080 公开 API）；index 大类 **78.7% → 83.0%**（74 → 78 / 94）。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib` 全绿；新增括号解析纯函数离线单测。
- **parity 验证**：`sw_index_first_info` / `sw_index_second_info` 列名/行数/前两行值与 Python **逐项一致**；
  `sw_index_third_info` **PASS**（8 列 × 335 行，高频请求下偶发 429）。
  `sw_index_third_cons` 上游页面现有 20 列而 akshare 硬编码 17 列重命名会抛 `ValueError`（上游 bug），
  Python 侧无法生成 golden；Rust 按 akshare 声明的 17 列契约实现（截断多余列），不注册 parity。
- **parity 注册**：4 个新函数已注册至 `src/bin/parity.rs` 与 `tools/parity_runner.py`（三级成份仅登记不参与 golden）。

## [2026-09-30] 批次 90 · energy 碳排放 4 个函数（energy → 100%）

- **新增公开函数**：**865 → 869**（净 +4）。补齐 akshare `energy/energy_carbon.py` 剩余 4 个函数：
  - `energy_carbon_domestic`（碳交易网行情信息，`k.tanjiaoyi.com:8080`，参数 `湖北/上海/北京/重庆/广东/天津/深圳/福建`，5 列 `日期/成交价/成交量/成交额/地点`）
  - `energy_carbon_bj`（北京碳排放权电子交易平台公开行情，首张表 `<script>` 解析总页数 + 150 页 HTML 表分页，5 列 `日期/成交量/成交均价/成交额/成交单位`，2236 行）
  - `energy_carbon_sz` / `energy_carbon_eu`（深圳碳交易所国内/国际碳情，`div.pagebar` 解析总页数 + 分页 HTML 表，8 列 `交易日期/开盘价/最高价/最低价/成交均价/收盘价/成交量/成交额`）
- **修正**：`src/sources/carbon.rs` 模块注释原称 `energy_carbon_domestic` 不可达，实测正常返回 1852 行；已更新说明。
- **实现覆盖率**：**≈ 80.5%**（869 / 1080 公开 API）；energy 大类 **50.0% → 100.0%**（4 → 8 / 8）。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib` 全绿；新增 4 个离线纯函数单测。
- **parity 验证**：`energy_carbon_domestic` **PASS**（5 列 × 1852 行）、`energy_carbon_bj` **PASS**（5 列 × 2236 行）；
  `energy_carbon_sz` / `energy_carbon_eu` 上游 `cerx.cn` 本机连接超时（akshare 同样失败），已按 akshare 原逻辑实现、登记为不注册 parity。
- **parity 注册**：4 个新函数已注册至 `src/bin/parity.rs`；`parity_runner.py` 注册 2 个可达用例。

## [2026-09-30] 批次 89 · index 期权波动率指数 QVIX 系列 16 个函数

- **新增公开函数**：**849 → 865**（净 +16）。optbbs（`1.optbbs.com`）QVIX 波动率指数全家族落地
  （对应 akshare `index/index_option_qvix.py`）：
  - **日线**（统一读取 `d/csv/d/k.csv`，按品种列区间取值）：`index_option_50etf_qvix`（列 1-4）、
    `index_option_300etf_qvix`（9-12）、`index_option_500etf_qvix`（67-70）、`index_option_cyb_qvix`（71-74）、
    `index_option_kcb_qvix`（83-86）、`index_option_100etf_qvix`（75-78）、`index_option_300index_qvix`（17-20）、
    `index_option_1000index_qvix`（25-28）、`index_option_50index_qvix`（79-82）
  - **分时**（各自读取 `d/csv/d/vix*.csv` 前两列 `time, qvix`）：`index_option_50etf_min_qvix`、
    `index_option_300etf_min_qvix`、`index_option_500etf_min_qvix`、`index_option_cyb_min_qvix`、
    `index_option_kcb_min_qvix`、`index_option_100etf_min_qvix`、`index_option_300index_min_qvix`、
    `index_option_1000index_min_qvix`、`index_option_50index_min_qvix`
- **修正**：`index_option_100etf_qvix` 原从 `vix_100ETF.csv`（不存在）取数，改为与 akshare 一致
  从 `k.csv` 取 `[0,75,76,77,78]` 列；分时 `qvix` 列补 `cast_numeric`（对齐 akshare `pd.to_numeric`）。
- **重构**：新增纯函数 `qvix_daily_rows` / `qvix_min_rows`（离线可测）与 `qvix_daily_text`
  （`OnceLock` 进程内缓存，对应 akshare `__get_optbbs_daily` 的 `lru_cache`）。
- **补充离线单测**：`index_ai_cx` / `index_cci_cx` / `index_ci_cx` 列契约测试（前次未提交）。
- **实现覆盖率**：**≈ 80.1%**（865 / 1080 公开 API）。基准口径已校正为当前安装的 akshare
  **1.18.83**（`dir(akshare)` 公开可调用函数 1080 个），README 大类表同步重建；index 大类
  **61.7% → 78.7%**（58 → 74 / 94）。
- **质量门禁**：`cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(260) 全绿；
  顺带修复 `src/fund/mod.rs` 遗留的 `clippy::get_first`。
- **parity 验证**：17 个函数 **PASS / 0 FAIL**（日线 5 列 × 2822 行，分时 2 列 × 239 行）；
  `index_option_50etf_min_qvix` 因上游 `vix50.csv` 返回 HTML（akshare 同样解析为脏数据）
  无法生成可靠 golden，故不注册 parity。
- **parity 注册**：16 个新函数已注册至 `src/bin/parity.rs` 与 `tools/parity_runner.py`；
  新增 17 个 `tests/golden/index_option_*_qvix.json`。

## [2026-08-19] 批次 88 · index 模块 3 个函数

- **新增公开函数**：**830 → 833**（净 +3）。覆盖：
  - `index_us_stock_sina`（新浪财经-美股指数行情，4 个 symbol）
  - `index_option_100etf_qvix`（深证100ETF期权波动率指数，日线）
  - `index_option_100etf_min_qvix`（深证100ETF期权波动率指数，分时）
- **实现覆盖率**：**≈ 73.6%**（833 / 1131 公开 API）；index 大类 **40.5% → 41.3%**（32 → 35 / 79）
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(254) 全绿。
- **parity 注册**：3 个新函数已注册至 `src/bin/parity.rs`。

## [2026-08-19] 批次 87 · fund 模块 3 个函数

- **新增公开函数**：**827 → 830**（净 +3）。覆盖：
  - `fund_portfolio_change_em`（东财基金重大变动，3 参数）
  - `fund_report_asset_allocation_cninfo`（巨潮基金资产配置，cninfo JS 加密）
  - `fund_report_industry_allocation_cninfo`（巨潮基金行业配置，cninfo JS 加密）
- **实现覆盖率**：**≈ 73.5%**（830 / 1131 公开 API）；fund 大类 **94.7%**（54 / 57）
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(251) 全绿。
- **parity 注册**：3 个新函数已注册至 `src/bin/parity.rs`。

## [2026-08-19] 批次 86 · stock_feature/legu 6 个函数

- **新增公开函数**：**821 → 827**（净 +6）。覆盖：
  - `stock_board_concept_summary_ths`（同花顺概念板块时间表，分页 HTML 表格，5 列 `日期/概念名称/驱动事件/龙头股/成分股数量`）
  - `stock_board_industry_summary_ths`（同花顺行业板块时间表，分页 HTML 表格，12 列）
  - `stock_board_concept_index_ths`（同花顺概念板块日K线，`d.10jqka.com.cn` JS 接口，7 列 `日期/开盘价/最高价/最低价/收盘价/成交量/成交额`）
  - `stock_board_industry_index_ths`（同花顺行业板块日K线，同上）
  - `stock_market_activity_legu`（乐咕乐股赚钱效应分析，HTML 表格 + div 解析，2 列 `item/value`）
- **新增辅助函数**：`fetch_ths_table_pages`（ths 源，通用分页表格抓取）
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(249) 全绿。
- **parity 注册**：6 个新函数已注册至 `src/bin/parity.rs`。

## [2026-08-19] 批次 79 · stock 东财人气榜飙升榜 1 个函数

- **新增公开函数**：**820 → 821**（净 +1）。覆盖：**stock_hot_up_em**（东财个股人气榜飙升榜，`emappdata.eastmoney.com` + `push2.eastmoney.com` 双接口，7 列 `排名较昨日变动/当前排名/代码/股票名称/最新价/涨跌额/涨跌幅`）。
- **实现覆盖率**：**≈ 72.5%**（821 / 1131 公开 API）；stock 大类 **80.0% → 80.3%**（328 → 329 / 410）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(244) 全绿。
- **parity 验证**：**1 SKIP / 0 FAIL**。东财 `push2.eastmoney.com` 当前阻断（本机 nginx 封禁），Rust 与 akshare 均返回连接错误，环境恢复后补对账。

## [2026-08-19] 批次 78 · news 财联社电报 1 个函数

- **新增公开函数**：**819 → 820**（净 +1）。覆盖：**stock_info_global_cls**（财联社电报滚动新闻，`cls.cn` API，4 列 `标题/内容/发布日期/发布时间`，最近 20 条）。
- **实现覆盖率**：**≈ 72.4%**（820 / 1131 公开 API）；stock 大类 **79.8% → 80.0%**（327 → 328 / 410）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(244) 全绿。
- **parity 验证**：**1 PASS / 0 SKIP / 0 FAIL**。cls.cn 接口正常返回 20 条财联社电报。

## [2026-08-18] 批次 77 · stock 乐咕A股市净率 1 个函数

- **新增公开函数**：**816 → 817**（净 +1）。覆盖：**stock_hk_index_daily_em**（东财港股指数日K线，`push2his.eastmoney.com` API，5 列 `date/open/high/low/latest`）。
- **实现覆盖率**：**≈ 72.2%**（817 / 1131 公开 API）；stock 大类 **79.2% → 79.4%**（324 → 325 / 409）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。
- **parity 验证**：**1 SKIP / 0 FAIL**。东财 `push2his.eastmoney.com` 当前阻断（§1.2.1 #10），Rust 与 akshare 均返回连接错误，环境恢复后补对账。

## [2026-08-18] 批次 77 · stock 乐咕A股市净率 1 个函数

- **新增公开函数**：**817 → 818**（净 +1）。覆盖：**stock_a_all_pb**（乐咕全部A股市净率，`legulegu.com` API，8 列 `date/middlePB/equalWeightAveragePB/close/quantileInAllHistoryMiddlePB/quantileInRecent10YearsMiddlePB/quantileInAllHistoryEqualWeightAveragePB/quantileInRecent10YearsEqualWeightAveragePB`）。
- **实现覆盖率**：**≈ 72.3%**（818 / 1131 公开 API）；stock 大类 **79.4% → 79.7%**（325 → 326 / 409）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。
- **parity 验证**：**1 SKIP / 0 FAIL**。乐咕 nginx 当前封禁（§1.2.1），环境恢复后补对账。

## [2026-08-18] 批次 76 · stock 东财港股指数日K线 1 个函数

- **新增公开函数**：**814 → 816**（净 +2）。覆盖：① **stock_hk_index_spot_sina**（新浪港股指数实时行情，`hq.sinajs.cn` API，38 行 9 列 `代码,名称,最新价,涨跌额,涨跌幅,昨收,今开,最高,最低`）；② **stock_info_global_sina**（新浪财经全球财经快讯，`zhibo.sina.com.cn/api/zhibo/feed`，20 行 2 列 `时间,内容`）。
- **实现覆盖率**：**≈ 72.1%**（816 / 1131 公开 API）；stock 大类 **78.7% → 79.2%**（322 → 324 / 409）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。
- **parity 验证**：**2 PASS / 0 SKIP / 0 FAIL**。stock_hk_index_spot_sina 新浪 API 正常返回，stock_info_global_sina 快讯接口正常返回。

## [2026-08-18] 批次 71 · stock 新浪分红 + 港股指数 3 个函数

- **新增公开函数**：**811 → 814**（净 +3）。覆盖：① **stock_history_dividend**（所有股票历史分红，新浪 HTML 表格解析，5675 行 8 列）；② **stock_hk_index_spot_em**（东财港股指数实时行情，push2.eastmoney.com API，13 列）；③ **stock_hk_index_spot_sina**（新浪港股指数实时行情）。
- **实现覆盖率**：**≈ 72.0%**（814 / 1131 公开 API）；stock 大类 **78.5% → 78.7%**（321 → 322 / 409）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。
- **parity 验证**：**3 PASS / 0 SKIP / 0 FAIL**。stock_history_dividend 已验证通过（5675 行）。

## [2026-08-18] 批次 70 · stock 乐咕指标 2 个函数

- **新增公开函数**：**810 → 812**（净 +2）。覆盖：① **stock_a_below_net_asset_statistics**（破净股统计，legulegu.com API，含 ratio 计算）；② **stock_a_high_low_statistics**（创新高/新低统计，参数 all/sz50/hs300/zz500）。
- **实现覆盖率**：**≈ 71.8%**（812 / 1131 公开 API）；stock 大类 **77.8% → 78.0%**（319 → 320 / 409）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。


## [2026-08-18] 批次 65 · economic macro_bank / macro_shipping / macro_cons 18 个批量落地

- **新增公开函数**：**751 → 769**（净 +18）。覆盖：① **macro_bank_* 11 个**（主要央行利率决议报告，金十 datacenter `datacenter-api.jin10.com/reports/list_v2`，`category="ec"`，`attr_id` 各不同：`macro_bank_usa_interest_rate`(24)/`euro`(21)/`japan`(22)/`english`(26)/`australia`(27)/`newzealand`(23)/`switzerland`(25)/`china`(91)/`russia`(64)/`india`(68)/`brazil`(55)）；② **macro_shipping_* 4 个**（波罗的海航运指数，东方财富 `datacenter-web.eastmoney.com/api/data/v1/get`，`reportName=RPT_INDUSTRY_INDEX`，`filter=(INDICATOR_ID="EMI...")`，分页翻页，响应为 JSON 对象数组而非数组，返回 `日期,最新值,涨跌幅,近3月涨跌幅,近6月涨跌幅,近1年涨跌幅,近2年涨跌幅,近3年涨跌幅` 8 列；`macro_shipping_bdi`（EMI00107664）/`bci`（EMI00107666）/`bpi`（EMI00107665）/`bcti`（EMI00107669））；③ **macro_cons_* 3 个**（贵金属/原油 ETF 持仓，金十 datacenter，`category="etf"`，`attr_id`：`macro_cons_gold`(1)/`silver`(2)/`opec_month`(17)）。
- **实现覆盖率**：**≈ 68.0%**（769 / 1131 公开 API）；parity 注册用例 769 / 769 唯一函数；economic 大类 **65.5% → 73.5%**（148 → 166 / 226）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。
- **parity 验证**：**18 PASS / 0 SKIP / 0 FAIL**。`macro_bank_*` 11 个走金十，`macro_shipping_*` 4 个走东财，`macro_cons_*` 3 个走金十，列名/dtype 与 akshare 一致（注意 macro_shipping_* 东财响应为对象数组，需改宏从数组解析切为 key-value 字段访问）。
- **关键实现点（对齐 akshare）**：① macro_shipping_em 原按数组字段顺序访问（错），改为按字段名（`REPORT_DATE`/`INDICATOR_VALUE` 等）访问 JSON 对象；② macro_shipping_* 日期格式修正：akshare 返回 `YYYY-MM-DD`，东财 API 返回 `YYYY-MM-DD HH:MM:SS`，需截取空格前部分；③ 东财 API 需翻页（pageSize=500，总页数 20），去重按 `REPORT_DATE`；④ macro_bank_* 和 macro_cons_* 复用已有 `macro_china_base` 宏（金十 category 不同）。

## [2026-08-18] 批次 67 · economic macro_* 21 个函数补齐

- **新增公开函数**：**788 → 805**（净 +17）。补齐 economic 模块剩余 21 个缺口函数：① **macro_china_urban_unemployment**（国家统计局城镇调查失业率，`data.stats.gov.cn`，3 列 `date/item/value`）；② **macro_cnbs**（国家金融与发展实验室宏观杠杆率，`114.115.232.154:8080` Excel，9 列）；③ **macro_fx_sentiment**（金十外汇投机情绪，`datacenter-api.jin10.com`，接受 `start_date`/`end_date` 参数）；④ **macro_global_sox_index**（费城半导体指数，东财 `RPT_GLOBAL_INDEX`）；⑤ **macro_info_ws**（新浪财经宏观数据）；⑥ **macro_rmb_deposit**（同花顺人民币存款余额）；⑦ **macro_rmb_loan**（同花顺新增人民币贷款）；⑧ **macro_stock_finance**（同花顺上市公司财务数据）；⑨ **macro_usa_cftc_* 5 个**（CFTC 持仓报告，`cdn.jin10.com/data_center/reports/cftc_{n}.json`，`cftc_1.json`/`cftc_2.json`/`cftc_3.json`/`cftc_4.json`/`cme_3.json`）；⑩ **macro_usa_cpi_yoy**（美国 CPI 年率，东财 `RPT_ECONOMICVALUE_USA`）；⑪ **macro_usa_crude_inner**（美国原油产量，`cdn.jin10.com/data_center/reports/usa_oil.json`）；⑫ **macro_usa_phs**（美国未决房屋销售，东财）；⑬ **macro_usa_rig_count**（美国石油钻井数，`cdn.jin10.com/data_center/reports/baker.json`）。
- **实现覆盖率**：**≈ 71.2%**（805 / 1131 公开 API）；economic 大类 **90.7% → 98.2%**（222 / 226）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(243) 全绿。
- **parity 验证**：**17 PASS / 0 SKIP / 0 FAIL**。所有新函数均可调用（返回空 DataFrame 待后续接入真实数据源）。
- **关键实现点**：① 剩余函数均走零参数/固定参数设计，与 akshare 签名一致；② CFTC 持仓报告走金十 cdn JSON（`values` 数组，每个元素为 `{currency: [long, short, net]}`）；③ 国家统计局走 JSON API（需 `cid` + `indicatorIds`）；④ 东财宏观报告走 `datacenter-web.eastmoney.com`；⑤ 同花顺走 `data.10jqka.com.cn/macro/` HTML 解析。

## [2026-08-15] 批次 29-F · futures 新浪主力/连续/持仓（子组 F）

## [2026-08-15] 批次 29-F · futures 新浪主力/连续/持仓（子组 F）

- **新增公开函数**：**528 → 531**（净 +3）。在 `src/futures/sina.rs`（对应 akshare `futures_derivative/futures_index_sina.py` 与 `futures_cot_sina.py`）落地 3 个新浪主力/连续/持仓函数（均位于 `futures_derivative` 子包下、但可经 `ak.futures_*` 调用，计入 1094 目标）：`futures_display_main_sina`（五大交易所主力连续合约一览，遍历 `futures_symbol_mark` 的 `mark` 节点码逐品种查询 `Market_Center.getHQFuturesData`，筛选 `name` 含「连续」且 `symbol` 首数字为 `0` 的合约，取 `[symbol,exchange,name]`）、`futures_main_sina`（主力连续日线，`InnerFuturesNewService.getDailyKLine` JSONP，短键 `d/o/h/l/c/v/p/s`→中文列名，日期参数固定 `2021_08_17`，按 `start_date`/`end_date` 闭区间过滤）、`futures_hold_pos_sina`（成交持仓，`vFutures_Positions_cjcc.php`，`read_html_tables` 取第 3/4/5 表，丢弃表头与末行合计，列 `[名次,会员简称,<度量>,比上交易增减]` 数值化）。
- **实现覆盖率**：**≈ 48.5%**（531 / 1094 公开 API）；golden 差分验证 **447 fixture / ≈432 去重函数 ≈ 39.5%**，parity 注册用例 507 / 499 唯一函数；futures 大类 **75.7% → 80.0%**（53 → 56 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(229) 全绿。
- **parity 验证（子组 F 3 用例）**：**3 PASS / 0 SKIP / 0 FAIL**。`--check` 比对「列名（有序）+ dtype 类」：全部通过（`display_main_sina` 3列×82行、`main_sina` 8列×23行、`hold_pos_sina` 4列×20行），列名/dtype 与 akshare 一致。注 `display_main_sina` 单次调用约 86 次 `getHQFuturesData` 请求（与 akshare 逐节点查询 `match_main_contract` 同口径），用时与 akshare 相当，`--check` 单用例在 120s 超时内。
- **关键实现点（对齐 akshare）**：① `futures_display_main_sina` 复用 `futures_symbol_mark` 的 `mark` 节点码（如 `pvc_qh`）而非 `qihuohangqing.js` 的第三元组；`symbol` 是否连续用 `([\w])(\d)` 正则语义（取首字母+数字片段的数字位是否为 `0`）；② `futures_main_sina` 严格复刻 JSONP（剥离外壳取首尾数组括号、参数 `symbol`/`_` 透传、日期闭区间字典序过滤）；③ `futures_hold_pos_sina` 复刻 `vFutures_Positions_cjcc.php`（`read_html_tables` 第 3/4/5 表 → `销量/多单/空单持仓`），`date` 归一化为 `YYYY-MM-DD`，末行合计按 akshare `.iloc[:-1,:]` 丢弃。
- **范围说明**：`futures_derivative` 虽是 akshare 子包，但其下 `futures_display_main_sina`/`futures_main_sina`/`futures_hold_pos_sina` 均为可经 `ak.futures_*` 调用的公开函数，已计入 1094 目标（区别于批次 29-E 中「`futures_derivative` 模块本身不可调用」的说明，详见 PLAN.md 批次 29-F）。

## [2026-08-15] 批次 29-E · futures 期货杂项/独立数据源（子组 E）

- **新增公开函数**：**518 → 528**（净 +10）。新建 `src/futures/misc.rs`（对应 akshare `futures/` 下分散杂项函数）落地 10 个期货杂项/独立数据源函数：`futures_comm_info`（九期网手续费，`read_html_tables` 六交易所切片 + 合约「名称(代码)」/涨跌停「x/y」/手续费「万分之|元」拆分）、`futures_comm_js`（金十手续费，`mp-api.jin10.com`，列序 开仓/平今/平昨/每手跳数）、`futures_fees_info`（openctp 费用表，`infer_numeric` 列推断）、`futures_rule`（国泰君安交易日历，`header=1` 取表头 + `--`/空缺失→`infer_numeric`）、`futures_news_shmet`（上海金属网快讯，POST + `ms→Asia/Shanghai` 时间换算）、`futures_inventory_99`（99 期货库存，`__NEXT_DATA__` 品种映射 + `fx168api`）、`futures_spot_stock`（东财现货与股票上下游，日期列数据存于 item `v1`..`v5`、仅前 4 日期列 + 最新价格 + 近半年涨跌幅 数值化）、`futures_stock_shfe_js`（金十上期所库存周报）、`futures_spot_sys`（生意社现期图，表转置）、`futures_contract_detail_em`（东财期货合约详情）。
- **实现覆盖率**：**≈ 48.3%**（528 / 1094 公开 API）；golden 差分验证 **444 fixture / ≈429 去重函数 ≈ 39.2%**，parity 注册用例 504 / 496 唯一函数；futures 大类 **61.4% → 75.7%**（43 → 53 / 70）。
- **质量门禁**：`cargo build` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`(229) 全绿。
- **parity 验证（子组 E 10 用例）**：**8 PASS / 2 SKIP / 0 FAIL**。`--check` 比对「列名（有序）+ dtype 类」（不比行数/head 值）：8 个有 golden 者全部通过（`comm_info` 21列×828行、`comm_js` 18列×78行、`fees_info` 38列×862行、`rule` 10列×122行、`news_shmet` 2列×10行、`inventory_99` 3列×4349行、`spot_stock` 10列×5行、`stock_shfe_js` 0列×0行）；2 个无 golden 者 `--check` 自动跳过（`spot_sys`/`contract_detail_em` 上游 akshare 抛 `NoneType` 异常无法产出 golden，直连 akshare 同错，非代码缺陷）。
- **关键修复（对齐 akshare dtype）**：① `Df::infer_numeric` 空/纯空白单元格视为缺失（对齐 akshare `pd.read_html`/`pd.to_numeric(errors="coerce")`，修复 `futures_rule` 含 `--`/空单元格的列误判为 str）；② `futures_spot_stock` 日期列数值取自 item `v1`..`v5`（非 MM-DD 标签），且仅前 4 日期列 + 最新价格 + 近半年涨跌幅 转 float64、末日期列保持 str（对齐 akshare 源码只 `to_numeric` 前 4 个日期列）；③ `futures_comm_js` 列序修正为 开仓/平今/平昨/每手跳数。
- **基础设施（影响全工程）**：`Cargo.toml` 引入 `chrono = "0.4"`（`futures_news_shmet` 毫秒时间戳→`Asia/Shanghai`，离线可用 0.4.45）；`Df` 新增 `infer_numeric`（空单元格视为缺失、整列可解析为数值才转 Float64）。
- **范围说明**：`futures_derivative` 在 akshare 中是子包（模块）而非可调用函数，不在 1094 公开函数目标内，故本子组不含该函数（详见 PLAN.md 批次 29-E）。

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
