#!/usr/bin/env python3
"""差分测试框架（A3）：对比 Rust 版输出与 Python akshare 输出。

用法：
  # 生成 golden fixture（需要 Python akshare 环境 + 网络）
  python3 tools/parity_runner.py --generate
  python3 tools/parity_runner.py --generate --only stock_zh_a_hist

  # 运行对比（需要 Rust parity bin + 网络）
  python3 tools/parity_runner.py --check
  python3 tools/parity_runner.py --check --only stock_zh_a_hist

  # 仅查看用例清单
  python3 tools/parity_runner.py --list

设计说明：
- 用例注册表 CASES：每个用例 = 函数名 + 参数 + 对比模式
- 对比模式 strict：列名/dtype/行数/head 值全部一致
- 对比模式 loose：仅列名与列数一致（实时行情类数据，值随时间变化）
- golden fixture 保存在 tests/golden/{func}.json（列名/dtype/行数/head）
- 对比容忍 float 字符串化差异（pandas 与 Rust 浮点打印规则不同）

退出码：0 = 全部通过或跳过；1 = 存在对比失败。
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import warnings

warnings.filterwarnings("ignore")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN_DIR = os.path.join(ROOT, "tests", "golden")
PARITY_BIN = os.path.join(ROOT, "target", "debug", "parity")
HEAD_N = 5
# 数值比较保留的有效位数：跨语言（pandas vs Rust）对大数（如总市值 ~1.9e10）
# 的浮点解析会差到 double 精度末位（~1e-15 相对误差），固定小数位比对会把这些
# 噪声当成差异。按有效位数归一可吸收浮点噪声，同时保留足够业务精度。
SIGFIGS = 9

# 用例注册表：函数名 → (参数, 对比模式, 说明)
# 参数与 Rust parity bin / Python akshare 同名函数的参数一致（全字符串）。
CASES: list[tuple[str, list[str], str, str]] = [
    ("stock_zh_a_hist", ["000001", "daily", "20240101", "20240131", ""], "strict", "A股日K历史"),
    ("stock_zh_a_hist_min_em", ["000001", "2026-01-01 09:00:00", "2026-12-31 15:00:00", "5", ""], "strict", "A股5分钟K线"),
    ("stock_individual_info_em", ["000001"], "strict", "个股信息"),
    ("stock_bid_ask_em", ["000001"], "strict", "五档盘口"),
    ("stock_board_industry_name_em", [], "loose", "行业板块列表"),
    ("stock_board_concept_name_em", [], "loose", "概念板块列表"),
    ("stock_board_industry_cons_em", ["小金属"], "loose", "行业板块成分"),
    ("stock_board_concept_cons_em", ["昨日连板"], "loose", "概念板块成分"),
    ("stock_board_industry_hist_em", ["小金属", "20240101", "20240131", "日K"], "strict", "行业板块历史"),
    ("stock_board_concept_hist_em", ["昨日连板", "daily", "20240101", "20240131", ""], "strict", "概念板块历史"),
    ("stock_zt_pool_em", ["20260807"], "strict", "涨停股池"),
    ("stock_zt_pool_previous_em", ["20260807"], "loose", "昨日涨停股池"),
    ("stock_zt_pool_strong_em", ["20260807"], "strict", "强势股池"),
    ("stock_zt_pool_sub_new_em", ["20260807"], "strict", "次新股池"),
    ("stock_zt_pool_zbgc_em", ["20260807"], "strict", "炸板股池"),
    ("stock_zt_pool_dtgc_em", ["20260807"], "strict", "跌停股池"),
    # === BATCH11 同行比较（RPT_PCF10_INDUSTRY_*，securities datacenter，loose 比列契约） ===
    ("stock_zh_growth_comparison_em", ["SZ000895"], "loose", "A股成长性比较"),
    ("stock_zh_dupont_comparison_em", ["SZ000895"], "loose", "A股杜邦分析比较"),
    ("stock_zh_scale_comparison_em", ["SZ000895"], "loose", "A股公司规模比较"),
    ("stock_hk_growth_comparison_em", ["03900"], "loose", "港股成长性比较"),
    ("stock_hk_scale_comparison_em", ["03900"], "loose", "港股规模比较"),
    # === BATCH12 港股 F10（RPT_HKF10_* / RPT_CUSTOM_HKF10_*，securities datacenter，loose 比列契约） ===
    ("stock_hk_security_profile_em", ["03900"], "loose", "港股证券资料"),
    ("stock_hk_company_profile_em", ["03900"], "loose", "港股公司资料"),
    ("stock_hk_financial_indicator_em", ["03900"], "loose", "港股最新指标"),
    ("stock_hk_dividend_payout_em", ["03900"], "loose", "港股分红派息"),
    ("stock_zh_valuation_comparison_em", ["SZ000895"], "loose", "A股估值比较"),
    ("stock_hk_valuation_comparison_em", ["03900"], "loose", "港股估值比较"),
    ("stock_individual_fund_flow", ["000001", "sh"], "strict", "个股资金流"),
    ("stock_lhb_detail_em", ["20240101", "20240131"], "strict", "龙虎榜详情"),
    ("stock_hsgt_fund_flow_summary_em", [], "loose", "沪深港通资金流"),
    ("stock_gpzy_profile_em", [], "loose", "股权质押统计"),
    ("stock_zh_a_spot_em", [], "loose", "A股实时行情"),
    ("stock_sh_a_spot_em", [], "loose", "沪A实时行情"),
    ("stock_sz_a_spot_em", [], "loose", "深A实时行情"),
    ("index_zh_a_hist", ["000001", "daily", "20240101", "20240131"], "strict", "指数日K"),
    ("index_zh_a_hist_min_em", ["399006", "5", "2026-01-01 09:00:00", "2026-12-31 15:00:00"], "strict", "指数分钟K线"),
    ("fund_etf_spot_em", [], "loose", "ETF实时行情"),
    ("fund_lof_spot_em", [], "loose", "LOF实时行情"),
    ("fund_etf_hist_em", ["510300", "daily", "20240101", "20240131", ""], "strict", "ETF日K"),
    ("stock_profile_cninfo", ["600030"], "strict", "巨潮公司概况"),
    ("stock_ipo_summary_cninfo", ["600030"], "strict", "巨潮上市相关"),
    ("stock_dividend_cninfo", ["600009"], "strict", "巨潮历史分红"),
    ("stock_new_ipo_cninfo", [], "strict", "巨潮新股发行"),
    ("fund_etf_category_ths", ["ETF", ""], "loose", "同花顺基金净值行情"),
    ("fund_etf_spot_ths", [""], "loose", "同花顺 ETF 实时行情"),
    ("stock_hk_spot", [], "loose", "新浪港股实时行情"),
    ("stock_zh_a_minute", ["sh600519", "5", ""], "loose", "新浪A股分钟线"),
    ("stock_margin_sse", ["20240801", "20240810"], "strict", "上交所融资融券汇总"),
    ("stock_margin_detail_sse", ["20240809"], "strict", "上交所融资融券明细"),
    ("stock_margin_szse", ["20240411"], "strict", "深交所融资融券汇总"),
    ("stock_hot_follow_xq", ["最热门"], "loose", "雪球关注排行榜"),
    ("stock_hot_tweet_xq", ["最热门"], "loose", "雪球讨论排行榜"),
    # 东方财富个股人气榜（emappdata.eastmoney.com/stockrank，POST-JSON；实时性高，loose 比列契约）
    ("stock_hot_rank_em", [], "loose", "东方财富人气榜"),
    ("stock_hot_up_em", [], "loose", "东方财富飙升榜"),
    ("stock_hot_rank_detail_em", ["SZ000665"], "loose", "人气榜历史趋势及粉丝特征"),
    ("stock_hot_rank_detail_realtime_em", ["SZ000665"], "loose", "人气榜实时变动"),
    ("stock_hot_keyword_em", ["SZ000665"], "loose", "人气榜热门关键词"),
    ("stock_hot_rank_latest_em", ["SZ000665"], "loose", "人气榜最新排名"),
    ("stock_hot_rank_relate_em", ["SZ000665"], "loose", "人气榜相关股票"),
    ("stock_zh_a_st_em", [], "loose", "ST股板块"),
    ("stock_zh_a_new_em", [], "loose", "新股板块"),
    ("stock_hk_spot_em", [], "loose", "东财港股实时行情"),
    # === BATCH24 新浪财经-ESG 评级中心（global.finance.sina.com.cn EsgService.*，纯 JSON，loose 比列契约） ===
    ("stock_esg_msci_sina", [], "loose", "新浪ESG-MSCI评级"),
    ("stock_esg_rft_sina", [], "loose", "新浪ESG-路孚特评级"),
    ("stock_esg_rate_sina", [], "loose", "新浪ESG-评级数据"),
    ("stock_esg_zd_sina", [], "loose", "新浪ESG-秩鼎评级"),
    ("stock_esg_hz_sina", [], "loose", "新浪ESG-华证指数评级"),
    # === BATCH25 同花顺-数据中心-资金流向（data.10jqka.com.cn/funds/*，HTML 表格，loose 比列契约） ===
    ("stock_fund_flow_individual", ["即时"], "loose", "同花顺个股资金流-即时"),
    ("stock_fund_flow_concept", ["即时"], "loose", "同花顺概念资金流-即时"),
    ("stock_fund_flow_industry", ["即时"], "loose", "同花顺行业资金流-即时"),
    ("stock_fund_flow_big_deal", [], "loose", "同花顺大单追踪"),
    # === BATCH26 东财 F10 股本结构/商誉/财务分析主要指标（datacenter securities/web，loose 比列契约） ===
    ("stock_zh_a_gbjg_em", ["603392.SH"], "loose", "东财股本结构"),
    ("stock_sy_em", ["20231231"], "loose", "东财个股商誉明细"),
    ("stock_financial_analysis_indicator_em", ["301389.SZ", "按单季度"], "loose", "东财A股财务分析主要指标-单季度"),
    ("stock_financial_hk_analysis_indicator_em", ["00700", "年度"], "loose", "东财港股财务分析主要指标"),
    ("stock_financial_us_analysis_indicator_em", ["TSLA", "年报"], "loose", "东财美股财务分析主要指标"),
    # === BATCH27 东财公告大全 / 主营构成（emweb F10 + np-anotice-stock，loose 比列契约） ===
    ("stock_zygc_em", ["SH688041"], "loose", "东财主营构成"),
    ("stock_notice_report", ["全部", "20220511"], "loose", "东财公告大全-按日期"),
    ("stock_individual_notice_report", ["300237", "全部", "20250101", "20260101"], "loose", "东财个股公告"),
    ("stock_zh_kcb_report_em", ["1", "1"], "loose", "科创板报告"),
    # stock_feature 东财系（Batch 1 Stage 1a）
    ("stock_cy_a_spot_em", [], "loose", "创业板实时行情"),
    ("stock_kc_a_spot_em", [], "loose", "科创板实时行情"),
    ("stock_zh_b_spot_em", [], "loose", "B股实时行情"),
    ("stock_new_a_spot_em", [], "loose", "新股实时行情"),
    ("stock_hk_main_board_spot_em", [], "loose", "港股主板实时行情"),
    ("stock_hk_ggt_components_em", [], "loose", "港股通成份股"),
    ("stock_zh_a_gdhs", ["最新"], "loose", "股东户数"),
    # stock_feature 东财 datacenter RPT_* 报表（Batch 1 Stage 1b）
    ("stock_margin_account_info", [], "loose", "融资融券账户信息"),
    ("stock_gdfx_free_holding_detail_em", ["20210930"], "loose", "股东自由流通持股明细"),
    ("stock_gdfx_holding_detail_em", ["20230331", "个人", "新进"], "loose", "股东持股明细"),
    ("stock_gdfx_free_holding_analyse_em", ["20230930"], "loose", "股东自由流通持股分析"),
    ("stock_gdfx_holding_analyse_em", ["20230331"], "loose", "股东持股分析"),
    ("stock_qsjy_em", ["20200731"], "loose", "券商业绩"),
    ("stock_gpzy_profile_em", [], "loose", "股权质押总览"),
    ("stock_gpzy_pledge_ratio_em", ["20240906"], "loose", "个股股权质押比例"),
    ("stock_gpzy_industry_data_em", [], "loose", "行业股权质押统计"),
    ("stock_value_em", ["300766"], "loose", "个股估值分析"),
    ("stock_gddh_em", [], "loose", "股东大会"),
    ("stock_zdhtmx_em", ["20200819", "20230819"], "loose", "重大合同明细"),
    ("stock_dxsyl_em", [], "loose", "打新收益率"),
    ("stock_sy_profile_em", [], "loose", "商誉市场统计"),
    # stock_feature 东财 datacenter 股东/质押明细（Batch 1 Stage 1c）
    ("stock_gpzy_pledge_ratio_detail_em", [], "loose", "重要股东股权质押明细"),
    ("stock_gpzy_individual_pledge_ratio_detail_em", ["603132"], "loose", "个股股权质押明细"),
    ("stock_ggcg_em", ["全部"], "loose", "高管持股变动"),
    # stock_feature 东财 datacenter 机构调研/分红/停复牌/增发配股/账户（Batch 1 Stage 1d）
    ("stock_jgdy_tj_em", ["20220101"], "loose", "机构调研统计"),
    ("stock_jgdy_detail_em", ["20260807"], "loose", "机构调研详细"),
    ("stock_fhps_em", ["20231231"], "loose", "分红送配"),
    ("stock_fhps_detail_em", ["300073"], "loose", "分红送配详情"),
    ("stock_tfp_em", ["20240426"], "loose", "停复牌信息"),
    ("stock_qbzf_em", [], "loose", "全部增发"),
    ("stock_pg_em", [], "loose", "配股"),
    ("stock_account_statistics_em", [], "loose", "股票账户统计"),
    # stock_feature 东财 datacenter 财报业绩/预告/预约披露（Batch 1 Stage 1e）
    ("stock_yjbb_em", ["20240331"], "loose", "业绩报表"),
    ("stock_yjkb_em", ["20240331"], "loose", "业绩快报"),
    ("stock_yjyg_em", ["20240331"], "loose", "业绩预告"),
    ("stock_yysj_em", ["沪深A股", "20240331"], "loose", "预约披露时间"),
    # stock_feature 东财 datacenter 千股千评/龙虎榜/股东分析统计变动（Batch 1 Stage 1f）
    ("stock_comment_em", [], "loose", "千股千评"),
    ("stock_lhb_stock_statistic_em", ["近一月"], "loose", "龙虎榜个股上榜统计"),
    ("stock_lhb_jgmmtj_em", ["20240417", "20240430"], "loose", "龙虎榜机构买卖每日统计"),
    ("stock_gdfx_free_holding_statistics_em", ["20210930"], "loose", "股东持股统计-十大流通股东"),
    ("stock_gdfx_holding_statistics_em", ["20210930"], "loose", "股东持股统计-十大股东"),
    ("stock_gdfx_free_holding_change_em", ["20210930"], "loose", "股东持股变动统计-十大流通股东"),
    ("stock_gdfx_holding_change_em", ["20210930"], "loose", "股东持股变动统计-十大股东"),
    # stock_feature 东财 datacenter 千股千评明细/沪深港通持股统计/商誉（Batch 1 Stage 1g）
    ("stock_comment_detail_zlkp_jgcyd_em", ["600000"], "loose", "千股千评-主力控盘-机构参与度"),
    ("stock_comment_detail_zhpj_lspf_em", ["600000"], "loose", "千股千评-综合评价-历史评分"),
    ("stock_hsgt_stock_statistics_em", ["20240110", "20240110"], "loose", "沪深港通持股-每日个股统计(北向)"),
    ("stock_sy_yq_em", ["20240630"], "loose", "商誉-商誉减值预期明细"),
    ("stock_sy_jz_em", ["20240630"], "loose", "商誉-个股商誉减值明细"),
    ("stock_zcfz_em", ["20240331"], "loose", "资产负债表"),
    ("stock_zcfz_bj_em", ["20240331"], "loose", "资产负债表(北交所)"),
    ("stock_lrb_em", ["20240331"], "loose", "利润表"),
    ("stock_xjll_em", ["20240331"], "loose", "现金流量表"),
    # stock_feature 东财 datacenter 质押分布/股东协作/千股千评明细/商誉行业（Batch 1 Stage 1i）
    ("stock_gpzy_distribute_statistics_company_em", [], "loose", "股权质押-证券公司分布统计"),
    ("stock_gpzy_distribute_statistics_bank_em", [], "loose", "股权质押-银行分布统计"),
    ("stock_zh_a_gdhs_detail_em", ["000001"], "loose", "股东户数-个股明细"),
    # 注：原 akshare 默认参数 symbol="全部" 对应 RPT_COOPFREEHOLDER 无过滤，
    # 服务端 pages≈3260（约 1.6M 行），超过 parity 的 120s 超时且 golden 体积过大；
    # 列契约与过滤后完全一致，故 parity 用例改用过滤值“券商”验证（代码仍支持“全部”，
    # 由 gdfx_team_offline 离线测试覆盖）。
    ("stock_gdfx_free_holding_teamwork_em", ["券商"], "loose", "股东协作-自由流通持股"),
    ("stock_gdfx_holding_teamwork_em", ["社保"], "loose", "股东协作-持股"),
    ("stock_comment_detail_scrd_focus_em", ["600000"], "loose", "千股千评-人气聚焦"),
    ("stock_comment_detail_scrd_desire_em", ["600000"], "loose", "千股千评-参与意愿"),
    ("stock_sy_hy_em", ["20240930"], "loose", "商誉-行业统计"),
    # stock_feature 东财 datacenter 龙虎榜明细/营业部/席位统计（Batch 1 Stage 1j）
    ("stock_lhb_jgstatistic_em", ["近一月"], "loose", "龙虎榜-机构席位追踪"),
    ("stock_lhb_hyyyb_em", ["20240401", "20240430"], "loose", "龙虎榜-每日活跃营业部"),
    ("stock_lhb_yybph_em", ["近一月"], "loose", "龙虎榜-营业部排行"),
    ("stock_lhb_traderstatistic_em", ["近一月"], "loose", "龙虎榜-营业部统计"),
    ("stock_lhb_stock_detail_date_em", ["600077"], "loose", "个股龙虎榜详情-日期"),
    ("stock_lhb_stock_detail_em", ["000788", "20220315", "卖出"], "loose", "个股龙虎榜详情"),
    ("stock_lhb_yyb_detail_em", ["10188715"], "loose", "营业部历史交易明细"),
    # 批次1 阶段1k 东财 datacenter 沪深港通 持股/成交/机构/板块排名（6个）
    ("stock_hsgt_hold_stock_em", ["沪股通", "5日排行", "20260807"], "loose", "沪深港通持股-个股排行"),
    ("stock_hsgt_institution_statistics_em", ["北向持股", "20240110", "20240110"], "loose", "沪深港通每日机构统计"),
    ("stock_hsgt_hist_em", ["北向资金"], "loose", "沪深港通历史资金流向"),
    ("stock_hsgt_board_rank_em", ["北向资金增持行业板块排行", "今日", "20240816"], "loose", "沪深港通板块排行"),
    ("stock_hsgt_individual_em", ["00700"], "loose", "沪深港通个股持股(港股)"),
    ("stock_hsgt_individual_detail_em", ["002008", "20240801", "20240831"], "loose", "沪深港通个股持股详情"),
    ("stock_xgsglb_em", ["全部股票"], "loose", "新股申购与中签查询"),
    ("stock_analyst_rank_em", ["2024"], "loose", "分析师指数排名"),
    ("stock_analyst_detail_em", ["11000200926", "最新跟踪成分股"], "loose", "分析师详情-最新跟踪成分股"),
    # 批次3 阶段3a 东财 datacenter 限售股解禁（4个）
    ("stock_restricted_release_summary_em", ["全部股票", "20221101", "20221209"], "loose", "限售股解禁-汇总"),
    ("stock_restricted_release_detail_em", ["20221202", "20221204"], "loose", "限售股解禁-详情"),
    ("stock_restricted_release_queue_em", ["600000"], "loose", "限售股解禁-个股批次"),
    ("stock_restricted_release_stockholder_em", ["600000", "20200904"], "loose", "限售股解禁-股东"),
    ("stock_restricted_release_queue_sina", ["sh600000"], "loose", "限售股解禁-新浪队列"),
    # 批次1 阶段2a/2b 同花顺数据中心-技术选股排名（HTML 表格 + v token Cookie，loose）
    ("stock_rank_cxg_ths", ["创月新高"], "loose", "技术选股-创新高"),
    ("stock_rank_cxd_ths", ["创月新低"], "loose", "技术选股-创新低"),
    ("stock_rank_lxsz_ths", [], "loose", "技术选股-连续上涨"),
    ("stock_rank_lxxd_ths", [], "loose", "技术选股-连续下跌"),
    ("stock_rank_cxfl_ths", [], "loose", "技术选股-持续放量"),
    ("stock_rank_cxsl_ths", [], "loose", "技术选股-持续缩量"),
    ("stock_rank_xstp_ths", ["5日均线"], "loose", "技术选股-向上突破"),
    ("stock_rank_xxtp_ths", ["5日均线"], "loose", "技术选股-向下突破"),
    ("stock_rank_ljqs_ths", [], "loose", "技术选股-量价齐升"),
    ("stock_rank_ljqd_ths", [], "loose", "技术选股-量价齐跌"),
    ("stock_rank_xzjp_ths", [], "loose", "技术选股-险资举牌"),
    # 批次2 期货交易所结算参数（5 个）：固定历史交易日数据不变，可安全 loose 对比
    ("futures_settle_cffex", ["20260119"], "loose", "中金所-结算参数"),
    ("futures_settle_czce", ["20260119"], "loose", "郑商所-结算参数"),
    ("futures_settle_gfex", ["20260119"], "loose", "广期所-结算参数"),
    ("futures_settle_shfe", ["20260119"], "loose", "上期所-结算参数"),
    ("futures_settle_ine", ["20260119"], "loose", "上能中心-结算参数"),
    # 批次2 期货结算参数统一入口（5 个）：20 列规范化，数据同各家原始接口
    ("futures_settle", ["20260119", "CFFEX"], "strict", "结算参数统一入口-中金所"),
    ("futures_settle", ["20260119", "CZCE"], "strict", "结算参数统一入口-郑商所"),
    ("futures_settle", ["20260119", "GFEX"], "strict", "结算参数统一入口-广期所"),
    ("futures_settle", ["20260119", "SHFE"], "strict", "结算参数统一入口-上期所"),
    ("futures_settle", ["20260119", "INE"], "strict", "结算参数统一入口-上能中心"),
    # 批次2 新浪期货合约详情：合约基础信息为静态数据，可安全 strict 对比
    ("futures_contract_detail", ["V2201"], "strict", "期货合约详情"),
    ("futures_comex_inventory", ["黄金"], "loose", "COMEX黄金库存"),
    ("futures_comex_inventory", ["白银"], "loose", "COMEX白银库存"),
    ("futures_inventory_em", ["a"], "loose", "期货库存-豆一"),
    # 批次29 子组A 东财国际期货 + 中证商品指数 + 东财期货规则
    ("futures_index_ccidx", ["中证商品期货指数"], "loose", "中证商品期货指数"),
    ("futures_index_ccidx", ["中证商品期货价格指数"], "loose", "中证商品期货价格指数"),
    ("futures_global_spot_em", [], "loose", "国际期货实时行情"),
    ("futures_global_hist_em", ["HG00Y"], "loose", "国际期货历史行情-铜"),
    # 批次29 子组B 新浪期货集群（国内 sina + 外盘 hq/foreign）
    ("futures_symbol_mark", [], "loose", "期货品种代码映射"),
    ("futures_zh_realtime", ["工业硅"], "loose", "期货品种实时合约"),
    ("futures_zh_spot", ["RB0", "CF", "0"], "loose", "期货实时行情"),
    ("futures_zh_daily_sina", ["RB0"], "loose", "期货日线"),
    ("futures_zh_minute_sina", ["RB0", "1"], "loose", "期货分钟线"),
    ("futures_hq_subscribe_exchange_symbol", [], "loose", "外盘品种对应表"),
    ("futures_foreign_commodity_realtime", ["CT,NID"], "loose", "外盘期货实时"),
    # futures_foreign_commodity_subscribe_exchange_symbol 上游返回 list（非 DataFrame），不注册 parity
    ("futures_foreign_detail", ["ZSD"], "loose", "外盘合约详情"),
    ("futures_foreign_hist", ["ZSD"], "loose", "外盘历史日线"),
    # 批次 29 子组 C：交易所官方数据-合约信息（中金所 XML / 郑商所 XML / 大商所 JSON / 广期所 JSON / 上期能源 JSON / 上期所 JSON）
    ("futures_contract_info_cffex", ["20240228"], "loose", "中金所-合约信息"),
    ("futures_contract_info_czce", ["20240228"], "loose", "郑商所-合约信息"),
    ("futures_contract_info_dce", [], "loose", "大商所-合约信息"),
    ("futures_contract_info_gfex", [], "loose", "广期所-合约信息"),
    ("futures_contract_info_ine", ["20241129"], "loose", "上期能源-合约信息"),
    ("futures_contract_info_shfe", ["20240513"], "loose", "上期所-合约信息"),
    # 批次 29 子组 C：交易所官方数据-仓单 / 交割 / 期转现 / 历史行情
    # 注：大商所 publicweb 接口反爬（412）、上期所 tsite.shfe.com.cn 本环境无法解析，
    # 这些用例无 golden（--generate 阶段 akshare 同样失败）→ --check 自动跳过。
    ("futures_warehouse_receipt_czce", ["20251014"], "loose", "郑商所-仓单日报"),
    ("futures_warehouse_receipt_dce", ["20251027"], "loose", "大商所-仓单日报"),
    ("futures_shfe_warehouse_receipt", ["20251014"], "loose", "上期所-仓单日报"),
    ("futures_gfex_warehouse_receipt", ["20240122"], "loose", "广期所-仓单日报"),
    ("futures_to_spot_shfe", ["202312"], "loose", "上期所-期转现"),
    ("futures_delivery_dce", ["202312"], "loose", "大商所-交割统计"),
    ("futures_to_spot_dce", ["202312"], "loose", "大商所-期转现"),
    ("futures_delivery_match_dce", ["a"], "loose", "大商所-交割配对"),
    ("futures_to_spot_czce", ["20251014"], "loose", "郑商所-期转现"),
    ("futures_delivery_czce", ["20210112"], "loose", "郑商所-月度交割"),
    ("futures_delivery_shfe", ["202312"], "loose", "上期所-交割情况"),
    ("futures_hist_daily_cffex", ["20260302"], "loose", "中金所-历史日线"),
    # 批次 29 子组 D：东财期货行情（品种对照表 / kline / SGX 结算价）
    # 注：futures_hist_em 与 futures_settlement_price_sgx 依赖 push2his.eastmoney.com
    # （当前环境 TCP 断连，直连 akshare 同错），无法生成 golden，--check 自动跳过；
    # futures_hist_table_em 走 futsse-static.eastmoney.com/redis 可读端点，正常对账。
    ("futures_hist_table_em", [], "loose", "东财-期货品种对照表"),
    ("futures_hist_em", ["热卷主连", "daily", "20240101", "20241231"], "loose", "东财-期货行情 kline"),
    ("futures_settlement_price_sgx", ["20231107"], "loose", "SGX-历史结算价"),
    # 批次 29 子组 E：期货杂项 / 独立数据源集群
    # 多源（9qihuo / gtjaqh / 100ppi / 99qh / shmet / openctp / jin10 / 东财现货）存在反爬或
    # DNS 限制，无 golden 的用例 --check 自动跳过；可达源正常对账。
    ("futures_comm_info", ["所有"], "loose", "九期网-期货手续费"),
    ("futures_comm_js", ["20250213"], "loose", "金十-期货手续费"),
    ("futures_fees_info", [], "loose", "openctp-期货交易费用"),
    ("futures_rule", ["20231205"], "loose", "国泰君安-交易日历"),
    ("futures_news_shmet", ["全部"], "loose", "上海金属网-快讯"),
    ("futures_inventory_99", ["豆一"], "loose", "99期货-大宗商品库存"),
    ("futures_spot_stock", ["能源"], "loose", "东财-现货与股票上下游"),
    ("futures_stock_shfe_js", ["20240419"], "loose", "金十-上期所库存周报"),
    ("futures_spot_sys", ["铜", "市场价格"], "loose", "生意社-现期图"),
    ("futures_contract_detail_em", ["v2602F"], "loose", "东财-期货合约详情"),
    # 批次20 利率：银行间拆借利率（东财 RPT_IMP_INTRESTRATEN）
    ("rate_interbank", ["上海银行同业拆借市场", "Shibor人民币", "3月"], "loose", "Shibor-3月"),
    ("rate_interbank", ["伦敦银行同业拆借市场", "Libor美元", "1月"], "loose", "Libor美元-1月"),
    ("rate_interbank", ["香港银行同业拆借市场", "Hibor港币", "隔夜"], "loose", "Hibor港币-隔夜"),
    # 批次3 阶段3b 同花顺财务指标（8 个）：报告期集合随时间增长，loose 只比列契约
    ("stock_financial_abstract_ths", ["000063", "按报告期"], "loose", "同花顺财务-主要指标"),
    ("stock_financial_debt_ths", ["000063", "按报告期"], "loose", "同花顺财务-资产负债表"),
    ("stock_financial_benefit_ths", ["000063", "按报告期"], "loose", "同花顺财务-利润表"),
    ("stock_financial_cash_ths", ["000063", "按报告期"], "loose", "同花顺财务-现金流量表"),
    ("stock_financial_abstract_new_ths", ["000063", "按报告期"], "loose", "同花顺财务-重要指标(新)"),
    ("stock_financial_debt_new_ths", ["000063", "按报告期"], "loose", "同花顺财务-资产负债表(新)"),
    ("stock_financial_benefit_new_ths", ["000063", "按报告期"], "loose", "同花顺财务-利润表(新)"),
    ("stock_financial_cash_new_ths", ["000063", "按报告期"], "loose", "同花顺财务-现金流量表(新)"),
    # 批次3 阶段3c 金十宏观 14 个：历史数据随时间追加，loose 只比列契约
    ("macro_china_gdp_yearly", [], "loose", "中国GDP年率"),
    ("macro_china_cpi_yearly", [], "loose", "中国CPI年率"),
    ("macro_china_cpi_monthly", [], "loose", "中国CPI月率"),
    ("macro_china_ppi_yearly", [], "loose", "中国PPI年率"),
    ("macro_china_exports_yoy", [], "loose", "中国出口年率"),
    ("macro_china_imports_yoy", [], "loose", "中国进口年率"),
    ("macro_china_trade_balance", [], "loose", "中国贸易帐"),
    ("macro_china_industrial_production_yoy", [], "loose", "中国规模以上工业增加值"),
    ("macro_china_pmi_yearly", [], "loose", "中国官方制造业PMI"),
    ("macro_china_cx_pmi_yearly", [], "loose", "中国财新制造业PMI"),
    ("macro_china_cx_services_pmi_yearly", [], "loose", "中国财新服务业PMI"),
    ("macro_china_non_man_pmi", [], "loose", "中国官方非制造业PMI"),
    ("macro_china_fx_reserves_yearly", [], "loose", "中国外汇储备"),
    ("macro_china_m2_yearly", [], "loose", "中国M2货币供应"),
    # 批次3 阶段3d 同花顺板块/新股/公司大事
    ("stock_board_industry_name_ths", [], "loose", "同花顺行业板块名称"),
    ("stock_board_industry_info_ths", ["半导体"], "loose", "同花顺行业板块简介"),
    ("stock_board_concept_name_ths", [], "loose", "同花顺概念板块名称"),
    ("stock_board_concept_info_ths", ["阿里巴巴概念"], "loose", "同花顺概念板块简介"),
    ("stock_ipo_ths", ["全部A股"], "loose", "同花顺新股申购"),
    ("stock_ipo_hk_ths", [], "loose", "同花顺港股新股申购"),
    ("stock_fhps_detail_ths", ["000063"], "loose", "同花顺分红详情"),
    ("stock_profit_forecast_ths", ["000063", "预测年报每股收益"], "loose", "同花顺盈利预测"),
    ("stock_management_change_ths", ["000063"], "loose", "同花顺高管持股变动"),
    ("stock_shareholder_change_ths", ["000063"], "loose", "同花顺股东持股变动"),
    # === BATCH8 注册制 IPO 审核信息（RPT_IPO_INFOALLNEW 系列，loose 比列契约） ===
    ("stock_register_all_em", [], "loose", "注册制IPO审核-全部"),
    ("stock_register_kcb", [], "loose", "注册制IPO审核-科创板"),
    ("stock_register_cyb", [], "loose", "注册制IPO审核-创业板"),
    ("stock_register_bj", [], "loose", "注册制IPO审核-北交所"),
    ("stock_register_sh", [], "loose", "注册制IPO审核-上海主板"),
    ("stock_register_sz", [], "loose", "注册制IPO审核-深圳主板"),
    ("stock_register_db", [], "loose", "注册制IPO审核-达标企业"),
    # === BATCH9 首发申报/上会/辅导备案（RPT_IPO_DECORGNEWEST / RPT_IPO_REVIEW / RPT_IPO_TUTRECORD，loose 比列契约） ===
    ("stock_ipo_declare_em", [], "loose", "首发申报企业信息"),
    ("stock_ipo_review_em", [], "loose", "新股上会信息"),
    ("stock_ipo_tutor_em", [], "loose", "IPO辅导备案信息"),
    # === BATCH10 盈利预测（RPT_WEB_RESPREDICT，动态 YEAR 列头，loose 比列契约） ===
    ("stock_profit_forecast_em", [""], "loose", "东财盈利预测"),
    # 批次3 阶段3e 乐咕系（历史序列长且持续追加，loose 只比列契约）
    ("stock_market_pe_lg", ["上证"], "loose", "乐咕主板市盈率"),
    ("stock_index_pe_lg", ["沪深300"], "loose", "乐咕指数市盈率"),
    ("stock_market_pb_lg", ["上证"], "loose", "乐咕主板市净率"),
    ("stock_index_pb_lg", ["上证50"], "loose", "乐咕指数市净率"),
    ("stock_a_congestion_lg", [], "loose", "乐咕大盘拥挤度"),
    ("stock_buffett_index_lg", [], "loose", "乐咕巴菲特指标"),
    ("stock_ebs_lg", [], "loose", "乐咕股债利差"),
    ("fund_stock_position_lg", [], "loose", "乐咕股票型基金仓位"),
    ("fund_balance_position_lg", [], "loose", "乐咕平衡混合型基金仓位"),
    ("fund_linghuo_position_lg", [], "loose", "乐咕灵活配置型基金仓位"),
    # 批次3 阶段3f 东财 datacenter-web 宏观（历史序列持续追加，loose 只比列契约）
    ("macro_china_hk_cpi", [], "loose", "香港CPI"),
    ("macro_china_hk_cpi_ratio", [], "loose", "香港CPI年率"),
    ("macro_china_hk_rate_of_unemployment", [], "loose", "香港失业率"),
    ("macro_china_hk_gbp", [], "loose", "香港GDP"),
    ("macro_china_hk_gbp_ratio", [], "loose", "香港GDP同比"),
    ("macro_china_hk_building_volume", [], "loose", "香港楼宇买卖合约数量"),
    ("macro_china_hk_building_amount", [], "loose", "香港楼宇买卖合约成交金额"),
    ("macro_china_hk_trade_diff_ratio", [], "loose", "香港商品贸易差额年率"),
    ("macro_china_hk_ppi", [], "loose", "香港制造业PPI年率"),
    ("macro_china_qyspjg", [], "loose", "企业商品价格指数"),
    ("macro_china_fdi", [], "loose", "外商直接投资数据"),
    # === BATCH2 OPTION (sina/exchange/em) ===
    # 阶段1 新浪中金所(CFFEX)：spot 实时行情(17列)、daily 日线(6列)，loose(实时/交易日变化)
    ("option_cffex_sz50_spot_sina", ["ho2303"], "loose", "中金所-上证50-实时行情"),
    ("option_cffex_sz50_daily_sina", ["ho2303P2350"], "loose", "中金所-上证50-日线"),
    ("option_cffex_hs300_spot_sina", ["io2209"], "loose", "中金所-沪深300-实时行情"),
    ("option_cffex_hs300_daily_sina", ["io2202P4350"], "loose", "中金所-沪深300-日线"),
    ("option_cffex_zz1000_spot_sina", ["mo2209"], "loose", "中金所-中证1000-实时行情"),
    ("option_cffex_zz1000_daily_sina", ["mo2208P6200"], "loose", "中金所-中证1000-日线"),
    # 阶段2 新浪上交所(SSE)：spot/greeks 实时字段值(2列)、minute 实时、daily 历史、金融分钟
    ("option_sse_spot_price_sina", ["10003045"], "loose", "上交所-期权实时量价"),
    ("option_sse_underlying_spot_price_sina", ["sh510300"], "loose", "上交所-标的物实时"),
    ("option_sse_greeks_sina", ["10003045"], "loose", "上交所-期权希腊字母"),
    ("option_sse_minute_sina", ["10003720"], "loose", "上交所-期权当日分钟"),
    ("option_sse_daily_sina", ["10003889"], "strict", "上交所-期权日线历史"),
    ("option_finance_minute_sina", ["10002530"], "loose", "金融期权-五分钟线"),
    ("option_sse_codes_sina", ["看涨期权", "202609", "510300"], "loose", "上交所-期权代码列表"),
    # 阶段3 新浪商品期权：历史日线
    ("option_commodity_hist_sina", ["au2012C392"], "strict", "商品期权-历史日线"),
    # 阶段4 交易所(上交所/深交所)：当日合约、每日统计、风险指标
    ("option_current_day_sse", [], "loose", "上交所-当日所有合约"),
    ("option_daily_stats_sse", ["20240626"], "strict", "上交所-每日统计"),
    ("option_daily_stats_szse", ["20240626"], "strict", "深交所-每日统计"),
    ("option_risk_indicator_sse", ["20240626"], "strict", "上交所-风险指标"),
    # 阶段5 东财期权：龙虎榜（其余东财实时/分析函数因本环境东财源不可达，见报告跳过）
    ("option_lhb_em", ["510050", "期权交易情况-认沽交易量", "20240626"], "strict", "东财-期权龙虎榜"),
    # 东财-期权实时/分钟/分析类（依赖东财接口，网络恢复后补充 golden）
    ("option_current_em", [], "loose", "东财-期权实时行情"),
    ("option_minute_em", ["510050"], "loose", "东财-期权分钟"),
    ("option_premium_analysis_em", [], "loose", "东财-溢价分析"),
    ("option_risk_analysis_em", [], "loose", "东财-风险分析"),
    ("option_value_analysis_em", [], "loose", "东财-价值分析"),
    # 阶段6 其他源：郑商所/大商所/广期所/上期所/中金所/openctp/上交所标的
    ("option_hist_czce", ["白糖期权", "20191017"], "strict", "郑商所-期权历史"),
    ("option_hist_yearly_czce", ["SR", "2021"], "loose", "郑商所-年度期权历史"),
    ("option_hist_dce", ["聚丙烯期权", "20220816"], "strict", "大商所-期权历史"),
    ("option_hist_gfex", ["工业硅", "20230724"], "strict", "广期所-期权历史"),
    ("option_hist_shfe", ["铝期权", "20250418"], "strict", "上期所-期权历史"),
    ("option_vol_shfe", ["铝期权", "20250418"], "strict", "上期所-隐含波动率"),
    ("option_vol_gfex", ["碳酸锂", "20230724"], "strict", "广期所-隐含波动率"),
    ("option_contract_info_ctp", [], "loose", "openctp-合约信息"),
    ("option_finance_board", ["嘉实沪深300ETF期权", "2306"], "loose", "金融期权-板块龙虎"),
    ("option_finance_sse_underlying", ["sh510300"], "loose", "上交所-标的实时行情"),
    # === BATCH3 ECONOMIC REMAINING (jin10/em datacenter) ===
    # 批次3 阶段3g 东财 datacenter-web 宏观产业指数 15 个（历史序列持续追加，loose 只比列契约）
    ("macro_china_agricultural_index", [], "loose", "农副指数"),
    ("macro_china_agricultural_product", [], "loose", "农产品批发价格总指数"),
    ("macro_china_bank_financing", [], "loose", "银行理财产品发行数量"),
    ("macro_china_bdti_index", [], "loose", "原油运输指数"),
    ("macro_china_bsi_index", [], "loose", "超灵便型船运价指数"),
    ("macro_china_commodity_price_index", [], "loose", "大宗商品价格指数"),
    ("macro_china_construction_index", [], "loose", "建材指数"),
    ("macro_china_construction_price_index", [], "loose", "建材价格指数"),
    ("macro_china_energy_index", [], "loose", "能源指数"),
    ("macro_china_insurance_income", [], "loose", "保险业经营情况"),
    ("macro_china_lpi_index", [], "loose", "物流业景气指数"),
    ("macro_china_mobile_number", [], "loose", "移动电话用户数"),
    ("macro_china_real_estate", [], "loose", "房地产开发景气指数"),
    ("macro_china_vegetable_basket", [], "loose", "菜篮子产品批发价格指数"),
    ("macro_china_yw_electronic_index", [], "loose", "义乌小商品指数"),
    # 批次3 阶段3h 东财 datacenter-web 宏观 21 个（历史序列持续追加，loose 只比列契约）
    ("macro_china_consumer_goods_retail", [], "loose", "消费品零售总额"),
    ("macro_china_cpi", [], "loose", "中国CPI"),
    ("macro_china_czsr", [], "loose", "全国财政收入"),
    ("macro_china_enterprise_boom_index", [], "loose", "企业景气指数"),
    ("macro_china_fx_gold", [], "loose", "外汇黄金储备"),
    ("macro_china_gdp", [], "loose", "中国GDP"),
    ("macro_china_gdzctz", [], "loose", "固定资产投资"),
    ("macro_china_gyzjz", [], "loose", "工业增加值"),
    ("macro_china_hgjck", [], "loose", "海关进出口"),
    ("macro_china_lpr", [], "loose", "贷款市场报价利率"),
    ("macro_china_money_supply", [], "loose", "货币供应"),
    ("macro_china_national_tax_receipts", [], "loose", "全国税收收入"),
    ("macro_china_new_financial_credit", [], "loose", "新增信贷"),
    ("macro_china_new_house_price", [], "loose", "70城新房价格"),
    ("macro_china_pmi", [], "loose", "制造业PMI"),
    ("macro_china_ppi", [], "loose", "中国PPI"),
    ("macro_china_reserve_requirement_ratio", [], "loose", "存款准备金率"),
    ("macro_china_stock_market_cap", [], "loose", "股市市值统计"),
    ("macro_china_wbck", [], "loose", "外汇存款"),
    ("macro_china_whxd", [], "loose", "外汇贷款"),
    ("macro_china_xfzxx", [], "loose", "消费者信心指数"),
    # 批次3 阶段3i 金十 cdn JSON 7 个（历史序列持续追加，loose 只比列契约）
    ("macro_china_au_report", [], "loose", "上海黄金交易所报告"),
    ("macro_china_rmb", [], "loose", "人民币汇率中间价"),
    ("macro_china_shibor_all", [], "loose", "Shibor利率"),
    ("macro_china_hk_market_info", [], "loose", "香港市场信息"),
    ("macro_china_market_margin_sh", [], "loose", "上海融资融券"),
    ("macro_china_market_margin_sz", [], "loose", "深圳融资融券"),
    ("macro_china_daily_energy", [], "loose", "日度沿海六大电库存"),
    # === BATCH6 海外宏观（RPT_ECONOMICVALUE_* 系列，loose 只比列契约） ===
    ("macro_australia_bank_rate", [], "loose", "澳大利亚-基准利率"),
    ("macro_australia_cpi_quarterly", [], "loose", "澳大利亚-CPI季率"),
    ("macro_australia_cpi_yearly", [], "loose", "澳大利亚-CPI年率"),
    ("macro_australia_ppi_quarterly", [], "loose", "澳大利亚-PPI季率"),
    ("macro_australia_retail_rate_monthly", [], "loose", "澳大利亚-零售销售月率"),
    ("macro_australia_trade", [], "loose", "澳大利亚-贸易帐"),
    ("macro_australia_unemployment_rate", [], "loose", "澳大利亚-失业率"),
    ("macro_canada_bank_rate", [], "loose", "加拿大-基准利率"),
    ("macro_canada_core_cpi_monthly", [], "loose", "加拿大-核心CPI月率"),
    ("macro_canada_core_cpi_yearly", [], "loose", "加拿大-核心CPI年率"),
    ("macro_canada_cpi_monthly", [], "loose", "加拿大-CPI月率"),
    ("macro_canada_cpi_yearly", [], "loose", "加拿大-CPI年率"),
    ("macro_canada_gdp_monthly", [], "loose", "加拿大-GDP月率"),
    ("macro_canada_new_house_rate", [], "loose", "加拿大-新屋开工"),
    ("macro_canada_retail_rate_monthly", [], "loose", "加拿大-零售销售月率"),
    ("macro_canada_trade", [], "loose", "加拿大-贸易帐"),
    ("macro_canada_unemployment_rate", [], "loose", "加拿大-失业率"),
    # === BATCH7a 海外宏观（RPT_ECONOMICVALUE_GER/JPAN/CH 系列，loose 只比列契约） ===
    ("macro_germany_ifo", [], "loose", "德国-IFO商业景气指数"),
    ("macro_germany_cpi_monthly", [], "loose", "德国-消费者物价指数月率终值"),
    ("macro_germany_cpi_yearly", [], "loose", "德国-消费者物价指数年率终值"),
    ("macro_germany_trade_adjusted", [], "loose", "德国-贸易帐(季调后)"),
    ("macro_germany_gdp", [], "loose", "德国-GDP"),
    ("macro_germany_retail_sale_monthly", [], "loose", "德国-实际零售销售月率"),
    ("macro_germany_retail_sale_yearly", [], "loose", "德国-实际零售销售年率"),
    ("macro_germany_zew", [], "loose", "德国-ZEW经济景气指数"),
    ("macro_japan_bank_rate", [], "loose", "日本-央行公布利率决议"),
    ("macro_japan_cpi_yearly", [], "loose", "日本-全国消费者物价指数年率"),
    ("macro_japan_core_cpi_yearly", [], "loose", "日本-全国核心消费者物价指数年率"),
    ("macro_japan_unemployment_rate", [], "loose", "日本-失业率"),
    ("macro_japan_head_indicator", [], "loose", "日本-领先指标终值"),
    ("macro_swiss_svme", [], "loose", "瑞士-SVME采购经理人指数"),
    ("macro_swiss_trade", [], "loose", "瑞士-贸易帐"),
    ("macro_swiss_cpi_yearly", [], "loose", "瑞士-消费者物价指数年率"),
    ("macro_swiss_gdp_quarterly", [], "loose", "瑞士-GDP季率"),
    ("macro_swiss_gbd_yearly", [], "loose", "瑞士-GDP年率"),
    ("macro_swiss_gbd_bank_rate", [], "loose", "瑞士-央行公布利率决议"),
    # === BATCH7b 海外宏观（RPT_ECONOMICVALUE_BRITAIN 系列，loose 只比列契约） ===
    ("macro_uk_halifax_monthly", [], "loose", "英国-Halifax房价指数月率"),
    ("macro_uk_halifax_yearly", [], "loose", "英国-Halifax房价指数年率"),
    ("macro_uk_trade", [], "loose", "英国-贸易帐"),
    ("macro_uk_bank_rate", [], "loose", "英国-央行公布利率决议"),
    ("macro_uk_core_cpi_yearly", [], "loose", "英国-核心消费者物价指数年率"),
    ("macro_uk_core_cpi_monthly", [], "loose", "英国-核心消费者物价指数月率"),
    ("macro_uk_cpi_yearly", [], "loose", "英国-消费者物价指数年率"),
    ("macro_uk_cpi_monthly", [], "loose", "英国-消费者物价指数月率"),
    ("macro_uk_retail_monthly", [], "loose", "英国-零售销售月率"),
    ("macro_uk_retail_yearly", [], "loose", "英国-零售销售年率"),
    ("macro_uk_rightmove_yearly", [], "loose", "英国-Rightmove房价指数年率"),
    ("macro_uk_rightmove_monthly", [], "loose", "英国-Rightmove房价指数月率"),
    ("macro_uk_gdp_quarterly", [], "loose", "英国-GDP季率初值"),
    ("macro_uk_gdp_yearly", [], "loose", "英国-GDP年率初值"),
    ("macro_uk_unemployment_rate", [], "loose", "英国-失业率"),
    # === BATCH3 STOCK_FUNDAMENTAL REMAINING (ths/sina/em) ===
    # 乐咕股息率（复用 legu 两步流，历史序列长，loose 比列契约）
    ("stock_a_gxl_lg", ["上证A股"], "loose", "乐咕A股股息率"),
    # 东财 datacenter 大宗交易系列（6 个，列契约由 finalize_report 键→中文映射锁定，loose）
    ("stock_dzjy_hygtj", ["近三月"], "loose", "大宗交易-活跃A股统计"),
    ("stock_dzjy_hyyybtj", ["近3日"], "loose", "大宗交易-活跃营业部统计"),
    ("stock_dzjy_mrmx", ["A股", "20240102", "20240103"], "loose", "大宗交易-每日明细(A股)"),
    ("stock_dzjy_mrtj", ["20240102", "20240103"], "loose", "大宗交易-每日统计"),
    ("stock_dzjy_sctj", [], "loose", "大宗交易-市场统计"),
    ("stock_dzjy_yybph", ["近三月"], "loose", "大宗交易-营业部排行"),
    # 东财 datacenter 股市日历/高管持股/股票回购系列（4 个，loose 比列契约）
    ("stock_gsrl_gsdt_em", ["20230808"], "loose", "股市日历-公司动态"),
    ("stock_hold_management_detail_em", [], "loose", "高管持股-变动明细"),
    ("stock_hold_management_person_em", ["001308", "吴远"], "loose", "高管持股-人员明细"),
    ("stock_repurchase_em", [], "loose", "股票回购数据"),
    # 东财 datacenter 基金持仓明细（RPT_MAINDATA_MAIN_POSITIONDETAILS，位置式列映射→键 rename，loose）
    ("stock_report_fund_hold_detail", ["008286", "20220331"], "loose", "基金持仓-明细"),
    # 东财 dataapi 基金持仓（位置式列映射→键 rename，loose）
    ("stock_report_fund_hold", ["基金持仓", "20210331"], "loose", "基金持仓"),
    # 雪球个股公司简介（需登录态 xq_a_token，无则返回 AuthRequired；无法生成 golden，
    # --check 阶段无 golden 自动跳过，计入登录态豁免，见报告说明）
    ("stock_individual_basic_info_xq", ["SH601127"], "loose", "雪球个股公司简介(A股)"),
    ("stock_individual_basic_info_hk_xq", ["02097"], "loose", "雪球个股公司简介(港股)"),
    ("stock_individual_basic_info_us_xq", ["NVDA"], "loose", "雪球个股公司简介(美股)"),
    # === BATCH4 BOND (chinamoney/jisilu/cninfo) ===
    ("bond_treasure_issue_cninfo", ["20210910", "20211109"], "strict", "巨潮-国债发行"),
    ("bond_local_government_issue_cninfo", ["20210911", "20211110"], "strict", "巨潮-地方债发行"),
    ("bond_corporate_issue_cninfo", ["20210911", "20211110"], "strict", "巨潮-企业债发行"),
    ("bond_cov_issue_cninfo", ["20210913", "20211112"], "strict", "巨潮-可转债发行"),
    ("bond_cov_stock_issue_cninfo", [], "strict", "巨潮-可转债转股"),
    # 阶段1: 外汇交易中心 chinamoney
    ("bond_spot_deal", [], "loose", "现券成交行情"),
    ("bond_spot_quote", [], "loose", "现券做市报价"),
    ("bond_china_close_return_map", [], "loose", "收盘收益率曲线映射表"),
    ("bond_china_close_return", ["国债", "1", "20260811", "20260811"], "loose", "收盘收益率曲线历史"),
    ("bond_info_cm", ["24国债01", "", "", "", "", "", "", ""], "loose", "债券信息查询"),
    ("bond_info_detail_cm", ["24国债01"], "loose", "债券详情"),
    ("bond_info_cm_query", ["债券类型"], "loose", "债券筛选条件查询"),
    # 阶段2: 集思录 jisilu
    ("bond_cb_jsl", [""], "loose", "集思录可转债列表"),
    ("bond_cb_redeem_jsl", [], "loose", "集思录可转债强赎"),
    ("bond_cb_index_jsl", [], "loose", "集思录可转债等权指数"),
    ("bond_cb_adj_logs_jsl", ["128013"], "loose", "集思录转股价调整记录"),
    # 阶段3: 巨潮 cninfo 债券发行（已由 main a8c1ae6 在 src/cninfo/mod.rs 实现，本分支跳过）
    # 阶段4: 东方财富 eastmoney 债券
    ("bond_buy_back_hist_em", ["204001"], "loose", "质押式回购历史"),
    ("bond_sh_buy_back_em", [], "loose", "上证质押式回购"),
    ("bond_sz_buy_back_em", [], "loose", "深证质押式回购"),
    ("bond_zh_hs_cov_min", ["sz128039", "15", "", "1979-09-01 09:32:00", "2222-01-01 09:32:00"], "loose", "可转债分钟行情"),
    ("bond_zh_hs_cov_pre_min", ["sh113570"], "loose", "可转债盘前分时"),
    ("bond_zh_cov", [], "loose", "可转债数据"),
    ("bond_zh_cov_info", ["123121", "基本信息"], "loose", "可转债详情"),
    ("bond_zh_cov_value_analysis", ["113527"], "loose", "可转债价值分析"),
    ("bond_cov_comparison", [], "loose", "可转债比价表"),
    ("bond_zh_us_rate", ["19901219"], "loose", "中美国国债收益率"),
    # 阶段5: 新浪 sina 债券
    ("bond_gb_zh_sina", ["中国10年期国债"], "loose", "中国国债收益率"),
    ("bond_gb_us_sina", ["美国10年期国债"], "loose", "美国国债收益率"),
    ("bond_zh_hs_daily", ["sh010107"], "loose", "沪深债券历史日K"),
    ("bond_zh_hs_cov_daily", ["sh010107"], "loose", "沪深可转债历史日K"),
    ("bond_zh_hs_spot", ["1", "10"], "loose", "沪深债券实时行情"),
    ("bond_zh_hs_cov_spot", [], "loose", "沪深可转债实时行情"),
    ("bond_cb_profile_sina", ["sz128039"], "loose", "可转债详情资料"),
    ("bond_cb_summary_sina", ["sh155255"], "loose", "可转债债券概况"),
    # 阶段6: BATCH28 bond g_calc（chinabond 中债指数 / 同花顺可转债 / 国债收益率）
    ("bond_available_index_cbond", [], "loose", "中债可选项中债指数列表"),
    ("bond_zh_cov_info_ths", [], "loose", "同花顺可转债行情"),
    ("bond_china_yield", ["20240101", "20240201"], "loose", "国债收益率曲线"),
    ("bond_index_general_cbond", ["新综合指数", "全价", "总值"], "loose", "中债指数-通用"),
    ("bond_treasury_index_cbond", ["财富", "5Y"], "loose", "中债-国债指数"),
    ("bond_new_composite_index_cbond", ["财富", "总值"], "loose", "中债-新综合指数"),
    ("bond_composite_index_cbond", ["财富", "总值"], "loose", "中债-综合指数"),
    # === BATCH5 LONGTAIL (spot/energy/currency/news/fx/fortune) ===
    ("fx_spot_quote", [], "loose", "外汇即期报价"),
    ("fx_swap_quote", [], "loose", "外汇远掉报价"),
    ("fx_pair_quote", [], "loose", "外币对即期报价"),
    ("fx_c_swap_cm", [], "loose", "外汇掉期C-Swap定盘曲线"),
    # spot - 搜猪网
    ("spot_hog_soozhu", [], "loose", "搜猪网生猪价格"),
    ("spot_hog_year_trend_soozhu", [], "loose", "搜猪网生猪年度走势"),
    ("spot_hog_lean_price_soozhu", [], "loose", "搜猪网瘦肉型猪价格"),
    ("spot_hog_three_way_soozhu", [], "loose", "搜猪网三元猪价格"),
    ("spot_hog_crossbred_soozhu", [], "loose", "搜猪网土杂猪价格"),
    ("spot_corn_price_soozhu", [], "loose", "搜猪网玉米价格"),
    ("spot_soybean_price_soozhu", [], "loose", "搜猪网大豆价格"),
    ("spot_mixed_feed_soozhu", [], "loose", "搜猪网混合饲料价格"),
    # spot - 新浪商品现货指数
    ("spot_goods", ["波罗的海干散货指数"], "strict", "新浪商品现货指数"),
    # spot - 上海黄金交易所
    ("spot_symbol_table_sge", [], "strict", "上金所品种表"),
    ("spot_golden_benchmark_sge", [], "loose", "上金所黄金基准价"),
    ("spot_silver_benchmark_sge", [], "loose", "上金所白银基准价"),
    ("spot_hist_sge", ["Au99.99"], "strict", "上金所历史行情"),
    ("spot_quotations_sge", ["Au99.99"], "loose", "上金所实时行情"),
    # spot - 99期货期现
    ("spot_price_table_qh", [], "strict", "99期货品种表"),
    ("spot_price_qh", ["螺纹钢"], "loose", "99期货期现价格"),
    # energy - 碳排放
    ("energy_carbon_gz", [], "loose", "广州碳排放行情"),
    ("energy_carbon_hb", [], "loose", "湖北碳排放每日概况"),
    # energy - 油价
    ("energy_oil_hist", [], "strict", "汽柴油历史调价"),
    ("energy_oil_detail", ["20240118"], "strict", "各地区汽柴油价格"),
    # currency - 新浪中行 / 外汇局
    ("currency_boc_sina", ["美元", "20230304", "20231110"], "strict", "新浪中行牌价历史"),
    ("currency_boc_safe", [], "loose", "外汇局人民币中间价(近期)"),
    # news - 百度股市通
    ("news_economic_baidu", ["20251126"], "strict", "百度经济数据日历"),
    ("news_trade_notify_suspend_baidu", ["20251126"], "strict", "百度停复牌提醒"),
    ("news_trade_notify_dividend_baidu", ["20251126"], "strict", "百度分红派息提醒"),
    ("news_report_time_baidu", ["20251126"], "strict", "百度财报披露时间"),
    # news - 央视
    ("news_cctv", ["20240424"], "loose", "新闻联播文字稿"),
    # fortune - 胡润研究院
    ("hurun_rank", ["胡润百富榜", "2023"], "loose", "胡润百富榜"),
    # stock_new_gh_cninfo: akshare 在空数据时 pd.DataFrame([]) 设置列名报
    # Length mismatch（上游 bug），无法生成 golden；Rust 侧已离线验证空表列契约
]


def pandas_dtype(dtype) -> str:
    """pandas dtype → 简化五类（与 Rust export_parity 对齐）。"""
    name = str(dtype)
    if name.startswith("int"):
        return "int64"
    if name.startswith("float"):
        return "float64"
    if name.startswith("bool"):
        return "bool"
    if name.startswith("datetime"):
        return "datetime"
    return "str"


def py_contract(func: str, args: list[str]) -> dict:
    """调用 Python akshare 同名函数，输出与 Rust export_parity 同构的契约。"""
    import akshare as ak

    fn = getattr(ak, func)
    df = fn(*args)
    columns = [
        {"name": str(c), "dtype": pandas_dtype(df[c].dtype)} for c in df.columns
    ]
    head: list[list] = []
    for _, row in df.head(HEAD_N).iterrows():
        cells = []
        for c in df.columns:
            v = row[c]
            if v is None or (isinstance(v, float) and v != v):  # NaN
                cells.append(None)
            else:
                cells.append(str(v))
        head.append(cells)
    return {"ok": True, "columns": columns, "height": int(len(df)), "head": head}


def rust_contract(func: str, args: list[str]) -> dict:
    """调用 Rust parity bin，解析契约 JSON。"""
    try:
        proc = subprocess.run(
            [
                PARITY_BIN,
                "--func",
                func,
                "--args",
                json.dumps(args),
                "--head",
                str(HEAD_N),
            ],
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        # 全市场类函数（如质押明细 ~12.6w 行、gdfx 全部 ~1.6M 行）分页耗时超过
        # 120s，超时不应中断整轮 --check：记为失败并继续后续用例。
        return {"ok": False, "error": "parity bin 超时（>120s，疑似全市场分页膨胀）"}
    if proc.returncode != 0:
        return {"ok": False, "error": f"parity bin 退出码 {proc.returncode}: {proc.stderr[:300]}"}
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        return {"ok": False, "error": f"parity bin 输出非 JSON: {e}; stdout={proc.stdout[:200]}"}


def golden_key(func: str, params: list[str]) -> str:
    """golden 文件名：单用例函数直接用函数名；同名函数多个参数用例（如
    futures_settle 分市场）追加参数摘要，避免互相覆盖。"""
    if sum(1 for c in CASES if c[0] == func) <= 1:
        return func
    parts = []
    for p in params:
        safe = "".join(ch for ch in p if ch.isalnum())
        parts.append(safe if safe else "_")
    return f"{func}_{'_'.join(parts)}"


def load_golden(func: str, params: list[str]) -> dict | None:
    path = os.path.join(GOLDEN_DIR, f"{golden_key(func, params)}.json")
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    return None


def save_golden(func: str, params: list[str], contract: dict) -> None:
    os.makedirs(GOLDEN_DIR, exist_ok=True)
    path = os.path.join(GOLDEN_DIR, f"{golden_key(func, params)}.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(contract, f, ensure_ascii=False, indent=2)


def norm_val(v) -> str | None:
    """归一化单元格值用于比较（按有效位数吸收跨语言浮点噪声）。"""
    if v is None:
        return None
    s = str(v).strip()
    if s in ("nan", "None", "NaT", ""):
        return None
    try:
        f = float(s)
    except ValueError:
        return s
    if f == 0:
        return "0"
    # 按有效位数四舍五入：大数（如 1.9e10）与小数字（如 37.19）都只保留 SIGFIGS
    # 位有效数字，从而忽略 double 末位的浮点解析噪声。
    import math

    mag = math.floor(math.log10(abs(f)))
    ndigits = SIGFIGS - 1 - mag
    r = round(f, ndigits)
    if r == int(r) and abs(r) < 1e15:
        return str(int(r))
    return f"{r:.{max(ndigits, 0)}f}".rstrip("0").rstrip(".")


def compare(func: str, golden: dict, actual: dict, mode: str) -> list[str]:
    """对比两个契约，返回失败项列表。"""
    issues: list[str] = []
    if golden.get("ok") is not True:
        return [f"golden 生成失败: {golden.get('error')}"]
    if actual.get("ok") is not True:
        return [f"rust 执行失败: {actual.get('error')}"]

    g_cols, a_cols = golden["columns"], actual["columns"]
    if [c["name"] for c in g_cols] != [c["name"] for c in a_cols]:
        issues.append(
            f"列名不一致\n  python: {[c['name'] for c in g_cols]}\n  rust:   {[c['name'] for c in a_cols]}"
        )
    # dtype 归一化：pandas 自动推断的 int64/float64 视为同一数值类（值仍严格比较）；
    # pandas 的 datetime64 与我们的 ISO 日期字符串表示等价（值仍严格比较）
    def norm_dtype(d):
        if d in ("int64", "float64"):
            return "num"
        if d in ("datetime", "str"):
            return "str"
        return d

    g_dt = [norm_dtype(c["dtype"]) for c in g_cols]
    a_dt = [norm_dtype(c["dtype"]) for c in a_cols]
    if g_dt != a_dt:
        issues.append(
            f"dtype 不一致\n  python: {[c['dtype'] for c in g_cols]}\n  rust:   {[c['dtype'] for c in a_cols]}"
        )

    if mode == "strict":
        if golden["height"] != actual["height"]:
            issues.append(f"行数不一致: python={golden['height']} rust={actual['height']}")
        g_head, a_head = golden["head"], actual["head"]
        for i, (grow, arow) in enumerate(zip(g_head, a_head)):
            g_norm = [norm_val(v) for v in grow]
            a_norm = [norm_val(v) for v in arow]
            if g_norm != a_norm:
                issues.append(f"head 第 {i} 行不一致\n  python: {g_norm}\n  rust:   {a_norm}")
                break
    return issues


def main() -> int:
    ap = argparse.ArgumentParser(description="parity 差分测试")
    ap.add_argument("--generate", action="store_true", help="生成 golden fixture")
    ap.add_argument("--check", action="store_true", help="对比 golden 与 rust 输出")
    ap.add_argument("--only", help="仅运行指定函数")
    ap.add_argument("--list", action="store_true", help="列出用例")
    args = ap.parse_args()

    if args.list:
        for func, params, mode, desc in CASES:
            key = golden_key(func, params)
            suffix = f" (golden: {key})" if key != func else ""
            print(f"{func}({', '.join(params) or '-'})  [{mode}]  {desc}{suffix}")
        return 0

    cases = CASES
    if args.only:
        cases = [c for c in CASES if c[0] == args.only]
        if not cases:
            print(f"未知函数: {args.only}")
            return 2

    failures = 0
    skipped = 0
    for func, params, mode, desc in cases:
        label = f"{func}({', '.join(params) or '-'}) [{mode}] {desc}"

        if args.generate:
            try:
                contract = py_contract(func, params)
                save_golden(func, params, contract)
                status = "生成" if contract.get("ok") else "失败"
                detail = (
                    f"{len(contract['columns'])} 列 x {contract['height']} 行"
                    if contract.get("ok")
                    else contract.get("error")
                )
            except Exception as e:  # noqa: BLE001
                contract = {"ok": False, "error": str(e)[:200]}
                status = "异常"
                detail = str(e)[:200]
            if not contract.get("ok"):
                failures += 1
            print(f"[{status}] {label} -> {detail}")

        if args.check:
            golden = load_golden(func, params)
            if golden is None:
                print(f"[跳过] {label} (无 golden fixture，先运行 --generate)")
                skipped += 1
                continue
            actual = rust_contract(func, params)
            issues = compare(func, golden, actual, mode)
            if issues:
                failures += 1
                print(f"[失败] {label}")
                for it in issues:
                    print(f"       {it}")
            else:
                print(
                    f"[通过] {label} ({len(golden['columns'])} 列 x {golden['height']} 行)"
                )

    print(f"\n汇总: {'失败' if failures else '全部通过'} (失败 {failures}, 跳过 {skipped})")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
