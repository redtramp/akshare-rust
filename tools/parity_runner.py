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
    ("stock_zh_a_st_em", [], "loose", "ST股板块"),
    ("stock_zh_a_new_em", [], "loose", "新股板块"),
    ("stock_hk_spot_em", [], "loose", "东财港股实时行情"),
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
    # === BATCH3 ECONOMIC REMAINING (jin10/em datacenter) ===
    # === BATCH3 STOCK_FUNDAMENTAL REMAINING (ths/sina/em) ===
    # === BATCH4 BOND (chinamoney/jisilu/cninfo/em/sina/exchange/calc) ===
    # 阶段1: 外汇交易中心 chinamoney
    ("bond_spot_deal", [], "loose", "现券成交行情"),
    ("bond_spot_quote", [], "loose", "现券做市报价"),
    ("bond_china_close_return_map", [], "loose", "收盘收益率曲线映射表"),
    ("bond_china_close_return", ["国债", "1", "20260811", "20260811"], "loose", "收盘收益率曲线历史"),
    ("bond_info_cm", ["24国债01", "", "", "", "", "", "", ""], "loose", "债券信息查询"),
    ("bond_info_detail_cm", ["24国债01"], "loose", "债券详情"),
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
    ("bond_zh_us_rate", ["19901219"], "loose", "中美国债收益率"),
    # 阶段5: 新浪 sina 债券
    ("bond_gb_zh_sina", ["中国10年期国债"], "loose", "中国国债收益率"),
    ("bond_gb_us_sina", ["美国10年期国债"], "loose", "美国国债收益率"),
    # === BATCH5 LONGTAIL (spot/energy/currency/news/fx/fortune) ===
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
