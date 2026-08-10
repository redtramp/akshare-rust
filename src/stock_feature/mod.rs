//! 股票特色数据接口（对应 akshare `stock_feature/*`）。
//!
//! 本期实现东财系接口：实时行情快照（push2 clist）+ 数据中心 `RPT_*` 报表
//! （datacenter-web）。复用 `sources::eastmoney` 的 `fetch_clist` /
//! `fetch_datacenter_pages` 与 `finalize_spot` / `finalize_report` 工具，
//! 列名与 akshare 逐字对齐（离线单测 + 与 Python akshare 实测列序核对）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::{
    fetch_clist, fetch_datacenter_pages, fetch_eastmoney_pages, finalize_report, finalize_spot,
    push2_urls,
};
use serde_json::{json, Map, Value};

/// 沪深京 A 股实时行情公共字段串（与 akshare `stock_zh_a_spot_em` 一致）。
const SPOT_FIELDS: &str = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152";
/// 港股实时行情字段串（与 akshare `stock_hk_spot_em` 一致）。
const HK_SPOT_FIELDS: &str = "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f12,f13,f14,f15,f16,f17,f18,f20,f21,f23,f24,f25,f22,f11,f62,f128,f136,f115,f152";

/// A 股快照列重命名（f-字段码 → 中文），含序号列。
const SPOT_RENAME: [(&str, &str); 23] = [
    ("index", "序号"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f5", "成交量"),
    ("f6", "成交额"),
    ("f7", "振幅"),
    ("f8", "换手率"),
    ("f9", "市盈率-动态"),
    ("f10", "量比"),
    ("f11", "5分钟涨跌"),
    ("f12", "代码"),
    ("f14", "名称"),
    ("f15", "最高"),
    ("f16", "最低"),
    ("f17", "今开"),
    ("f18", "昨收"),
    ("f20", "总市值"),
    ("f21", "流通市值"),
    ("f22", "60日涨跌幅"),
    ("f23", "年初至今涨跌幅"),
    ("f24", "涨速"),
    ("f25", "市净率"),
];
/// A 股快照输出列顺序。
const SPOT_SELECT: [&str; 23] = [
    "序号",
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "涨跌额",
    "成交量",
    "成交额",
    "振幅",
    "最高",
    "最低",
    "今开",
    "昨收",
    "量比",
    "换手率",
    "市盈率-动态",
    "市净率",
    "总市值",
    "流通市值",
    "涨速",
    "5分钟涨跌",
    "60日涨跌幅",
    "年初至今涨跌幅",
];
/// A 股快照数值列。
const SPOT_NUMERIC: [&str; 20] = [
    "最新价",
    "涨跌幅",
    "涨跌额",
    "成交量",
    "成交额",
    "振幅",
    "最高",
    "最低",
    "今开",
    "昨收",
    "量比",
    "换手率",
    "市盈率-动态",
    "市净率",
    "总市值",
    "流通市值",
    "涨速",
    "5分钟涨跌",
    "60日涨跌幅",
    "年初至今涨跌幅",
];

/// 新股快照：在标准 A 股快照基础上增加「上市日期」(`f26`)。
const NEW_A_RENAME: [(&str, &str); 24] = [
    ("index", "序号"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f5", "成交量"),
    ("f6", "成交额"),
    ("f7", "振幅"),
    ("f8", "换手率"),
    ("f9", "市盈率-动态"),
    ("f10", "量比"),
    ("f11", "5分钟涨跌"),
    ("f12", "代码"),
    ("f14", "名称"),
    ("f15", "最高"),
    ("f16", "最低"),
    ("f17", "今开"),
    ("f18", "昨收"),
    ("f20", "总市值"),
    ("f21", "流通市值"),
    ("f22", "60日涨跌幅"),
    ("f23", "年初至今涨跌幅"),
    ("f24", "涨速"),
    ("f25", "市净率"),
    ("f26", "上市日期"),
];
/// 新股快照输出列顺序（上市日期在标准快照的「市净率」之后）。
const NEW_A_SELECT: [&str; 24] = [
    "序号",
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "涨跌额",
    "成交量",
    "成交额",
    "振幅",
    "最高",
    "最低",
    "今开",
    "昨收",
    "量比",
    "换手率",
    "市盈率-动态",
    "市净率",
    "上市日期",
    "总市值",
    "流通市值",
    "涨速",
    "5分钟涨跌",
    "60日涨跌幅",
    "年初至今涨跌幅",
];
/// 新股快照数值列（上市日期为日期，不数值化）。
const NEW_A_NUMERIC: [&str; 20] = SPOT_NUMERIC;

/// 港股快照列重命名（f-字段码 → 中文），含序号列。
const HK_RENAME: [(&str, &str); 17] = [
    ("index", "序号"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
    ("f4", "涨跌额"),
    ("f5", "成交量"),
    ("f6", "成交额"),
    ("f7", "振幅"),
    ("f8", "换手率"),
    ("f9", "市盈率-动态"),
    ("f10", "量比"),
    ("f12", "代码"),
    ("f14", "名称"),
    ("f15", "最高"),
    ("f16", "最低"),
    ("f17", "今开"),
    ("f18", "昨收"),
    ("f25", "市净率"),
];
/// 港股快照输出列顺序（与 akshare `stock_hk_spot_em` 一致）。
const HK_SELECT: [&str; 12] = [
    "序号",
    "代码",
    "名称",
    "最新价",
    "涨跌额",
    "涨跌幅",
    "今开",
    "最高",
    "最低",
    "昨收",
    "成交量",
    "成交额",
];
/// 港股快照数值列。
const HK_NUMERIC: [&str; 9] = [
    "最新价",
    "涨跌额",
    "涨跌幅",
    "今开",
    "最高",
    "最低",
    "昨收",
    "成交量",
    "成交额",
];

/// 股东户数（`RPT_HOLDERNUMLATEST` / `RPT_HOLDERNUM_DET`）字段键 → 中文列名。
const GDHS_RENAME: [(&str, &str); 16] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("END_DATE", "股东户数统计截止日-本次"),
    ("INTERVAL_CHRATE", "区间涨跌幅"),
    ("AVG_MARKET_CAP", "户均持股市值"),
    ("AVG_HOLD_NUM", "户均持股数量"),
    ("TOTAL_MARKET_CAP", "总市值"),
    ("TOTAL_A_SHARES", "总股本"),
    ("HOLD_NOTICE_DATE", "公告日期"),
    ("HOLDER_NUM", "股东户数-本次"),
    ("PRE_HOLDER_NUM", "股东户数-上次"),
    ("HOLDER_NUM_CHANGE", "股东户数-增减"),
    ("HOLDER_NUM_RATIO", "股东户数-增减比例"),
    ("PRE_END_DATE", "股东户数统计截止日-上次"),
    ("f2", "最新价"),
    ("f3", "涨跌幅"),
];
/// 股东户数输出列顺序（与 akshare `stock_zh_a_gdhs` 一致）。
const GDHS_SELECT: [&str; 16] = [
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "股东户数-本次",
    "股东户数-上次",
    "股东户数-增减",
    "股东户数-增减比例",
    "区间涨跌幅",
    "股东户数统计截止日-本次",
    "股东户数统计截止日-上次",
    "户均持股市值",
    "户均持股数量",
    "总市值",
    "总股本",
    "公告日期",
];
/// 股东户数数值列（日期列单独 `cast_date`）。
const GDHS_NUMERIC: [&str; 11] = [
    "最新价",
    "涨跌幅",
    "股东户数-本次",
    "股东户数-上次",
    "股东户数-增减",
    "股东户数-增减比例",
    "区间涨跌幅",
    "户均持股市值",
    "户均持股数量",
    "总市值",
    "总股本",
];
/// 股东户数日期列。
const GDHS_DATE: [&str; 3] = [
    "股东户数统计截止日-本次",
    "股东户数统计截止日-上次",
    "公告日期",
];

// ===== 融资融券账户信息（RPTA_WEB_MARGIN_DAILYTRADE）=====
const MARGIN_RENAME: [(&str, &str); 13] = [
    ("STATISTICS_DATE", "日期"),
    ("FIN_BALANCE", "融资余额"),
    ("LOAN_BALANCE", "融券余额"),
    ("FIN_BUY_AMT", "融资买入额"),
    ("LOAN_SELL_AMT", "融券卖出额"),
    ("SECURITY_ORG_NUM", "证券公司数量"),
    ("OPERATEDEPT_NUM", "营业部数量"),
    ("PERSONAL_INVESTOR_NUM", "个人投资者数量"),
    ("ORG_INVESTOR_NUM", "机构投资者数量"),
    ("INVESTOR_NUM", "参与交易的投资者数量"),
    ("MARGINLIAB_INVESTOR_NUM", "有融资融券负债的投资者数量"),
    ("TOTAL_GUARANTEE", "担保物总价值"),
    ("AVG_GUARANTEE_RATIO", "平均维持担保比例"),
];
const MARGIN_SELECT: [&str; 13] = [
    "日期",
    "融资余额",
    "融券余额",
    "融资买入额",
    "融券卖出额",
    "证券公司数量",
    "营业部数量",
    "个人投资者数量",
    "机构投资者数量",
    "参与交易的投资者数量",
    "有融资融券负债的投资者数量",
    "担保物总价值",
    "平均维持担保比例",
];
const MARGIN_NUMERIC: [&str; 12] = [
    "融资余额",
    "融券余额",
    "融资买入额",
    "融券卖出额",
    "证券公司数量",
    "营业部数量",
    "个人投资者数量",
    "机构投资者数量",
    "参与交易的投资者数量",
    "有融资融券负债的投资者数量",
    "担保物总价值",
    "平均维持担保比例",
];
const MARGIN_DATE: [&str; 1] = ["日期"];

// ===== 股东自由持股明细（RPT_F10_EH_FREEHOLDERS）=====
const FREE_HOLD_RENAME: [(&str, &str); 12] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_TYPE", "股东类型"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("END_DATE", "报告期"),
    ("HOLD_NUM", "期末持股-数量"),
    ("XZCHANGE", "期末持股-数量变化"),
    ("CHANGE_RATIO", "期末持股-数量变化比例"),
    ("HOLDNUM_CHANGE_NAME", "期末持股-持股变动"),
    ("HOLDER_MARKET_CAP", "期末持股-流通市值"),
    ("UPDATE_DATE", "公告日"),
    ("REPORT_DATE_NAME", "报告名称"),
];
const FREE_HOLD_SELECT: [&str; 11] = [
    "股东名称",
    "股东类型",
    "股票代码",
    "股票简称",
    "报告期",
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-持股变动",
    "期末持股-流通市值",
    "公告日",
];
const FREE_HOLD_NUMERIC: [&str; 4] = [
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-流通市值",
];
const FREE_HOLD_DATE: [&str; 2] = ["报告期", "公告日"];

// ===== 股东持股明细（RPT_DMSK_HOLDERS）=====
const HOLD_DETAIL_RENAME: [(&str, &str); 12] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_NEWTYPE", "股东类型"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("END_DATE", "报告期"),
    ("HOLD_NUM", "期末持股-数量"),
    ("HOLD_NUM_CHANGE", "期末持股-数量变化"),
    ("HOLD_RATIO_CHANGE", "期末持股-数量变化比例"),
    ("HOLDNUM_CHANGE_NAME", "期末持股-持股变动"),
    ("HOLDER_MARKET_CAP", "期末持股-流通市值"),
    ("NOTICE_DATE", "公告日"),
    ("RANK", "股东排名"),
];
const HOLD_DETAIL_SELECT: [&str; 12] = [
    "股东名称",
    "股东类型",
    "股票代码",
    "股票简称",
    "报告期",
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-持股变动",
    "期末持股-流通市值",
    "公告日",
    "股东排名",
];
const HOLD_DETAIL_NUMERIC: [&str; 5] = [
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-流通市值",
    "股东排名",
];
const HOLD_DETAIL_DATE: [&str; 2] = ["报告期", "公告日"];

// ===== 股东自由持股分析（RPT_CUSTOM_F10_EH_FREEHOLDERS_JOIN_FREEHOLDER_SHAREANALYSIS）=====
const FREE_ANALYSE_RENAME: [(&str, &str); 14] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_TYPE", "股东类型"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("END_DATE", "报告期"),
    ("HOLD_NUM", "期末持股-数量"),
    ("XZCHANGE", "期末持股-数量变化"),
    ("HOLD_RATIO_CHANGE", "期末持股-数量变化比例"),
    ("HOLDNUM_CHANGE_NAME", "期末持股-持股变动"),
    ("HOLDER_MARKET_CAP", "期末持股-流通市值"),
    ("UPDATE_DATE", "公告日"),
    ("D10_ADJCHRATE", "公告日后涨跌幅-10个交易日"),
    ("D30_ADJCHRATE", "公告日后涨跌幅-30个交易日"),
    ("D60_ADJCHRATE", "公告日后涨跌幅-60个交易日"),
];
const FREE_ANALYSE_SELECT: [&str; 14] = [
    "股东名称",
    "股东类型",
    "股票代码",
    "股票简称",
    "报告期",
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-持股变动",
    "期末持股-流通市值",
    "公告日",
    "公告日后涨跌幅-10个交易日",
    "公告日后涨跌幅-30个交易日",
    "公告日后涨跌幅-60个交易日",
];
const FREE_ANALYSE_NUMERIC: [&str; 7] = [
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-流通市值",
    "公告日后涨跌幅-10个交易日",
    "公告日后涨跌幅-30个交易日",
    "公告日后涨跌幅-60个交易日",
];
const FREE_ANALYSE_DATE: [&str; 2] = ["报告期", "公告日"];

// ===== 股东持股分析（RPT_CUSTOM_DMSK_HOLDERS_JOIN_HOLDER_SHAREANALYSIS）=====
const HOLD_ANALYSE_RENAME: [(&str, &str); 14] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_TYPE_ORG", "股东类型"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("END_DATE", "报告期"),
    ("HOLD_NUM", "期末持股-数量"),
    ("HOLD_NUM_CHANGE", "期末持股-数量变化"),
    ("HOLD_RATIO_CHANGE", "期末持股-数量变化比例"),
    ("NOTICE_DATE", "公告日"),
    ("HOLDER_MARKET_CAP", "期末持股-流通市值"),
    ("HOLDNUM_CHANGE_NAME", "期末持股-持股变动"),
    ("D10_ADJCHRATE", "公告日后涨跌幅-10个交易日"),
    ("D30_ADJCHRATE", "公告日后涨跌幅-30个交易日"),
    ("D60_ADJCHRATE", "公告日后涨跌幅-60个交易日"),
];
const HOLD_ANALYSE_SELECT: [&str; 14] = [
    "股东名称",
    "股东类型",
    "股票代码",
    "股票简称",
    "报告期",
    "期末持股-数量",
    "期末持股-数量变化",
    "期末持股-数量变化比例",
    "期末持股-持股变动",
    "期末持股-流通市值",
    "公告日",
    "公告日后涨跌幅-10个交易日",
    "公告日后涨跌幅-30个交易日",
    "公告日后涨跌幅-60个交易日",
];
const HOLD_ANALYSE_NUMERIC: [&str; 7] = FREE_ANALYSE_NUMERIC;
const HOLD_ANALYSE_DATE: [&str; 2] = ["报告期", "公告日"];

// ===== 券商业绩（RPT_PERFORMANCE）=====
const QSJY_RENAME: [(&str, &str); 14] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "简称"),
    ("NETPROFIT", "当月净利润-净利润"),
    ("NP_YOY", "当月净利润-同比增长"),
    ("NP_QOQ", "当月净利润-环比增长"),
    ("ACCUMPROFIT", "当年累计净利润-累计净利润"),
    ("ACCUMPROFIT_YOY", "当年累计净利润-同比增长"),
    ("OPERATE_INCOME", "当月营业收入-营业收入"),
    ("OI_YOY", "当月营业收入-环比增长"),
    ("OI_QOQ", "当月营业收入-同比增长"),
    ("ACCUMOI", "当年累计营业收入-累计营业收入"),
    ("ACCUMOI_YOY", "当年累计营业收入-同比增长"),
    ("NET_ASSETS", "净资产-净资产"),
    ("NA_YOY", "净资产-同比增长"),
];
const QSJY_SELECT: [&str; 14] = [
    "简称",
    "代码",
    "当月净利润-净利润",
    "当月净利润-同比增长",
    "当月净利润-环比增长",
    "当年累计净利润-累计净利润",
    "当年累计净利润-同比增长",
    "当月营业收入-营业收入",
    "当月营业收入-环比增长",
    "当月营业收入-同比增长",
    "当年累计营业收入-累计营业收入",
    "当年累计营业收入-同比增长",
    "净资产-净资产",
    "净资产-同比增长",
];
const QSJY_NUMERIC: [&str; 12] = [
    "当月净利润-净利润",
    "当月净利润-同比增长",
    "当月净利润-环比增长",
    "当年累计净利润-累计净利润",
    "当年累计净利润-同比增长",
    "当月营业收入-营业收入",
    "当月营业收入-环比增长",
    "当月营业收入-同比增长",
    "当年累计营业收入-累计营业收入",
    "当年累计营业收入-同比增长",
    "净资产-净资产",
    "净资产-同比增长",
];

// ===== 股权质押总览（RPT_CSDC_STATISTICS）=====
const GPZY_PROFILE_RENAME: [(&str, &str); 8] = [
    ("TRADE_DATE", "交易日期"),
    ("TOTAL_PLEDGED_SHARES", "质押总股数"),
    ("PLEDGE_MARKET_VALUE", "质押总市值"),
    ("CSI_300_INDEX", "沪深300指数"),
    ("CSI_300_CHG", "涨跌幅"),
    ("PM_RATIO", "A股质押总比例"),
    ("PLEDGE_CO_NUM", "质押公司数量"),
    ("DAILY_STATISTICS", "质押笔数"),
];
const GPZY_PROFILE_SELECT: [&str; 8] = [
    "交易日期",
    "A股质押总比例",
    "质押公司数量",
    "质押笔数",
    "质押总股数",
    "质押总市值",
    "沪深300指数",
    "涨跌幅",
];
const GPZY_PROFILE_NUMERIC: [&str; 7] = [
    "A股质押总比例",
    "质押公司数量",
    "质押笔数",
    "质押总股数",
    "质押总市值",
    "沪深300指数",
    "涨跌幅",
];
const GPZY_PROFILE_DATE: [&str; 1] = ["交易日期"];

// ===== 个股股权质押比例（RPT_CSDC_LIST）=====
const GPZY_PLEDGE_RENAME: [(&str, &str); 12] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("TRADE_DATE", "交易日期"),
    ("PLEDGE_RATIO", "质押比例"),
    ("REPURCHASE_BALANCE", "质押股数"),
    ("PLEDGE_DEAL_NUM", "质押笔数"),
    ("REPURCHASE_UNLIMITED_BALANCE", "无限售股质押数"),
    ("REPURCHASE_LIMITED_BALANCE", "限售股质押数"),
    ("PLEDGE_MARKET_CAP", "质押市值"),
    ("INDUSTRY", "所属行业"),
    ("Y1_CLOSE_ADJCHRATE", "近一年涨跌幅"),
    ("INDUSTRY_CODE", "所属行业代码"),
];
const GPZY_PLEDGE_SELECT: [&str; 12] = [
    "股票代码",
    "股票简称",
    "交易日期",
    "所属行业",
    "质押比例",
    "质押股数",
    "质押市值",
    "质押笔数",
    "无限售股质押数",
    "限售股质押数",
    "近一年涨跌幅",
    "所属行业代码",
];
const GPZY_PLEDGE_NUMERIC: [&str; 7] = [
    "质押比例",
    "质押股数",
    "质押市值",
    "质押笔数",
    "无限售股质押数",
    "限售股质押数",
    "近一年涨跌幅",
];
const GPZY_PLEDGE_DATE: [&str; 1] = ["交易日期"];

// ===== 行业股权质押统计（RPT_CSDC_INDUSTRY_STATISTICS）=====
const GPZY_INDUSTRY_RENAME: [(&str, &str); 7] = [
    ("INDUSTRY", "行业"),
    ("TRADE_DATE", "统计时间"),
    ("AVERAGE_PLEDGE_RATIO", "平均质押比例"),
    ("ORG_NUM", "公司家数"),
    ("PLEDGE_TOTAL_NUM", "质押总笔数"),
    ("TOTAL_PLEDGE_SHARES", "质押总股本"),
    ("PLEDGE_TOTAL_MARKETCAP", "最新质押市值"),
];
const GPZY_INDUSTRY_SELECT: [&str; 7] = [
    "行业",
    "平均质押比例",
    "公司家数",
    "质押总笔数",
    "质押总股本",
    "最新质押市值",
    "统计时间",
];
const GPZY_INDUSTRY_NUMERIC: [&str; 5] = [
    "平均质押比例",
    "公司家数",
    "质押总笔数",
    "质押总股本",
    "最新质押市值",
];
const GPZY_INDUSTRY_DATE: [&str; 1] = ["统计时间"];

// ===== 个股估值分析（RPT_VALUEANALYSIS_DET）=====
const VALUE_RENAME: [(&str, &str); 13] = [
    ("TRADE_DATE", "数据日期"),
    ("CLOSE_PRICE", "当日收盘价"),
    ("CHANGE_RATE", "当日涨跌幅"),
    ("TOTAL_MARKET_CAP", "总市值"),
    ("NOTLIMITED_MARKETCAP_A", "流通市值"),
    ("TOTAL_SHARES", "总股本"),
    ("FREE_SHARES_A", "流通股本"),
    ("PE_TTM", "PE(TTM)"),
    ("PE_LAR", "PE(静)"),
    ("PB_MRQ", "市净率"),
    ("PEG_CAR", "PEG值"),
    ("PCF_OCF_TTM", "市现率"),
    ("PS_TTM", "市销率"),
];
const VALUE_SELECT: [&str; 13] = [
    "数据日期",
    "当日收盘价",
    "当日涨跌幅",
    "总市值",
    "流通市值",
    "总股本",
    "流通股本",
    "PE(TTM)",
    "PE(静)",
    "市净率",
    "PEG值",
    "市现率",
    "市销率",
];
const VALUE_NUMERIC: [&str; 12] = [
    "当日收盘价",
    "当日涨跌幅",
    "总市值",
    "流通市值",
    "总股本",
    "流通股本",
    "PE(TTM)",
    "PE(静)",
    "市净率",
    "PEG值",
    "市现率",
    "市销率",
];
const VALUE_DATE: [&str; 1] = ["数据日期"];

// ===== 股东大会（RPT_GENERALMEETING_DETAIL）=====
const GDDH_RENAME: [(&str, &str); 12] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "简称"),
    ("MEETING_TITLE", "股东大会名称"),
    ("START_ADJUST_DATE", "召开开始日"),
    ("EQUITY_RECORD_DATE", "股权登记日"),
    ("ONSITE_RECORD_DATE", "现场登记日"),
    ("DECISION_NOTICE_DATE", "决议公告日"),
    ("NOTICE_DATE", "公告日"),
    ("WEB_START_DATE", "网络投票时间-开始日"),
    ("WEB_END_DATE", "网络投票时间-结束日"),
    ("SERIAL_NUM", "序列号"),
    ("PROPOSAL", "提案"),
];
const GDDH_SELECT: [&str; 12] = [
    "代码",
    "简称",
    "股东大会名称",
    "召开开始日",
    "股权登记日",
    "现场登记日",
    "网络投票时间-开始日",
    "网络投票时间-结束日",
    "决议公告日",
    "公告日",
    "序列号",
    "提案",
];
const GDDH_DATE: [&str; 7] = [
    "召开开始日",
    "股权登记日",
    "现场登记日",
    "网络投票时间-开始日",
    "网络投票时间-结束日",
    "决议公告日",
    "公告日",
];

// ===== 重大合同明细（RPTA_WEB_ZDHT_LIST）=====
const ZDHT_RENAME: [(&str, &str); 14] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("SIGNATORY", "签署主体"),
    ("SIGNATORYREL", "签署主体-与上市公司关系"),
    ("COUNTERPARTY", "其他签署方"),
    ("COUNTERPARTYREL", "其他签署方-与上市公司关系"),
    ("CONTRACTTYPENAME", "合同类型"),
    ("CONTRACTNAME", "合同名称"),
    ("AMOUNTS", "合同金额"),
    ("SNDYYSR", "上年度营业收入"),
    ("ZSNDYYSRBL", "占上年度营业收入比例"),
    ("OPERATEREVE", "最新财务报表的营业收入"),
    ("SIGNDATE", "签署日期"),
    ("DIM_RDATE", "公告日期"),
];
const ZDHT_SELECT: [&str; 14] = [
    "股票代码",
    "股票简称",
    "签署主体",
    "签署主体-与上市公司关系",
    "其他签署方",
    "其他签署方-与上市公司关系",
    "合同类型",
    "合同名称",
    "合同金额",
    "上年度营业收入",
    "占上年度营业收入比例",
    "最新财务报表的营业收入",
    "签署日期",
    "公告日期",
];
const ZDHT_NUMERIC: [&str; 4] = [
    "合同金额",
    "上年度营业收入",
    "占上年度营业收入比例",
    "最新财务报表的营业收入",
];
const ZDHT_DATE: [&str; 2] = ["签署日期", "公告日期"];

// ===== 打新收益率（RPTA_APP_IPOAPPLY）=====
const DXSYL_RENAME: [(&str, &str); 16] = [
    ("SECURITY_CODE", "股票代码"),
    ("f14", "股票简称"),
    ("ISSUE_PRICE", "发行价"),
    ("LATELY_PRICE", "最新价"),
    ("ONLINE_ISSUE_LWR", "网上-发行中签率"),
    ("ONLINE_VA_SHARES", "网上-有效申购股数"),
    ("ONLINE_VA_NUM", "网上-有效申购户数"),
    ("ONLINE_ES_MULTIPLE", "网上-超额认购倍数"),
    ("OFFLINE_VAP_RATIO", "网下-配售中签率"),
    ("OFFLINE_VATS", "网下-有效申购股数"),
    ("OFFLINE_VAP_OBJECT", "网下-有效申购户数"),
    ("OFFLINE_VAS_MULTIPLE", "网下-配售认购倍数"),
    ("ISSUE_NUM", "总发行数量"),
    ("LD_OPEN_PREMIUM", "开盘溢价"),
    ("LD_CLOSE_CHANGE", "首日涨幅"),
    ("LISTING_DATE", "上市日期"),
];
const DXSYL_SELECT: [&str; 16] = [
    "股票代码",
    "股票简称",
    "发行价",
    "最新价",
    "网上-发行中签率",
    "网上-有效申购股数",
    "网上-有效申购户数",
    "网上-超额认购倍数",
    "网下-配售中签率",
    "网下-有效申购股数",
    "网下-有效申购户数",
    "网下-配售认购倍数",
    "总发行数量",
    "开盘溢价",
    "首日涨幅",
    "上市日期",
];
const DXSYL_NUMERIC: [&str; 13] = [
    "发行价",
    "最新价",
    "网上-发行中签率",
    "网上-有效申购股数",
    "网上-有效申购户数",
    "网上-超额认购倍数",
    "网下-配售中签率",
    "网下-有效申购股数",
    "网下-有效申购户数",
    "网下-配售认购倍数",
    "总发行数量",
    "开盘溢价",
    "首日涨幅",
];
const DXSYL_DATE: [&str; 1] = ["上市日期"];

// ===== 商誉市场统计（RPT_GOODWILL_MARKETSTATISTICS）=====
const SY_PROFILE_RENAME: [(&str, &str); 8] = [
    ("REPORT_DATE", "报告期"),
    ("GOODWILL", "商誉"),
    ("GOODWILL_CHANGE", "商誉减值"),
    ("SUMSHEQUITY", "净资产"),
    ("SUMSHEQUITY_RATIO", "商誉占净资产比例"),
    ("SUMSHEQUITY_CHANGE_RATIO", "商誉减值占净资产比例"),
    ("PARENTNETPROFIT", "净利润规模"),
    ("PNP_CHANGE_RATIO", "商誉减值占净利润比例"),
];
const SY_PROFILE_SELECT: [&str; 8] = [
    "报告期",
    "商誉",
    "商誉减值",
    "净资产",
    "商誉占净资产比例",
    "商誉减值占净资产比例",
    "净利润规模",
    "商誉减值占净利润比例",
];
const SY_PROFILE_NUMERIC: [&str; 7] = [
    "商誉",
    "商誉减值",
    "净资产",
    "商誉占净资产比例",
    "商誉减值占净资产比例",
    "净利润规模",
    "商誉减值占净利润比例",
];
const SY_PROFILE_DATE: [&str; 1] = ["报告期"];

// ===== 重要股东股权质押明细（RPTA_APP_ACCUMDETAILS）=====
// 序号由 Rust 生成（东财原始 JSON 无 index 键，与 akshare reset_index 一致）。
const PLEDGE_DETAIL_RENAME: [(&str, &str); 14] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("HOLDER_NAME", "股东名称"),
    ("NOTICE_DATE", "公告日期"),
    ("PF_ORG", "质押机构"),
    ("PF_NUM", "质押股份数量"),
    ("PF_HOLD_RATIO", "占所持股份比例"),
    ("PF_TSR", "占总股本比例"),
    ("CLOSE_FORWARD_ADJPRICE", "质押日收盘价"),
    ("PF_START_DATE", "质押开始日期"),
    ("ACTUAL_UNFREEZE_DATE", "质押结束日期"),
    ("UNFREEZE_STATE", "状态"),
    ("WARNING_LINE", "预估平仓线"),
    ("CLOSE_PRICE", "最新价"),
];
const PLEDGE_DETAIL_SELECT: [&str; 14] = [
    "股票代码",
    "股票简称",
    "股东名称",
    "质押股份数量",
    "占所持股份比例",
    "占总股本比例",
    "质押机构",
    "最新价",
    "质押日收盘价",
    "预估平仓线",
    "质押开始日期",
    "质押结束日期",
    "状态",
    "公告日期",
];
const PLEDGE_DETAIL_NUMERIC: [&str; 6] = [
    "质押股份数量",
    "占所持股份比例",
    "占总股本比例",
    "最新价",
    "质押日收盘价",
    "预估平仓线",
];
const PLEDGE_DETAIL_DATE: [&str; 3] = ["公告日期", "质押开始日期", "质押结束日期"];

// ===== 高管持股变动（RPT_SHARE_HOLDER_INCREASE）=====
// 含 quoteColumns 注入的最新价/涨跌幅；akshare 输出无「序号」列。
const GGCG_RENAME: [(&str, &str); 16] = [
    ("CHANGE_NUM", "持股变动信息-变动数量"),
    ("NOTICE_DATE", "公告日"),
    ("SECURITY_CODE", "代码"),
    ("HOLDER_NAME", "股东名称"),
    ("AFTER_CHANGE_RATE", "持股变动信息-占总股本比例"),
    ("END_DATE", "变动截止日"),
    ("AFTER_HOLDER_NUM", "变动后持股情况-持股总数"),
    ("HOLD_RATIO", "变动后持股情况-占总股本比例"),
    ("FREE_SHARES_RATIO", "变动后持股情况-占流通股比例"),
    ("FREE_SHARES", "变动后持股情况-持流通股数"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("DIRECTION", "持股变动信息-增减"),
    ("CHANGE_FREE_RATIO", "持股变动信息-占流通股比例"),
    ("START_DATE", "变动开始日"),
    ("NEWEST_PRICE", "最新价"),
    ("CHANGE_RATE_QUOTES", "涨跌幅"),
];
const GGCG_SELECT: [&str; 16] = [
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "股东名称",
    "持股变动信息-增减",
    "持股变动信息-变动数量",
    "持股变动信息-占总股本比例",
    "持股变动信息-占流通股比例",
    "变动后持股情况-持股总数",
    "变动后持股情况-占总股本比例",
    "变动后持股情况-持流通股数",
    "变动后持股情况-占流通股比例",
    "变动开始日",
    "变动截止日",
    "公告日",
];
const GGCG_NUMERIC: [&str; 9] = [
    "最新价",
    "涨跌幅",
    "持股变动信息-变动数量",
    "持股变动信息-占总股本比例",
    "持股变动信息-占流通股比例",
    "变动后持股情况-持股总数",
    "变动后持股情况-占总股本比例",
    "变动后持股情况-持流通股数",
    "变动后持股情况-占流通股比例",
];
const GGCG_DATE: [&str; 3] = ["变动开始日", "变动截止日", "公告日"];

/// 东财 datacenter-web 固定 token（akshare 源码硬编码常量）。
const EM_TOKEN: &str = "894050c76af8597a853f5b408b759f5d";

/// 构造 push2 clist 公共参数（pn/pz/po/np/fltt/invt 固定）。
fn clist_params(fs: &str, fid: &str, fields: &str) -> Map<String, Value> {
    let params = json!({
        "pn": "1", "pz": "100", "po": "1", "np": "1",
        "ut": "bd1d9ddb04089700cf9c27f6f7426281",
        "fltt": "2", "invt": "2", "fid": fid,
        "fs": fs, "fields": fields,
    });
    params.as_object().cloned().unwrap_or_default()
}

/// 构造 datacenter 报表公共 `extra`（sort 必填；filter/quoteColumns/token/quoteType 可选）。
fn report_extra(
    sort_columns: &str,
    sort_types: &str,
    filter: Option<&str>,
    quote_columns: Option<&str>,
    token: Option<&str>,
    quote_type: Option<&str>,
) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("sortColumns".into(), json!(sort_columns));
    m.insert("sortTypes".into(), json!(sort_types));
    if let Some(f) = filter {
        m.insert("filter".into(), json!(f));
    }
    if let Some(q) = quote_columns {
        m.insert("quoteColumns".into(), json!(q));
    }
    if let Some(t) = token {
        m.insert("token".into(), json!(t));
    }
    if let Some(qt) = quote_type {
        m.insert("quoteType".into(), json!(qt));
    }
    m
}

/// 拉取 datacenter 报表全部分页。
fn datacenter(
    report: &str,
    columns: &str,
    extra: &Map<String, Value>,
    page_size: &str,
) -> Result<Vec<Value>> {
    let http = HttpClient::default();
    fetch_datacenter_pages(&http, report, columns, extra, page_size)
}

/// 把 `YYYYMMDD` 转换为 `YYYY-MM-DD`；非法格式报错。
fn fmt_ymd(d: &str) -> Result<String> {
    if d.len() != 8 || !d.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AkshareError::Param(format!(
            "无效日期: {d}（应为 YYYYMMDD）"
        )));
    }
    Ok(format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]))
}

/// 创业板实时行情（对应 akshare [`akshare.stock_cy_a_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率, 总市值, 流通市值, 涨速,
/// 5分钟涨跌, 60日涨跌幅, 年初至今涨跌幅`
pub fn stock_cy_a_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(
        &http,
        &push2_urls("/api/qt/clist/get"),
        &clist_params("m:0 t:80", "f12", SPOT_FIELDS),
    )?;
    finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC)
}

/// 科创板实时行情（对应 akshare [`akshare.stock_kc_a_spot_em`]）。
///
/// # 返回列
/// 与 [`stock_cy_a_spot_em`] 一致。
pub fn stock_kc_a_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(
        &http,
        &push2_urls("/api/qt/clist/get"),
        &clist_params("m:1 t:23", "f12", SPOT_FIELDS),
    )?;
    finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC)
}

/// B 股实时行情（对应 akshare [`akshare.stock_zh_b_spot_em`]）。
///
/// # 返回列
/// 与 [`stock_cy_a_spot_em`] 一致。
pub fn stock_zh_b_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(
        &http,
        &push2_urls("/api/qt/clist/get"),
        &clist_params("m:0 t:7,m:1 t:3", "f12", SPOT_FIELDS),
    )?;
    finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC)
}

/// 新股实时行情（对应 akshare [`akshare.stock_new_a_spot_em`]）。
///
/// 在标准 A 股快照基础上增加「上市日期」列。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 涨跌额, 成交量, 成交额, 振幅, 最高, 最低,
/// 今开, 昨收, 量比, 换手率, 市盈率-动态, 市净率, 上市日期, 总市值, 流通市值, 涨速,
/// 5分钟涨跌, 60日涨跌幅, 年初至今涨跌幅`
pub fn stock_new_a_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(
        &http,
        &push2_urls("/api/qt/clist/get"),
        &clist_params("m:0 f:8,m:1 f:8", "f26", SPOT_FIELDS),
    )?;
    finalize_spot(df, &NEW_A_RENAME, &NEW_A_SELECT, &NEW_A_NUMERIC)
}

/// 港股主板实时行情（对应 akshare [`akshare.stock_hk_main_board_spot_em`]）。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌额, 涨跌幅, 今开, 最高, 最低, 昨收, 成交量, 成交额`
pub fn stock_hk_main_board_spot_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(
        &http,
        &push2_urls("/api/qt/clist/get"),
        &clist_params("m:128 t:3", "f12", HK_SPOT_FIELDS),
    )?;
    finalize_spot(df, &HK_RENAME, &HK_SELECT, &HK_NUMERIC)
}

/// 港股通成份股（对应 akshare [`akshare.stock_hk_ggt_components_em`]）。
///
/// # 返回列
/// 与 [`stock_hk_main_board_spot_em`] 一致。
pub fn stock_hk_ggt_components_em() -> Result<Df> {
    let http = HttpClient::default();
    let df = fetch_clist(
        &http,
        &push2_urls("/api/qt/clist/get"),
        &clist_params("b:DLMK0146,b:DLMK0144", "f12", HK_SPOT_FIELDS),
    )?;
    finalize_spot(df, &HK_RENAME, &HK_SELECT, &HK_NUMERIC)
}

/// 股东户数（对应 akshare [`akshare.stock_zh_a_gdhs`]）。
///
/// `symbol`：`"最新"` 或季度末日期 `YYYYMMDD`（如 `"20230930"`）。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌幅, 股东户数-本次, 股东户数-上次, 股东户数-增减,
/// 股东户数-增减比例, 区间涨跌幅, 股东户数统计截止日-本次, 股东户数统计截止日-上次,
/// 户均持股市值, 户均持股数量, 总市值, 总股本, 公告日期`
pub fn stock_zh_a_gdhs(symbol: &str) -> Result<Df> {
    let (report_name, filter) = if symbol == "最新" {
        ("RPT_HOLDERNUMLATEST", None)
    } else {
        if symbol.len() != 8 || !symbol.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {symbol}（应为 '最新' 或 YYYYMMDD）"
            )));
        }
        let d = format!("{}-{}-{}", &symbol[..4], &symbol[4..6], &symbol[6..]);
        ("RPT_HOLDERNUM_DET", Some(format!("(END_DATE='{d}')")))
    };
    // 注意：akshare 原版 `columns` 含重复的 `END_DATE`（服务端归一化为 `PRE_END_DATE`），
    // 必须原样发送；`f2,f3` 仅出现在 `quoteColumns`（见 `extra`），不可并入 `columns`，
    // 否则 datacenter 报 `f2返回字段不存在`（code 9501）。
    let columns = "SECURITY_CODE,SECURITY_NAME_ABBR,END_DATE,INTERVAL_CHRATE,AVG_MARKET_CAP,AVG_HOLD_NUM,TOTAL_MARKET_CAP,TOTAL_A_SHARES,HOLD_NOTICE_DATE,HOLDER_NUM,PRE_HOLDER_NUM,HOLDER_NUM_CHANGE,HOLDER_NUM_RATIO,END_DATE,PRE_END_DATE";
    let mut extra = Map::new();
    extra.insert(
        "sortColumns".into(),
        json!("HOLD_NOTICE_DATE,SECURITY_CODE"),
    );
    extra.insert("sortTypes".into(), json!("-1,-1"));
    if let Some(f) = filter {
        extra.insert("filter".into(), Value::String(f));
    }
    extra.insert("quoteColumns".into(), json!("f2,f3"));
    let http = HttpClient::default();
    let rows = fetch_datacenter_pages(&http, report_name, columns, &extra, "500")?;
    let mut df = finalize_report(&rows, &GDHS_RENAME, &GDHS_SELECT, &GDHS_NUMERIC, None)?;
    df.cast_date(&GDHS_DATE)?;
    Ok(df)
}

/// 融资融券账户信息（对应 akshare [`akshare.stock_margin_account_info`]）。
///
/// # 返回列
/// `日期, 融资余额, 融券余额, 融资买入额, 融券卖出额, 证券公司数量, 营业部数量,
/// 个人投资者数量, 机构投资者数量, 参与交易的投资者数量, 有融资融券负债的投资者数量,
/// 担保物总价值, 平均维持担保比例`
pub fn stock_margin_account_info() -> Result<Df> {
    let extra = report_extra("STATISTICS_DATE", "-1", None, None, None, None);
    let rows = datacenter("RPTA_WEB_MARGIN_DAILYTRADE", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &MARGIN_RENAME, &MARGIN_SELECT, &MARGIN_NUMERIC, None)?;
    df.cast_date(&MARGIN_DATE)?;
    Ok(df)
}

/// 股东自由流通持股明细（对应 akshare [`akshare.stock_gdfx_free_holding_detail_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（如 `"20210930"`）。
///
/// # 返回列
/// `序号, 股东名称, 股东类型, 股票代码, 股票简称, 报告期, 期末持股-数量,
/// 期末持股-数量变化, 期末持股-数量变化比例, 期末持股-持股变动, 期末持股-流通市值, 公告日`
pub fn stock_gdfx_free_holding_detail_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(END_DATE='{d}')");
    let extra = report_extra(
        "UPDATE_DATE,SECURITY_CODE,HOLDER_RANK",
        "-1,1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_F10_EH_FREEHOLDERS", "ALL", &extra, "2000")?;
    let mut df = finalize_report(
        &rows,
        &FREE_HOLD_RENAME,
        &FREE_HOLD_SELECT,
        &FREE_HOLD_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&FREE_HOLD_DATE)?;
    Ok(df)
}

/// 股东持股明细（对应 akshare [`akshare.stock_gdfx_holding_detail_em`]）。
///
/// `date`：报告期 `YYYYMMDD`；`indicator`：股东类型（如 `"个人"`）；`symbol`：持股变动（如 `"新进"`）。
///
/// # 返回列
/// `序号, 股东名称, 股东类型, 股票代码, 股票简称, 报告期, 期末持股-数量,
/// 期末持股-数量变化, 期末持股-数量变化比例, 期末持股-持股变动, 期末持股-流通市值, 公告日, 股东排名`
pub fn stock_gdfx_holding_detail_em(date: &str, indicator: &str, symbol: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(
        "(HOLDER_NEWTYPE=\"{indicator}\")(HOLDNUM_CHANGE_NAME=\"{symbol}\")(END_DATE='{d}')"
    );
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE,RANK",
        "-1,1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_DMSK_HOLDERS", "ALL", &extra, "50")?;
    let mut df = finalize_report(
        &rows,
        &HOLD_DETAIL_RENAME,
        &HOLD_DETAIL_SELECT,
        &HOLD_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&HOLD_DETAIL_DATE)?;
    Ok(df)
}

/// 股东自由流通持股分析（对应 akshare [`akshare.stock_gdfx_free_holding_analyse_em`]）。
///
/// `date`：报告期 `YYYYMMDD`。
///
/// # 返回列
/// `序号, 股东名称, 股东类型, 股票代码, 股票简称, 报告期, 期末持股-数量,
/// 期末持股-数量变化, 期末持股-数量变化比例, 期末持股-持股变动, 期末持股-流通市值, 公告日,
/// 公告日后涨跌幅-10个交易日, 公告日后涨跌幅-30个交易日, 公告日后涨跌幅-60个交易日`
pub fn stock_gdfx_free_holding_analyse_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(END_DATE='{d}')");
    let extra = report_extra(
        "UPDATE_DATE,SECURITY_CODE,HOLDER_RANK",
        "-1,1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter(
        "RPT_CUSTOM_F10_EH_FREEHOLDERS_JOIN_FREEHOLDER_SHAREANALYSIS",
        "ALL;D10_ADJCHRATE,D30_ADJCHRATE,D60_ADJCHRATE",
        &extra,
        "500",
    )?;
    let mut df = finalize_report(
        &rows,
        &FREE_ANALYSE_RENAME,
        &FREE_ANALYSE_SELECT,
        &FREE_ANALYSE_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&FREE_ANALYSE_DATE)?;
    Ok(df)
}

/// 股东持股分析（对应 akshare [`akshare.stock_gdfx_holding_analyse_em`]）。
///
/// `date`：报告期 `YYYYMMDD`。
///
/// # 返回列
/// 与 [`stock_gdfx_free_holding_analyse_em`] 一致。
pub fn stock_gdfx_holding_analyse_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(END_DATE='{d}')");
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE,RANK",
        "-1,1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter(
        "RPT_CUSTOM_DMSK_HOLDERS_JOIN_HOLDER_SHAREANALYSIS",
        "ALL;D10_ADJCHRATE,D30_ADJCHRATE,D60_ADJCHRATE",
        &extra,
        "500",
    )?;
    let mut df = finalize_report(
        &rows,
        &HOLD_ANALYSE_RENAME,
        &HOLD_ANALYSE_SELECT,
        &HOLD_ANALYSE_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&HOLD_ANALYSE_DATE)?;
    Ok(df)
}

/// 券商业绩月度数据（对应 akshare [`akshare.stock_qsjy_em`]）。
///
/// `date`：统计月份 `YYYYMMDD`（如 `"20200731"`）。
///
/// # 返回列
/// `简称, 代码, 当月净利润-净利润, 当月净利润-同比增长, 当月净利润-环比增长,
/// 当年累计净利润-累计净利润, 当年累计净利润-同比增长, 当月营业收入-营业收入,
/// 当月营业收入-环比增长, 当月营业收入-同比增长, 当年累计营业收入-累计营业收入,
/// 当年累计营业收入-同比增长, 净资产-净资产, 净资产-同比增长`
pub fn stock_qsjy_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(END_DATE='{d}')");
    let extra = report_extra("END_DATE", "-1", Some(&filter), None, None, None);
    let columns = "SECURITY_CODE,SECURITY_NAME_ABBR,END_DATE,NETPROFIT,NP_YOY,NP_QOQ,ACCUMPROFIT,ACCUMPROFIT_YOY,OPERATE_INCOME,OI_YOY,OI_QOQ,ACCUMOI,ACCUMOI_YOY,NET_ASSETS,NA_YOY";
    let rows = datacenter("RPT_PERFORMANCE", columns, &extra, "500")?;
    let df = finalize_report(&rows, &QSJY_RENAME, &QSJY_SELECT, &QSJY_NUMERIC, None)?;
    Ok(df)
}

/// 股权质押市场总览（对应 akshare [`akshare.stock_gpzy_profile_em`]）。
///
/// 注：`A股质押总比例` = 服务端 `PM_RATIO` / 100（与 akshare 一致）。
///
/// # 返回列
/// `交易日期, A股质押总比例, 质押公司数量, 质押笔数, 质押总股数, 质押总市值, 沪深300指数, 涨跌幅`
pub fn stock_gpzy_profile_em() -> Result<Df> {
    let extra = report_extra("TRADE_DATE", "-1", None, Some(""), None, None);
    let rows = datacenter("RPT_CSDC_STATISTICS", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &GPZY_PROFILE_RENAME,
        &GPZY_PROFILE_SELECT,
        &GPZY_PROFILE_NUMERIC,
        None,
    )?;
    df.scale("A股质押总比例", 100.0)?;
    df.cast_date(&GPZY_PROFILE_DATE)?;
    Ok(df)
}

/// 个股股权质押比例（对应 akshare [`akshare.stock_gpzy_pledge_ratio_em`]）。
///
/// `date`：交易日期 `YYYYMMDD`（如 `"20240906"`）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 交易日期, 所属行业, 质押比例, 质押股数, 质押市值,
/// 质押笔数, 无限售股质押数, 限售股质押数, 近一年涨跌幅, 所属行业代码`
pub fn stock_gpzy_pledge_ratio_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(TRADE_DATE='{d}')");
    let extra = report_extra("PLEDGE_RATIO", "-1", Some(&filter), Some(""), None, None);
    let rows = datacenter("RPT_CSDC_LIST", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &GPZY_PLEDGE_RENAME,
        &GPZY_PLEDGE_SELECT,
        &GPZY_PLEDGE_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&GPZY_PLEDGE_DATE)?;
    Ok(df)
}

/// 行业股权质押统计（对应 akshare [`akshare.stock_gpzy_industry_data_em`]）。
///
/// # 返回列
/// `序号, 行业, 平均质押比例, 公司家数, 质押总笔数, 质押总股本, 最新质押市值, 统计时间`
pub fn stock_gpzy_industry_data_em() -> Result<Df> {
    let extra = report_extra("AVERAGE_PLEDGE_RATIO", "-1", None, Some(""), None, None);
    let columns = "INDUSTRY_CODE,INDUSTRY,TRADE_DATE,AVERAGE_PLEDGE_RATIO,ORG_NUM,PLEDGE_TOTAL_NUM,TOTAL_PLEDGE_SHARES,PLEDGE_TOTAL_MARKETCAP";
    let rows = datacenter("RPT_CSDC_INDUSTRY_STATISTICS", columns, &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &GPZY_INDUSTRY_RENAME,
        &GPZY_INDUSTRY_SELECT,
        &GPZY_INDUSTRY_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&GPZY_INDUSTRY_DATE)?;
    Ok(df)
}

/// 个股估值分析（对应 akshare [`akshare.stock_value_em`]）。
///
/// `symbol`：股票代码（如 `"300766"`）。
///
/// # 返回列
/// `数据日期, 当日收盘价, 当日涨跌幅, 总市值, 流通市值, 总股本, 流通股本, PE(TTM),
/// PE(静), 市净率, PEG值, 市现率, 市销率`
pub fn stock_value_em(symbol: &str) -> Result<Df> {
    let filter = format!("(SECURITY_CODE=\"{symbol}\")");
    let extra = report_extra("TRADE_DATE", "-1", Some(&filter), Some(""), None, None);
    let rows = datacenter("RPT_VALUEANALYSIS_DET", "ALL", &extra, "5000")?;
    let mut df = finalize_report(&rows, &VALUE_RENAME, &VALUE_SELECT, &VALUE_NUMERIC, None)?;
    df.cast_date(&VALUE_DATE)?;
    Ok(df)
}

/// 股东大会（对应 akshare [`akshare.stock_gddh_em`]）。
///
/// # 返回列
/// `代码, 简称, 股东大会名称, 召开开始日, 股权登记日, 现场登记日, 网络投票时间-开始日,
/// 网络投票时间-结束日, 决议公告日, 公告日, 序列号, 提案`
pub fn stock_gddh_em() -> Result<Df> {
    let filter = "(IS_LASTDATE=\"1\")";
    let extra = report_extra("NOTICE_DATE", "-1", Some(filter), None, None, None);
    let columns = "SECURITY_CODE,SECURITY_NAME_ABBR,MEETING_TITLE,START_ADJUST_DATE,EQUITY_RECORD_DATE,ONSITE_RECORD_DATE,DECISION_NOTICE_DATE,NOTICE_DATE,WEB_START_DATE,WEB_END_DATE,SERIAL_NUM,PROPOSAL";
    let rows = datacenter("RPT_GENERALMEETING_DETAIL", columns, &extra, "500")?;
    let mut df = finalize_report(&rows, &GDDH_RENAME, &GDDH_SELECT, &[], None)?;
    df.cast_date(&GDDH_DATE)?;
    Ok(df)
}

/// 重大合同明细（对应 akshare [`akshare.stock_zdhtmx_em`]）。
///
/// `start_date` / `end_date`：起止日期 `YYYYMMDD`（如 `"20200819"`）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 签署主体, 签署主体-与上市公司关系, 其他签署方,
/// 其他签署方-与上市公司关系, 合同类型, 合同名称, 合同金额, 上年度营业收入,
/// 占上年度营业收入比例, 最新财务报表的营业收入, 签署日期, 公告日期`
pub fn stock_zdhtmx_em(start_date: &str, end_date: &str) -> Result<Df> {
    let sd = fmt_ymd(start_date)?;
    let ed = fmt_ymd(end_date)?;
    let filter = format!("(DIM_RDATE>='{sd}')(DIM_RDATE<='{ed}')");
    let extra = report_extra("DIM_RDATE", "-1", Some(&filter), None, Some(EM_TOKEN), None);
    let rows = datacenter("RPTA_WEB_ZDHT_LIST", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &ZDHT_RENAME,
        &ZDHT_SELECT,
        &ZDHT_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&ZDHT_DATE)?;
    Ok(df)
}

/// 打新收益率（对应 akshare [`akshare.stock_dxsyl_em`]）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 发行价, 最新价, 网上-发行中签率, 网上-有效申购股数,
/// 网上-有效申购户数, 网上-超额认购倍数, 网下-配售中签率, 网下-有效申购股数,
/// 网下-有效申购户数, 网下-配售认购倍数, 总发行数量, 开盘溢价, 首日涨幅, 上市日期`
pub fn stock_dxsyl_em() -> Result<Df> {
    let filter = "((APPLY_DATE>'2010-01-01')(|@APPLY_DATE=\"NULL\"))((LISTING_DATE>'2010-01-01')(|@LISTING_DATE=\"NULL\"))(TRADE_MARKET_CODE!=\"069001017\")";
    let extra = report_extra(
        "LISTING_DATE,SECURITY_CODE",
        "-1,-1",
        Some(filter),
        Some("f2~01~SECURITY_CODE,f14~01~SECURITY_CODE"),
        None,
        Some("0"),
    );
    let rows = datacenter("RPTA_APP_IPOAPPLY", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &DXSYL_RENAME,
        &DXSYL_SELECT,
        &DXSYL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&DXSYL_DATE)?;
    Ok(df)
}

/// 商誉市场统计（对应 akshare [`akshare.stock_sy_profile_em`]）。
///
/// # 返回列
/// `报告期, 商誉, 商誉减值, 净资产, 商誉占净资产比例, 商誉减值占净资产比例,
/// 净利润规模, 商誉减值占净利润比例`
pub fn stock_sy_profile_em() -> Result<Df> {
    let filter = "((GOODWILL_STATE=\"1\")( | IMPAIRMENT_STATE=\"1\"))(TRADE_BOARD=\"all\")";
    let extra = report_extra(
        "REPORT_DATE",
        "-1",
        Some(filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_GOODWILL_MARKETSTATISTICS", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &SY_PROFILE_RENAME,
        &SY_PROFILE_SELECT,
        &SY_PROFILE_NUMERIC,
        None,
    )?;
    df.cast_date(&SY_PROFILE_DATE)?;
    Ok(df)
}

/// 重要股东股权质押明细（对应 akshare [`akshare.stock_gpzy_pledge_ratio_detail_em`]）。
///
/// 拉取全市场重要股东股权质押明细（无日期筛选），`序号` 由 Rust 生成。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 股东名称, 质押股份数量, 占所持股份比例, 占总股本比例,
/// 质押机构, 最新价, 质押日收盘价, 预估平仓线, 质押开始日期, 质押结束日期, 状态, 公告日`
pub fn stock_gpzy_pledge_ratio_detail_em() -> Result<Df> {
    let extra = report_extra("NOTICE_DATE", "-1", None, Some(""), None, None);
    let rows = datacenter("RPTA_APP_ACCUMDETAILS", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &PLEDGE_DETAIL_RENAME,
        &PLEDGE_DETAIL_SELECT,
        &PLEDGE_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&PLEDGE_DETAIL_DATE)?;
    Ok(df)
}

/// 个股重要股东股权质押明细（对应 akshare [`akshare.stock_gpzy_individual_pledge_ratio_detail_em`]）。
///
/// `symbol`：股票代码（如 `"603132"`），按 `SECURITY_CODE` 过滤；`序号` 由 Rust 生成。
///
/// # 返回列
/// 与 [`stock_gpzy_pledge_ratio_detail_em`] 一致。
pub fn stock_gpzy_individual_pledge_ratio_detail_em(symbol: &str) -> Result<Df> {
    let filter = format!("(SECURITY_CODE=\"{symbol}\")");
    let extra = report_extra("NOTICE_DATE", "-1", Some(&filter), Some(""), None, None);
    let rows = datacenter("RPTA_APP_ACCUMDETAILS", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &PLEDGE_DETAIL_RENAME,
        &PLEDGE_DETAIL_SELECT,
        &PLEDGE_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&PLEDGE_DETAIL_DATE)?;
    Ok(df)
}

/// 高管持股变动（对应 akshare [`akshare.stock_ggcg_em`]）。
///
/// `symbol`：选择范围，取值 `全部` / `股东增持` / `股东减持`（其余值报错）。
/// 通过 `quoteColumns` 注入最新价与涨跌幅；akshare 输出无「序号」列。
///
/// # 返回列
/// `代码, 名称, 最新价, 涨跌幅, 股东名称, 持股变动信息-增减, 持股变动信息-变动数量,
/// 持股变动信息-占总股本比例, 持股变动信息-占流通股比例, 变动后持股情况-持股总数,
/// 变动后持股情况-占总股本比例, 变动后持股情况-持流通股数, 变动后持股情况-占流通股比例,
/// 变动开始日, 变动截止日, 公告日`
pub fn stock_ggcg_em(symbol: &str) -> Result<Df> {
    let filter = match symbol {
        "全部" => "",
        "股东增持" => "(DIRECTION=\"增持\")",
        "股东减持" => "(DIRECTION=\"减持\")",
        other => {
            return Err(AkshareError::Param(format!(
                "无效 symbol: {other}（应为 全部/股东增持/股东减持）"
            )))
        }
    };
    let quote = "f2~01~SECURITY_CODE~NEWEST_PRICE,f3~01~SECURITY_CODE~CHANGE_RATE_QUOTES";
    let extra = report_extra(
        "END_DATE,SECURITY_CODE,EITIME",
        "-1,-1,-1",
        Some(filter),
        Some(quote),
        None,
        Some("0"),
    );
    let rows = datacenter("RPT_SHARE_HOLDER_INCREASE", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &GGCG_RENAME, &GGCG_SELECT, &GGCG_NUMERIC, None)?;
    df.cast_date(&GGCG_DATE)?;
    Ok(df)
}

// ===== 机构调研统计（RPT_ORG_SURVEYNEW）=====
// 序号由 Rust 生成（东财原始 JSON 无 index 键，与 akshare reset_index 一致）。
// 列序参照 akshare `big_df.columns` 与实时拉取的 JSON 键序（columns=ALL +
// quoteColumns 追加 CLOSE_PRICE/CHANGE_RATE）逐位对齐。
const JGDY_TJ_RENAME: [(&str, &str); 10] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("NOTICE_DATE", "公告日期"),
    ("RECEIVE_START_DATE", "接待日期"),
    ("RECEIVE_PLACE", "接待地点"),
    ("RECEIVE_WAY_EXPLAIN", "接待方式"),
    ("RECEPTIONIST", "接待人员"),
    ("SUM", "接待机构数量"),
    ("CLOSE_PRICE", "最新价"),
    ("CHANGE_RATE", "涨跌幅"),
];
const JGDY_TJ_SELECT: [&str; 10] = [
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "接待机构数量",
    "接待方式",
    "接待人员",
    "接待地点",
    "接待日期",
    "公告日期",
];
const JGDY_TJ_NUMERIC: [&str; 3] = ["最新价", "涨跌幅", "接待机构数量"];
const JGDY_TJ_DATE: [&str; 2] = ["接待日期", "公告日期"];

/// 机构调研统计（对应 akshare [`akshare.stock_jgdy_tj_em`]）。
///
/// `date`：开始时间 `YYYYMMDD`（如 `"20220101"`），仅返回该日之后有机构调研记录的股票。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 接待机构数量, 接待方式, 接待人员,
/// 接待地点, 接待日期, 公告日期`
pub fn stock_jgdy_tj_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(NUMBERNEW=\"1\")(IS_SOURCE=\"1\")(NOTICE_DATE>'{d}')");
    let extra = report_extra(
        "NOTICE_DATE,SUM,RECEIVE_START_DATE,SECURITY_CODE",
        "-1,-1,-1,1",
        Some(&filter),
        Some("f2~01~SECURITY_CODE~CLOSE_PRICE,f3~01~SECURITY_CODE~CHANGE_RATE"),
        None,
        None,
    );
    let rows = datacenter("RPT_ORG_SURVEYNEW", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &JGDY_TJ_RENAME,
        &JGDY_TJ_SELECT,
        &JGDY_TJ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&JGDY_TJ_DATE)?;
    Ok(df)
}

// ===== 机构调研详细（RPT_ORG_SURVEY）=====
// 序号由 Rust 生成（东财原始 JSON 无 index 键）。columns 为显式字符串，键序即列序；
// quoteColumns 追加 CLOSE_PRICE/CHANGE_RATE（位于显式键之后）。
const JGDY_DETAIL_RENAME: [(&str, &str); 12] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("NOTICE_DATE", "公告日期"),
    ("RECEIVE_START_DATE", "调研日期"),
    ("RECEIVE_OBJECT", "调研机构"),
    ("RECEIVE_PLACE", "接待地点"),
    ("RECEIVE_WAY_EXPLAIN", "接待方式"),
    ("INVESTIGATORS", "调研人员"),
    ("RECEPTIONIST", "接待人员"),
    ("ORG_TYPE", "机构类型"),
    ("CLOSE_PRICE", "最新价"),
    ("CHANGE_RATE", "涨跌幅"),
];
const JGDY_DETAIL_SELECT: [&str; 12] = [
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "调研机构",
    "机构类型",
    "调研人员",
    "接待方式",
    "接待人员",
    "接待地点",
    "调研日期",
    "公告日期",
];
const JGDY_DETAIL_NUMERIC: [&str; 2] = ["最新价", "涨跌幅"];
const JGDY_DETAIL_DATE: [&str; 2] = ["调研日期", "公告日期"];

/// 机构调研详细（对应 akshare [`akshare.stock_jgdy_detail_em`]）。
///
/// `date`：开始时间 `YYYYMMDD`（如 `"20241211"`），仅返回该日之后有调研记录的明细。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 调研机构, 机构类型, 调研人员, 接待方式,
/// 接待人员, 接待地点, 调研日期, 公告日期`
pub fn stock_jgdy_detail_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(IS_SOURCE=\"1\")(RECEIVE_START_DATE>'{d}')");
    let extra = report_extra(
        "NOTICE_DATE,RECEIVE_START_DATE,SECURITY_CODE,NUMBERNEW",
        "-1,-1,1,-1",
        Some(&filter),
        Some("f2~01~SECURITY_CODE~CLOSE_PRICE,f3~01~SECURITY_CODE~CHANGE_RATE"),
        None,
        Some("0"),
    );
    let columns = "SECUCODE,SECURITY_CODE,SECURITY_NAME_ABBR,NOTICE_DATE,RECEIVE_START_DATE,RECEIVE_OBJECT,RECEIVE_PLACE,RECEIVE_WAY_EXPLAIN,INVESTIGATORS,RECEPTIONIST,ORG_TYPE";
    let rows = datacenter("RPT_ORG_SURVEY", columns, &extra, "50")?;
    let mut df = finalize_report(
        &rows,
        &JGDY_DETAIL_RENAME,
        &JGDY_DETAIL_SELECT,
        &JGDY_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&JGDY_DETAIL_DATE)?;
    Ok(df)
}

// ===== 分红送配（RPT_SHAREBONUS_DET）=====
// 无序号列。列序参照 akshare `big_df.columns`（columns=ALL）与实时拉取的 JSON 键序逐位对齐。
const FHPS_RENAME: [(&str, &str); 18] = [
    ("SECURITY_NAME_ABBR", "名称"),
    ("SECURITY_CODE", "代码"),
    ("BONUS_IT_RATIO", "送转股份-送转总比例"),
    ("BONUS_RATIO", "送转股份-送转比例"),
    ("IT_RATIO", "送转股份-转股比例"),
    ("PRETAX_BONUS_RMB", "现金分红-现金分红比例"),
    ("PLAN_NOTICE_DATE", "预案公告日"),
    ("EQUITY_RECORD_DATE", "股权登记日"),
    ("EX_DIVIDEND_DATE", "除权除息日"),
    ("ASSIGN_PROGRESS", "方案进度"),
    ("NOTICE_DATE", "最新公告日期"),
    ("BASIC_EPS", "每股收益"),
    ("BVPS", "每股净资产"),
    ("PER_CAPITAL_RESERVE", "每股公积金"),
    ("PER_UNASSIGN_PROFIT", "每股未分配利润"),
    ("PNP_YOY_RATIO", "净利润同比增长"),
    ("TOTAL_SHARES", "总股本"),
    ("DIVIDENT_RATIO", "现金分红-股息率"),
];
const FHPS_SELECT: [&str; 18] = [
    "代码",
    "名称",
    "送转股份-送转总比例",
    "送转股份-送转比例",
    "送转股份-转股比例",
    "现金分红-现金分红比例",
    "现金分红-股息率",
    "每股收益",
    "每股净资产",
    "每股公积金",
    "每股未分配利润",
    "净利润同比增长",
    "总股本",
    "预案公告日",
    "股权登记日",
    "除权除息日",
    "方案进度",
    "最新公告日期",
];
const FHPS_NUMERIC: [&str; 11] = [
    "送转股份-送转总比例",
    "送转股份-送转比例",
    "送转股份-转股比例",
    "现金分红-现金分红比例",
    "现金分红-股息率",
    "每股收益",
    "每股净资产",
    "每股公积金",
    "每股未分配利润",
    "净利润同比增长",
    "总股本",
];
const FHPS_DATE: [&str; 4] = ["预案公告日", "股权登记日", "除权除息日", "最新公告日期"];

/// 分红送配（对应 akshare [`akshare.stock_fhps_em`]）。
///
/// `date`：分红送配报告期 `YYYYMMDD`（如 `"20231231"`）。
///
/// # 返回列
/// `代码, 名称, 送转股份-送转总比例, 送转股份-送转比例, 送转股份-转股比例,
/// 现金分红-现金分红比例, 现金分红-股息率, 每股收益, 每股净资产, 每股公积金,
/// 每股未分配利润, 净利润同比增长, 总股本, 预案公告日, 股权登记日, 除权除息日, 方案进度, 最新公告日期`
pub fn stock_fhps_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(REPORT_DATE='{d}')");
    let extra = report_extra(
        "PLAN_NOTICE_DATE",
        "-1",
        Some(&filter),
        Some(""),
        None,
        None,
    );
    let rows = datacenter("RPT_SHAREBONUS_DET", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &FHPS_RENAME, &FHPS_SELECT, &FHPS_NUMERIC, None)?;
    df.cast_date(&FHPS_DATE)?;
    Ok(df)
}

// ===== 分红送配详情（RPT_SHAREBONUS_DET，按个股过滤）=====
// 无序号列。与 [`stock_fhps_em`] 同报表但 akshare 使用了不同的中文列名/顺序。
const FHPS_DETAIL_RENAME: [(&str, &str); 19] = [
    ("BONUS_IT_RATIO", "送转股份-送转总比例"),
    ("BONUS_RATIO", "送转股份-送股比例"),
    ("IT_RATIO", "送转股份-转股比例"),
    ("PRETAX_BONUS_RMB", "现金分红-现金分红比例"),
    ("PLAN_NOTICE_DATE", "业绩披露日期"),
    ("EQUITY_RECORD_DATE", "股权登记日"),
    ("EX_DIVIDEND_DATE", "除权除息日"),
    ("REPORT_DATE", "报告期"),
    ("ASSIGN_PROGRESS", "方案进度"),
    ("IMPL_PLAN_PROFILE", "现金分红-现金分红比例描述"),
    ("NOTICE_DATE", "最新公告日期"),
    ("BASIC_EPS", "每股收益"),
    ("BVPS", "每股净资产"),
    ("PER_CAPITAL_RESERVE", "每股公积金"),
    ("PER_UNASSIGN_PROFIT", "每股未分配利润"),
    ("PNP_YOY_RATIO", "净利润同比增长"),
    ("TOTAL_SHARES", "总股本"),
    ("PUBLISH_DATE", "预案公告日"),
    ("DIVIDENT_RATIO", "现金分红-股息率"),
];
const FHPS_DETAIL_SELECT: [&str; 19] = [
    "报告期",
    "业绩披露日期",
    "送转股份-送转总比例",
    "送转股份-送股比例",
    "送转股份-转股比例",
    "现金分红-现金分红比例",
    "现金分红-现金分红比例描述",
    "现金分红-股息率",
    "每股收益",
    "每股净资产",
    "每股公积金",
    "每股未分配利润",
    "净利润同比增长",
    "总股本",
    "预案公告日",
    "股权登记日",
    "除权除息日",
    "方案进度",
    "最新公告日期",
];
const FHPS_DETAIL_NUMERIC: [&str; 11] = [
    "送转股份-送转总比例",
    "送转股份-送股比例",
    "送转股份-转股比例",
    "现金分红-现金分红比例",
    "现金分红-股息率",
    "每股收益",
    "每股净资产",
    "每股公积金",
    "每股未分配利润",
    "净利润同比增长",
    "总股本",
];
const FHPS_DETAIL_DATE: [&str; 6] = [
    "报告期",
    "业绩披露日期",
    "预案公告日",
    "股权登记日",
    "除权除息日",
    "最新公告日期",
];

/// 分红送配详情（对应 akshare [`akshare.stock_fhps_detail_em`]）。
///
/// `symbol`：股票代码（如 `"300073"`），按 `SECURITY_CODE` 过滤。
///
/// # 返回列
/// `报告期, 业绩披露日期, 送转股份-送转总比例, 送转股份-送股比例, 送转股份-转股比例,
/// 现金分红-现金分红比例, 现金分红-现金分红比例描述, 现金分红-股息率, 每股收益,
/// 每股净资产, 每股公积金, 每股未分配利润, 净利润同比增长, 总股本, 预案公告日,
/// 股权登记日, 除权除息日, 方案进度, 最新公告日期`
pub fn stock_fhps_detail_em(symbol: &str) -> Result<Df> {
    let filter = format!("(SECURITY_CODE=\"{symbol}\")");
    let extra = report_extra("REPORT_DATE", "-1", Some(&filter), Some(""), None, None);
    let rows = datacenter("RPT_SHAREBONUS_DET", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &FHPS_DETAIL_RENAME,
        &FHPS_DETAIL_SELECT,
        &FHPS_DETAIL_NUMERIC,
        None,
    )?;
    df.cast_date(&FHPS_DETAIL_DATE)?;
    Ok(df)
}

// ===== 停复牌信息（RPT_CUSTOM_SUSPEND_DATA_INTERFACE）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns` 与实时拉取的 JSON 键序逐位对齐。
const TFP_RENAME: [(&str, &str); 8] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("SUSPEND_START_TIME", "停牌时间"),
    ("SUSPEND_END_TIME", "停牌截止时间"),
    ("SUSPEND_EXPIRE", "停牌期限"),
    ("SUSPEND_REASON", "停牌原因"),
    ("TRADE_MARKET", "所属市场"),
    ("PREDICT_RESUME_DATE", "预计复牌时间"),
];
const TFP_SELECT: [&str; 8] = [
    "代码",
    "名称",
    "停牌时间",
    "停牌截止时间",
    "停牌期限",
    "停牌原因",
    "所属市场",
    "预计复牌时间",
];
const TFP_NUMERIC: [&str; 0] = [];
const TFP_DATE: [&str; 3] = ["停牌时间", "停牌截止时间", "预计复牌时间"];

/// 停复牌信息（对应 akshare [`akshare.stock_tfp_em`]）。
///
/// `date`：查询日期 `YYYYMMDD`（如 `"20240426"`），返回该日全市场停复牌记录。
///
/// # 返回列
/// `序号, 代码, 名称, 停牌时间, 停牌截止时间, 停牌期限, 停牌原因, 所属市场, 预计复牌时间`
pub fn stock_tfp_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(MARKET=\"全部\")(DATETIME='{d}')");
    let extra = report_extra("SUSPEND_START_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_CUSTOM_SUSPEND_DATA_INTERFACE", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &TFP_RENAME, &TFP_SELECT, &TFP_NUMERIC, Some("序号"))?;
    df.cast_date(&TFP_DATE)?;
    Ok(df)
}

// ===== 全部增发（RPT_SEO_DETAIL）=====
// 无序号列。列名直接来自 akshare `big_df.rename(columns={...})`；quoteColumns 注入最新价。
const QBZF_RENAME: [(&str, &str); 11] = [
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("SECURITY_CODE", "股票代码"),
    ("CORRECODE", "增发代码"),
    ("SEO_TYPE", "发行方式"),
    ("ISSUE_NUM", "发行总数"),
    ("ONLINE_ISSUE_NUM", "网上发行"),
    ("ISSUE_PRICE", "发行价格"),
    ("NEW_PRICE", "最新价"),
    ("ISSUE_DATE", "发行日期"),
    ("ISSUE_LISTING_DATE", "增发上市日期"),
    ("LOCKIN_PERIOD", "锁定期"),
];
const QBZF_SELECT: [&str; 11] = [
    "股票代码",
    "股票简称",
    "增发代码",
    "发行方式",
    "发行总数",
    "网上发行",
    "发行价格",
    "最新价",
    "发行日期",
    "增发上市日期",
    "锁定期",
];
const QBZF_NUMERIC: [&str; 3] = ["发行总数", "发行价格", "最新价"];
const QBZF_DATE: [&str; 2] = ["发行日期", "增发上市日期"];

/// 全部增发（对应 akshare [`akshare.stock_qbzf_em`]）。
///
/// 拉取全市场增发记录（无参数）；`发行方式` 由东财原始 `SEO_TYPE`（1=定向增发/2=公开增发）映射。
///
/// # 返回列
/// `股票代码, 股票简称, 增发代码, 发行方式, 发行总数, 网上发行, 发行价格,
/// 最新价, 发行日期, 增发上市日期, 锁定期`
pub fn stock_qbzf_em() -> Result<Df> {
    let extra = report_extra(
        "ISSUE_DATE",
        "-1",
        None,
        Some("f2~01~SECURITY_CODE~NEW_PRICE"),
        None,
        Some("0"),
    );
    let rows = datacenter("RPT_SEO_DETAIL", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &QBZF_RENAME, &QBZF_SELECT, &QBZF_NUMERIC, None)?;
    df.cast_date(&QBZF_DATE)?;
    Ok(df)
}

// ===== 配股（RPT_IPO_ALLOTMENT）=====
// 无序号列。列序参照 akshare `big_df.columns`（columns=ALL）与实时拉取的 JSON 键序逐位对齐；
// quoteColumns 注入最新价（位于键序末尾）。
const PG_RENAME: [(&str, &str); 13] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("CORRECODE", "配售代码"),
    ("PLACING_RATIO", "配股比例"),
    ("ISSUE_PRICE", "配股价"),
    ("TOTAL_SHARES_BEFORE", "配股前总股本"),
    ("ISSUE_NUM", "配股数量"),
    ("TOTAL_SHARES_AFTER", "配股后总股本"),
    ("EQUITY_RECORD_DATE", "股权登记日"),
    ("PAY_START_DATE", "缴款起始日期"),
    ("PAY_END_DATE", "缴款截止日期"),
    ("LISTING_DATE", "上市日"),
    ("NEW_PRICE", "最新价"),
];
const PG_SELECT: [&str; 13] = [
    "股票代码",
    "股票简称",
    "配售代码",
    "配股数量",
    "配股比例",
    "配股价",
    "最新价",
    "配股前总股本",
    "配股后总股本",
    "股权登记日",
    "缴款起始日期",
    "缴款截止日期",
    "上市日",
];
const PG_NUMERIC: [&str; 5] = [
    "配股数量",
    "配股价",
    "最新价",
    "配股前总股本",
    "配股后总股本",
];
const PG_DATE: [&str; 4] = ["股权登记日", "缴款起始日期", "缴款截止日期", "上市日"];

/// 配股（对应 akshare [`akshare.stock_pg_em`]）。
///
/// 拉取全市场配股记录（无参数）。akshare 将 `配股比例` 前缀为 `"10配"` 文本，本实现保留原始数值。
///
/// # 返回列
/// `股票代码, 股票简称, 配售代码, 配股数量, 配股比例, 配股价, 最新价, 配股前总股本,
/// 配股后总股本, 股权登记日, 缴款起始日期, 缴款截止日期, 上市日`
pub fn stock_pg_em() -> Result<Df> {
    let extra = report_extra(
        "EQUITY_RECORD_DATE",
        "-1",
        None,
        Some("f2~01~SECURITY_CODE~NEW_PRICE"),
        None,
        Some("0"),
    );
    let rows = datacenter("RPT_IPO_ALLOTMENT", "ALL", &extra, "50000")?;
    let mut df = finalize_report(&rows, &PG_RENAME, &PG_SELECT, &PG_NUMERIC, None)?;
    df.cast_date(&PG_DATE)?;
    Ok(df)
}

// ===== 股票账户统计（RPT_STOCK_OPEN_DATA）=====
// 无序号列。列序参照 akshare `big_df.columns`（columns=ALL）与实时拉取的 JSON 键序逐位对齐。
const ACCOUNT_RENAME: [(&str, &str); 11] = [
    ("STATISTICS_DATE", "数据日期"),
    ("ADD_INVESTOR", "新增投资者-数量"),
    ("ADD_INVESTOR_QOQ", "新增投资者-环比"),
    ("ADD_INVESTOR_YOY", "新增投资者-同比"),
    ("END_INVESTOR", "期末投资者-总量"),
    ("END_INVESTOR_A", "期末投资者-A股账户"),
    ("END_INVESTOR_B", "期末投资者-B股账户"),
    ("CLOSE_PRICE", "上证指数-收盘"),
    ("CHANGE_RATE", "上证指数-涨跌幅"),
    ("TOTAL_MARKET_CAP", "沪深总市值"),
    ("AVERAGE_MARKET_CAP", "沪深户均市值"),
];
const ACCOUNT_SELECT: [&str; 11] = [
    "数据日期",
    "新增投资者-数量",
    "新增投资者-环比",
    "新增投资者-同比",
    "期末投资者-总量",
    "期末投资者-A股账户",
    "期末投资者-B股账户",
    "沪深总市值",
    "沪深户均市值",
    "上证指数-收盘",
    "上证指数-涨跌幅",
];
const ACCOUNT_NUMERIC: [&str; 10] = [
    "新增投资者-数量",
    "新增投资者-环比",
    "新增投资者-同比",
    "期末投资者-总量",
    "期末投资者-A股账户",
    "期末投资者-B股账户",
    "沪深总市值",
    "沪深户均市值",
    "上证指数-收盘",
    "上证指数-涨跌幅",
];
const ACCOUNT_DATE: [&str; 1] = ["数据日期"];

/// 股票账户统计（对应 akshare [`akshare.stock_account_statistics_em`]）。
///
/// 拉取股票账户统计数据（无参数），按数据日期倒序。
///
/// # 返回列
/// `数据日期, 新增投资者-数量, 新增投资者-环比, 新增投资者-同比, 期末投资者-总量,
/// 期末投资者-A股账户, 期末投资者-B股账户, 沪深总市值, 沪深户均市值,
/// 上证指数-收盘, 上证指数-涨跌幅`
pub fn stock_account_statistics_em() -> Result<Df> {
    let extra = report_extra("STATISTICS_DATE", "-1", None, None, None, None);
    let rows = datacenter("RPT_STOCK_OPEN_DATA", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &ACCOUNT_RENAME,
        &ACCOUNT_SELECT,
        &ACCOUNT_NUMERIC,
        None,
    )?;
    df.cast_date(&ACCOUNT_DATE)?;
    Ok(df)
}

// ===== 业绩报表（RPT_LICO_FN_CPD）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns`（columns=ALL，38 个位置，序号占 0 位，
// 原始 JSON 键紧随其后）与实时拉取的 JSON 键序逐位对齐：JSON 键 0-based 序号 k 对应位置 k+1。
// 仅保留 select 中的列（akshare 已丢弃 "_" 列）。
const YJBB_RENAME: [(&str, &str); 15] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("UPDATE_DATE", "最新公告日期"),
    ("BASIC_EPS", "每股收益"),
    ("TOTAL_OPERATE_INCOME", "营业总收入-营业总收入"),
    ("PARENT_NETPROFIT", "净利润-净利润"),
    ("WEIGHTAVG_ROE", "净资产收益率"),
    ("YSTZ", "营业总收入-同比增长"),
    ("SJLTZ", "净利润-同比增长"),
    ("BPS", "每股净资产"),
    ("MGJYXJJE", "每股经营现金流量"),
    ("XSMLL", "销售毛利率"),
    ("YSHZ", "营业总收入-季度环比增长"),
    ("SJLHZ", "净利润-季度环比增长"),
    ("PUBLISHNAME", "所处行业"),
];
const YJBB_SELECT: [&str; 15] = [
    "股票代码",
    "股票简称",
    "每股收益",
    "营业总收入-营业总收入",
    "营业总收入-同比增长",
    "营业总收入-季度环比增长",
    "净利润-净利润",
    "净利润-同比增长",
    "净利润-季度环比增长",
    "每股净资产",
    "净资产收益率",
    "每股经营现金流量",
    "销售毛利率",
    "所处行业",
    "最新公告日期",
];
const YJBB_NUMERIC: [&str; 11] = [
    "每股收益",
    "营业总收入-营业总收入",
    "营业总收入-同比增长",
    "营业总收入-季度环比增长",
    "净利润-净利润",
    "净利润-同比增长",
    "净利润-季度环比增长",
    "每股净资产",
    "净资产收益率",
    "每股经营现金流量",
    "销售毛利率",
];
const YJBB_DATE: [&str; 1] = ["最新公告日期"];

/// 业绩报表（对应 akshare [`akshare.stock_yjbb_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（如 `"20200331"`、`"20200630"`、`"20200930"`、`"20201231"`，从 20100331 开始）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 每股收益, 营业总收入-营业总收入, 营业总收入-同比增长,
/// 营业总收入-季度环比增长, 净利润-净利润, 净利润-同比增长, 净利润-季度环比增长,
/// 每股净资产, 净资产收益率, 每股经营现金流量, 销售毛利率, 所处行业, 最新公告日期`
pub fn stock_yjbb_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(REPORTDATE='{d}')");
    let extra = report_extra(
        "UPDATE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_LICO_FN_CPD", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &YJBB_RENAME,
        &YJBB_SELECT,
        &YJBB_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&YJBB_DATE)?;
    Ok(df)
}

// ===== 业绩快报（RPT_FCI_PERFORMANCEE）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns`（columns=ALL，29 个位置，序号占 0 位，
// 原始 JSON 键紧随其后）与实时拉取的 JSON 键序逐位对齐：JSON 键 0-based 序号 k 对应位置 k+1。
const YJKB_RENAME: [(&str, &str); 15] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("UPDATE_DATE", "公告日期"),
    ("BASIC_EPS", "每股收益"),
    ("TOTAL_OPERATE_INCOME", "营业收入-营业收入"),
    ("TOTAL_OPERATE_INCOME_SQ", "营业收入-去年同期"),
    ("PARENT_NETPROFIT", "净利润-净利润"),
    ("PARENT_NETPROFIT_SQ", "净利润-去年同期"),
    ("PARENT_BVPS", "每股净资产"),
    ("WEIGHTAVG_ROE", "净资产收益率"),
    ("YSTZ", "营业收入-同比增长"),
    ("JLRTBZCL", "净利润-同比增长"),
    ("DJDYSHZ", "营业收入-季度环比增长"),
    ("DJDJLHZ", "净利润-季度环比增长"),
    ("PUBLISHNAME", "所处行业"),
];
const YJKB_SELECT: [&str; 15] = [
    "股票代码",
    "股票简称",
    "每股收益",
    "营业收入-营业收入",
    "营业收入-去年同期",
    "营业收入-同比增长",
    "营业收入-季度环比增长",
    "净利润-净利润",
    "净利润-去年同期",
    "净利润-同比增长",
    "净利润-季度环比增长",
    "每股净资产",
    "净资产收益率",
    "所处行业",
    "公告日期",
];
const YJKB_NUMERIC: [&str; 11] = [
    "每股收益",
    "营业收入-营业收入",
    "营业收入-去年同期",
    "营业收入-同比增长",
    "营业收入-季度环比增长",
    "净利润-净利润",
    "净利润-去年同期",
    "净利润-同比增长",
    "净利润-季度环比增长",
    "每股净资产",
    "净资产收益率",
];
const YJKB_DATE: [&str; 1] = ["公告日期"];

/// 业绩快报（对应 akshare [`akshare.stock_yjkb_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（如 `"20200331"`、`"20200630"`、`"20200930"`、`"20201231"`，从 20100331 开始）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 每股收益, 营业收入-营业收入, 营业收入-去年同期, 营业收入-同比增长,
/// 营业收入-季度环比增长, 净利润-净利润, 净利润-去年同期, 净利润-同比增长, 净利润-季度环比增长,
/// 每股净资产, 净资产收益率, 所处行业, 公告日期`
pub fn stock_yjkb_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(
        "(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE!=\"069001017\")(REPORT_DATE='{d}')"
    );
    let extra = report_extra(
        "UPDATE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_FCI_PERFORMANCEE", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &YJKB_RENAME,
        &YJKB_SELECT,
        &YJKB_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&YJKB_DATE)?;
    Ok(df)
}

// ===== 业绩预告（RPT_PUBLIC_OP_NEWPREDICT）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns`（columns=ALL，28 个位置，序号占 0 位，
// 原始 JSON 键紧随其后）与实时拉取的 JSON 键序逐位对齐：JSON 键 0-based 序号 k 对应位置 k+1。
const YJYG_RENAME: [(&str, &str); 10] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("NOTICE_DATE", "公告日期"),
    ("PREDICT_FINANCE", "预测指标"),
    ("PREDICT_CONTENT", "业绩变动"),
    ("CHANGE_REASON_EXPLAIN", "业绩变动原因"),
    ("PREDICT_TYPE", "预告类型"),
    ("PREYEAR_SAME_PERIOD", "上年同期值"),
    ("INCREASE_JZ", "业绩变动幅度"),
    ("FORECAST_JZ", "预测数值"),
];
const YJYG_SELECT: [&str; 10] = [
    "股票代码",
    "股票简称",
    "预测指标",
    "业绩变动",
    "预测数值",
    "业绩变动幅度",
    "业绩变动原因",
    "预告类型",
    "上年同期值",
    "公告日期",
];
const YJYG_NUMERIC: [&str; 3] = ["业绩变动幅度", "预测数值", "上年同期值"];
const YJYG_DATE: [&str; 1] = ["公告日期"];

/// 业绩预告（对应 akshare [`akshare.stock_yjyg_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（如 `"20200331"`、`"20200630"`、`"20200930"`、`"20201231"`，从 20081231 开始）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 预测指标, 业绩变动, 预测数值, 业绩变动幅度,
/// 业绩变动原因, 预告类型, 上年同期值, 公告日期`
pub fn stock_yjyg_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(REPORT_DATE='{d}')");
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_PUBLIC_OP_NEWPREDICT", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &YJYG_RENAME,
        &YJYG_SELECT,
        &YJYG_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&YJYG_DATE)?;
    Ok(df)
}

// ===== 预约披露时间（RPT_PUBLIC_BS_APPOIN）=====
// 序号由 Rust 生成。列名直接来自 akshare `big_df.rename(columns={...})`（键→中文显式映射）。
const YYSJ_RENAME: [(&str, &str); 7] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("FIRST_APPOINT_DATE", "首次预约时间"),
    ("FIRST_CHANGE_DATE", "一次变更日期"),
    ("SECOND_CHANGE_DATE", "二次变更日期"),
    ("THIRD_CHANGE_DATE", "三次变更日期"),
    ("ACTUAL_PUBLISH_DATE", "实际披露时间"),
];
const YYSJ_SELECT: [&str; 7] = [
    "股票代码",
    "股票简称",
    "首次预约时间",
    "一次变更日期",
    "二次变更日期",
    "三次变更日期",
    "实际披露时间",
];
const YYSJ_NUMERIC: [&str; 0] = [];
const YYSJ_DATE: [&str; 5] = [
    "首次预约时间",
    "一次变更日期",
    "二次变更日期",
    "三次变更日期",
    "实际披露时间",
];

/// 预约披露时间（对应 akshare [`akshare.stock_yysj_em`]）。
///
/// `symbol`：市场分类（默认 `"沪深A股"`；亦可取 `"沪市A股"`、`"科创板"`、`"深市A股"`、`"创业板"`、`"京市A股"`、`"ST板"`）。
/// `date`：报告期 `YYYYMMDD`（如 `"20190331"`、`"20190630"`、`"20190930"`、`"20191231"`，从 20081231 开始）。
///
/// 本实现对应 akshare 默认的 `"沪深A股"` 分支（其余分支过滤条件不同，列契约一致）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 首次预约时间, 一次变更日期, 二次变更日期, 三次变更日期, 实际披露时间`
pub fn stock_yysj_em(symbol: &str, date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    // 默认分支（沪深A股）：在全市场 A 股（剔除北交所 069001017）范围内按报告期过滤。
    let filter = format!(
        "(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE!=\"069001017\")(REPORT_DATE='{d}')"
    );
    let _ = symbol;
    let extra = report_extra(
        "FIRST_APPOINT_DATE,SECURITY_CODE",
        "1,1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_PUBLIC_BS_APPOIN", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &YYSJ_RENAME,
        &YYSJ_SELECT,
        &YYSJ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&YYSJ_DATE)?;
    Ok(df)
}

// ===== 千股千评（RPT_DMSK_TS_STOCKNEW）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns`（columns=ALL，序号占 0 位，
// 原始 JSON 键紧随其后）与实时拉取的 JSON 键序逐位对齐：JSON 键 0-based 序号 k 对应位置 k+1。
// 含 quoteColumns 注入的最新价/换手率/涨跌幅/动态 PE。
const COMMENT_RENAME: [(&str, &str); 13] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("TRADE_DATE", "交易日"),
    ("CLOSE_PRICE", "最新价"),
    ("CHANGE_RATE", "涨跌幅"),
    ("TURNOVERRATE", "换手率"),
    ("PE_DYNAMIC", "市盈率"),
    ("PRIME_COST", "主力成本"),
    ("ORG_PARTICIPATE", "机构参与度"),
    ("TOTALSCORE", "综合得分"),
    ("RANK_UP", "上升"),
    ("RANK", "目前排名"),
    ("FOCUS", "关注指数"),
];
const COMMENT_SELECT: [&str; 13] = [
    "代码",
    "名称",
    "最新价",
    "涨跌幅",
    "换手率",
    "市盈率",
    "主力成本",
    "机构参与度",
    "综合得分",
    "上升",
    "目前排名",
    "关注指数",
    "交易日",
];
const COMMENT_NUMERIC: [&str; 10] = [
    "最新价",
    "涨跌幅",
    "换手率",
    "市盈率",
    "主力成本",
    "机构参与度",
    "综合得分",
    "上升",
    "目前排名",
    "关注指数",
];
const COMMENT_DATE: [&str; 1] = ["交易日"];

/// 千股千评（对应 akshare [`akshare.stock_comment_em`]）。
///
/// 无参数。
///
/// # 返回列
/// `序号, 代码, 名称, 最新价, 涨跌幅, 换手率, 市盈率, 主力成本, 机构参与度,
/// 综合得分, 上升, 目前排名, 关注指数, 交易日`
pub fn stock_comment_em() -> Result<Df> {
    let quote = "f2~01~SECURITY_CODE~CLOSE_PRICE,f8~01~SECURITY_CODE~TURNOVERRATE,f3~01~SECURITY_CODE~CHANGE_RATE,f9~01~SECURITY_CODE~PE_DYNAMIC";
    let extra = report_extra(
        "SECURITY_CODE",
        "1",
        Some(""),
        Some(quote),
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_DMSK_TS_STOCKNEW", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &COMMENT_RENAME,
        &COMMENT_SELECT,
        &COMMENT_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&COMMENT_DATE)?;
    Ok(df)
}

// ===== 个股上榜统计（RPT_BILLBOARD_TRADEALL）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns` 与实时拉取 JSON 键序逐位对齐。
const LHB_STAT_RENAME: [(&str, &str); 19] = [
    ("SECURITY_CODE", "代码"),
    ("LATEST_TDATE", "最近上榜日"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("IPCT1M", "近1个月涨跌幅"),
    ("IPCT3M", "近3个月涨跌幅"),
    ("IPCT6M", "近6个月涨跌幅"),
    ("IPCT1Y", "近1年涨跌幅"),
    ("CHANGE_RATE", "涨跌幅"),
    ("CLOSE_PRICE", "收盘价"),
    ("BILLBOARD_DEAL_AMT", "龙虎榜总成交额"),
    ("BILLBOARD_NET_BUY", "龙虎榜净买额"),
    ("ORG_NET_BUY", "机构买入净额"),
    ("BILLBOARD_TIMES", "上榜次数"),
    ("BILLBOARD_BUY_AMT", "龙虎榜买入额"),
    ("BILLBOARD_SELL_AMT", "龙虎榜卖出额"),
    ("ORG_BUY_AMT", "机构买入总额"),
    ("ORG_SELL_AMT", "机构卖出总额"),
    ("ORG_BUY_TIMES", "买方机构次数"),
    ("ORG_SELL_TIMES", "卖方机构次数"),
];
const LHB_STAT_SELECT: [&str; 19] = [
    "代码",
    "名称",
    "最近上榜日",
    "收盘价",
    "涨跌幅",
    "上榜次数",
    "龙虎榜净买额",
    "龙虎榜买入额",
    "龙虎榜卖出额",
    "龙虎榜总成交额",
    "买方机构次数",
    "卖方机构次数",
    "机构买入净额",
    "机构买入总额",
    "机构卖出总额",
    "近1个月涨跌幅",
    "近3个月涨跌幅",
    "近6个月涨跌幅",
    "近1年涨跌幅",
];
const LHB_STAT_NUMERIC: [&str; 16] = [
    "收盘价",
    "涨跌幅",
    "上榜次数",
    "龙虎榜净买额",
    "龙虎榜买入额",
    "龙虎榜卖出额",
    "龙虎榜总成交额",
    "买方机构次数",
    "卖方机构次数",
    "机构买入净额",
    "机构买入总额",
    "机构卖出总额",
    "近1个月涨跌幅",
    "近3个月涨跌幅",
    "近6个月涨跌幅",
    "近1年涨跌幅",
];
const LHB_STAT_DATE: [&str; 1] = ["最近上榜日"];

/// 个股上榜统计（对应 akshare [`akshare.stock_lhb_stock_statistic_em`]）。
///
/// `symbol`：统计周期，可选 `"近一月"`(默认)/`"近三月"`/`"近六月"`/`"近一年"`，
/// 分别映射 `STATISTICS_CYCLE` 为 `01`/`02`/`03`/`04`。
///
/// # 返回列
/// `序号, 代码, 名称, 最近上榜日, 收盘价, 涨跌幅, 上榜次数, 龙虎榜净买额, 龙虎榜买入额,
/// 龙虎榜卖出额, 龙虎榜总成交额, 买方机构次数, 卖方机构次数, 机构买入净额, 机构买入总额,
/// 机构卖出总额, 近1个月涨跌幅, 近3个月涨跌幅, 近6个月涨跌幅, 近1年涨跌幅`
pub fn stock_lhb_stock_statistic_em(symbol: &str) -> Result<Df> {
    let cycle = match symbol {
        "近一月" => "01",
        "近三月" => "02",
        "近六月" => "03",
        "近一年" => "04",
        other => {
            return Err(AkshareError::Param(format!(
                "未知统计周期: {other}（应为 近一月/近三月/近六月/近一年）"
            )))
        }
    };
    let filter = format!(r#"(STATISTICS_CYCLE="{cycle}")"#);
    let extra = report_extra(
        "BILLBOARD_TIMES,LATEST_TDATE,SECURITY_CODE",
        "-1,-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_BILLBOARD_TRADEALL", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &LHB_STAT_RENAME,
        &LHB_STAT_SELECT,
        &LHB_STAT_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&LHB_STAT_DATE)?;
    Ok(df)
}

// ===== 机构买卖每日统计（RPT_ORGANIZATION_TRADE_DETAILS）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns` 与实时拉取 JSON 键序逐位对齐。
const LHB_JG_RENAME: [(&str, &str); 15] = [
    ("SECURITY_NAME_ABBR", "名称"),
    ("SECURITY_CODE", "代码"),
    ("TRADE_DATE", "上榜日期"),
    ("CLOSE_PRICE", "收盘价"),
    ("CHANGE_RATE", "涨跌幅"),
    ("BUY_TIMES", "买方机构数"),
    ("SELL_TIMES", "卖方机构数"),
    ("BUY_AMT", "机构买入总额"),
    ("SELL_AMT", "机构卖出总额"),
    ("NET_BUY_AMT", "机构买入净额"),
    ("ACCUM_AMOUNT", "市场总成交额"),
    ("RATIO", "机构净买额占总成交额比"),
    ("TURNOVERRATE", "换手率"),
    ("FREECAP", "流通市值"),
    ("EXPLANATION", "上榜原因"),
];
const LHB_JG_SELECT: [&str; 15] = [
    "代码",
    "名称",
    "收盘价",
    "涨跌幅",
    "买方机构数",
    "卖方机构数",
    "机构买入总额",
    "机构卖出总额",
    "机构买入净额",
    "市场总成交额",
    "机构净买额占总成交额比",
    "换手率",
    "流通市值",
    "上榜原因",
    "上榜日期",
];
const LHB_JG_NUMERIC: [&str; 11] = [
    "收盘价",
    "涨跌幅",
    "买方机构数",
    "卖方机构数",
    "机构买入总额",
    "机构卖出总额",
    "机构买入净额",
    "市场总成交额",
    "机构净买额占总成交额比",
    "换手率",
    "流通市值",
];
const LHB_JG_DATE: [&str; 1] = ["上榜日期"];

/// 机构买卖每日统计（对应 akshare [`akshare.stock_lhb_jgmmtj_em`]）。
///
/// `start_date` / `end_date`：开始/结束日期 `YYYYMMDD`（默认 `"20240417"`/`"20240430"`）。
///
/// # 返回列
/// `序号, 代码, 名称, 收盘价, 涨跌幅, 买方机构数, 卖方机构数, 机构买入总额, 机构卖出总额,
/// 机构买入净额, 市场总成交额, 机构净买额占总成交额比, 换手率, 流通市值, 上榜原因, 上榜日期`
pub fn stock_lhb_jgmmtj_em(start_date: &str, end_date: &str) -> Result<Df> {
    let s = fmt_ymd(start_date)?;
    let e = fmt_ymd(end_date)?;
    let filter = format!("(TRADE_DATE>='{s}')(TRADE_DATE<='{e}')");
    let extra = report_extra(
        "NET_BUY_AMT,TRADE_DATE,SECURITY_CODE",
        "-1,-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_ORGANIZATION_TRADE_DETAILS", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &LHB_JG_RENAME,
        &LHB_JG_SELECT,
        &LHB_JG_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&LHB_JG_DATE)?;
    Ok(df)
}

// ===== 龙虎榜明细/营业部/席位统计（批次1 阶段1j，RPT_* datacenter）=====
// 序号由 Rust 生成（finalize_report 的 index_name）。各报表 RENAME 取自 akshare 的
// 列重命名/列顺序，并对照实时拉取的 JSON 键序逐位对齐（positional 函数经 live 抓取验证）。

/// 龙虎榜统计周期 `近一月/近三月/近六月/近一年` → 东财 `STATISTICSCYCLE` 编码。
fn lhb_cycle(symbol: &str) -> Result<&'static str> {
    Ok(match symbol {
        "近一月" => "01",
        "近三月" => "02",
        "近六月" => "03",
        "近一年" => "04",
        other => {
            return Err(AkshareError::Param(format!(
                "未知统计周期: {other}（应为 近一月/近三月/近六月/近一年）"
            )))
        }
    })
}

// ===== 龙虎榜详情（RPT_DAILYBILLBOARD_DETAILSNEW）=====
const LHB_DETAIL_RENAME: [(&str, &str); 20] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("TRADE_DATE", "上榜日"),
    ("EXPLAIN", "解读"),
    ("CLOSE_PRICE", "收盘价"),
    ("CHANGE_RATE", "涨跌幅"),
    ("BILLBOARD_NET_AMT", "龙虎榜净买额"),
    ("BILLBOARD_BUY_AMT", "龙虎榜买入额"),
    ("BILLBOARD_SELL_AMT", "龙虎榜卖出额"),
    ("BILLBOARD_DEAL_AMT", "龙虎榜成交额"),
    ("ACCUM_AMOUNT", "市场总成交额"),
    ("DEAL_NET_RATIO", "净买额占总成交比"),
    ("DEAL_AMOUNT_RATIO", "成交额占总成交比"),
    ("TURNOVERRATE", "换手率"),
    ("FREE_MARKET_CAP", "流通市值"),
    ("EXPLANATION", "上榜原因"),
    ("D1_CLOSE_ADJCHRATE", "上榜后1日"),
    ("D2_CLOSE_ADJCHRATE", "上榜后2日"),
    ("D5_CLOSE_ADJCHRATE", "上榜后5日"),
    ("D10_CLOSE_ADJCHRATE", "上榜后10日"),
];
const LHB_DETAIL_SELECT: [&str; 20] = [
    "代码",
    "名称",
    "上榜日",
    "解读",
    "收盘价",
    "涨跌幅",
    "龙虎榜净买额",
    "龙虎榜买入额",
    "龙虎榜卖出额",
    "龙虎榜成交额",
    "市场总成交额",
    "净买额占总成交比",
    "成交额占总成交比",
    "换手率",
    "流通市值",
    "上榜原因",
    "上榜后1日",
    "上榜后2日",
    "上榜后5日",
    "上榜后10日",
];
const LHB_DETAIL_NUMERIC: [&str; 15] = [
    "收盘价",
    "涨跌幅",
    "龙虎榜净买额",
    "龙虎榜买入额",
    "龙虎榜卖出额",
    "龙虎榜成交额",
    "市场总成交额",
    "净买额占总成交比",
    "成交额占总成交比",
    "换手率",
    "流通市值",
    "上榜后1日",
    "上榜后2日",
    "上榜后5日",
    "上榜后10日",
];
const LHB_DETAIL_DATE: [&str; 1] = ["上榜日"];

/// 龙虎榜详情（对应 akshare [`akshare.stock_lhb_detail_em`]）。
///
/// `start_date` / `end_date`：开始/结束日期 `YYYYMMDD`（默认 `"20230403"`/`"20230417"`）。
///
/// # 返回列
/// `序号, 代码, 名称, 上榜日, 解读, 收盘价, 涨跌幅, 龙虎榜净买额, 龙虎榜买入额,
/// 龙虎榜卖出额, 龙虎榜成交额, 市场总成交额, 净买额占总成交比, 成交额占总成交比,
/// 换手率, 流通市值, 上榜原因, 上榜后1日, 上榜后2日, 上榜后5日, 上榜后10日`
pub fn stock_lhb_detail_em(start_date: &str, end_date: &str) -> Result<Df> {
    let s = fmt_ymd(start_date)?;
    let e = fmt_ymd(end_date)?;
    let filter = format!("(TRADE_DATE<='{e}')(TRADE_DATE>='{s}')");
    let extra = report_extra(
        "SECURITY_CODE,TRADE_DATE",
        "1,-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter(
        "RPT_DAILYBILLBOARD_DETAILSNEW",
        "SECURITY_CODE,SECUCODE,SECURITY_NAME_ABBR,TRADE_DATE,EXPLAIN,CLOSE_PRICE,CHANGE_RATE,BILLBOARD_NET_AMT,BILLBOARD_BUY_AMT,BILLBOARD_SELL_AMT,BILLBOARD_DEAL_AMT,ACCUM_AMOUNT,DEAL_NET_RATIO,DEAL_AMOUNT_RATIO,TURNOVERRATE,FREE_MARKET_CAP,EXPLANATION,D1_CLOSE_ADJCHRATE,D2_CLOSE_ADJCHRATE,D5_CLOSE_ADJCHRATE,D10_CLOSE_ADJCHRATE,SECURITY_TYPE_CODE",
        &extra,
        "5000",
    )?;
    let mut df = finalize_report(
        &rows,
        &LHB_DETAIL_RENAME,
        &LHB_DETAIL_SELECT,
        &LHB_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&LHB_DETAIL_DATE)?;
    Ok(df)
}

// ===== 机构席位追踪（RPT_ORGANIZATION_SEATNEW）=====
const LHB_JGSTAT_RENAME: [(&str, &str); 15] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("CLOSE_PRICE", "收盘价"),
    ("CHANGE_RATE", "涨跌幅"),
    ("AMOUNT", "龙虎榜成交金额"),
    ("ONLIST_TIMES", "上榜次数"),
    ("BUY_AMT", "机构买入额"),
    ("BUY_TIMES", "机构买入次数"),
    ("SELL_AMT", "机构卖出额"),
    ("SELL_TIMES", "机构卖出次数"),
    ("NET_BUY_AMT", "机构净买额"),
    ("M1_CLOSE_ADJCHRATE", "近1个月涨跌幅"),
    ("M3_CLOSE_ADJCHRATE", "近3个月涨跌幅"),
    ("M6_CLOSE_ADJCHRATE", "近6个月涨跌幅"),
    ("Y1_CLOSE_ADJCHRATE", "近1年涨跌幅"),
];
const LHB_JGSTAT_SELECT: [&str; 15] = [
    "代码",
    "名称",
    "收盘价",
    "涨跌幅",
    "龙虎榜成交金额",
    "上榜次数",
    "机构买入额",
    "机构买入次数",
    "机构卖出额",
    "机构卖出次数",
    "机构净买额",
    "近1个月涨跌幅",
    "近3个月涨跌幅",
    "近6个月涨跌幅",
    "近1年涨跌幅",
];
const LHB_JGSTAT_NUMERIC: [&str; 13] = [
    "收盘价",
    "涨跌幅",
    "龙虎榜成交金额",
    "上榜次数",
    "机构买入额",
    "机构买入次数",
    "机构卖出额",
    "机构卖出次数",
    "机构净买额",
    "近1个月涨跌幅",
    "近3个月涨跌幅",
    "近6个月涨跌幅",
    "近1年涨跌幅",
];

/// 机构席位追踪（对应 akshare [`akshare.stock_lhb_jgstatistic_em`]）。
///
/// `symbol`：统计周期，取值 `近一月/近三月/近六月/近一年`（默认 `"近一月"`）。
///
/// # 返回列
/// `序号, 代码, 名称, 收盘价, 涨跌幅, 龙虎榜成交金额, 上榜次数, 机构买入额, 机构买入次数,
/// 机构卖出额, 机构卖出次数, 机构净买额, 近1个月涨跌幅, 近3个月涨跌幅, 近6个月涨跌幅, 近1年涨跌幅`
pub fn stock_lhb_jgstatistic_em(symbol: &str) -> Result<Df> {
    let cycle = lhb_cycle(symbol)?;
    let filter = format!(r#"(STATISTICSCYCLE="{cycle}")"#);
    let extra = report_extra(
        "ONLIST_TIMES,SECURITY_CODE",
        "-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_ORGANIZATION_SEATNEW", "ALL", &extra, "5000")?;
    let df = finalize_report(
        &rows,
        &LHB_JGSTAT_RENAME,
        &LHB_JGSTAT_SELECT,
        &LHB_JGSTAT_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 每日活跃营业部（RPT_OPERATEDEPT_ACTIVE）=====
// positional 对齐：live 抓取确认 JSON 键序为
// [OPERATEDEPT_NAME, ONLIST_DATE, BUYER_APPEAR_NUM, SELLER_APPEAR_NUM, TOTAL_BUYAMT,
//  TOTAL_SELLAMT, TOTAL_NETAMT, BUY_STOCK, OPERATEDEPT_CODE, SECURITY_NAME_ABBR,
//  OPERATEDEPT_CODE_OLD, ORG_NAME_ABBR]；akshare 第 8/11/12 位为占位 "-"，丢弃。
const LHB_HYYYB_RENAME: [(&str, &str); 9] = [
    ("OPERATEDEPT_NAME", "营业部名称"),
    ("ONLIST_DATE", "上榜日"),
    ("BUYER_APPEAR_NUM", "买入个股数"),
    ("SELLER_APPEAR_NUM", "卖出个股数"),
    ("TOTAL_BUYAMT", "买入总金额"),
    ("TOTAL_SELLAMT", "卖出总金额"),
    ("TOTAL_NETAMT", "总买卖净额"),
    ("SECURITY_NAME_ABBR", "买入股票"),
    ("OPERATEDEPT_CODE", "营业部代码"),
];
const LHB_HYYYB_SELECT: [&str; 9] = [
    "营业部名称",
    "上榜日",
    "买入个股数",
    "卖出个股数",
    "买入总金额",
    "卖出总金额",
    "总买卖净额",
    "买入股票",
    "营业部代码",
];
const LHB_HYYYB_NUMERIC: [&str; 5] = [
    "买入个股数",
    "卖出个股数",
    "买入总金额",
    "卖出总金额",
    "总买卖净额",
];
const LHB_HYYYB_DATE: [&str; 1] = ["上榜日"];

/// 每日活跃营业部（对应 akshare [`akshare.stock_lhb_hyyyb_em`]）。
///
/// `start_date` / `end_date`：开始/结束日期 `YYYYMMDD`（默认 `"20220324"`）。
///
/// # 返回列
/// `序号, 营业部名称, 上榜日, 买入个股数, 卖出个股数, 买入总金额, 卖出总金额,
/// 总买卖净额, 买入股票, 营业部代码`
pub fn stock_lhb_hyyyb_em(start_date: &str, end_date: &str) -> Result<Df> {
    let s = fmt_ymd(start_date)?;
    let e = fmt_ymd(end_date)?;
    let filter = format!("(ONLIST_DATE>='{s}')(ONLIST_DATE<='{e}')");
    let extra = report_extra(
        "TOTAL_NETAMT,ONLIST_DATE,OPERATEDEPT_CODE",
        "-1,-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_OPERATEDEPT_ACTIVE", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &LHB_HYYYB_RENAME,
        &LHB_HYYYB_SELECT,
        &LHB_HYYYB_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&LHB_HYYYB_DATE)?;
    Ok(df)
}

// ===== 营业部排行（RPT_RATEDEPT_RETURNT_RANKING）=====
const LHB_YYBPH_RENAME: [(&str, &str); 16] = [
    ("OPERATEDEPT_NAME", "营业部名称"),
    ("TOTAL_BUYER_SALESTIMES_1DAY", "上榜后1天-买入次数"),
    ("AVERAGE_INCREASE_1DAY", "上榜后1天-平均涨幅"),
    ("RISE_PROBABILITY_1DAY", "上榜后1天-上涨概率"),
    ("TOTAL_BUYER_SALESTIMES_2DAY", "上榜后2天-买入次数"),
    ("AVERAGE_INCREASE_2DAY", "上榜后2天-平均涨幅"),
    ("RISE_PROBABILITY_2DAY", "上榜后2天-上涨概率"),
    ("TOTAL_BUYER_SALESTIMES_3DAY", "上榜后3天-买入次数"),
    ("AVERAGE_INCREASE_3DAY", "上榜后3天-平均涨幅"),
    ("RISE_PROBABILITY_3DAY", "上榜后3天-上涨概率"),
    ("TOTAL_BUYER_SALESTIMES_5DAY", "上榜后5天-买入次数"),
    ("AVERAGE_INCREASE_5DAY", "上榜后5天-平均涨幅"),
    ("RISE_PROBABILITY_5DAY", "上榜后5天-上涨概率"),
    ("TOTAL_BUYER_SALESTIMES_10DAY", "上榜后10天-买入次数"),
    ("AVERAGE_INCREASE_10DAY", "上榜后10天-平均涨幅"),
    ("RISE_PROBABILITY_10DAY", "上榜后10天-上涨概率"),
];
const LHB_YYBPH_SELECT: [&str; 16] = [
    "营业部名称",
    "上榜后1天-买入次数",
    "上榜后1天-平均涨幅",
    "上榜后1天-上涨概率",
    "上榜后2天-买入次数",
    "上榜后2天-平均涨幅",
    "上榜后2天-上涨概率",
    "上榜后3天-买入次数",
    "上榜后3天-平均涨幅",
    "上榜后3天-上涨概率",
    "上榜后5天-买入次数",
    "上榜后5天-平均涨幅",
    "上榜后5天-上涨概率",
    "上榜后10天-买入次数",
    "上榜后10天-平均涨幅",
    "上榜后10天-上涨概率",
];
const LHB_YYBPH_NUMERIC: [&str; 15] = [
    "上榜后1天-买入次数",
    "上榜后1天-平均涨幅",
    "上榜后1天-上涨概率",
    "上榜后2天-买入次数",
    "上榜后2天-平均涨幅",
    "上榜后2天-上涨概率",
    "上榜后3天-买入次数",
    "上榜后3天-平均涨幅",
    "上榜后3天-上涨概率",
    "上榜后5天-买入次数",
    "上榜后5天-平均涨幅",
    "上榜后5天-上涨概率",
    "上榜后10天-买入次数",
    "上榜后10天-平均涨幅",
    "上榜后10天-上涨概率",
];

/// 营业部排行（对应 akshare [`akshare.stock_lhb_yybph_em`]）。
///
/// `symbol`：统计周期，取值 `近一月/近三月/近六月/近一年`（默认 `"近一月"`）。
///
/// # 返回列
/// `序号, 营业部名称, 上榜后1天-买入次数, 上榜后1天-平均涨幅, 上榜后1天-上涨概率,
/// 上榜后2天-买入次数, 上榜后2天-平均涨幅, 上榜后2天-上涨概率, 上榜后3天-买入次数,
/// 上榜后3天-平均涨幅, 上榜后3天-上涨概率, 上榜后5天-买入次数, 上榜后5天-平均涨幅,
/// 上榜后5天-上涨概率, 上榜后10天-买入次数, 上榜后10天-平均涨幅, 上榜后10天-上涨概率`
pub fn stock_lhb_yybph_em(symbol: &str) -> Result<Df> {
    let cycle = lhb_cycle(symbol)?;
    let filter = format!(r#"(STATISTICSCYCLE="{cycle}")"#);
    let extra = report_extra(
        "TOTAL_BUYER_SALESTIMES_1DAY,OPERATEDEPT_CODE",
        "-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_RATEDEPT_RETURNT_RANKING", "ALL", &extra, "5000")?;
    let df = finalize_report(
        &rows,
        &LHB_YYBPH_RENAME,
        &LHB_YYBPH_SELECT,
        &LHB_YYBPH_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 营业部统计（RPT_OPERATEDEPT_LIST_STATISTICS）=====
const LHB_TRADER_RENAME: [(&str, &str); 7] = [
    ("OPERATEDEPT_NAME", "营业部名称"),
    ("AMOUNT", "龙虎榜成交金额"),
    ("SALES_ONLIST_TIMES", "上榜次数"),
    ("ACT_BUY", "买入额"),
    ("TOTAL_BUYER_SALESTIMES", "买入次数"),
    ("ACT_SELL", "卖出额"),
    ("TOTAL_SELLER_SALESTIMES", "卖出次数"),
];
const LHB_TRADER_SELECT: [&str; 7] = [
    "营业部名称",
    "龙虎榜成交金额",
    "上榜次数",
    "买入额",
    "买入次数",
    "卖出额",
    "卖出次数",
];
const LHB_TRADER_NUMERIC: [&str; 6] = [
    "龙虎榜成交金额",
    "上榜次数",
    "买入额",
    "买入次数",
    "卖出额",
    "卖出次数",
];

/// 营业部统计（对应 akshare [`akshare.stock_lhb_traderstatistic_em`]）。
///
/// `symbol`：统计周期，取值 `近一月/近三月/近六月/近一年`（默认 `"近一月"`）。
///
/// # 返回列
/// `序号, 营业部名称, 龙虎榜成交金额, 上榜次数, 买入额, 买入次数, 卖出额, 卖出次数`
pub fn stock_lhb_traderstatistic_em(symbol: &str) -> Result<Df> {
    let cycle = lhb_cycle(symbol)?;
    let filter = format!(r#"(STATISTICSCYCLE="{cycle}")"#);
    let extra = report_extra(
        "AMOUNT,OPERATEDEPT_CODE",
        "-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_OPERATEDEPT_LIST_STATISTICS", "ALL", &extra, "5000")?;
    let df = finalize_report(
        &rows,
        &LHB_TRADER_RENAME,
        &LHB_TRADER_SELECT,
        &LHB_TRADER_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 个股龙虎榜详情-日期（RPT_LHB_BOARDDATE）=====
// columns 为显式列表，positional 对齐：第2/3/4位 = SECURITY_CODE/TRADE_DATE/TR_DATE；
// 第4位 TR_DATE 为占位 "-" 丢弃。
const LHB_BOARDDATE_RENAME: [(&str, &str); 2] =
    [("SECURITY_CODE", "股票代码"), ("TRADE_DATE", "交易日")];
const LHB_BOARDDATE_SELECT: [&str; 2] = ["股票代码", "交易日"];
const LHB_BOARDDATE_NUMERIC: [&str; 0] = [];
const LHB_BOARDDATE_DATE: [&str; 1] = ["交易日"];

/// 个股龙虎榜详情-日期（对应 akshare [`akshare.stock_lhb_stock_detail_date_em`]）。
///
/// `symbol`：股票代码（如 `"600077"`）。
///
/// # 返回列
/// `序号, 股票代码, 交易日`
pub fn stock_lhb_stock_detail_date_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let extra = report_extra(
        "TRADE_DATE",
        "-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter(
        "RPT_LHB_BOARDDATE",
        "SECURITY_CODE,TRADE_DATE,TR_DATE",
        &extra,
        "1000",
    )?;
    let mut df = finalize_report(
        &rows,
        &LHB_BOARDDATE_RENAME,
        &LHB_BOARDDATE_SELECT,
        &LHB_BOARDDATE_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&LHB_BOARDDATE_DATE)?;
    Ok(df)
}

// ===== 个股龙虎榜详情（RPT_BILLBOARD_DAILYDETAILSBUY / RPT_BILLBOARD_DAILYDETAILSSELL）=====
// 多分支：flag `买入`→BUY 报表，flag `卖出`→SELL 报表（akshare 默认 `卖出`）。
// 两报表 JSON 键序不同，但输出列映射的键（OPERATEDEPT_NAME/EXPLANATION/BUY/SELL/NET/
// TOTAL_BUYRIO/TOTAL_SELLRIO）在两分支一致，故共用同一 RENAME。
// 注：akshare 末会对 `类型`(=EXPLANATION) 升序重排并重排 序号；本实现保持抓取顺序的 序号，
// 列契约与值一致，仅行序未做该重排（parity loose 模式不比对行序）。
const LHB_DETAIL_STOCK_RENAME: [(&str, &str); 7] = [
    ("OPERATEDEPT_NAME", "交易营业部名称"),
    ("EXPLANATION", "类型"),
    ("BUY", "买入金额"),
    ("SELL", "卖出金额"),
    ("NET", "净额"),
    ("TOTAL_BUYRIO", "买入金额-占总成交比例"),
    ("TOTAL_SELLRIO", "卖出金额-占总成交比例"),
];
const LHB_DETAIL_STOCK_SELECT: [&str; 7] = [
    "交易营业部名称",
    "买入金额",
    "买入金额-占总成交比例",
    "卖出金额",
    "卖出金额-占总成交比例",
    "净额",
    "类型",
];
const LHB_DETAIL_STOCK_NUMERIC: [&str; 5] = [
    "买入金额",
    "买入金额-占总成交比例",
    "卖出金额",
    "卖出金额-占总成交比例",
    "净额",
];

/// 个股龙虎榜详情（对应 akshare [`akshare.stock_lhb_stock_detail_em`]）。
///
/// `symbol`：股票代码；`date`：龙虎榜日期 `YYYYMMDD`（需先经
/// [`stock_lhb_stock_detail_date_em`] 获取有数据的日期）；`flag`：`买入` 或 `卖出`
///（默认 `"卖出"`）。
///
/// # 返回列
/// `序号, 交易营业部名称, 买入金额, 买入金额-占总成交比例, 卖出金额, 卖出金额-占总成交比例, 净额, 类型`
pub fn stock_lhb_stock_detail_em(symbol: &str, date: &str, flag: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let (report, sort_col) = match flag {
        "买入" => ("RPT_BILLBOARD_DAILYDETAILSBUY", "BUY"),
        "卖出" => ("RPT_BILLBOARD_DAILYDETAILSSELL", "SELL"),
        other => {
            return Err(AkshareError::Param(format!(
                "未知 flag: {other}（应为 买入/卖出）"
            )))
        }
    };
    let filter = format!(r#"(TRADE_DATE='{}')(SECURITY_CODE="{}")"#, d, symbol);
    let extra = report_extra(sort_col, "-1", Some(&filter), None, Some(EM_TOKEN), None);
    let rows = datacenter(report, "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &LHB_DETAIL_STOCK_RENAME,
        &LHB_DETAIL_STOCK_SELECT,
        &LHB_DETAIL_STOCK_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 营业部历史交易明细（RPT_OPERATEDEPT_TRADE_DETAILSNEW）=====
const LHB_YYB_DETAIL_RENAME: [(&str, &str); 18] = [
    ("OPERATEDEPT_CODE", "营业部代码"),
    ("OPERATEDEPT_NAME", "营业部名称"),
    ("ORG_NAME_ABBR", "营业部简称"),
    ("TRADE_DATE", "交易日期"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票名称"),
    ("CHANGE_RATE", "涨跌幅"),
    ("ACT_BUY", "买入金额"),
    ("ACT_SELL", "卖出金额"),
    ("NET_AMT", "净额"),
    ("EXPLANATION", "上榜原因"),
    ("D1_CLOSE_ADJCHRATE", "1日后涨跌幅"),
    ("D2_CLOSE_ADJCHRATE", "2日后涨跌幅"),
    ("D3_CLOSE_ADJCHRATE", "3日后涨跌幅"),
    ("D5_CLOSE_ADJCHRATE", "5日后涨跌幅"),
    ("D10_CLOSE_ADJCHRATE", "10日后涨跌幅"),
    ("D20_CLOSE_ADJCHRATE", "20日后涨跌幅"),
    ("D30_CLOSE_ADJCHRATE", "30日后涨跌幅"),
];
const LHB_YYB_DETAIL_SELECT: [&str; 18] = [
    "营业部代码",
    "营业部名称",
    "营业部简称",
    "交易日期",
    "股票代码",
    "股票名称",
    "涨跌幅",
    "买入金额",
    "卖出金额",
    "净额",
    "上榜原因",
    "1日后涨跌幅",
    "2日后涨跌幅",
    "3日后涨跌幅",
    "5日后涨跌幅",
    "10日后涨跌幅",
    "20日后涨跌幅",
    "30日后涨跌幅",
];
const LHB_YYB_DETAIL_NUMERIC: [&str; 11] = [
    "涨跌幅",
    "买入金额",
    "卖出金额",
    "净额",
    "1日后涨跌幅",
    "2日后涨跌幅",
    "3日后涨跌幅",
    "5日后涨跌幅",
    "10日后涨跌幅",
    "20日后涨跌幅",
    "30日后涨跌幅",
];
const LHB_YYB_DETAIL_DATE: [&str; 1] = ["交易日期"];

/// 营业部历史交易明细（对应 akshare [`akshare.stock_lhb_yyb_detail_em`]）。
///
/// `symbol`：营业部代码（如 `"10188715"`，由 [`stock_lhb_hyyyb_em`] 获取）。
///
/// # 返回列
/// `序号, 营业部代码, 营业部名称, 营业部简称, 交易日期, 股票代码, 股票名称, 涨跌幅,
/// 买入金额, 卖出金额, 净额, 上榜原因, 1日后涨跌幅, 2日后涨跌幅, 3日后涨跌幅,
/// 5日后涨跌幅, 10日后涨跌幅, 20日后涨跌幅, 30日后涨跌幅`
pub fn stock_lhb_yyb_detail_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(OPERATEDEPT_CODE="{symbol}")"#);
    let extra = report_extra(
        "TRADE_DATE,SECURITY_CODE",
        "-1,1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_OPERATEDEPT_TRADE_DETAILSNEW", "ALL", &extra, "100")?;
    let mut df = finalize_report(
        &rows,
        &LHB_YYB_DETAIL_RENAME,
        &LHB_YYB_DETAIL_SELECT,
        &LHB_YYB_DETAIL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&LHB_YYB_DETAIL_DATE)?;
    Ok(df)
}

// ===== 股东持股统计-十大流通股东/十大股东（RPT_COOPFREEHOLDERS_ANALYSIS / RPT_COOPHOLDERS_ANALYSIS）=====
// 序号由 Rust 生成。两报表 JSON 键序一致，共用同一套 RENAME/SELECT/NUMERIC。
// 列序参照 akshare `big_df.columns` 与实时拉取 JSON 键序逐位对齐。无日期列。
const GDFX_STAT_RENAME: [(&str, &str); 13] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_TYPE", "股东类型"),
    ("STATISTICS_TIMES", "统计次数"),
    ("AVG_CHANGE_10TD", "公告日后涨幅统计-10个交易日-平均涨幅"),
    ("MAX_CHANGE_10TD", "公告日后涨幅统计-10个交易日-最大涨幅"),
    ("MIN_CHANGE_10TD", "公告日后涨幅统计-10个交易日-最小涨幅"),
    ("AVG_CHANGE_30TD", "公告日后涨幅统计-30个交易日-平均涨幅"),
    ("MAX_CHANGE_30TD", "公告日后涨幅统计-30个交易日-最大涨幅"),
    ("MIN_CHANGE_30TD", "公告日后涨幅统计-30个交易日-最小涨幅"),
    ("AVG_CHANGE_60TD", "公告日后涨幅统计-60个交易日-平均涨幅"),
    ("MAX_CHANGE_60TD", "公告日后涨幅统计-60个交易日-最大涨幅"),
    ("MIN_CHANGE_60TD", "公告日后涨幅统计-60个交易日-最小涨幅"),
    ("SEAB_JOIN", "持有个股"),
];
const GDFX_STAT_SELECT: [&str; 13] = [
    "股东名称",
    "股东类型",
    "统计次数",
    "公告日后涨幅统计-10个交易日-平均涨幅",
    "公告日后涨幅统计-10个交易日-最大涨幅",
    "公告日后涨幅统计-10个交易日-最小涨幅",
    "公告日后涨幅统计-30个交易日-平均涨幅",
    "公告日后涨幅统计-30个交易日-最大涨幅",
    "公告日后涨幅统计-30个交易日-最小涨幅",
    "公告日后涨幅统计-60个交易日-平均涨幅",
    "公告日后涨幅统计-60个交易日-最大涨幅",
    "公告日后涨幅统计-60个交易日-最小涨幅",
    "持有个股",
];
const GDFX_STAT_NUMERIC: [&str; 10] = [
    "统计次数",
    "公告日后涨幅统计-10个交易日-平均涨幅",
    "公告日后涨幅统计-10个交易日-最大涨幅",
    "公告日后涨幅统计-10个交易日-最小涨幅",
    "公告日后涨幅统计-30个交易日-平均涨幅",
    "公告日后涨幅统计-30个交易日-最大涨幅",
    "公告日后涨幅统计-30个交易日-最小涨幅",
    "公告日后涨幅统计-60个交易日-平均涨幅",
    "公告日后涨幅统计-60个交易日-最大涨幅",
    "公告日后涨幅统计-60个交易日-最小涨幅",
];

/// 股东持股统计-十大流通股东（对应 akshare [`akshare.stock_gdfx_free_holding_statistics_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20210630"`）。
///
/// # 返回列
/// `序号, 股东名称, 股东类型, 统计次数, 公告日后涨幅统计-10个交易日-平均涨幅,
/// 公告日后涨幅统计-10个交易日-最大涨幅, 公告日后涨幅统计-10个交易日-最小涨幅,
/// 公告日后涨幅统计-30个交易日-平均涨幅, 公告日后涨幅统计-30个交易日-最大涨幅,
/// 公告日后涨幅统计-30个交易日-最小涨幅, 公告日后涨幅统计-60个交易日-平均涨幅,
/// 公告日后涨幅统计-60个交易日-最大涨幅, 公告日后涨幅统计-60个交易日-最小涨幅, 持有个股`
pub fn stock_gdfx_free_holding_statistics_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(r#"(HOLDNUM_CHANGE_TYPE="001")(END_DATE='{d}')"#);
    let extra = report_extra(
        "STATISTICS_TIMES,COOPERATION_HOLDER_MARK",
        "-1,-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_COOPFREEHOLDERS_ANALYSIS", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GDFX_STAT_RENAME,
        &GDFX_STAT_SELECT,
        &GDFX_STAT_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

/// 股东持股统计-十大股东（对应 akshare [`akshare.stock_gdfx_holding_statistics_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20210930"`）。
///
/// # 返回列
/// 与 [`stock_gdfx_free_holding_statistics_em`] 一致。
pub fn stock_gdfx_holding_statistics_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(r#"(HOLDNUM_CHANGE_TYPE="001")(END_DATE='{d}')"#);
    let extra = report_extra(
        "STATISTICS_TIMES,COOPERATION_HOLDER_MARK",
        "-1,-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_COOPHOLDERS_ANALYSIS", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GDFX_STAT_RENAME,
        &GDFX_STAT_SELECT,
        &GDFX_STAT_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 股东持股变动统计-十大流通股东/十大股东（RPT_FREEHOLDERS_BASIC_INFO / RPT_HOLDERS_BASIC_INFO）=====
// 序号由 Rust 生成。两报表 JSON 键序中「持有个股」(SEAB_JOIN) 与「流通市值统计」(HOLDER_MARKET_CAP)
// 位置不同，但键名一致，故共用同一套 RENAME/SELECT/NUMERIC。无日期列。
const GDFX_CHG_RENAME: [(&str, &str); 9] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_TYPE", "股东类型"),
    ("HOLDER_NUM", "期末持股只数统计-总持有"),
    ("HOLDADD_NUM", "期末持股只数统计-新进"),
    ("HOLDUP_NUM", "期末持股只数统计-增加"),
    ("HOLDDOWN_NUM", "期末持股只数统计-减少"),
    ("HOLDUNCHANGED_NUM", "期末持股只数统计-不变"),
    ("HOLDER_MARKET_CAP", "流通市值统计"),
    ("SEAB_JOIN", "持有个股"),
];
const GDFX_CHG_SELECT: [&str; 9] = [
    "股东名称",
    "股东类型",
    "期末持股只数统计-总持有",
    "期末持股只数统计-新进",
    "期末持股只数统计-增加",
    "期末持股只数统计-不变",
    "期末持股只数统计-减少",
    "流通市值统计",
    "持有个股",
];
const GDFX_CHG_NUMERIC: [&str; 6] = [
    "期末持股只数统计-总持有",
    "期末持股只数统计-新进",
    "期末持股只数统计-增加",
    "期末持股只数统计-不变",
    "期末持股只数统计-减少",
    "流通市值统计",
];

/// 股东持股变动统计-十大流通股东（对应 akshare [`akshare.stock_gdfx_free_holding_change_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20210930"`）。
///
/// # 返回列
/// `序号, 股东名称, 股东类型, 期末持股只数统计-总持有, 期末持股只数统计-新进,
/// 期末持股只数统计-增加, 期末持股只数统计-不变, 期末持股只数统计-减少, 流通市值统计, 持有个股`
pub fn stock_gdfx_free_holding_change_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(END_DATE='{d}')");
    let extra = report_extra(
        "HOLDER_NUM,HOLDER_NEW",
        "-1,-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_FREEHOLDERS_BASIC_INFO", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GDFX_CHG_RENAME,
        &GDFX_CHG_SELECT,
        &GDFX_CHG_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

/// 股东持股变动统计-十大股东（对应 akshare [`akshare.stock_gdfx_holding_change_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20210930"`）。
///
/// # 返回列
/// 与 [`stock_gdfx_free_holding_change_em`] 一致。
pub fn stock_gdfx_holding_change_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(END_DATE='{d}')");
    let extra = report_extra(
        "HOLDER_NUM,HOLDER_NEW",
        "-1,-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_HOLDERS_BASIC_INFO", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GDFX_CHG_RENAME,
        &GDFX_CHG_SELECT,
        &GDFX_CHG_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 千股千评-主力控盘-机构参与度（RPT_DMSK_TS_STOCKEVALUATE）=====
// 无 序号（akshare 仅 reset_index(drop=True)）。机构参与度在 akshare 中 ×100。
const COMMENT_JGCYD_RENAME: [(&str, &str); 2] =
    [("TRADE_DATE", "交易日"), ("ORG_PARTICIPATE", "机构参与度")];
const COMMENT_JGCYD_SELECT: [&str; 2] = ["交易日", "机构参与度"];
const COMMENT_JGCYD_NUMERIC: [&str; 1] = ["机构参与度"];
const COMMENT_JGCYD_DATE: [&str; 1] = ["交易日"];

/// 千股千评-主力控盘-机构参与度（对应 akshare [`akshare.stock_comment_detail_zlkp_jgcyd_em`]）。
///
/// `symbol`：股票代码（默认 `"600000"`）。
///
/// # 返回列
/// `交易日, 机构参与度`（机构参与度 = akshare `ORG_PARTICIPATE × 100`）
pub fn stock_comment_detail_zlkp_jgcyd_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let extra = report_extra(
        "TRADE_DATE",
        "-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_DMSK_TS_STOCKEVALUATE", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &COMMENT_JGCYD_RENAME,
        &COMMENT_JGCYD_SELECT,
        &COMMENT_JGCYD_NUMERIC,
        None,
    )?;
    df.cast_date(&COMMENT_JGCYD_DATE)?;
    // akshare: ORG_PARTICIPATE * 100（scale 语义为 ÷factor，故 0.01 即 ×100）
    df.scale("机构参与度", 0.01)?;
    Ok(df)
}

// ===== 千股千评-综合评价-历史评分（RPT_STOCK_HISTORYMARK）=====
// 无 序号（akshare 仅 reset_index(drop=True)）。
const COMMENT_LSPF_RENAME: [(&str, &str); 2] =
    [("DIAGNOSE_DATE", "交易日"), ("TOTAL_SCORE", "评分")];
const COMMENT_LSPF_SELECT: [&str; 2] = ["交易日", "评分"];
const COMMENT_LSPF_NUMERIC: [&str; 1] = ["评分"];
const COMMENT_LSPF_DATE: [&str; 1] = ["交易日"];

/// 千股千评-综合评价-历史评分（对应 akshare [`akshare.stock_comment_detail_zhpj_lspf_em`]）。
///
/// `symbol`：股票代码（默认 `"600000"`）。
///
/// # 返回列
/// `交易日, 评分`
pub fn stock_comment_detail_zhpj_lspf_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let extra = report_extra(
        "DIAGNOSE_DATE",
        "1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_STOCK_HISTORYMARK", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &COMMENT_LSPF_RENAME,
        &COMMENT_LSPF_SELECT,
        &COMMENT_LSPF_NUMERIC,
        None,
    )?;
    df.cast_date(&COMMENT_LSPF_DATE)?;
    Ok(df)
}

// ===== 沪深港通持股-每日个股统计（北向持股，RPT_MUTUAL_STOCK_NORTHSTA）=====
// 对应 akshare [`akshare.stock_hsgt_stock_statistics_em`] 默认 北向持股 分支。
// 无 序号（akshare 北向分支仅 DataFrame + 列重命名，无 index 列）。
// 列集与 南向 HOLDRANKS 报表一致：SECURITY_CODE/SECURITY_NAME 分别对应 股票代码/股票简称
// （北向/南向两套报表的 JSON 键序不同，但键名一致，故 RENAME 复用同一组键名）。
const HSGT_STAT_RENAME: [(&str, &str); 11] = [
    ("TRADE_DATE", "持股日期"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME", "股票简称"),
    ("CLOSE_PRICE", "当日收盘价"),
    ("CHANGE_RATE", "当日涨跌幅"),
    ("HOLD_SHARES", "持股数量"),
    ("HOLD_MARKET_CAP", "持股市值"),
    ("HOLD_SHARES_RATIO", "持股数量占发行股百分比"),
    ("HOLD_MARKETCAP_CHG1", "持股市值变化-1日"),
    ("HOLD_MARKETCAP_CHG5", "持股市值变化-5日"),
    ("HOLD_MARKETCAP_CHG10", "持股市值变化-10日"),
];
const HSGT_STAT_SELECT: [&str; 11] = [
    "持股日期",
    "股票代码",
    "股票简称",
    "当日收盘价",
    "当日涨跌幅",
    "持股数量",
    "持股市值",
    "持股数量占发行股百分比",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const HSGT_STAT_NUMERIC: [&str; 8] = [
    "当日收盘价",
    "当日涨跌幅",
    "持股数量",
    "持股市值",
    "持股数量占发行股百分比",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const HSGT_STAT_DATE: [&str; 1] = ["持股日期"];

/// 沪深港通持股-每日个股统计（北向持股，对应 akshare [`akshare.stock_hsgt_stock_statistics_em`] 默认 北向持股 分支）。
///
/// `start_date` / `end_date`：开始/结束日期 `YYYYMMDD`。
///
/// 注：akshare 原版该接口签名为 `(symbol, start_date, end_date)`，此处实现其
/// **默认** 北向持股分支 `RPT_MUTUAL_STOCK_NORTHSTA`，滤网
/// `(INTERVAL_TYPE="1")(MUTUAL_TYPE in ("001","003"))(TRADE_DATE>='<start>')(TRADE_DATE<='<end>')`
/// （南向 `RPT_MUTUAL_STOCK_HOLDRANKS` 为非默认分支，未实现）。
///
/// # 返回列
/// `持股日期, 股票代码, 股票简称, 当日收盘价, 当日涨跌幅, 持股数量, 持股市值,
/// 持股数量占发行股百分比, 持股市值变化-1日, 持股市值变化-5日, 持股市值变化-10日`
pub fn stock_hsgt_stock_statistics_em(start_date: &str, end_date: &str) -> Result<Df> {
    let s = fmt_ymd(start_date)?;
    let e = fmt_ymd(end_date)?;
    let filter = format!(
        r#"(INTERVAL_TYPE="1")(MUTUAL_TYPE in ("001","003"))(TRADE_DATE>='{s}')(TRADE_DATE<='{e}')"#
    );
    let extra = report_extra(
        "TRADE_DATE",
        "-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_MUTUAL_STOCK_NORTHSTA", "ALL", &extra, "1000")?;
    let mut df = finalize_report(
        &rows,
        &HSGT_STAT_RENAME,
        &HSGT_STAT_SELECT,
        &HSGT_STAT_NUMERIC,
        None,
    )?;
    df.cast_date(&HSGT_STAT_DATE)?;
    Ok(df)
}

// ===== 沪深港通持股-个股排行（RPT_MUTUAL_STOCK_NORTHSTA）=====
// 对应 akshare [`akshare.stock_hsgt_hold_stock_em`]。
// 多分支：market ∈ {北向, 沪股通, 深股通} 决定 MUTUAL_TYPE 过滤；
// indicator ∈ {今日排行, 3日排行, 5日排行, 10日排行, 月排行, 季排行, 年排行}
// 决定 INTERVAL_TYPE，且输出中“增持估计-*”列名带 indicator 前缀（如 “5日增持估计-股数”）。
// 注意：akshare 原版从 HTML 抓取 TRADE_DATE；此处改为显式 `date` 参数（YYYYMMDD）。
// 列契约按 akshare 位置重命名推导；因 eastmoney 限速未能实时核验全部 JSON 键，
// 6 个“增持估计/占总股本比/所属板块”键名为按 eastmoney 命名惯例推断
// （HOLD_MARKETCAP_RATIO / ADD_SHARES_REPAIR / ADD_SHARES_AMP /
// ADD_SHARES_RATIO / ADD_MARKETCAP_RATIO / INDUSTRY），其余键名已与
// `stock_hsgt_stock_statistics_em` 共用的 RPT_MUTUAL_STOCK_NORTHSTA 报表核对。
const HOLD_RENAME: [(&str, &str); 15] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME", "名称"),
    ("CLOSE_PRICE", "今日收盘价"),
    ("CHANGE_RATE", "今日涨跌幅"),
    ("HOLD_SHARES", "今日持股-股数"),
    ("HOLD_MARKET_CAP", "今日持股-市值"),
    ("HOLD_SHARES_RATIO", "今日持股-占流通股比"),
    ("HOLD_MARKETCAP_RATIO", "今日持股-占总股本比"), // 推断
    ("ADD_SHARES_REPAIR", "增持估计-股数"),          // 推断
    ("ADD_MARKET_CAP", "增持估计-市值"),
    ("ADD_SHARES_AMP", "增持估计-市值增幅"),        // 推断
    ("ADD_SHARES_RATIO", "增持估计-占流通股比"),    // 推断
    ("ADD_MARKETCAP_RATIO", "增持估计-占总股本比"), // 推断
    ("INDUSTRY", "所属板块"),                       // 推断
    ("TRADE_DATE", "日期"),
];
const HOLD_SELECT: [&str; 15] = [
    "代码",
    "名称",
    "今日收盘价",
    "今日涨跌幅",
    "今日持股-股数",
    "今日持股-市值",
    "今日持股-占流通股比",
    "今日持股-占总股本比",
    "增持估计-股数",
    "增持估计-市值",
    "增持估计-市值增幅",
    "增持估计-占流通股比",
    "增持估计-占总股本比",
    "所属板块",
    "日期",
];
const HOLD_NUMERIC: [&str; 11] = [
    "今日收盘价",
    "今日涨跌幅",
    "今日持股-股数",
    "今日持股-市值",
    "今日持股-占流通股比",
    "今日持股-占总股本比",
    "增持估计-股数",
    "增持估计-市值",
    "增持估计-市值增幅",
    "增持估计-占流通股比",
    "增持估计-占总股本比",
];
const HOLD_DATE: [&str; 1] = ["日期"];

/// 沪深港通持股-个股排行（对应 akshare [`akshare.stock_hsgt_hold_stock_em`]）。
///
/// `market`：`北向` / `沪股通` / `深股通`；`indicator`：
/// `今日排行` / `3日排行` / `5日排行` / `10日排行` / `月排行` / `季排行` / `年排行`；
/// `date`：交易日期 `YYYYMMDD`（原 akshare 从 HTML 抓取，此处显式传入）。
///
/// # 返回列
/// `序号, 代码, 名称, 今日收盘价, 今日涨跌幅, 今日持股-股数, 今日持股-市值,
/// 今日持股-占流通股比, 今日持股-占总股本比, {indicator前缀}增持估计-股数,
/// {indicator前缀}增持估计-市值, {indicator前缀}增持估计-市值增幅,
/// {indicator前缀}增持估计-占流通股比, {indicator前缀}增持估计-占总股本比,
/// 所属板块, 日期`
pub fn stock_hsgt_hold_stock_em(market: &str, indicator: &str, date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let it = match indicator {
        "今日排行" => "1",
        "3日排行" => "3",
        "5日排行" => "5",
        "10日排行" => "10",
        "月排行" => "M",
        "季排行" => "Q",
        "年排行" => "Y",
        other => return Err(AkshareError::Param(format!("未知 indicator: {other}"))),
    };
    let mt = match market {
        "北向" => "",
        "沪股通" => "001",
        "深股通" => "003",
        other => return Err(AkshareError::Param(format!("未知 market: {other}"))),
    };
    let filter = if mt.is_empty() {
        format!(r#"(TRADE_DATE='{d}')(INTERVAL_TYPE="{it}")"#)
    } else {
        format!(r#"(TRADE_DATE='{d}')(INTERVAL_TYPE="{it}")(MUTUAL_TYPE="{mt}")"#)
    };
    let extra = report_extra("ADD_MARKET_CAP", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_MUTUAL_STOCK_NORTHSTA", "ALL", &extra, "5000")?;
    let mut df = finalize_report(
        &rows,
        &HOLD_RENAME,
        &HOLD_SELECT,
        &HOLD_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&HOLD_DATE)?;
    // indicator 前缀拼接到 “增持估计-*” 列名（对应 akshare indicator.split('排')[0]）
    let prefix = indicator.split('排').next().unwrap_or(indicator);
    let names: Vec<String> = std::iter::once("序号".to_string())
        .chain(HOLD_SELECT.iter().map(|c| {
            if c.starts_with("增持估计-") {
                format!("{prefix}{c}")
            } else {
                (*c).to_string()
            }
        }))
        .collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    df.rename_columns(&refs)?;
    Ok(df)
}

// ===== 沪深港通每日机构统计（PRT_MUTUAL_ORG_STA，PRT_ 前缀）=====
// 对应 akshare [`akshare.stock_hsgt_institution_statistics_em`]。
// 多分支：market ∈ {北向持股, 南向持股, 沪股通持股, 深股通持股} → MARKET_TYPE。
// 列序已与实时 JSON 键序核对。
const INST_RENAME: [(&str, &str); 7] = [
    ("HOLD_DATE", "持股日期"),
    ("ORG_NAME", "机构名称"),
    ("HOLD_NUM", "持股只数"),
    ("HOLD_MARKET_CAP", "持股市值"),
    ("HOLD_MARKET_CAPONE", "持股市值变化-1日"),
    ("HOLD_MARKET_CAPFIVE", "持股市值变化-5日"),
    ("HOLD_MARKET_CAPTEN", "持股市值变化-10日"),
];
const INST_SELECT: [&str; 7] = [
    "持股日期",
    "机构名称",
    "持股只数",
    "持股市值",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const INST_NUMERIC: [&str; 5] = [
    "持股只数",
    "持股市值",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const INST_DATE: [&str; 1] = ["持股日期"];

/// 沪深港通每日机构统计（对应 akshare [`akshare.stock_hsgt_institution_statistics_em`]）。
///
/// `market`：`北向持股` / `南向持股` / `沪股通持股` / `深股通持股`；
/// `start_date` / `end_date`：起止日期 `YYYYMMDD`。
///
/// # 返回列
/// `持股日期, 机构名称, 持股只数, 持股市值, 持股市值变化-1日, 持股市值变化-5日, 持股市值变化-10日`
pub fn stock_hsgt_institution_statistics_em(
    market: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    let s = fmt_ymd(start_date)?;
    let e = fmt_ymd(end_date)?;
    let mt = match market {
        "北向持股" => "N",
        "南向持股" => "S",
        "沪股通持股" => "001",
        "深股通持股" => "003",
        other => return Err(AkshareError::Param(format!("未知 market: {other}"))),
    };
    let filter = format!(r#"(MARKET_TYPE="{mt}")(HOLD_DATE>='{s}')(HOLD_DATE<='{e}')"#);
    let extra = report_extra("HOLD_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("PRT_MUTUAL_ORG_STA", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &INST_RENAME, &INST_SELECT, &INST_NUMERIC, None)?;
    df.cast_date(&INST_DATE)?;
    Ok(df)
}

// ===== 沪深港通历史资金流向（RPT_MUTUAL_DEAL_HISTORY）=====
// 对应 akshare [`akshare.stock_hsgt_hist_em`]。
// 多分支：symbol → MUTUAL_TYPE（00{suffix}）；输出含动态指数列
// （沪深300/上证指数/深证指数/恒生指数 及其涨跌幅）。
// 列序已与实时 JSON 键序核对。
const HIST_RNAME: [(&str, &str); 13] = [
    ("TRADE_DATE", "日期"),
    ("NET_DEAL_AMT", "当日成交净买额"),
    ("BUY_AMT", "买入成交额"),
    ("SELL_AMT", "卖出成交额"),
    ("ACCUM_DEAL_AMT", "历史累计净买额"),
    ("FUND_INFLOW", "当日资金流入"),
    ("QUOTA_BALANCE", "当日余额"),
    ("HOLD_MARKET_CAP", "持股市值"),
    ("LEAD_STOCKS_NAME", "领涨股"),
    ("LEAD_STOCKS_CODE", "领涨股-代码"),
    ("LS_CHANGE_RATE", "领涨股-涨跌幅"),
    ("INDEX_CLOSE_PRICE", "__INDEX__"),
    ("INDEX_CHANGE_RATE", "__INDEXCHG__"),
];
const HIST_SELECT: [&str; 13] = [
    "日期",
    "当日成交净买额",
    "买入成交额",
    "卖出成交额",
    "历史累计净买额",
    "当日资金流入",
    "当日余额",
    "持股市值",
    "领涨股",
    "领涨股-涨跌幅",
    "__INDEX__",
    "__INDEXCHG__",
    "领涨股-代码",
];
const HIST_NUMERIC: [&str; 10] = [
    "当日成交净买额",
    "买入成交额",
    "卖出成交额",
    "历史累计净买额",
    "当日资金流入",
    "当日余额",
    "持股市值",
    "领涨股-涨跌幅",
    "__INDEX__",
    "__INDEXCHG__",
];
const HIST_DATE: [&str; 1] = ["日期"];

/// 沪深港通历史资金流向（对应 akshare [`akshare.stock_hsgt_hist_em`]）。
///
/// `symbol`：`北向资金` / `沪股通` / `深股通` / `南向资金` / `港股通沪` / `港股通深`。
/// 输出含动态指数列（`沪深300` / `上证指数` / `深证指数` / `恒生指数` 及其涨跌幅）。
///
/// # 返回列
/// `日期, 当日成交净买额, 买入成交额, 卖出成交额, 历史累计净买额, 当日资金流入,
/// 当日余额, 持股市值, 领涨股, 领涨股-涨跌幅, {指数}, {指数}-涨跌幅, 领涨股-代码`
///
/// 注：akshare 对数值列做了 `/100`（部分 `/100/10000`）缩放；本实现保留原始值，
/// 仅保证列名与数值类型同 akshare 对齐（parity 采用 loose 模式，不比对数值）。
pub fn stock_hsgt_hist_em(symbol: &str) -> Result<Df> {
    let suffix = match symbol {
        "北向资金" => "5",
        "沪股通" => "1",
        "深股通" => "3",
        "南向资金" => "6",
        "港股通沪" => "2",
        "港股通深" => "4",
        other => return Err(AkshareError::Param(format!("未知 symbol: {other}"))),
    };
    let index_name = match symbol {
        "北向资金" | "南向资金" => "沪深300",
        "沪股通" => "上证指数",
        "深股通" => "深证指数",
        "港股通沪" | "港股通深" => "恒生指数",
        _ => "沪深300",
    };
    let filter = format!(r#"(MUTUAL_TYPE="00{suffix}")"#);
    let extra = report_extra("TRADE_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_MUTUAL_DEAL_HISTORY", "ALL", &extra, "1000")?;
    let mut df = finalize_report(&rows, &HIST_RNAME, &HIST_SELECT, &HIST_NUMERIC, None)?;
    df.cast_date(&HIST_DATE)?;
    let names: Vec<String> = HIST_SELECT
        .iter()
        .map(|c| {
            if *c == "__INDEX__" {
                index_name.to_string()
            } else if *c == "__INDEXCHG__" {
                format!("{index_name}-涨跌幅")
            } else {
                (*c).to_string()
            }
        })
        .collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    df.rename_columns(&refs)?;
    Ok(df)
}

// ===== 沪深港通板块排行（RPT_MUTUAL_BOARD_HOLDRANK_WEB）=====
// 对应 akshare [`akshare.stock_hsgt_board_rank_em`]。
// 多分支：symbol → BOARD_TYPE（行业5/概念4/地域3）；indicator → INTERVAL_TYPE。
// 列序已与实时 35 键 JSON 核对。
// 注：akshare 原版位置重命名表与当前实时 schema 长度不一致（原版已失效），
// 本实现按语义映射；`今日增持/减持最大股` 两对列对应实时 schema 中
// 最大/最小增持个股名称字段（akshare 原版亦误标为“市值/占比”），故保留为字符串。
const BOARD_RENAME: [(&str, &str); 16] = [
    ("BOARD_NAME", "名称"),
    ("INDEX_CHANGE_RATIO", "最新涨跌幅"),
    ("COMPOSITION_QUANTITY", "北向资金今日持股-股票只数"),
    ("HK_VALUE", "北向资金今日持股-市值"),
    ("BOARD_HK_RATIO", "北向资金今日持股-占板块比"),
    ("HK_BOARD_RATIO", "北向资金今日持股-占北向资金比"),
    ("COMPOSITION_QUANTITY_ADD", "北向资金今日增持估计-股票只数"),
    ("ADD_MARKET_CAP", "北向资金今日增持估计-市值"),
    ("ADD_RATIO", "北向资金今日增持估计-市值增幅"),
    ("ADD_HK_RATIO", "北向资金今日增持估计-占板块比"),
    ("ADD_BOARD_RATIO", "北向资金今日增持估计-占北向资金比"),
    ("MAXADD_SECURITY_NAME", "今日增持最大股-市值"),
    ("MAXADD_RATIO_SECURITY_NAME", "今日增持最大股-占总市值比"),
    ("MINADD_SECURITY_NAME", "今日减持最大股-市值"),
    ("MINADD_RATIO_SECURITY_NAME", "今日减持最大股-占总市值比"),
    ("TRADE_DATE", "报告时间"),
];
const BOARD_SELECT: [&str; 16] = [
    "名称",
    "最新涨跌幅",
    "北向资金今日持股-股票只数",
    "北向资金今日持股-市值",
    "北向资金今日持股-占板块比",
    "北向资金今日持股-占北向资金比",
    "北向资金今日增持估计-股票只数",
    "北向资金今日增持估计-市值",
    "北向资金今日增持估计-市值增幅",
    "北向资金今日增持估计-占板块比",
    "北向资金今日增持估计-占北向资金比",
    "今日增持最大股-市值",
    "今日增持最大股-占总市值比",
    "今日减持最大股-市值",
    "今日减持最大股-占总市值比",
    "报告时间",
];
const BOARD_NUMERIC: [&str; 10] = [
    "最新涨跌幅",
    "北向资金今日持股-股票只数",
    "北向资金今日持股-市值",
    "北向资金今日持股-占板块比",
    "北向资金今日持股-占北向资金比",
    "北向资金今日增持估计-股票只数",
    "北向资金今日增持估计-市值",
    "北向资金今日增持估计-市值增幅",
    "北向资金今日增持估计-占板块比",
    "北向资金今日增持估计-占北向资金比",
];
const BOARD_DATE: [&str; 1] = ["报告时间"];

/// 沪深港通板块排行（对应 akshare [`akshare.stock_hsgt_board_rank_em`]）。
///
/// `symbol`：`北向资金增持行业板块排行` / `北向资金增持概念板块排行` / `北向资金增持地域板块排行`；
/// `indicator`：`今日` / `3日` / `5日` / `10日` / `1月` / `1季` / `1年`；
/// `date`：交易日期 `YYYYMMDD`（原 akshare 从 HTML `bkph_date` 抓取，此处显式传入）。
///
/// # 返回列
/// `序号, 名称, 最新涨跌幅, 北向资金今日持股-股票只数, 北向资金今日持股-市值,
/// 北向资金今日持股-占板块比, 北向资金今日持股-占北向资金比, 北向资金今日增持估计-股票只数,
/// 北向资金今日增持估计-市值, 北向资金今日增持估计-市值增幅, 北向资金今日增持估计-占板块比,
/// 北向资金今日增持估计-占北向资金比, 今日增持最大股-市值, 今日增持最大股-占总市值比,
/// 今日减持最大股-市值, 今日减持最大股-占总市值比, 报告时间`
pub fn stock_hsgt_board_rank_em(symbol: &str, indicator: &str, date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let bt = match symbol {
        "北向资金增持行业板块排行" => "5",
        "北向资金增持概念板块排行" => "4",
        "北向资金增持地域板块排行" => "3",
        other => return Err(AkshareError::Param(format!("未知 symbol: {other}"))),
    };
    let it = match indicator {
        "今日" => "1",
        "3日" => "3",
        "5日" => "5",
        "10日" => "10",
        "1月" => "M",
        "1季" => "Q",
        "1年" => "Y",
        other => return Err(AkshareError::Param(format!("未知 indicator: {other}"))),
    };
    let filter = format!(r#"(BOARD_TYPE="{bt}")(TRADE_DATE='{d}')(INTERVAL_TYPE="{it}")"#);
    let extra = report_extra(
        "ADD_MARKET_CAP",
        "-1",
        Some(&filter),
        Some("f3~05~SECURITY_CODE~INDEX_CHANGE_RATIO"),
        None,
        None,
    );
    let rows = datacenter("RPT_MUTUAL_BOARD_HOLDRANK_WEB", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &BOARD_RENAME,
        &BOARD_SELECT,
        &BOARD_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&BOARD_DATE)?;
    Ok(df)
}

// ===== 沪深港通个股持股（港股通，RPT_MUTUAL_STOCK_HOLDRANKS）=====
// 对应 akshare [`akshare.stock_hsgt_individual_em`] 港股代码分支（len(symbol)!=6）。
// 列序已与实时 JSON 键序核对。
const INDIV_RNAME: [(&str, &str); 9] = [
    ("TRADE_DATE", "持股日期"),
    ("CLOSE_PRICE", "当日收盘价"),
    ("CHANGE_RATE", "当日涨跌幅"),
    ("HOLD_SHARES", "持股数量"),
    ("HOLD_MARKET_CAP", "持股市值"),
    ("HOLD_SHARES_RATIO", "持股数量占A股百分比"),
    ("HOLD_MARKETCAP_CHG1", "持股市值变化-1日"),
    ("HOLD_MARKETCAP_CHG5", "持股市值变化-5日"),
    ("HOLD_MARKETCAP_CHG10", "持股市值变化-10日"),
];
const INDIV_SELECT: [&str; 9] = [
    "持股日期",
    "当日收盘价",
    "当日涨跌幅",
    "持股数量",
    "持股市值",
    "持股数量占A股百分比",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const INDIV_NUMERIC: [&str; 8] = [
    "当日收盘价",
    "当日涨跌幅",
    "持股数量",
    "持股市值",
    "持股数量占A股百分比",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const INDIV_DATE: [&str; 1] = ["持股日期"];

/// 沪深港通个股持股（港股通，对应 akshare [`akshare.stock_hsgt_individual_em`] 港股代码分支）。
///
/// `symbol`：港股代码（如 `"00700"`，内部拼 `.HK`）。对应 report `RPT_MUTUAL_STOCK_HOLDRANKS`。
///
/// # 返回列
/// `持股日期, 当日收盘价, 当日涨跌幅, 持股数量, 持股市值, 持股数量占A股百分比,
/// 持股市值变化-1日, 持股市值变化-5日, 持股市值变化-10日`
pub fn stock_hsgt_individual_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECUCODE="{symbol}.HK")(MUTUAL_TYPE="002")"#);
    let extra = report_extra("TRADE_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_MUTUAL_STOCK_HOLDRANKS", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &INDIV_RNAME, &INDIV_SELECT, &INDIV_NUMERIC, None)?;
    df.cast_date(&INDIV_DATE)?;
    Ok(df)
}

// ===== 沪深港通个股持股详情（RPT_MUTUAL_HOLD_DET）=====
// 对应 akshare [`akshare.stock_hsgt_individual_detail_em`]。
// 优先 MARKET_CODE="003"（深股通），无数据则回退 "001"（沪股通）。
// 列序已与实时 JSON 键序核对。
const INDDET_RNAME: [(&str, &str); 10] = [
    ("HOLD_DATE", "持股日期"),
    ("CLOSE_PRICE", "当日收盘价"),
    ("CHANGE_RATE", "当日涨跌幅"),
    ("ORG_NAME", "机构名称"),
    ("HOLD_NUM", "持股数量"),
    ("HOLD_MARKET_CAP", "持股市值"),
    ("HOLD_SHARES_RATIO", "持股数量占A股百分比"),
    ("HOLD_MARKET_CAPONE", "持股市值变化-1日"),
    ("HOLD_MARKET_CAPFIVE", "持股市值变化-5日"),
    ("HOLD_MARKET_CAPTEN", "持股市值变化-10日"),
];
const INDDET_SELECT: [&str; 10] = [
    "持股日期",
    "当日收盘价",
    "当日涨跌幅",
    "机构名称",
    "持股数量",
    "持股市值",
    "持股数量占A股百分比",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const INDDET_NUMERIC: [&str; 8] = [
    "当日收盘价",
    "当日涨跌幅",
    "持股数量",
    "持股市值",
    "持股数量占A股百分比",
    "持股市值变化-1日",
    "持股市值变化-5日",
    "持股市值变化-10日",
];
const INDDET_DATE: [&str; 1] = ["持股日期"];

/// 沪深港通个股持股详情（对应 akshare [`akshare.stock_hsgt_individual_detail_em`]）。
///
/// `symbol`：A 股代码；`start_date` / `end_date`：起止日期 `YYYYMMDD`。
/// 优先 `MARKET_CODE="003"`（深股通），无数据则回退 `MARKET_CODE="001"`（沪股通）。
///
/// # 返回列
/// `持股日期, 当日收盘价, 当日涨跌幅, 机构名称, 持股数量, 持股市值,
/// 持股数量占A股百分比, 持股市值变化-1日, 持股市值变化-5日, 持股市值变化-10日`
pub fn stock_hsgt_individual_detail_em(
    symbol: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Df> {
    let s = fmt_ymd(start_date)?;
    let e = fmt_ymd(end_date)?;
    let build_filter = |mk: &str| {
        format!(
            r#"(SECURITY_CODE="{symbol}")(MARKET_CODE="{mk}")(HOLD_DATE>='{s}')(HOLD_DATE<='{e}')"#
        )
    };
    let mut rows = {
        let filter = build_filter("003");
        let extra = report_extra("HOLD_DATE", "-1", Some(&filter), None, None, None);
        datacenter("RPT_MUTUAL_HOLD_DET", "ALL", &extra, "500")?
    };
    if rows.is_empty() {
        let filter = build_filter("001");
        let extra = report_extra("HOLD_DATE", "-1", Some(&filter), None, None, None);
        rows = datacenter("RPT_MUTUAL_HOLD_DET", "ALL", &extra, "500")?;
    }
    let mut df = finalize_report(&rows, &INDDET_RNAME, &INDDET_SELECT, &INDDET_NUMERIC, None)?;
    df.cast_date(&INDDET_DATE)?;
    Ok(df)
}

/// 交易市场代码 → 中文名（对应 akshare `交易市场.map({...})`，仅商誉类报表用到）。
///
/// `key` 为原始 JSON 键名（`sy_yq` 用 `TRADE_MARKET`，`sy_jz` 用 `TRADE_BOARD`）；
/// 未知代码保持原值（对应 akshare `.map` 未命中 → NaN 兜底）。
fn map_trade_market(rows: &[Value], key: &str) -> Vec<Value> {
    rows.iter()
        .map(|r| {
            let mut r = r.clone();
            if let Some(obj) = r.as_object_mut() {
                if let Some(v) = obj.get_mut(key) {
                    if let Some(s) = v.as_str() {
                        let mapped = match s {
                            "shzb" => "沪市主板",
                            "kcb" => "科创板",
                            "szzb" => "深市主板",
                            "cyb" => "创业板",
                            other => other,
                        };
                        *v = Value::String(mapped.to_string());
                    }
                }
            }
            r
        })
        .collect()
}

// ===== 商誉-商誉减值预期明细（RPT_GOODWILL_STOCKPREDICT）=====
// 序号由 Rust 生成。列序参照 akshare rename + select 逐位对齐。
const SY_YQ_RENAME: [(&str, &str); 13] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("TRADE_MARKET", "交易市场"),
    ("NOTICE_DATE", "公告日期"),
    ("PREDICT_NETPROFIT_LOWER", "预计净利润-下限"),
    ("PREDICT_NETPROFIT_UPPER", "预计净利润-上限"),
    ("PERFORM_CHANGE_UPPER", "业绩变动幅度-上限"),
    ("PERFORM_CHANGE_LOWER", "业绩变动幅度-下限"),
    ("PERFORM_CHANGE_EXPLAIN", "业绩变动原因"),
    ("PE_SAMEREPORT_NETPROFIT", "上年度同期净利润"),
    ("PE_GOODWILL", "上年商誉"),
    ("NEWEST_REPORT_DATE", "最新商誉报告期"),
    ("NEWEST_GOODWILL", "最新一期商誉"),
];
const SY_YQ_SELECT: [&str; 13] = [
    "股票代码",
    "股票简称",
    "业绩变动原因",
    "最新商誉报告期",
    "最新一期商誉",
    "上年商誉",
    "预计净利润-下限",
    "预计净利润-上限",
    "业绩变动幅度-下限",
    "业绩变动幅度-上限",
    "上年度同期净利润",
    "公告日期",
    "交易市场",
];
const SY_YQ_NUMERIC: [&str; 7] = [
    "最新一期商誉",
    "上年商誉",
    "预计净利润-下限",
    "预计净利润-上限",
    "业绩变动幅度-下限",
    "业绩变动幅度-上限",
    "上年度同期净利润",
];
const SY_YQ_DATE: [&str; 2] = ["最新商誉报告期", "公告日期"];

/// 商誉-商誉减值预期明细（对应 akshare [`akshare.stock_sy_yq_em`]）。
///
/// `date`：数据日期 `YYYYMMDD`（默认 `"20240630"`）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 业绩变动原因, 最新商誉报告期, 最新一期商誉, 上年商誉,
/// 预计净利润-下限, 预计净利润-上限, 业绩变动幅度-下限, 业绩变动幅度-上限,
/// 上年度同期净利润, 公告日期, 交易市场`
pub fn stock_sy_yq_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(REPORT_DATE='{d}')");
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_GOODWILL_STOCKPREDICT", "ALL", &extra, "5000")?;
    let rows = map_trade_market(&rows, "TRADE_MARKET");
    let mut df = finalize_report(
        &rows,
        &SY_YQ_RENAME,
        &SY_YQ_SELECT,
        &SY_YQ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&SY_YQ_DATE)?;
    Ok(df)
}

// ===== 商誉-个股商誉减值明细（RPT_GOODWILL_STOCKDETAILS）=====
// 序号由 Rust 生成。列序参照 akshare rename + select 逐位对齐。
const SY_JZ_RENAME: [(&str, &str); 10] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("TRADE_BOARD", "交易市场"),
    ("GOODWILL", "商誉"),
    ("GOODWILL_CHANGE", "商誉减值"),
    ("SUMSHEQUITY_RATIO", "商誉占净资产比例"),
    ("SE_CHANGE_RATIO", "商誉减值占净资产比例"),
    ("PARENTNETPROFIT", "净利润"),
    ("PNP_CHANGE_RATIO", "商誉减值占净利润比例"),
    ("NOTICE_DATE", "公告日期"),
];
const SY_JZ_SELECT: [&str; 10] = [
    "股票代码",
    "股票简称",
    "商誉",
    "商誉减值",
    "商誉占净资产比例",
    "商誉减值占净资产比例",
    "净利润",
    "商誉减值占净利润比例",
    "公告日期",
    "交易市场",
];
const SY_JZ_NUMERIC: [&str; 6] = [
    "商誉",
    "商誉减值",
    "商誉占净资产比例",
    "商誉减值占净资产比例",
    "净利润",
    "商誉减值占净利润比例",
];
const SY_JZ_DATE: [&str; 1] = ["公告日期"];

/// 商誉-个股商誉减值明细（对应 akshare [`akshare.stock_sy_jz_em`]）。
///
/// `date`：数据日期 `YYYYMMDD`（默认 `"20240630"`）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 商誉, 商誉减值, 商誉占净资产比例, 商誉减值占净资产比例,
/// 净利润, 商誉减值占净利润比例, 公告日期, 交易市场`
pub fn stock_sy_jz_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(REPORT_DATE='{d}')");
    let extra = report_extra(
        "GOODWILL_CHANGE",
        "-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_GOODWILL_STOCKDETAILS", "ALL", &extra, "5000")?;
    let rows = map_trade_market(&rows, "TRADE_BOARD");
    let mut df = finalize_report(
        &rows,
        &SY_JZ_RENAME,
        &SY_JZ_SELECT,
        &SY_JZ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&SY_JZ_DATE)?;
    Ok(df)
}

// ===== 资产负债表（RPT_DMSK_FN_BALANCE）=====
// 序号由 Rust 生成（东财原始 JSON 无 index 键，与 akshare reset_index 一致）。
// RENAME 由 akshare 全量列位置表（位置 0 = 序号，之后逐位对应 columns=ALL 的 JSON 键序）
// 经实时拉取的 JSON 键序逐位核对得出：JSON 键 0-based 序号 k 对应位置 k+1。
// zcfz_em 与 zcfz_bj_em 共用同一张列契约（北交所仅过滤条件不同）。
const ZCFZ_RENAME: [(&str, &str); 14] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("MONETARYFUNDS", "资产-货币资金"),
    ("ACCOUNTS_RECE", "资产-应收账款"),
    ("INVENTORY", "资产-存货"),
    ("TOTAL_ASSETS", "资产-总资产"),
    ("TOTAL_ASSETS_RATIO", "资产-总资产同比"),
    ("ACCOUNTS_PAYABLE", "负债-应付账款"),
    ("ADVANCE_RECEIVABLES", "负债-预收账款"),
    ("TOTAL_LIABILITIES", "负债-总负债"),
    ("TOTAL_LIAB_RATIO", "负债-总负债同比"),
    ("DEBT_ASSET_RATIO", "资产负债率"),
    ("TOTAL_EQUITY", "股东权益合计"),
    ("NOTICE_DATE", "公告日期"),
];
const ZCFZ_SELECT: [&str; 14] = [
    "股票代码",
    "股票简称",
    "资产-货币资金",
    "资产-应收账款",
    "资产-存货",
    "资产-总资产",
    "资产-总资产同比",
    "负债-应付账款",
    "负债-预收账款",
    "负债-总负债",
    "负债-总负债同比",
    "资产负债率",
    "股东权益合计",
    "公告日期",
];
const ZCFZ_NUMERIC: [&str; 11] = [
    "资产-货币资金",
    "资产-应收账款",
    "资产-存货",
    "资产-总资产",
    "资产-总资产同比",
    "负债-应付账款",
    "负债-预收账款",
    "负债-总负债",
    "负债-总负债同比",
    "资产负债率",
    "股东权益合计",
];
const ZCFZ_DATE: [&str; 1] = ["公告日期"];

/// 资产负债表（对应 akshare [`akshare.stock_zcfz_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20240331"`），剔除北交所（`069001017`）与
/// 非 A 股类型（`058001001`/`058001008`）。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 资产-货币资金, 资产-应收账款, 资产-存货, 资产-总资产,
/// 资产-总资产同比, 负债-应付账款, 负债-预收账款, 负债-总负债, 负债-总负债同比,
/// 资产负债率, 股东权益合计, 公告日期`
pub fn stock_zcfz_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(
        "(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE!=\"069001017\")(REPORT_DATE='{d}')"
    );
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_DMSK_FN_BALANCE", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &ZCFZ_RENAME,
        &ZCFZ_SELECT,
        &ZCFZ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&ZCFZ_DATE)?;
    Ok(df)
}

/// 资产负债表-北交所（对应 akshare [`akshare.stock_zcfz_bj_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20240331"`），仅北交所（`069001017`）。
/// 与 [`stock_zcfz_em`] 共用同一张列契约，仅过滤条件不同。
///
/// # 返回列
/// 与 [`stock_zcfz_em`] 一致。
pub fn stock_zcfz_bj_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(TRADE_MARKET_CODE=\"069001017\")(REPORT_DATE='{d}')");
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_DMSK_FN_BALANCE", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &ZCFZ_RENAME,
        &ZCFZ_SELECT,
        &ZCFZ_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&ZCFZ_DATE)?;
    Ok(df)
}

// ===== 利润表（RPT_DMSK_FN_INCOME）=====
// 序号由 Rust 生成。RENAME 由 akshare 全量列位置表（含 lrb 同比列 known off-by-one）
// 经实时拉取的 JSON 键序逐位核对得出；按位置派生即自动复现该 quirks，切勿“修正”。
const LRB_RENAME: [(&str, &str); 14] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("PARENT_NETPROFIT", "净利润"),
    ("PARENT_NETPROFIT_RATIO", "净利润同比"),
    ("TOTAL_OPERATE_INCOME", "营业总收入"),
    ("TOI_RATIO", "营业总收入同比"),
    ("OPERATE_COST", "营业总支出-营业支出"),
    ("SALE_EXPENSE", "营业总支出-销售费用"),
    ("MANAGE_EXPENSE", "营业总支出-管理费用"),
    ("FINANCE_EXPENSE", "营业总支出-财务费用"),
    ("TOTAL_OPERATE_COST", "营业总支出-营业总支出"),
    ("OPERATE_PROFIT", "营业利润"),
    ("TOTAL_PROFIT", "利润总额"),
    ("NOTICE_DATE", "公告日期"),
];
const LRB_SELECT: [&str; 14] = [
    "股票代码",
    "股票简称",
    "净利润",
    "净利润同比",
    "营业总收入",
    "营业总收入同比",
    "营业总支出-营业支出",
    "营业总支出-销售费用",
    "营业总支出-管理费用",
    "营业总支出-财务费用",
    "营业总支出-营业总支出",
    "营业利润",
    "利润总额",
    "公告日期",
];
const LRB_NUMERIC: [&str; 11] = [
    "净利润",
    "净利润同比",
    "营业总收入",
    "营业总收入同比",
    "营业总支出-营业支出",
    "营业总支出-销售费用",
    "营业总支出-管理费用",
    "营业总支出-财务费用",
    "营业总支出-营业总支出",
    "营业利润",
    "利润总额",
];
const LRB_DATE: [&str; 1] = ["公告日期"];

/// 利润表（对应 akshare [`akshare.stock_lrb_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20240331"`），剔除北交所与非 A 股类型。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 净利润, 净利润同比, 营业总收入, 营业总收入同比,
/// 营业总支出-营业支出, 营业总支出-销售费用, 营业总支出-管理费用, 营业总支出-财务费用,
/// 营业总支出-营业总支出, 营业利润, 利润总额, 公告日期`
pub fn stock_lrb_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(
        "(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE!=\"069001017\")(REPORT_DATE='{d}')"
    );
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_DMSK_FN_INCOME", "ALL", &extra, "500")?;
    let mut df = finalize_report(&rows, &LRB_RENAME, &LRB_SELECT, &LRB_NUMERIC, Some("序号"))?;
    df.cast_date(&LRB_DATE)?;
    Ok(df)
}

// ===== 现金流量表（RPT_DMSK_FN_CASHFLOW）=====
// 序号由 Rust 生成。RENAME 由 akshare 全量列位置表经实时拉取的 JSON 键序逐位核对得出。
const XJLL_RENAME: [(&str, &str); 11] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票简称"),
    ("CCE_ADD", "净现金流-净现金流"),
    ("CCE_ADD_RATIO", "净现金流-同比增长"),
    ("NETCASH_OPERATE", "经营性现金流-现金流量净额"),
    ("NETCASH_OPERATE_RATIO", "经营性现金流-净现金流占比"),
    ("NETCASH_INVEST", "投资性现金流-现金流量净额"),
    ("NETCASH_INVEST_RATIO", "投资性现金流-净现金流占比"),
    ("NETCASH_FINANCE", "融资性现金流-现金流量净额"),
    ("NETCASH_FINANCE_RATIO", "融资性现金流-净现金流占比"),
    ("NOTICE_DATE", "公告日期"),
];
const XJLL_SELECT: [&str; 11] = [
    "股票代码",
    "股票简称",
    "净现金流-净现金流",
    "净现金流-同比增长",
    "经营性现金流-现金流量净额",
    "经营性现金流-净现金流占比",
    "投资性现金流-现金流量净额",
    "投资性现金流-净现金流占比",
    "融资性现金流-现金流量净额",
    "融资性现金流-净现金流占比",
    "公告日期",
];
const XJLL_NUMERIC: [&str; 8] = [
    "净现金流-净现金流",
    "净现金流-同比增长",
    "经营性现金流-现金流量净额",
    "经营性现金流-净现金流占比",
    "投资性现金流-现金流量净额",
    "投资性现金流-净现金流占比",
    "融资性现金流-现金流量净额",
    "融资性现金流-净现金流占比",
];
const XJLL_DATE: [&str; 1] = ["公告日期"];

/// 现金流量表（对应 akshare [`akshare.stock_xjll_em`]）。
///
/// `date`：报告期 `YYYYMMDD`（默认 `"20240331"`），剔除北交所与非 A 股类型。
///
/// # 返回列
/// `序号, 股票代码, 股票简称, 净现金流-净现金流, 净现金流-同比增长, 经营性现金流-现金流量净额,
/// 经营性现金流-净现金流占比, 投资性现金流-现金流量净额, 投资性现金流-净现金流占比,
/// 融资性现金流-现金流量净额, 融资性现金流-净现金流占比, 公告日期`
pub fn stock_xjll_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!(
        "(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE!=\"069001017\")(REPORT_DATE='{d}')"
    );
    let extra = report_extra(
        "NOTICE_DATE,SECURITY_CODE",
        "-1,-1",
        Some(&filter),
        None,
        None,
        None,
    );
    let rows = datacenter("RPT_DMSK_FN_CASHFLOW", "ALL", &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &XJLL_RENAME,
        &XJLL_SELECT,
        &XJLL_NUMERIC,
        Some("序号"),
    )?;
    df.cast_date(&XJLL_DATE)?;
    Ok(df)
}

// ===== 股权质押-质押机构分布统计（RPT_GDZY_ZYJG_SUM）=====
// 序号由 Rust 生成。列序参照 akshare `big_df.columns`（columns=ALL，序号占 0 位，
// 原始 JSON 键紧随其后）与实时拉取的 JSON 键序逐位对齐：JSON 键 0-based 序号 k 对应位置 k+1。
// 该报表 `PFORG_TYPE` 实际取值为 `证券Ⅱ`/`银行Ⅱ`，akshare 原版过滤 `证券`/`银行` 已无数据，
// 此处原样镜像 akshare 过滤条件（返回空表与之等价）。
const GPZY_DIST_RENAME: [(&str, &str); 7] = [
    ("SECURITY_NAME_ABBR", "质押机构"),
    ("ORG_NUM", "质押公司数量"),
    ("PLEDGE_DEAL_NUM", "质押笔数"),
    ("PLEDGE_NUM", "质押数量"),
    ("WARNING_STATE_1_RATE", "未达预警线比例"),
    ("WARNING_STATE_2_RATE", "达到预警线未达平仓线比例"),
    ("WARNING_STATE_3_RATE", "达到平仓线比例"),
];
const GPZY_DIST_SELECT: [&str; 7] = [
    "质押机构",
    "质押公司数量",
    "质押笔数",
    "质押数量",
    "未达预警线比例",
    "达到预警线未达平仓线比例",
    "达到平仓线比例",
];
const GPZY_DIST_NUMERIC: [&str; 6] = [
    "质押公司数量",
    "质押笔数",
    "质押数量",
    "未达预警线比例",
    "达到预警线未达平仓线比例",
    "达到平仓线比例",
];

/// 股权质押-质押机构分布统计-证券公司（对应 akshare [`akshare.stock_gpzy_distribute_statistics_company_em`]）。
///
/// 无参数；序号由 Rust 生成。
///
/// # 返回列
/// `序号, 质押机构, 质押公司数量, 质押笔数, 质押数量, 未达预警线比例,
/// 达到预警线未达平仓线比例, 达到平仓线比例`
pub fn stock_gpzy_distribute_statistics_company_em() -> Result<Df> {
    let extra = report_extra(
        "ORG_NUM",
        "-1",
        Some(r#"(PFORG_TYPE="证券")"#),
        Some(""),
        None,
        None,
    );
    let rows = datacenter("RPT_GDZY_ZYJG_SUM", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GPZY_DIST_RENAME,
        &GPZY_DIST_SELECT,
        &GPZY_DIST_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

/// 股权质押-质押机构分布统计-银行（对应 akshare [`akshare.stock_gpzy_distribute_statistics_bank_em`]）。
///
/// 无参数；序号由 Rust 生成。
///
/// # 返回列
/// 与 [`stock_gpzy_distribute_statistics_company_em`] 一致。
pub fn stock_gpzy_distribute_statistics_bank_em() -> Result<Df> {
    let extra = report_extra(
        "ORG_NUM",
        "-1",
        Some(r#"(PFORG_TYPE="银行")"#),
        Some(""),
        None,
        None,
    );
    let rows = datacenter("RPT_GDZY_ZYJG_SUM", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GPZY_DIST_RENAME,
        &GPZY_DIST_SELECT,
        &GPZY_DIST_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 股东户数详情（RPT_HOLDERNUM_DET）=====
// 无 序号。列序参照 akshare `big_df.columns`（columns=显式 17 键 + quoteColumns f2,f3；
// 服务端对重复 END_DATE 去重，故返回 18 键）与实时拉取的 JSON 键序逐位对齐。
// akshare 将末端 3 个位置（PRE_END_DATE/f2/f3）命名为 `_` 后丢弃，本实现仅映射并选择保留列。
const GDHS_DETAIL_RENAME: [(&str, &str); 15] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "名称"),
    ("CHANGE_SHARES", "股本变动"),
    ("CHANGE_REASON", "股本变动原因"),
    ("END_DATE", "股东户数统计截止日"),
    ("INTERVAL_CHRATE", "区间涨跌幅"),
    ("AVG_MARKET_CAP", "户均持股市值"),
    ("AVG_HOLD_NUM", "户均持股数量"),
    ("TOTAL_MARKET_CAP", "总市值"),
    ("TOTAL_A_SHARES", "总股本"),
    ("HOLD_NOTICE_DATE", "股东户数公告日期"),
    ("HOLDER_NUM", "股东户数-本次"),
    ("PRE_HOLDER_NUM", "股东户数-上次"),
    ("HOLDER_NUM_CHANGE", "股东户数-增减"),
    ("HOLDER_NUM_RATIO", "股东户数-增减比例"),
];
const GDHS_DETAIL_SELECT: [&str; 15] = [
    "股东户数统计截止日",
    "区间涨跌幅",
    "股东户数-本次",
    "股东户数-上次",
    "股东户数-增减",
    "股东户数-增减比例",
    "户均持股市值",
    "户均持股数量",
    "总市值",
    "总股本",
    "股本变动",
    "股本变动原因",
    "股东户数公告日期",
    "代码",
    "名称",
];
const GDHS_DETAIL_NUMERIC: [&str; 10] = [
    "区间涨跌幅",
    "股东户数-本次",
    "股东户数-上次",
    "股东户数-增减",
    "股东户数-增减比例",
    "户均持股市值",
    "户均持股数量",
    "总市值",
    "总股本",
    "股本变动",
];
const GDHS_DETAIL_DATE: [&str; 2] = ["股东户数统计截止日", "股东户数公告日期"];

/// 股东户数详情（对应 akshare [`akshare.stock_zh_a_gdhs_detail_em`]）。
///
/// `symbol`：股票代码（如 `"000001"`），按 `SECURITY_CODE` 过滤。
/// 通过 `quoteColumns` 注入最新价/涨跌幅，但 akshare 输出不保留这两列，本实现同样丢弃。
/// 无 序号 列。
///
/// # 返回列
/// `股东户数统计截止日, 区间涨跌幅, 股东户数-本次, 股东户数-上次, 股东户数-增减,
/// 股东户数-增减比例, 户均持股市值, 户均持股数量, 总市值, 总股本, 股本变动,
/// 股本变动原因, 股东户数公告日期, 代码, 名称`
pub fn stock_zh_a_gdhs_detail_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let columns = "SECURITY_CODE,SECURITY_NAME_ABBR,CHANGE_SHARES,CHANGE_REASON,END_DATE,INTERVAL_CHRATE,AVG_MARKET_CAP,AVG_HOLD_NUM,TOTAL_MARKET_CAP,TOTAL_A_SHARES,HOLD_NOTICE_DATE,HOLDER_NUM,PRE_HOLDER_NUM,HOLDER_NUM_CHANGE,HOLDER_NUM_RATIO,END_DATE,PRE_END_DATE";
    let extra = report_extra("END_DATE", "-1", Some(&filter), Some("f2,f3"), None, None);
    let rows = datacenter("RPT_HOLDERNUM_DET", columns, &extra, "500")?;
    let mut df = finalize_report(
        &rows,
        &GDHS_DETAIL_RENAME,
        &GDHS_DETAIL_SELECT,
        &GDHS_DETAIL_NUMERIC,
        None,
    )?;
    df.cast_date(&GDHS_DETAIL_DATE)?;
    Ok(df)
}

// ===== 股东协同-十大流通股东/十大股东（RPT_COOPFREEHOLDER / RPT_TENHOLDERS_COOPHOLDERS）=====
// 序号由 Rust 生成。两报表 JSON 键序一致，共用同一套 RENAME/SELECT/NUMERIC。
// 列序参照 akshare `big_df.columns`（columns=ALL）与实时拉取的 JSON 键序逐位对齐。无日期列。
const GDFX_TEAM_RENAME: [(&str, &str); 6] = [
    ("HOLDER_NAME", "股东名称"),
    ("HOLDER_TYPE", "股东类型"),
    ("COOPERAT_HOLDER_NAME", "协同股东名称"),
    ("COOPERAT_HOLDER_TYPE", "协同股东类型"),
    ("COOPERAT_NUM", "协同次数"),
    ("PINGJIE", "个股详情"),
];
const GDFX_TEAM_SELECT: [&str; 6] = [
    "股东名称",
    "股东类型",
    "协同股东名称",
    "协同股东类型",
    "协同次数",
    "个股详情",
];
const GDFX_TEAM_NUMERIC: [&str; 1] = ["协同次数"];

/// 股东协同-十大流通股东（对应 akshare [`akshare.stock_gdfx_free_holding_teamwork_em`]）。
///
/// `symbol`：股东类型，默认 `"全部"`；亦可取 `个人`/`基金`/`QFII`/`社保`/`券商`/`信托`。
/// 仅当 `symbol != "全部"` 时按 `HOLDER_TYPE` 过滤。序号由 Rust 生成。
///
/// # 返回列
/// `序号, 股东名称, 股东类型, 协同股东名称, 协同股东类型, 协同次数, 个股详情`
pub fn stock_gdfx_free_holding_teamwork_em(symbol: &str) -> Result<Df> {
    let filter = if symbol == "全部" {
        None
    } else {
        Some(format!(r#"(HOLDER_TYPE="{symbol}")"#))
    };
    let extra = report_extra(
        "COOPERAT_NUM,HOLDER_NEW,COOPERAT_HOLDER_NEW",
        "-1,-1,-1",
        filter.as_deref(),
        Some(""),
        None,
        None,
    );
    let rows = datacenter("RPT_COOPFREEHOLDER", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GDFX_TEAM_RENAME,
        &GDFX_TEAM_SELECT,
        &GDFX_TEAM_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

/// 股东协同-十大股东（对应 akshare [`akshare.stock_gdfx_holding_teamwork_em`]）。
///
/// `symbol`：股东类型，默认 `"全部"`；其余取值同 [`stock_gdfx_free_holding_teamwork_em`]。
/// 仅当 `symbol != "全部"` 时按 `HOLDER_TYPE` 过滤。序号由 Rust 生成。
///
/// # 返回列
/// 与 [`stock_gdfx_free_holding_teamwork_em`] 一致。
pub fn stock_gdfx_holding_teamwork_em(symbol: &str) -> Result<Df> {
    let filter = if symbol == "全部" {
        None
    } else {
        Some(format!(r#"(HOLDER_TYPE="{symbol}")"#))
    };
    let extra = report_extra(
        "COOPERAT_NUM,HOLDER_NEW,COOPERAT_HOLDER_NEW",
        "-1,-1,-1",
        filter.as_deref(),
        Some(""),
        None,
        None,
    );
    let rows = datacenter("RPT_TENHOLDERS_COOPHOLDERS", "ALL", &extra, "500")?;
    let df = finalize_report(
        &rows,
        &GDFX_TEAM_RENAME,
        &GDFX_TEAM_SELECT,
        &GDFX_TEAM_NUMERIC,
        Some("序号"),
    )?;
    Ok(df)
}

// ===== 千股千评-市场热度-用户关注指数（RPT_STOCK_MARKETFOCUS）=====
// 无 序号（akshare 仅 rename + 选择，无 index 列）。键名直接映射。
const COMMENT_FOCUS_RENAME: [(&str, &str); 2] =
    [("TRADE_DATE", "交易日"), ("MARKET_FOCUS", "用户关注指数")];
const COMMENT_FOCUS_SELECT: [&str; 2] = ["交易日", "用户关注指数"];
const COMMENT_FOCUS_NUMERIC: [&str; 1] = ["用户关注指数"];
const COMMENT_FOCUS_DATE: [&str; 1] = ["交易日"];

/// 千股千评-市场热度-用户关注指数（对应 akshare [`akshare.stock_comment_detail_scrd_focus_em`]）。
///
/// `symbol`：股票代码（如 `"600000"`），按 `SECURITY_CODE` 过滤。无 序号 列。
///
/// # 返回列
/// `交易日, 用户关注指数`
pub fn stock_comment_detail_scrd_focus_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let extra = report_extra("TRADE_DATE", "-1", Some(&filter), None, None, None);
    let rows = datacenter("RPT_STOCK_MARKETFOCUS", "ALL", &extra, "30")?;
    let mut df = finalize_report(
        &rows,
        &COMMENT_FOCUS_RENAME,
        &COMMENT_FOCUS_SELECT,
        &COMMENT_FOCUS_NUMERIC,
        None,
    )?;
    df.cast_date(&COMMENT_FOCUS_DATE)?;
    Ok(df)
}

// ===== 千股千评-市场热度-市场参与意愿（RPT_STOCK_PARTICIPATION）=====
// 无 序号（akshare 仅 rename + 选择，无 index 列）。键名直接映射。
// 东财该接口携带 `callback` 时以 JSONP（`callback(...);`）包裹返回，已在
// `fetch_datacenter_pages` 内剥离外层后再解析。
const COMMENT_DESIRE_RENAME: [(&str, &str); 7] = [
    ("SECURITY_INNER_CODE", "内部代码"),
    ("SECURITY_CODE", "股票代码"),
    ("TRADE_DATE", "交易日期"),
    ("PARTICIPATION_WISH", "参与意愿"),
    ("PARTICIPATION_WISH_5DAYS", "5日平均参与意愿"),
    ("PARTICIPATION_WISH_CHANGE", "参与意愿变化"),
    ("PARTICIPATION_WISH_5DAYSCHANGE", "5日平均变化"),
];
const COMMENT_DESIRE_SELECT: [&str; 6] = [
    "交易日期",
    "股票代码",
    "参与意愿",
    "5日平均参与意愿",
    "参与意愿变化",
    "5日平均变化",
];
const COMMENT_DESIRE_NUMERIC: [&str; 4] =
    ["参与意愿", "5日平均参与意愿", "参与意愿变化", "5日平均变化"];
const COMMENT_DESIRE_DATE: [&str; 1] = ["交易日期"];

/// 千股千评-市场热度-市场参与意愿（对应 akshare [`akshare.stock_comment_detail_scrd_desire_em`]）。
///
/// `symbol`：股票代码（如 `"600000"`），按 `SECURITY_CODE` 过滤。无 序号 列，
/// 内部代码列被丢弃（与 akshare `del temp_df["内部代码"]` 一致）。
///
/// # 返回列
/// `交易日期, 股票代码, 参与意愿, 5日平均参与意愿, 参与意愿变化, 5日平均变化`
pub fn stock_comment_detail_scrd_desire_em(symbol: &str) -> Result<Df> {
    let filter = format!(r#"(SECURITY_CODE="{symbol}")"#);
    let mut extra = report_extra("TRADE_DATE", "-1", Some(&filter), None, None, None);
    // 触发东财 JSONP 包裹返回（akshare 同样发送 callback），由 fetch 层剥离。
    extra.insert("callback".to_string(), json!("jQuery_parity_jsonp"));
    let rows = datacenter("RPT_STOCK_PARTICIPATION", "ALL", &extra, "30")?;
    let mut df = finalize_report(
        &rows,
        &COMMENT_DESIRE_RENAME,
        &COMMENT_DESIRE_SELECT,
        &COMMENT_DESIRE_NUMERIC,
        None,
    )?;
    df.cast_date(&COMMENT_DESIRE_DATE)?;
    Ok(df)
}

// ===== 商誉-行业商誉（RPT_GOODWILL_INDUSTATISTICS）=====
// 无 序号（akshare 仅 rename + 选择，无 index 列）。键名直接映射；行业代码/商誉减值/
// 商誉减值占净资产比例/商誉减值占净利润比例 列被丢弃。输出不含日期列。
const SY_HY_RENAME: [(&str, &str); 6] = [
    ("INDUSTRY_NAME", "行业名称"),
    ("ORG_NUM", "公司家数"),
    ("GOODWILL", "商誉规模"),
    ("SUMSHEQUITY", "净资产"),
    ("SUMSHEQUITY_RATIO", "商誉规模占净资产规模比例"),
    ("PARENTNETPROFIT", "净利润规模"),
];
const SY_HY_SELECT: [&str; 6] = [
    "行业名称",
    "公司家数",
    "商誉规模",
    "净资产",
    "商誉规模占净资产规模比例",
    "净利润规模",
];
const SY_HY_NUMERIC: [&str; 5] = [
    "公司家数",
    "商誉规模",
    "净资产",
    "商誉规模占净资产规模比例",
    "净利润规模",
];

/// 商誉-行业商誉（对应 akshare [`akshare.stock_sy_hy_em`]）。
///
/// `date`：数据日期 `YYYYMMDD`（如 `"20240930"`），按 `REPORT_DATE` 过滤（需 `token: EM_TOKEN`）。
/// 无 序号 列，输出不含日期列（akshare 丢弃 `数据日期`）。
///
/// # 返回列
/// `行业名称, 公司家数, 商誉规模, 净资产, 商誉规模占净资产规模比例, 净利润规模`
pub fn stock_sy_hy_em(date: &str) -> Result<Df> {
    let d = fmt_ymd(date)?;
    let filter = format!("(REPORT_DATE='{d}')");
    let extra = report_extra(
        "SUMSHEQUITY_RATIO",
        "-1",
        Some(&filter),
        None,
        Some(EM_TOKEN),
        None,
    );
    let rows = datacenter("RPT_GOODWILL_INDUSTATISTICS", "ALL", &extra, "5000")?;
    let df = finalize_report(&rows, &SY_HY_RENAME, &SY_HY_SELECT, &SY_HY_NUMERIC, None)?;
    Ok(df)
}

// ===== 新股申购与中签查询（stock_xgsglb_em）=====
// 北交所分支：RPT_NEEQ_ISSUEINFO_LIST（source=NEEQSELECT，需 quoteColumns 补充简称），无 序号列。
// 其余分支：RPTA_APP_IPOAPPLY（source=WEB），按 symbol 选择市场过滤，无 序号列。
// 列名逐字对齐 akshare（akshare 对该函数使用 key 重命名 + 位置选择，无 序号）。
const XGSG_NEEQ_QUOTE: &str = "f14~01~SECURITY_CODE~SECURITY_NAME_ABBR";
const XGSG_NEEQ_RENAME: [(&str, &str); 22] = [
    ("SECURITY_CODE", "代码"),
    ("SECURITY_NAME_ABBR", "简称"),
    ("APPLY_CODE", "申购代码"),
    ("EXPECT_ISSUE_NUM", "发行总数"),
    ("ONLINE_ISSUE_NUM", "网上-发行数量"),
    ("APPLY_NUM_UPPER", "网上-申购上限"),
    ("APPLY_AMT_UPPER", "网上-顶格所需资金"),
    ("ISSUE_PRICE", "发行价格"),
    ("APPLY_DATE", "申购日"),
    ("ONLINE_ISSUE_LWR", "中签率"),
    ("APPLY_AMT_100", "稳获百股需配资金"),
    ("NEWEST_PRICE", "最新价格-价格"),
    // 最新价格-累计涨幅 为计算列 = 首日收盘价 / 最新价格-价格，预写入每行的 COMPUTED_CUMCHG 键
    ("COMPUTED_CUMCHG", "最新价格-累计涨幅"),
    ("SELECT_LISTING_DATE", "上市首日-上市日"),
    ("AVERAGE_PRICE", "上市首日-均价"),
    ("LD_CLOSE_CHANGE", "上市首日-涨幅"),
    ("PER_SHARES_INCOME", "上市首日-每百股获利"),
    ("CAPTURE_PROFIT", "上市首日-约合年化收益"),
    ("ISSUE_PE_RATIO", "发行市盈率"),
    ("INDUSTRY_PE_RATIO", "行业市盈率"),
    ("VA_AMT", "参与申购资金"),
    ("ORG_VAN", "参与申购人数"),
];
const XGSG_NEEQ_SELECT: [&str; 22] = [
    "代码",
    "简称",
    "申购代码",
    "发行总数",
    "网上-发行数量",
    "网上-申购上限",
    "网上-顶格所需资金",
    "发行价格",
    "申购日",
    "中签率",
    "稳获百股需配资金",
    "最新价格-价格",
    "最新价格-累计涨幅",
    "上市首日-上市日",
    "上市首日-均价",
    "上市首日-涨幅",
    "上市首日-每百股获利",
    "上市首日-约合年化收益",
    "发行市盈率",
    "行业市盈率",
    "参与申购资金",
    "参与申购人数",
];
const XGSG_NEEQ_NUMERIC: [&str; 17] = [
    "发行总数",
    "网上-发行数量",
    "网上-申购上限",
    "网上-顶格所需资金",
    "发行价格",
    "中签率",
    "稳获百股需配资金",
    "最新价格-价格",
    "最新价格-累计涨幅",
    "上市首日-均价",
    "上市首日-涨幅",
    "上市首日-每百股获利",
    "上市首日-约合年化收益",
    "发行市盈率",
    "行业市盈率",
    "参与申购资金",
    "参与申购人数",
];
const XGSG_NEEQ_DATE: [&str; 2] = ["申购日", "上市首日-上市日"];

const XGSG_IPO_COLUMNS: &str = "SECURITY_CODE,SECURITY_NAME,TRADE_MARKET_CODE,APPLY_CODE,TRADE_MARKET,MARKET_TYPE,ORG_TYPE,ISSUE_NUM,ONLINE_ISSUE_NUM,OFFLINE_PLACING_NUM,TOP_APPLY_MARKETCAP,PREDICT_ONFUND_UPPER,ONLINE_APPLY_UPPER,PREDICT_ONAPPLY_UPPER,ISSUE_PRICE,LATELY_PRICE,CLOSE_PRICE,APPLY_DATE,BALLOT_NUM_DATE,BALLOT_PAY_DATE,LISTING_DATE,AFTER_ISSUE_PE,ONLINE_ISSUE_LWR,INITIAL_MULTIPLE,INDUSTRY_PE_NEW,OFFLINE_EP_OBJECT,CONTINUOUS_1WORD_NUM,TOTAL_CHANGE,PROFIT,LIMIT_UP_PRICE,INFO_CODE,OPEN_PRICE,LD_OPEN_PREMIUM,LD_CLOSE_CHANGE,TURNOVERRATE,LD_HIGH_CHANG,LD_AVERAGE_PRICE,OPEN_DATE,OPEN_AVERAGE_PRICE,PREDICT_PE,PREDICT_ISSUE_PRICE2,PREDICT_ISSUE_PRICE,PREDICT_ISSUE_PRICE1,PREDICT_ISSUE_PE,PREDICT_PE_THREE,ONLINE_APPLY_PRICE,MAIN_BUSINESS";
const XGSG_IPO_RENAME: [(&str, &str); 24] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME", "股票简称"),
    ("APPLY_CODE", "申购代码"),
    ("TRADE_MARKET", "交易所"),
    ("MARKET_TYPE", "板块"),
    ("ISSUE_NUM", "发行总数"),
    ("ONLINE_ISSUE_NUM", "网上发行"),
    ("TOP_APPLY_MARKETCAP", "顶格申购需配市值"),
    ("ONLINE_APPLY_UPPER", "申购上限"),
    ("ISSUE_PRICE", "发行价格"),
    ("LATELY_PRICE", "最新价"),
    ("CLOSE_PRICE", "首日收盘价"),
    ("APPLY_DATE", "申购日期"),
    ("BALLOT_NUM_DATE", "中签号公布日"),
    ("BALLOT_PAY_DATE", "中签缴款日期"),
    ("LISTING_DATE", "上市日期"),
    ("AFTER_ISSUE_PE", "发行市盈率"),
    ("ONLINE_ISSUE_LWR", "中签率"),
    ("INITIAL_MULTIPLE", "询价累计报价倍数"),
    ("INDUSTRY_PE_NEW", "行业市盈率"),
    ("OFFLINE_EP_OBJECT", "配售对象报价家数"),
    ("CONTINUOUS_1WORD_NUM", "连续一字板数量"),
    ("TOTAL_CHANGE", "涨幅"),
    ("PROFIT", "每中一签获利"),
];
const XGSG_IPO_SELECT: [&str; 24] = [
    "股票代码",
    "股票简称",
    "申购代码",
    "交易所",
    "板块",
    "发行总数",
    "网上发行",
    "顶格申购需配市值",
    "申购上限",
    "发行价格",
    "最新价",
    "首日收盘价",
    "申购日期",
    "中签号公布日",
    "中签缴款日期",
    "上市日期",
    "发行市盈率",
    "行业市盈率",
    "中签率",
    "询价累计报价倍数",
    "配售对象报价家数",
    "连续一字板数量",
    "涨幅",
    "每中一签获利",
];
const XGSG_IPO_NUMERIC: [&str; 14] = [
    "发行总数",
    "网上发行",
    "顶格申购需配市值",
    "申购上限",
    "发行价格",
    "最新价",
    "首日收盘价",
    "发行市盈率",
    "行业市盈率",
    "中签率",
    "询价累计报价倍数",
    "配售对象报价家数",
    "涨幅",
    "每中一签获利",
];
const XGSG_IPO_DATE: [&str; 4] = ["申购日期", "中签号公布日", "中签缴款日期", "上市日期"];

/// 新股申购与中签查询（对应 akshare [`akshare.stock_xgsglb_em`]）。
///
/// - `symbol="北交所"`：走 `RPT_NEEQ_ISSUEINFO_LIST`（datacenter-web，`source=NEEQSELECT`，
///   通过 `quoteColumns` 补充股票简称）；无 序号列；含计算列 `最新价格-累计涨幅`
///   （= 首日收盘价 / 最新价格-价格）。
/// - `symbol` 为 `全部股票`/`沪市主板`/`科创板`/`深市主板`/`创业板`：走 `RPTA_APP_IPOAPPLY`
///   （datacenter-web，`source=WEB`），按市场过滤；无 序号列。
///
/// # 参数
/// `symbol`：可选 `全部股票`/`沪市主板`/`科创板`/`深市主板`/`创业板`/`北交所`。
///
/// # 返回列
/// - 北交所（22 列）：`代码, 简称, 申购代码, 发行总数, 网上-发行数量, 网上-申购上限,
///   网上-顶格所需资金, 发行价格, 申购日, 中签率, 稳获百股需配资金, 最新价格-价格,
///   最新价格-累计涨幅, 上市首日-上市日, 上市首日-均价, 上市首日-涨幅, 上市首日-每百股获利,
///   上市首日-约合年化收益, 发行市盈率, 行业市盈率, 参与申购资金, 参与申购人数`
/// - 其余（24 列）：`股票代码, 股票简称, 申购代码, 交易所, 板块, 发行总数, 网上发行,
///   顶格申购需配市值, 申购上限, 发行价格, 最新价, 首日收盘价, 申购日期, 中签号公布日,
///   中签缴款日期, 上市日期, 发行市盈率, 行业市盈率, 中签率, 询价累计报价倍数,
///   配售对象报价家数, 连续一字板数量, 涨幅, 每中一签获利`
pub fn stock_xgsglb_em(symbol: &str) -> Result<Df> {
    let http = HttpClient::default();
    if symbol == "北交所" {
        let extra = report_extra("APPLY_DATE", "-1", None, Some(XGSG_NEEQ_QUOTE), None, None);
        let mut rows = fetch_eastmoney_pages(
            &http,
            "https://datacenter-web.eastmoney.com/api/data/v1/get",
            "RPT_NEEQ_ISSUEINFO_LIST",
            "ALL",
            &extra,
            "500",
            "NEEQSELECT",
            "WEB",
        )?;
        // 预计算 最新价格-累计涨幅 = 首日收盘价 / 最新价格-价格
        let to_f = |v: &Value| -> Option<f64> {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
        };
        for row in &mut rows {
            if let Some(obj) = row.as_object_mut() {
                let close = obj.get("CLOSE_PRICE").and_then(to_f);
                let newest = obj.get("NEWEST_PRICE").and_then(to_f);
                let v = match (close, newest) {
                    (Some(c), Some(n)) if n != 0.0 => (c / n).to_string(),
                    _ => String::new(),
                };
                obj.insert("COMPUTED_CUMCHG".into(), Value::String(v));
            }
        }
        let mut df = finalize_report(
            &rows,
            &XGSG_NEEQ_RENAME,
            &XGSG_NEEQ_SELECT,
            &XGSG_NEEQ_NUMERIC,
            None,
        )?;
        df.cast_date(&XGSG_NEEQ_DATE)?;
        Ok(df)
    } else {
        let filter = match symbol {
            "全部股票" => "(APPLY_DATE>'2010-01-01')",
            "沪市主板" => "(APPLY_DATE>'2010-01-01')(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE in (\"069001001001\",\"069001001003\",\"069001001006\"))",
            "科创板" => "(APPLY_DATE>'2010-01-01')(SECURITY_TYPE_CODE in (\"058001001\",\"058001008\"))(TRADE_MARKET_CODE=\"069001001006\")",
            "深市主板" => "(APPLY_DATE>'2010-01-01')(SECURITY_TYPE_CODE=\"058001001\")(TRADE_MARKET_CODE in (\"069001002001\",\"069001002002\",\"069001002003\",\"069001002005\"))",
            "创业板" => "(APPLY_DATE>'2010-01-01')(SECURITY_TYPE_CODE=\"058001001\")(TRADE_MARKET_CODE=\"069001002002\")",
            other => {
                return Err(AkshareError::Param(format!(
                    "未知 symbol: {other}（可选：全部股票/沪市主板/科创板/深市主板/创业板/北交所）"
                )))
            }
        };
        let extra = report_extra(
            "APPLY_DATE,SECURITY_CODE",
            "-1,-1",
            Some(filter),
            None,
            None,
            None,
        );
        let rows = fetch_eastmoney_pages(
            &http,
            "https://datacenter-web.eastmoney.com/api/data/v1/get",
            "RPTA_APP_IPOAPPLY",
            XGSG_IPO_COLUMNS,
            &extra,
            "5000",
            "WEB",
            "WEB",
        )?;
        let mut df = finalize_report(
            &rows,
            &XGSG_IPO_RENAME,
            &XGSG_IPO_SELECT,
            &XGSG_IPO_NUMERIC,
            None,
        )?;
        df.cast_date(&XGSG_IPO_DATE)?;
        Ok(df)
    }
}

// ===== 东方财富分析师指数（stock_analyst_rank_em / stock_analyst_detail_em）=====
// 这两个接口使用与 datacenter-web 不同的 host：
// - 排名：`https://data.eastmoney.com/dataapi/invest/list`（reportName=RPT_ANALYST_INDEX_RANK）
// - 详情：`https://datacenter.eastmoney.com/special/api/data/v1/get`
// 排名的 `{year}年收益率` / `{year}最新个股评级-*` 为随 year 参数变化的动态列，
// 列契约由 [`analyst_rank_cols`] 依 year 动态构造。详情三分支列名固定。

/// 分析师指数排名的动态列契约（依 `year` 构造）。
///
/// 返回 `(rename, select, numeric, date)`，其中 `rename` 的 `{year}` 前缀列名为动态生成，
/// 以 `String` 持有；调用方需以 `as_str()` 借用后传入 [`finalize_report`]。
/// 序号列由 [`finalize_report`] 的 `index_name=Some("序号")` 生成。
#[allow(clippy::type_complexity)]
fn analyst_rank_cols(year: &str) -> (Vec<(String, String)>, Vec<String>, Vec<String>, String) {
    let year_yield = format!("{year}年收益率");
    let year_name = format!("{year}最新个股评级-股票名称");
    let year_code = format!("{year}最新个股评级-股票代码");
    let rename = vec![
        ("ANALYST_CODE".to_string(), "分析师ID".to_string()),
        ("ANALYST_NAME".to_string(), "分析师名称".to_string()),
        ("TRADE_DATE".to_string(), "更新日期".to_string()),
        ("YEAR".to_string(), "年度".to_string()),
        ("ORG_NAME".to_string(), "分析师单位".to_string()),
        ("INDEX_VALUE".to_string(), "年度指数".to_string()),
        ("YEAR_YIELD".to_string(), year_yield.clone()),
        ("YIELD_3".to_string(), "3个月收益率".to_string()),
        ("YIELD_6".to_string(), "6个月收益率".to_string()),
        ("YIELD_12".to_string(), "12个月收益率".to_string()),
        ("SECURITY_COUNT".to_string(), "成分股个数".to_string()),
        ("SECURITY_NAME_ABBR".to_string(), year_name.clone()),
        ("SECURITY_CODE".to_string(), year_code.clone()),
        ("INDUSTRY_CODE".to_string(), "行业代码".to_string()),
        ("INDUSTRY_NAME".to_string(), "行业".to_string()),
    ];
    let select = vec![
        "分析师名称".to_string(),
        "分析师单位".to_string(),
        "年度指数".to_string(),
        year_yield.clone(),
        "3个月收益率".to_string(),
        "6个月收益率".to_string(),
        "12个月收益率".to_string(),
        "成分股个数".to_string(),
        year_name.clone(),
        year_code.clone(),
        "分析师ID".to_string(),
        "行业代码".to_string(),
        "行业".to_string(),
        "更新日期".to_string(),
        "年度".to_string(),
    ];
    let numeric = vec![
        "年度指数".to_string(),
        year_yield.clone(),
        "3个月收益率".to_string(),
        "6个月收益率".to_string(),
        "12个月收益率".to_string(),
        "成分股个数".to_string(),
    ];
    (rename, select, numeric, "更新日期".to_string())
}

/// 东方财富分析师指数排名（对应 akshare [`akshare.stock_analyst_rank_em`]）。
///
/// 走 `https://data.eastmoney.com/dataapi/invest/list`，`RPT_ANALYST_INDEX_RANK`，
/// 按 `YEAR="{year}"` 过滤、按 `YEAR_YIELD` 降序取 top100（去重 `ANALYST_CODE`）。
/// 含动态列 `{year}年收益率` / `{year}最新个股评级-股票名称` / `{year}最新个股评级-股票代码`
/// （由 [`analyst_rank_cols`] 依 `year` 生成）。生成 1-based 序号列。
///
/// # 参数
/// `year`：年份字符串，如 `"2024"`（2015 年至今）。
///
/// # 返回列（16 列）
/// `序号, 分析师名称, 分析师单位, 年度指数, {year}年收益率, 3个月收益率, 6个月收益率,
/// 12个月收益率, 成分股个数, {year}最新个股评级-股票名称, {year}最新个股评级-股票代码,
/// 分析师ID, 行业代码, 行业, 更新日期, 年度`
pub fn stock_analyst_rank_em(year: &str) -> Result<Df> {
    let http = HttpClient::default();
    let url = "https://data.eastmoney.com/dataapi/invest/list";
    let mut extra = Map::new();
    extra.insert("sortColumns".into(), json!("YEAR_YIELD"));
    extra.insert("sortTypes".into(), json!("-1"));
    extra.insert("filter".into(), json!(format!(r#"(YEAR="{year}")"#)));
    extra.insert("distinct".into(), json!("ANALYST_CODE"));
    extra.insert("limit".into(), json!("top100"));
    let rows = fetch_eastmoney_pages(
        &http,
        url,
        "RPT_ANALYST_INDEX_RANK",
        "ALL",
        &extra,
        "500",
        "WEB",
        "WEB",
    )?;

    let (rename, select, numeric, date) = analyst_rank_cols(year);
    let rename_ref: Vec<(&str, &str)> = rename
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let select_ref: Vec<&str> = select.iter().map(|s| s.as_str()).collect();
    let numeric_ref: Vec<&str> = numeric.iter().map(|s| s.as_str()).collect();
    let date_ref: Vec<&str> = vec![date.as_str()];

    let mut df = finalize_report(&rows, &rename_ref, &select_ref, &numeric_ref, Some("序号"))?;
    df.cast_date(&date_ref)?;
    Ok(df)
}

// 分析师详情三分支列契约（详情接口为位置重命名，此处按 live JSON 键序对齐）。
const ANALYST_NTCS_RENAME: [(&str, &str); 8] = [
    ("RATING_DATE", "最新评级日期"),
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票名称"),
    ("CHANGE_DATE", "调入日期"),
    ("RATING_NAME", "当前评级名称"),
    ("CLOSE_FORWARD_ADJPRICE", "成交价格(前复权)"),
    ("NEW_PRICE", "最新价格"),
    ("CURRENT_CHANGE", "阶段涨跌幅"),
];
const ANALYST_NTCS_SELECT: [&str; 8] = [
    "股票代码",
    "股票名称",
    "调入日期",
    "最新评级日期",
    "当前评级名称",
    "成交价格(前复权)",
    "最新价格",
    "阶段涨跌幅",
];
const ANALYST_NTCS_NUMERIC: [&str; 3] = ["成交价格(前复权)", "最新价格", "阶段涨跌幅"];
const ANALYST_NTCS_DATE: [&str; 2] = ["调入日期", "最新评级日期"];

const ANALYST_HISCS_RENAME: [(&str, &str); 7] = [
    ("SECURITY_CODE", "股票代码"),
    ("SECURITY_NAME_ABBR", "股票名称"),
    ("CHANGE_DATE", "调入日期"),
    ("BFCHANGE_DATE", "调出日期"),
    ("RATING_NAME", "调入时评级名称"),
    ("REASON", "调出原因"),
    ("CHANGE_RATE", "累计涨跌幅"),
];
const ANALYST_HISCS_SELECT: [&str; 7] = [
    "股票代码",
    "股票名称",
    "调入日期",
    "调出日期",
    "调入时评级名称",
    "调出原因",
    "累计涨跌幅",
];
const ANALYST_HISCS_NUMERIC: [&str; 1] = ["累计涨跌幅"];
const ANALYST_HISCS_DATE: [&str; 2] = ["调入日期", "调出日期"];

const ANALYST_HISIDX_RENAME: [(&str, &str); 2] =
    [("TRADE_DATE", "date"), ("INDEX_HVALUE", "value")];
const ANALYST_HISIDX_SELECT: [&str; 2] = ["date", "value"];
const ANALYST_HISIDX_NUMERIC: [&str; 1] = ["value"];
const ANALYST_HISIDX_DATE: [&str; 1] = ["date"];

/// 分析师详情（对应 akshare [`akshare.stock_analyst_detail_em`]）。
///
/// 走 `https://datacenter.eastmoney.com/special/api/data/v1/get`，按 `indicator` 选择
/// reportName：
/// - `最新跟踪成分股` → `RPT_RESEARCHER_NTCSTOCK`（含 序号列）
/// - `历史跟踪成分股` → `RPT_RESEARCHER_HISTORYSTOCK`（含 序号列）
/// - `历史指数` → `RPT_RESEARCHER_DETAILS`（无 序号列，按 date 升序，列名为 `date, value`）
///
/// # 参数
/// `analyst_id`：分析师 ID（来自 [`stock_analyst_rank_em`]）；`indicator`：可选
/// `最新跟踪成分股`/`历史跟踪成分股`/`历史指数`。
///
/// # 返回列
/// - 最新跟踪成分股（9 列）：`序号, 股票代码, 股票名称, 调入日期, 最新评级日期,
///   当前评级名称, 成交价格(前复权), 最新价格, 阶段涨跌幅`
/// - 历史跟踪成分股（8 列）：`序号, 股票代码, 股票名称, 调入日期, 调出日期,
///   调入时评级名称, 调出原因, 累计涨跌幅`
/// - 历史指数（2 列）：`date, value`
pub fn stock_analyst_detail_em(analyst_id: &str, indicator: &str) -> Result<Df> {
    let http = HttpClient::default();
    let url = "https://datacenter.eastmoney.com/special/api/data/v1/get";
    let filter = format!(r#"(ANALYST_CODE="{analyst_id}")"#);
    match indicator {
        "最新跟踪成分股" => {
            let extra = report_extra("CHANGE_DATE", "-1", Some(&filter), None, None, None);
            let rows = fetch_eastmoney_pages(
                &http,
                url,
                "RPT_RESEARCHER_NTCSTOCK",
                "ALL",
                &extra,
                "1000",
                "WEB",
                "WEB",
            )?;
            let mut df = finalize_report(
                &rows,
                &ANALYST_NTCS_RENAME,
                &ANALYST_NTCS_SELECT,
                &ANALYST_NTCS_NUMERIC,
                Some("序号"),
            )?;
            df.cast_date(&ANALYST_NTCS_DATE)?;
            Ok(df)
        }
        "历史跟踪成分股" => {
            let extra = report_extra("CHANGE_DATE", "-1", Some(&filter), None, None, None);
            let rows = fetch_eastmoney_pages(
                &http,
                url,
                "RPT_RESEARCHER_HISTORYSTOCK",
                "ALL",
                &extra,
                "1000",
                "WEB",
                "WEB",
            )?;
            let mut df = finalize_report(
                &rows,
                &ANALYST_HISCS_RENAME,
                &ANALYST_HISCS_SELECT,
                &ANALYST_HISCS_NUMERIC,
                Some("序号"),
            )?;
            df.cast_date(&ANALYST_HISCS_DATE)?;
            Ok(df)
        }
        "历史指数" => {
            // akshare 请求无 pageSize（返回全量），并本地按 date 升序；故 page_size="0"。
            let extra = report_extra("TRADE_DATE", "-1", Some(&filter), None, None, None);
            let mut rows = fetch_eastmoney_pages(
                &http,
                url,
                "RPT_RESEARCHER_DETAILS",
                "ALL",
                &extra,
                "0",
                "WEB",
                "WEB",
            )?;
            rows.sort_by(|a, b| {
                let ka = a.get("TRADE_DATE").and_then(Value::as_str).unwrap_or("");
                let kb = b.get("TRADE_DATE").and_then(Value::as_str).unwrap_or("");
                ka.cmp(kb)
            });
            let mut df = finalize_report(
                &rows,
                &ANALYST_HISIDX_RENAME,
                &ANALYST_HISIDX_SELECT,
                &ANALYST_HISIDX_NUMERIC,
                None,
            )?;
            df.cast_date(&ANALYST_HISIDX_DATE)?;
            Ok(df)
        }
        other => Err(AkshareError::Param(format!(
            "未知 indicator: {other}（可选：最新跟踪成分股/历史跟踪成分股/历史指数）"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 离线验证 A 股快照列契约（与 akshare 输出一致）。
    #[test]
    fn spot_contract_offline() {
        let rows = json!([
            {"f2":"10.5","f3":"9.9","f4":"0.9","f5":"100000","f6":"1050000","f7":"3.2",
             "f8":"0.5","f9":"8.1","f10":"1.2","f11":"0.1","f12":"000001","f14":"平安银行",
             "f15":"10.8","f16":"10.2","f17":"10.3","f18":"9.6","f20":"200000000000",
             "f21":"180000000000","f22":"5.0","f23":"12.0","f24":"0.05","f25":"0.9"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = crate::sources::eastmoney::finalize_clist(rows).unwrap();
        let df = finalize_spot(df, &SPOT_RENAME, &SPOT_SELECT, &SPOT_NUMERIC).unwrap();
        assert_eq!(df.column_names(), SPOT_SELECT);
        assert_eq!(df.height(), 1);
        let px = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(10.5));
        let pct = df.inner().column("涨跌幅").unwrap().f64().unwrap();
        assert_eq!(pct.get(0), Some(9.9));
    }

    /// 离线验证新股快照列契约（含上市日期）。
    #[test]
    fn new_a_contract_offline() {
        let rows = json!([
            {"f2":"10.5","f3":"9.9","f4":"0.9","f5":"100000","f6":"1050000","f7":"3.2",
             "f8":"0.5","f9":"8.1","f10":"1.2","f11":"0.1","f12":"001234","f14":"新股",
             "f15":"10.8","f16":"10.2","f17":"10.3","f18":"9.6","f20":"200000000000",
             "f21":"180000000000","f22":"5.0","f23":"12.0","f24":"0.05","f25":"0.9","f26":"20240101"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = crate::sources::eastmoney::finalize_clist(rows).unwrap();
        let df = finalize_spot(df, &NEW_A_RENAME, &NEW_A_SELECT, &NEW_A_NUMERIC).unwrap();
        assert_eq!(df.column_names(), NEW_A_SELECT);
        let list = df.inner().column("上市日期").unwrap().str().unwrap();
        assert_eq!(list.get(0), Some("20240101"));
    }

    /// 离线验证港股快照列契约。
    #[test]
    fn hk_spot_contract_offline() {
        let rows = json!([
            {"f2":"100.0","f3":"2.0","f4":"2.0","f5":"50000","f6":"5000000","f7":"1.5",
             "f8":"0.3","f9":"12.0","f10":"0.8","f12":"00700","f14":"腾讯控股",
             "f15":"101.0","f16":"99.0","f17":"99.5","f18":"98.0","f25":"3.0"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = crate::sources::eastmoney::finalize_clist(rows).unwrap();
        let df = finalize_spot(df, &HK_RENAME, &HK_SELECT, &HK_NUMERIC).unwrap();
        assert_eq!(df.column_names(), HK_SELECT);
        assert_eq!(df.height(), 1);
    }

    /// 离线验证股东户数报表列契约 + 数值化 + 日期归一化。
    #[test]
    fn gdhs_report_offline() {
        let rows = json!([{
            "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
            "END_DATE":"2023-09-30T00:00:00","INTERVAL_CHRATE":"-1.23",
            "AVG_MARKET_CAP":"123456.78","AVG_HOLD_NUM":"5000",
            "TOTAL_MARKET_CAP":"200000000000","TOTAL_A_SHARES":"190000000000",
            "HOLD_NOTICE_DATE":"2023-10-01T00:00:00","HOLDER_NUM":"120000",
            "PRE_HOLDER_NUM":"130000","HOLDER_NUM_CHANGE":"-10000",
            "HOLDER_NUM_RATIO":"-7.69","PRE_END_DATE":"2023-06-30T00:00:00",
            "f2":"10.5","f3":"1.2"
        }]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &GDHS_RENAME, &GDHS_SELECT, &GDHS_NUMERIC, None).unwrap();
        df.cast_date(&GDHS_DATE).unwrap();
        assert_eq!(df.column_names(), GDHS_SELECT);
        assert_eq!(df.height(), 1);
        // 数值列已转 f64
        let holder = df.inner().column("股东户数-本次").unwrap().f64().unwrap();
        assert_eq!(holder.get(0), Some(120000.0));
        let chg = df
            .inner()
            .column("股东户数-增减比例")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(chg.get(0), Some(-7.69));
        // 日期列归一化为 YYYY-MM-DD
        let d = df
            .inner()
            .column("股东户数统计截止日-本次")
            .unwrap()
            .str()
            .unwrap();
        assert_eq!(d.get(0), Some("2023-09-30"));
        let notice = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2023-10-01"));
    }

    /// 离线验证「无 序号」报表（融资融券账户信息）列契约 + 数值化。
    #[test]
    fn margin_report_offline() {
        let rows = json!([
            {"STATISTICS_DATE":"2024-01-02T00:00:00","FIN_BALANCE":"12345.6","LOAN_BALANCE":"678.9",
             "FIN_BUY_AMT":"100.0","LOAN_SELL_AMT":"20.0","SECURITY_ORG_NUM":"90",
             "OPERATEDEPT_NUM":"8000","PERSONAL_INVESTOR_NUM":"500.1","ORG_INVESTOR_NUM":"10",
             "INVESTOR_NUM":"1000","MARGINLIAB_INVESTOR_NUM":"300","TOTAL_GUARANTEE":"99999.0",
             "AVG_GUARANTEE_RATIO":"250.0"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &MARGIN_RENAME, &MARGIN_SELECT, &MARGIN_NUMERIC, None).unwrap();
        df.cast_date(&MARGIN_DATE).unwrap();
        assert_eq!(df.column_names(), MARGIN_SELECT);
        assert_eq!(df.height(), 1);
        let balance = df.inner().column("融资余额").unwrap().f64().unwrap();
        assert_eq!(balance.get(0), Some(12345.6));
    }

    /// 离线验证「序号」列生成（1-based）。
    #[test]
    fn index_column_offline() {
        let rows = json!([
            {"HOLDER_NAME":"甲","SECURITY_CODE":"000001","END_DATE":"2023-09-30T00:00:00",
             "HOLD_NUM":"100","XZCHANGE":"0","CHANGE_RATIO":"0.0",
             "HOLDNUM_CHANGE_NAME":"不变","HOLDER_MARKET_CAP":"1000.0","UPDATE_DATE":"2023-10-01T00:00:00"},
            {"HOLDER_NAME":"乙","SECURITY_CODE":"000002","END_DATE":"2023-09-30T00:00:00",
             "HOLD_NUM":"200","XZCHANGE":"10","CHANGE_RATIO":"5.0",
             "HOLDNUM_CHANGE_NAME":"增加","HOLDER_MARKET_CAP":"2000.0","UPDATE_DATE":"2023-10-01T00:00:00"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &FREE_HOLD_RENAME,
            &FREE_HOLD_SELECT,
            &FREE_HOLD_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&FREE_HOLD_DATE).unwrap();
        // 序号在最前且为 1-based
        assert_eq!(df.column_names()[0], "序号");
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        assert_eq!(idx.get(1), Some(2.0));
        assert_eq!(df.height(), 2);
    }

    /// 离线验证股权质押总览的 `/100` 缩放（A股质押总比例 = PM_RATIO / 100）。
    #[test]
    fn gpzy_scale_offline() {
        let rows = json!([{
            "TRADE_DATE":"2024-01-02T00:00:00","TOTAL_PLEDGED_SHARES":"17506029.46",
            "PLEDGE_MARKET_VALUE":"162995584.5491","CSI_300_INDEX":"2168.358",
            "CSI_300_CHG":"-0.4871","PM_RATIO":"673.194806773449","PLEDGE_CO_NUM":"1609",
            "DAILY_STATISTICS":"7987"
        }]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &GPZY_PROFILE_RENAME,
            &GPZY_PROFILE_SELECT,
            &GPZY_PROFILE_NUMERIC,
            None,
        )
        .unwrap();
        df.scale("A股质押总比例", 100.0).unwrap();
        df.cast_date(&GPZY_PROFILE_DATE).unwrap();
        let ratio = df.inner().column("A股质押总比例").unwrap().f64().unwrap();
        // 673.1948 / 100 ≈ 6.7319
        assert!((ratio.get(0).unwrap() - 6.731948).abs() < 1e-4);
    }

    /// 错误路径：非法 symbol 应明确报错（合法 YYYYMMDD 格式即使月份无效也交由服务端返回空表）。
    #[test]
    fn gdhs_invalid_symbol() {
        assert!(stock_zh_a_gdhs("2023-09-30").is_err());
        assert!(stock_zh_a_gdhs("abc").is_err());
        assert!(stock_zh_a_gdhs("202399").is_err());
    }

    /// 错误路径：非法日期格式应报错。
    #[test]
    fn bad_date_rejected() {
        assert!(stock_gdfx_free_holding_detail_em("202399").is_err());
        assert!(stock_gdfx_free_holding_detail_em("abcd").is_err());
    }

    /// 离线验证质押明细列契约：序号生成 + 列序 + 数值/日期列。
    #[test]
    fn pledge_detail_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000408","SECURITY_NAME_ABBR":"藏格矿业","HOLDER_NAME":"藏格集团",
                "NOTICE_DATE":"2026-08-08 00:00:00","PF_ORG":"中信证券","PF_NUM":"17000000",
                "PF_HOLD_RATIO":"10.24","PF_TSR":"1.08","CLOSE_FORWARD_ADJPRICE":"86.79",
                "PF_START_DATE":"2026-08-06 00:00:00","ACTUAL_UNFREEZE_DATE":null,
                "UNFREEZE_STATE":"未解押","WARNING_LINE":"69.432","CLOSE_PRICE":"89.2"
            },
            {
                "SECURITY_CODE":"600030","SECURITY_NAME_ABBR":"中信证券","HOLDER_NAME":"某股东",
                "NOTICE_DATE":"2026-07-01 00:00:00","PF_ORG":"券商","PF_NUM":"5000000",
                "PF_HOLD_RATIO":"2.0","PF_TSR":"0.5","CLOSE_FORWARD_ADJPRICE":"20.0",
                "PF_START_DATE":"2026-06-01 00:00:00","ACTUAL_UNFREEZE_DATE":"2026-12-01 00:00:00",
                "UNFREEZE_STATE":"已解押","WARNING_LINE":"15.0","CLOSE_PRICE":"25.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &PLEDGE_DETAIL_RENAME,
            &PLEDGE_DETAIL_SELECT,
            &PLEDGE_DETAIL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&PLEDGE_DETAIL_DATE).unwrap();
        // 序号在最前且 1-based
        assert_eq!(df.column_names()[0], "序号");
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        assert_eq!(idx.get(1), Some(2.0));
        // 列序与 akshare 一致（不含 序号 的 SELECT 在前）
        assert_eq!(df.column_names()[1..], PLEDGE_DETAIL_SELECT);
        // 数值列已转数值（含 序号）
        let ratio = df.inner().column("占所持股份比例").unwrap().f64().unwrap();
        assert_eq!(ratio.get(0), Some(10.24));
        // 日期截断到 YYYY-MM-DD；空值保持 None
        let end = df.inner().column("质押结束日期").unwrap().str().unwrap();
        assert_eq!(end.get(0), None);
        assert_eq!(end.get(1), Some("2026-12-01"));
    }

    /// 离线验证高管持股变动列契约（无 序号 列）。
    #[test]
    fn ggcg_offline() {
        let rows = json!([
            {
                "CHANGE_NUM":"5.99","NOTICE_DATE":"2026-08-07 00:00:00","SECURITY_CODE":"920493",
                "HOLDER_NAME":"某基金","AFTER_CHANGE_RATE":"0.098874326522","END_DATE":"2026-08-07 00:00:00",
                "AFTER_HOLDER_NUM":"166.3851","HOLD_RATIO":"2.75","FREE_SHARES_RATIO":"3.73",
                "FREE_SHARES":"166.3851","SECURITY_NAME_ABBR":"并行科技","DIRECTION":"减持",
                "CHANGE_FREE_RATIO":"0.13","START_DATE":"2026-08-05 00:00:00",
                "NEWEST_PRICE":"120.21","CHANGE_RATE_QUOTES":"-2.88"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &GGCG_RENAME, &GGCG_SELECT, &GGCG_NUMERIC, None).unwrap();
        df.cast_date(&GGCG_DATE).unwrap();
        // akshare 该接口无 序号 列
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), GGCG_SELECT);
        let price = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(price.get(0), Some(120.21));
        let dir = df
            .inner()
            .column("持股变动信息-增减")
            .unwrap()
            .str()
            .unwrap();
        assert_eq!(dir.get(0), Some("减持"));
    }

    /// 离线验证机构调研统计列契约：序号生成 + 列序 + 数值/日期列。
    #[test]
    fn jgdy_tj_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","NOTICE_DATE":"2026-08-08 00:00:00",
                "RECEIVE_START_DATE":"2026-08-05 00:00:00","RECEIVE_PLACE":"公司会议室",
                "RECEIVE_WAY_EXPLAIN":"实地调研","RECEPTIONIST":"董秘","SUM":"35",
                "CLOSE_PRICE":"45.6","CHANGE_RATE":"-1.23"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &JGDY_TJ_RENAME,
            &JGDY_TJ_SELECT,
            &JGDY_TJ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&JGDY_TJ_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        assert_eq!(df.column_names()[1..], JGDY_TJ_SELECT);
        let price = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(price.get(0), Some(45.6));
        let notice = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2026-08-08"));
    }

    /// 离线验证机构调研详细列契约。
    #[test]
    fn jgdy_detail_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","NOTICE_DATE":"2026-08-08 00:00:00",
                "RECEIVE_START_DATE":"2026-08-05 00:00:00","RECEIVE_OBJECT":"券商资管","RECEIVE_PLACE":"北京",
                "RECEIVE_WAY_EXPLAIN":"电话会议","INVESTIGATORS":"基金经理","RECEPTIONIST":"董秘",
                "ORG_TYPE":"证券公司","CLOSE_PRICE":"45.6","CHANGE_RATE":"-1.23"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &JGDY_DETAIL_RENAME,
            &JGDY_DETAIL_SELECT,
            &JGDY_DETAIL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&JGDY_DETAIL_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], JGDY_DETAIL_SELECT);
        let rate = df.inner().column("涨跌幅").unwrap().f64().unwrap();
        assert_eq!(rate.get(0), Some(-1.23));
        let d = df.inner().column("调研日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-08-05"));
    }

    /// 离线验证分红送配列契约（无序号）。
    #[test]
    fn fhps_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","BONUS_IT_RATIO":"3.0",
                "BONUS_RATIO":"2.0","IT_RATIO":"1.0","PRETAX_BONUS_RMB":"5.0","PLAN_NOTICE_DATE":"2026-03-31 00:00:00",
                "EQUITY_RECORD_DATE":"2026-04-10 00:00:00","EX_DIVIDEND_DATE":"2026-04-11 00:00:00",
                "ASSIGN_PROGRESS":"实施","NOTICE_DATE":"2026-04-12 00:00:00","BASIC_EPS":"1.5",
                "BVPS":"10.0","PER_CAPITAL_RESERVE":"5.0","PER_UNASSIGN_PROFIT":"3.0","PNP_YOY_RATIO":"12.0",
                "TOTAL_SHARES":"500000000","DIVIDENT_RATIO":"1.2"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &FHPS_RENAME, &FHPS_SELECT, &FHPS_NUMERIC, None).unwrap();
        df.cast_date(&FHPS_DATE).unwrap();
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), FHPS_SELECT);
        let total = df.inner().column("总股本").unwrap().f64().unwrap();
        assert_eq!(total.get(0), Some(500000000.0));
        let notice = df.inner().column("最新公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2026-04-12"));
    }

    /// 离线验证分红送配详情列契约（无序号）。
    #[test]
    fn fhps_detail_offline() {
        let rows = json!([
            {
                "BONUS_IT_RATIO":"3.0","BONUS_RATIO":"2.0","IT_RATIO":"1.0","PRETAX_BONUS_RMB":"5.0",
                "PLAN_NOTICE_DATE":"2026-03-31 00:00:00","EQUITY_RECORD_DATE":"2026-04-10 00:00:00",
                "EX_DIVIDEND_DATE":"2026-04-11 00:00:00","REPORT_DATE":"2025-12-31 00:00:00",
                "ASSIGN_PROGRESS":"实施","IMPL_PLAN_PROFILE":"10派5元","NOTICE_DATE":"2026-04-12 00:00:00",
                "BASIC_EPS":"1.5","BVPS":"10.0","PER_CAPITAL_RESERVE":"5.0","PER_UNASSIGN_PROFIT":"3.0",
                "PNP_YOY_RATIO":"12.0","TOTAL_SHARES":"500000000","PUBLISH_DATE":"2026-03-30 00:00:00",
                "DIVIDENT_RATIO":"1.2"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &FHPS_DETAIL_RENAME,
            &FHPS_DETAIL_SELECT,
            &FHPS_DETAIL_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&FHPS_DETAIL_DATE).unwrap();
        assert_eq!(df.column_names(), FHPS_DETAIL_SELECT);
        let bonus = df
            .inner()
            .column("现金分红-现金分红比例")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(bonus.get(0), Some(5.0));
        let report = df.inner().column("报告期").unwrap().str().unwrap();
        assert_eq!(report.get(0), Some("2025-12-31"));
    }

    /// 离线验证停复牌信息列契约：序号生成 + 空数值列 + 日期列。
    #[test]
    fn tfp_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技",
                "SUSPEND_START_TIME":"2026-08-05 00:00:00","SUSPEND_END_TIME":"2026-08-10 00:00:00",
                "SUSPEND_EXPIRE":"连续停牌","SUSPEND_REASON":"重大事项","TRADE_MARKET":"创业板",
                "PREDICT_RESUME_DATE":"2026-08-11 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &TFP_RENAME, &TFP_SELECT, &TFP_NUMERIC, Some("序号")).unwrap();
        df.cast_date(&TFP_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], TFP_SELECT);
        // 无数值列，日期需正确截断
        let start = df.inner().column("停牌时间").unwrap().str().unwrap();
        assert_eq!(start.get(0), Some("2026-08-05"));
    }

    /// 离线验证全部增发列契约（无序号，含 quote 最新价）。
    #[test]
    fn qbzf_offline() {
        let rows = json!([
            {
                "SECURITY_NAME_ABBR":"当升科技","SECURITY_CODE":"300073","CORRECODE":"380073",
                "SEO_TYPE":"1","ISSUE_NUM":"10000000","ONLINE_ISSUE_NUM":"3000000",
                "ISSUE_PRICE":"20.5","NEW_PRICE":"45.6","ISSUE_DATE":"2026-03-31 00:00:00",
                "ISSUE_LISTING_DATE":"2026-04-15 00:00:00","LOCKIN_PERIOD":"6个月"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &QBZF_RENAME, &QBZF_SELECT, &QBZF_NUMERIC, None).unwrap();
        df.cast_date(&QBZF_DATE).unwrap();
        assert_eq!(df.column_names(), QBZF_SELECT);
        let price = df.inner().column("发行价格").unwrap().f64().unwrap();
        assert_eq!(price.get(0), Some(20.5));
        let listing = df.inner().column("增发上市日期").unwrap().str().unwrap();
        assert_eq!(listing.get(0), Some("2026-04-15"));
    }

    /// 离线验证配股列契约（无序号，含 quote 最新价）。
    #[test]
    fn pg_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"600030","SECURITY_NAME_ABBR":"中信证券","CORRECODE":"700030",
                "PLACING_RATIO":"0.3","ISSUE_PRICE":"14.0","TOTAL_SHARES_BEFORE":"1000000000",
                "ISSUE_NUM":"300000000","TOTAL_SHARES_AFTER":"1300000000",
                "EQUITY_RECORD_DATE":"2026-04-10 00:00:00","PAY_START_DATE":"2026-04-15 00:00:00",
                "PAY_END_DATE":"2026-04-20 00:00:00","LISTING_DATE":"2026-05-10 00:00:00",
                "NEW_PRICE":"25.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(&rows, &PG_RENAME, &PG_SELECT, &PG_NUMERIC, None).unwrap();
        df.cast_date(&PG_DATE).unwrap();
        assert_eq!(df.column_names(), PG_SELECT);
        let num = df.inner().column("配股数量").unwrap().f64().unwrap();
        assert_eq!(num.get(0), Some(300000000.0));
        let listing = df.inner().column("上市日").unwrap().str().unwrap();
        assert_eq!(listing.get(0), Some("2026-05-10"));
    }

    /// 离线验证股票账户统计列契约（无序号）。
    #[test]
    fn account_offline() {
        let rows = json!([
            {
                "STATISTICS_DATE":"2026-03-31 00:00:00","ADD_INVESTOR":"2000000","ADD_INVESTOR_QOQ":"5.1",
                "ADD_INVESTOR_YOY":"3.2","END_INVESTOR":"220000000","END_INVESTOR_A":"210000000",
                "END_INVESTOR_B":"10000000","CLOSE_PRICE":"3200.5","CHANGE_RATE":"-0.5",
                "TOTAL_MARKET_CAP":"80000000000000","AVERAGE_MARKET_CAP":"360000"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &ACCOUNT_RENAME,
            &ACCOUNT_SELECT,
            &ACCOUNT_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&ACCOUNT_DATE).unwrap();
        assert_eq!(df.column_names(), ACCOUNT_SELECT);
        let add = df.inner().column("新增投资者-数量").unwrap().f64().unwrap();
        assert_eq!(add.get(0), Some(2000000.0));
        let d = df.inner().column("数据日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-03-31"));
    }

    /// 离线验证业绩报表列契约（序号 + 数值 + 日期）。
    #[test]
    fn yjbb_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","UPDATE_DATE":"2026-04-10 00:00:00",
                "BASIC_EPS":"1.5","TOTAL_OPERATE_INCOME":"5000000000","PARENT_NETPROFIT":"600000000",
                "WEIGHTAVG_ROE":"12.3","YSTZ":"25.0","SJLTZ":"30.0","BPS":"10.0","MGJYXJJE":"0.8",
                "XSMLL":"18.5","YSHZ":"5.0","SJLHZ":"6.0","PUBLISHNAME":"电池"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &YJBB_RENAME,
            &YJBB_SELECT,
            &YJBB_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&YJBB_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], YJBB_SELECT);
        let eps = df.inner().column("每股收益").unwrap().f64().unwrap();
        assert_eq!(eps.get(0), Some(1.5));
        let notice = df.inner().column("最新公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2026-04-10"));
    }

    /// 离线验证业绩快报列契约（序号 + 数值 + 日期）。
    #[test]
    fn yjkb_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","UPDATE_DATE":"2026-04-10 00:00:00",
                "BASIC_EPS":"1.5","TOTAL_OPERATE_INCOME":"5000000000","TOTAL_OPERATE_INCOME_SQ":"4000000000",
                "PARENT_NETPROFIT":"600000000","PARENT_NETPROFIT_SQ":"500000000","PARENT_BVPS":"10.0",
                "WEIGHTAVG_ROE":"12.3","YSTZ":"25.0","JLRTBZCL":"20.0","DJDYSHZ":"5.0","DJDJLHZ":"6.0",
                "PUBLISHNAME":"电池"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &YJKB_RENAME,
            &YJKB_SELECT,
            &YJKB_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&YJKB_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], YJKB_SELECT);
        let income = df
            .inner()
            .column("营业收入-营业收入")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(income.get(0), Some(5000000000.0));
        let notice = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2026-04-10"));
    }

    /// 离线验证业绩预告列契约（序号 + 数值 + 日期，无通用序号外数值）。
    #[test]
    fn yjyg_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","NOTICE_DATE":"2026-04-10 00:00:00",
                "PREDICT_FINANCE":"净利润","PREDICT_CONTENT":"大幅上升","CHANGE_REASON_EXPLAIN":"需求旺盛",
                "PREDICT_TYPE":"预增","PREYEAR_SAME_PERIOD":"500000000","INCREASE_JZ":"50.0","FORECAST_JZ":"750000000"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &YJYG_RENAME,
            &YJYG_SELECT,
            &YJYG_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&YJYG_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], YJYG_SELECT);
        let pre = df.inner().column("预测数值").unwrap().f64().unwrap();
        assert_eq!(pre.get(0), Some(750000000.0));
        let notice = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2026-04-10"));
    }

    /// 离线验证预约披露时间列契约（序号 + 纯日期列，无数值列）。
    #[test]
    fn yysj_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技",
                "FIRST_APPOINT_DATE":"2026-04-20 00:00:00","FIRST_CHANGE_DATE":"2026-04-15 00:00:00",
                "SECOND_CHANGE_DATE":null,"THIRD_CHANGE_DATE":null,"ACTUAL_PUBLISH_DATE":"2026-04-18 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &YYSJ_RENAME,
            &YYSJ_SELECT,
            &YYSJ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&YYSJ_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], YYSJ_SELECT);
        // 无数值列；日期需正确截断，且 null 保持 null
        let first = df.inner().column("首次预约时间").unwrap().str().unwrap();
        assert_eq!(first.get(0), Some("2026-04-20"));
        let third = df.inner().column("三次变更日期").unwrap().str().unwrap();
        assert_eq!(third.get(0), None);
    }

    /// 离线验证千股千评列契约（序号 + 数值化 + 日期截断 + null 保留）。
    #[test]
    fn comment_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "TRADE_DATE":"2026-04-10 00:00:00","CLOSE_PRICE":"10.5","CHANGE_RATE":"9.9",
                "TURNOVERRATE":"0.5","PE_DYNAMIC":"8.1","PRIME_COST":"9.0",
                "ORG_PARTICIPATE":"75.0","TOTALSCORE":null,"RANK_UP":"3.0","RANK":"12",
                "FOCUS":"88.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_RENAME,
            &COMMENT_SELECT,
            &COMMENT_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&COMMENT_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], COMMENT_SELECT);
        let px = df.inner().column("最新价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(10.5));
        let date = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(date.get(0), Some("2026-04-10"));
        let score = df.inner().column("综合得分").unwrap().f64().unwrap();
        assert_eq!(score.get(0), None);
    }

    /// 离线验证个股上榜统计列契约（序号 + 数值化 + 日期截断）。
    #[test]
    fn lhb_stock_statistic_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "LATEST_TDATE":"2026-04-10 00:00:00","CHANGE_RATE":"9.9","CLOSE_PRICE":"10.5",
                "BILLBOARD_TIMES":"5","BILLBOARD_NET_BUY":"100.0","BILLBOARD_BUY_AMT":"200.0",
                "BILLBOARD_SELL_AMT":"100.0","BILLBOARD_DEAL_AMT":"300.0","ORG_BUY_TIMES":"2",
                "ORG_SELL_TIMES":"1","ORG_NET_BUY":"50.0","ORG_BUY_AMT":"80.0","ORG_SELL_AMT":"30.0",
                "IPCT1M":"1.0","IPCT3M":"2.0","IPCT6M":"3.0","IPCT1Y":"4.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &LHB_STAT_RENAME,
            &LHB_STAT_SELECT,
            &LHB_STAT_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&LHB_STAT_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_STAT_SELECT);
        let times = df.inner().column("上榜次数").unwrap().f64().unwrap();
        assert_eq!(times.get(0), Some(5.0));
        let date = df.inner().column("最近上榜日").unwrap().str().unwrap();
        assert_eq!(date.get(0), Some("2026-04-10"));
    }

    /// 离线验证机构买卖每日统计列契约（序号 + 数值化 + 日期截断）。
    #[test]
    fn lhb_jgmmtj_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "TRADE_DATE":"2026-04-10 00:00:00","CLOSE_PRICE":"10.5","CHANGE_RATE":"9.9",
                "BUY_TIMES":"2","SELL_TIMES":"1","BUY_AMT":"80.0","SELL_AMT":"30.0",
                "NET_BUY_AMT":"1000.5","ACCUM_AMOUNT":"5000.0","RATIO":"0.2",
                "TURNOVERRATE":"0.5","FREECAP":"2000000000","EXPLANATION":"活跃"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &LHB_JG_RENAME,
            &LHB_JG_SELECT,
            &LHB_JG_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&LHB_JG_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_JG_SELECT);
        let net = df.inner().column("机构买入净额").unwrap().f64().unwrap();
        assert_eq!(net.get(0), Some(1000.5));
        let date = df.inner().column("上榜日期").unwrap().str().unwrap();
        assert_eq!(date.get(0), Some("2026-04-10"));
    }

    /// 离线验证股东持股统计列契约（序号 + 数值化，无日期列）。
    #[test]
    fn gdfx_stat_offline() {
        let rows = json!([
            {
                "HOLDER_NAME":"香港中央结算有限公司","HOLDER_TYPE":"其他","STATISTICS_TIMES":"8",
                "AVG_CHANGE_10TD":"1.23","MAX_CHANGE_10TD":"2.0","MIN_CHANGE_10TD":"0.5",
                "AVG_CHANGE_30TD":"3.45","MAX_CHANGE_30TD":"5.0","MIN_CHANGE_30TD":"1.0",
                "AVG_CHANGE_60TD":"6.78","MAX_CHANGE_60TD":"9.0","MIN_CHANGE_60TD":"2.0",
                "SEAB_JOIN":"100"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &GDFX_STAT_RENAME,
            &GDFX_STAT_SELECT,
            &GDFX_STAT_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], GDFX_STAT_SELECT);
        let t = df.inner().column("统计次数").unwrap().f64().unwrap();
        assert_eq!(t.get(0), Some(8.0));
        let avg = df
            .inner()
            .column("公告日后涨幅统计-10个交易日-平均涨幅")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(avg.get(0), Some(1.23));
    }

    /// 离线验证股东持股变动统计列契约（序号 + 数值化 + null 保留，无日期列）。
    #[test]
    fn gdfx_change_offline() {
        let rows = json!([
            {
                "HOLDER_NAME":"香港中央结算有限公司","HOLDER_TYPE":"其他","HOLDER_NUM":"100",
                "HOLDADD_NUM":null,"HOLDUP_NUM":"20","HOLDDOWN_NUM":"10","HOLDUNCHANGED_NUM":"30",
                "HOLDER_MARKET_CAP":"2000000000","SEAB_JOIN":"50"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &GDFX_CHG_RENAME,
            &GDFX_CHG_SELECT,
            &GDFX_CHG_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], GDFX_CHG_SELECT);
        let total = df
            .inner()
            .column("期末持股只数统计-总持有")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(total.get(0), Some(100.0));
        let add = df
            .inner()
            .column("期末持股只数统计-新进")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(add.get(0), None);
    }

    /// 真实网络冒烟：拉取实时列契约，与 akshare 实测列序核对（需联网，默认忽略）。
    /// 东财 push2 对本机 IP 偶发 TLS 重置（与 akshare Python 同样受影响），故不 `expect`，
    /// 仅打印结果以便人工核对。
    /// 离线验证千股千评-主力控盘-机构参与度列契约（无 序号；机构参与度 ×100）。
    #[test]
    fn comment_jgcyd_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2026-04-10 00:00:00","ORG_PARTICIPATE":"0.5"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_JGCYD_RENAME,
            &COMMENT_JGCYD_SELECT,
            &COMMENT_JGCYD_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&COMMENT_JGCYD_DATE).unwrap();
        df.scale("机构参与度", 0.01).unwrap();
        // 无 序号 列
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), COMMENT_JGCYD_SELECT);
        // 机构参与度 = 0.5 × 100 = 50.0
        let v = df.inner().column("机构参与度").unwrap().f64().unwrap();
        assert_eq!(v.get(0), Some(50.0));
        let d = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证千股千评-综合评价-历史评分列契约（无 序号）。
    #[test]
    fn comment_lspf_offline() {
        let rows = json!([
            {
                "DIAGNOSE_DATE":"2026-04-10 00:00:00","TOTAL_SCORE":"92.3"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_LSPF_RENAME,
            &COMMENT_LSPF_SELECT,
            &COMMENT_LSPF_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&COMMENT_LSPF_DATE).unwrap();
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), COMMENT_LSPF_SELECT);
        let score = df.inner().column("评分").unwrap().f64().unwrap();
        assert_eq!(score.get(0), Some(92.3));
        let d = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证沪深港通持股-每日个股统计（北向）列契约（无 序号 + 数值 + 日期截断）。
    #[test]
    fn hsgt_stat_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2026-04-10 00:00:00","SECURITY_CODE":"06098",
                "SECURITY_NAME":"碧桂园服务","CLOSE_PRICE":"6.11","CHANGE_RATE":"3.2095",
                "HOLD_SHARES":"1029614338","HOLD_MARKET_CAP":"6290943605.18",
                "HOLD_SHARES_RATIO":"30.79","HOLD_MARKETCAP_CHG1":null,
                "HOLD_MARKETCAP_CHG5":null,"HOLD_MARKETCAP_CHG10":null
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &HSGT_STAT_RENAME,
            &HSGT_STAT_SELECT,
            &HSGT_STAT_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&HSGT_STAT_DATE).unwrap();
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), HSGT_STAT_SELECT);
        let cap = df.inner().column("持股市值").unwrap().f64().unwrap();
        assert_eq!(cap.get(0), Some(6290943605.18));
        let shares = df.inner().column("持股数量").unwrap().f64().unwrap();
        assert_eq!(shares.get(0), Some(1029614338.0));
        let d = df.inner().column("持股日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证商誉-商誉减值预期明细列契约（序号 + 数值 + 日期截断）。
    #[test]
    fn sy_yq_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","TRADE_MARKET":"cyb",
                "NOTICE_DATE":"2026-04-10 00:00:00","PREDICT_NETPROFIT_LOWER":"50000000",
                "PREDICT_NETPROFIT_UPPER":"60000000","PERFORM_CHANGE_UPPER":"25.0",
                "PERFORM_CHANGE_LOWER":"20.0","PERFORM_CHANGE_EXPLAIN":"业绩预增",
                "PE_SAMEREPORT_NETPROFIT":"40000000","PE_GOODWILL":"8000000",
                "NEWEST_REPORT_DATE":"2025-12-31 00:00:00","NEWEST_GOODWILL":"9000000"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let rows = map_trade_market(&rows, "TRADE_MARKET");
        let mut df = finalize_report(
            &rows,
            &SY_YQ_RENAME,
            &SY_YQ_SELECT,
            &SY_YQ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&SY_YQ_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], SY_YQ_SELECT);
        let g = df.inner().column("最新一期商誉").unwrap().f64().unwrap();
        assert_eq!(g.get(0), Some(9000000.0));
        let notice = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(notice.get(0), Some("2026-04-10"));
        let report = df.inner().column("最新商誉报告期").unwrap().str().unwrap();
        assert_eq!(report.get(0), Some("2025-12-31"));
        // 交易市场 代码 → 中文名映射
        let market = df.inner().column("交易市场").unwrap().str().unwrap();
        assert_eq!(market.get(0), Some("创业板"));
    }

    /// 离线验证商誉-个股商誉减值明细列契约（序号 + 数值 + 日期截断）。
    #[test]
    fn sy_jz_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"300073","SECURITY_NAME_ABBR":"当升科技","TRADE_BOARD":"cyb",
                "GOODWILL":"9000000","GOODWILL_CHANGE":"1000000","SUMSHEQUITY_RATIO":"5.0",
                "SE_CHANGE_RATIO":"0.5","PARENTNETPROFIT":"60000000",
                "PNP_CHANGE_RATIO":"1.5","NOTICE_DATE":"2026-04-10 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let rows = map_trade_market(&rows, "TRADE_BOARD");
        let mut df = finalize_report(
            &rows,
            &SY_JZ_RENAME,
            &SY_JZ_SELECT,
            &SY_JZ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&SY_JZ_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], SY_JZ_SELECT);
        let g = df.inner().column("商誉").unwrap().f64().unwrap();
        assert_eq!(g.get(0), Some(9000000.0));
        let ratio = df
            .inner()
            .column("商誉占净资产比例")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(ratio.get(0), Some(5.0));
        let d = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
        // 交易市场 代码 → 中文名映射
        let market = df.inner().column("交易市场").unwrap().str().unwrap();
        assert_eq!(market.get(0), Some("创业板"));
    }

    /// 离线验证千股千评-主力控盘-机构参与度列契约（无序号 + 机构参与度 ×100）。
    #[test]
    fn comment_detail_zlkp_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2026-04-10 00:00:00","ORG_PARTICIPATE":"0.4457392"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_JGCYD_RENAME,
            &COMMENT_JGCYD_SELECT,
            &COMMENT_JGCYD_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&COMMENT_JGCYD_DATE).unwrap();
        // akshare: ORG_PARTICIPATE × 100
        df.scale("机构参与度", 0.01).unwrap();
        assert_eq!(df.column_names(), COMMENT_JGCYD_SELECT);
        let p = df.inner().column("机构参与度").unwrap().f64().unwrap();
        assert_eq!(p.get(0), Some(44.57392));
        let d = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证千股千评-综合评价-历史评分列契约（无序号 + 数值化）。
    #[test]
    fn comment_detail_zhpj_offline() {
        let rows = json!([
            {
                "DIAGNOSE_DATE":"2026-04-10 00:00:00","TOTAL_SCORE":"64.83121714"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_LSPF_RENAME,
            &COMMENT_LSPF_SELECT,
            &COMMENT_LSPF_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&COMMENT_LSPF_DATE).unwrap();
        assert_eq!(df.column_names(), COMMENT_LSPF_SELECT);
        let s = df.inner().column("评分").unwrap().f64().unwrap();
        assert_eq!(s.get(0), Some(64.83121714));
        let d = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证沪深港通持股-每日个股统计（北向）列契约（无序号 + 数值化 + 日期截断）。
    #[test]
    fn hsgt_stock_statistics_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2026-04-10 00:00:00","SECURITY_CODE":"000001","SECURITY_NAME":"平安银行",
                "CLOSE_PRICE":"10.5","CHANGE_RATE":"9.9","HOLD_SHARES":"100000","HOLD_MARKET_CAP":"1050000",
                "HOLD_SHARES_RATIO":"0.5","HOLD_MARKETCAP_CHG1":"100.0","HOLD_MARKETCAP_CHG5":"200.0",
                "HOLD_MARKETCAP_CHG10":"300.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &HSGT_STAT_RENAME,
            &HSGT_STAT_SELECT,
            &HSGT_STAT_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&HSGT_STAT_DATE).unwrap();
        assert_eq!(df.column_names(), HSGT_STAT_SELECT);
        let px = df.inner().column("当日收盘价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(10.5));
        let d = df.inner().column("持股日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证沪深港通持股-个股排行列契约（序号 + indicator 前缀 + 数值化 + 日期截断）。
    #[test]
    fn hsgt_hold_stock_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME":"平安银行","CLOSE_PRICE":"10.5",
                "CHANGE_RATE":"9.9","HOLD_SHARES":"100000","HOLD_MARKET_CAP":"1050000",
                "HOLD_SHARES_RATIO":"0.5","HOLD_MARKETCAP_RATIO":"1.2",
                "ADD_SHARES_REPAIR":"5000","ADD_MARKET_CAP":"60000","ADD_SHARES_AMP":"0.3",
                "ADD_SHARES_RATIO":"0.1","ADD_MARKETCAP_RATIO":"0.2","INDUSTRY":"银行",
                "TRADE_DATE":"2026-08-07 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &HOLD_RENAME,
            &HOLD_SELECT,
            &HOLD_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&HOLD_DATE).unwrap();
        let mut expect: Vec<String> = vec!["序号".to_string()];
        expect.extend(HOLD_SELECT.iter().map(|s| s.to_string()));
        assert_eq!(df.column_names(), expect);
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        let px = df.inner().column("今日收盘价").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(10.5));
        let d = df.inner().column("日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-08-07"));
        // indicator 前缀拼接到 “增持估计-*” 列名
        let prefix = "5日";
        let names: Vec<String> = std::iter::once("序号".to_string())
            .chain(HOLD_SELECT.iter().map(|c| {
                if c.starts_with("增持估计-") {
                    format!("{prefix}{c}")
                } else {
                    (*c).to_string()
                }
            }))
            .collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        df.rename_columns(&refs).unwrap();
        assert_eq!(df.column_names()[9], "5日增持估计-股数");
        assert_eq!(df.column_names()[13], "5日增持估计-占总股本比");
    }

    /// 离线验证沪深港通每日机构统计列契约（无 序号 + 数值化 + 日期截断）。
    #[test]
    fn hsgt_institution_offline() {
        let rows = json!([
            {
                "HOLD_DATE":"2024-01-10 00:00:00","ORG_NAME":"兴证国际证券","HOLD_NUM":"153",
                "HOLD_MARKET_CAP":"478362459.57","HOLD_MARKET_CAPONE":"73582.4",
                "HOLD_MARKET_CAPFIVE":"-12226217.84","HOLD_MARKET_CAPTEN":"-4930840.39"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &INST_RENAME, &INST_SELECT, &INST_NUMERIC, None).unwrap();
        df.cast_date(&INST_DATE).unwrap();
        assert_eq!(df.column_names(), INST_SELECT);
        let n = df.inner().column("持股只数").unwrap().f64().unwrap();
        assert_eq!(n.get(0), Some(153.0));
        let d = df.inner().column("持股日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-01-10"));
    }

    /// 离线验证沪深港通历史资金流向列契约（动态指数列 + 数值化 + 日期截断）。
    #[test]
    fn hsgt_hist_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2014-11-17 00:00:00","NET_DEAL_AMT":"120.82","BUY_AMT":"120.82",
                "SELL_AMT":"10.0","ACCUM_DEAL_AMT":"0.012","FUND_INFLOW":"130.0","QUOTA_BALANCE":"50.0",
                "HOLD_MARKET_CAP":"0.0","LEAD_STOCKS_NAME":"唐山港","LEAD_STOCKS_CODE":"601000.SH",
                "LS_CHANGE_RATE":"9.98","INDEX_CLOSE_PRICE":"2474.01","INDEX_CHANGE_RATE":"-0.19"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &HIST_RNAME, &HIST_SELECT, &HIST_NUMERIC, None).unwrap();
        df.cast_date(&HIST_DATE).unwrap();
        let names: Vec<String> = HIST_SELECT
            .iter()
            .map(|c| {
                if *c == "__INDEX__" {
                    "沪深300".to_string()
                } else if *c == "__INDEXCHG__" {
                    "沪深300-涨跌幅".to_string()
                } else {
                    (*c).to_string()
                }
            })
            .collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        df.rename_columns(&refs).unwrap();
        let expect: Vec<String> = vec![
            "日期",
            "当日成交净买额",
            "买入成交额",
            "卖出成交额",
            "历史累计净买额",
            "当日资金流入",
            "当日余额",
            "持股市值",
            "领涨股",
            "领涨股-涨跌幅",
            "沪深300",
            "沪深300-涨跌幅",
            "领涨股-代码",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(df.column_names(), expect);
        let idx = df.inner().column("沪深300").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(2474.01));
        let d = df.inner().column("日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2014-11-17"));
    }

    /// 离线验证沪深港通板块排行列契约（序号 + 最大/最小增持个股为字符串 + 日期截断）。
    #[test]
    fn hsgt_board_rank_offline() {
        let rows = json!([
            {
                "BOARD_NAME":"银行","INDEX_CHANGE_RATIO":"0.57","COMPOSITION_QUANTITY":"42",
                "HK_VALUE":"187873414383.39","BOARD_HK_RATIO":"0.1066","HK_BOARD_RATIO":"0.0157",
                "COMPOSITION_QUANTITY_ADD":"32","ADD_MARKET_CAP":"1190196450.66","ADD_RATIO":"0.645",
                "ADD_HK_RATIO":"0.0006756","ADD_BOARD_RATIO":"9.95e-05",
                "MAXADD_SECURITY_NAME":"农业银行","MAXADD_RATIO_SECURITY_NAME":"齐鲁银行",
                "MINADD_SECURITY_NAME":"宁波银行","MINADD_RATIO_SECURITY_NAME":"南都电源",
                "TRADE_DATE":"2024-08-16 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &BOARD_RENAME,
            &BOARD_SELECT,
            &BOARD_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&BOARD_DATE).unwrap();
        let mut expect: Vec<String> = vec!["序号".to_string()];
        expect.extend(BOARD_SELECT.iter().map(|s| s.to_string()));
        assert_eq!(df.column_names(), expect);
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        let nm = df.inner().column("名称").unwrap().str().unwrap();
        assert_eq!(nm.get(0), Some("银行"));
        let d = df.inner().column("报告时间").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-08-16"));
        // 最大/最小增持个股两对列对应实时 schema 的个股名称字段，应为字符串列
        let col = df.inner().column("今日增持最大股-市值").unwrap();
        assert!(col.str().is_ok());
    }

    /// 离线验证沪深港通个股持股（港股通）列契约（无 序号 + 数值化 + 日期截断）。
    #[test]
    fn hsgt_individual_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2024-08-12 00:00:00","CLOSE_PRICE":"375.0","CHANGE_RATE":"1.3514",
                "HOLD_SHARES":"929121231","HOLD_MARKET_CAP":"348420461625.0","HOLD_SHARES_RATIO":"9.95",
                "HOLD_MARKETCAP_CHG1":"100.0","HOLD_MARKETCAP_CHG5":"200.0","HOLD_MARKETCAP_CHG10":"300.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &INDIV_RNAME, &INDIV_SELECT, &INDIV_NUMERIC, None).unwrap();
        df.cast_date(&INDIV_DATE).unwrap();
        assert_eq!(df.column_names(), INDIV_SELECT);
        let h = df.inner().column("持股数量").unwrap().f64().unwrap();
        assert_eq!(h.get(0), Some(929121231.0));
        let d = df.inner().column("持股日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-08-12"));
    }

    /// 离线验证沪深港通个股持股详情列契约（无 序号 + 数值化 + 日期截断）。
    #[test]
    fn hsgt_individual_detail_offline() {
        let rows = json!([
            {
                "HOLD_DATE":"2024-09-30 00:00:00","CLOSE_PRICE":"23.53","CHANGE_RATE":"10.0047",
                "ORG_NAME":"云锋证券","HOLD_NUM":"700","HOLD_MARKET_CAP":"16471",
                "HOLD_SHARES_RATIO":"0","HOLD_MARKET_CAPONE":"16471","HOLD_MARKET_CAPFIVE":"16471",
                "HOLD_MARKET_CAPTEN":"16471"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &INDDET_RNAME, &INDDET_SELECT, &INDDET_NUMERIC, None).unwrap();
        df.cast_date(&INDDET_DATE).unwrap();
        assert_eq!(df.column_names(), INDDET_SELECT);
        let o = df.inner().column("机构名称").unwrap().str().unwrap();
        assert_eq!(o.get(0), Some("云锋证券"));
        let d = df.inner().column("持股日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-09-30"));
    }

    /// 离线验证资产负债表列契约（序号 + 数值化 + 日期截断，含 lrb/xjll 共用的 JSON 键）。
    #[test]
    fn zcfz_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "MONETARYFUNDS":"1000000","ACCOUNTS_RECE":"500000","INVENTORY":"300000",
                "TOTAL_ASSETS":"5000000","TOTAL_ASSETS_RATIO":"5.0",
                "ACCOUNTS_PAYABLE":"200000","ADVANCE_RECEIVABLES":"80000",
                "TOTAL_LIABILITIES":"2000000","TOTAL_LIAB_RATIO":"10.0",
                "DEBT_ASSET_RATIO":"40.0","TOTAL_EQUITY":"3000000",
                "NOTICE_DATE":"2024-04-10 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &ZCFZ_RENAME,
            &ZCFZ_SELECT,
            &ZCFZ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&ZCFZ_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], ZCFZ_SELECT);
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        let ta = df.inner().column("资产-总资产").unwrap().f64().unwrap();
        assert_eq!(ta.get(0), Some(5000000.0));
        let dar = df.inner().column("资产负债率").unwrap().f64().unwrap();
        assert_eq!(dar.get(0), Some(40.0));
        let d = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-04-10"));
    }

    /// 离线验证利润表列契约（序号 + 数值化 + 日期截断；含同比列 known off-by-one）。
    #[test]
    fn lrb_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "PARENT_NETPROFIT":"600000","PARENT_NETPROFIT_RATIO":"8.0",
                "TOTAL_OPERATE_INCOME":"3000000","TOI_RATIO":"12.0",
                "OPERATE_COST":"1500000","SALE_EXPENSE":"100000","MANAGE_EXPENSE":"120000",
                "FINANCE_EXPENSE":"50000","TOTAL_OPERATE_COST":"2000000",
                "OPERATE_PROFIT":"900000","TOTAL_PROFIT":"880000",
                "NOTICE_DATE":"2024-04-10 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df =
            finalize_report(&rows, &LRB_RENAME, &LRB_SELECT, &LRB_NUMERIC, Some("序号")).unwrap();
        df.cast_date(&LRB_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LRB_SELECT);
        let np = df.inner().column("净利润").unwrap().f64().unwrap();
        assert_eq!(np.get(0), Some(600000.0));
        let yoy = df.inner().column("净利润同比").unwrap().f64().unwrap();
        assert_eq!(yoy.get(0), Some(8.0));
        let d = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-04-10"));
    }

    /// 离线验证现金流量表列契约（序号 + 数值化 + 日期截断）。
    #[test]
    fn xjll_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "CCE_ADD":"700000","CCE_ADD_RATIO":"6.0",
                "NETCASH_OPERATE":"400000","NETCASH_OPERATE_RATIO":"57.0",
                "NETCASH_INVEST":"-150000","NETCASH_INVEST_RATIO":"-21.0",
                "NETCASH_FINANCE":"450000","NETCASH_FINANCE_RATIO":"64.0",
                "NOTICE_DATE":"2024-04-10 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &XJLL_RENAME,
            &XJLL_SELECT,
            &XJLL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&XJLL_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], XJLL_SELECT);
        let cce = df
            .inner()
            .column("净现金流-净现金流")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(cce.get(0), Some(700000.0));
        let op = df
            .inner()
            .column("经营性现金流-净现金流占比")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(op.get(0), Some(57.0));
        let d = df.inner().column("公告日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-04-10"));
    }

    /// 离线验证北交所资产负债表复用同一列契约。
    #[test]
    fn zcfz_bj_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"830799","SECURITY_NAME_ABBR":"北交所股",
                "MONETARYFUNDS":"100000","ACCOUNTS_RECE":"50000","INVENTORY":"30000",
                "TOTAL_ASSETS":"500000","TOTAL_ASSETS_RATIO":"5.0",
                "ACCOUNTS_PAYABLE":"20000","ADVANCE_RECEIVABLES":"8000",
                "TOTAL_LIABILITIES":"200000","TOTAL_LIAB_RATIO":"10.0",
                "DEBT_ASSET_RATIO":"40.0","TOTAL_EQUITY":"300000",
                "NOTICE_DATE":"2024-04-10 00:00:00"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &ZCFZ_RENAME,
            &ZCFZ_SELECT,
            &ZCFZ_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&ZCFZ_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], ZCFZ_SELECT);
        let ta = df.inner().column("资产-总资产").unwrap().f64().unwrap();
        assert_eq!(ta.get(0), Some(500000.0));
    }

    /// 离线验证股权质押-质押机构分布统计（序号 + 数值化，无日期列）。
    #[test]
    fn gpzy_distribute_offline() {
        let rows = json!([
            {
                "SECURITY_NAME_ABBR":"国泰海通","ORG_NUM":"282","PLEDGE_DEAL_NUM":"775",
                "PLEDGE_NUM":"982237.3809","WARNING_STATE_1_RATE":"0.788387096774",
                "WARNING_STATE_2_RATE":"0.038709677419","WARNING_STATE_3_RATE":"0.172903225806"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &GPZY_DIST_RENAME,
            &GPZY_DIST_SELECT,
            &GPZY_DIST_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], GPZY_DIST_SELECT);
        let org = df.inner().column("质押公司数量").unwrap().f64().unwrap();
        assert_eq!(org.get(0), Some(282.0));
        let rate = df.inner().column("达到平仓线比例").unwrap().f64().unwrap();
        assert_eq!(rate.get(0), Some(0.172903225806));
    }

    /// 离线验证股东户数详情列契约（无序号 + 数值化 + 日期截断，quote f2/f3 被丢弃）。
    #[test]
    fn gdhs_detail_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "CHANGE_SHARES":"1000","CHANGE_REASON":"增发",
                "END_DATE":"2023-09-30T00:00:00","INTERVAL_CHRATE":"-1.23",
                "AVG_MARKET_CAP":"123456.78","AVG_HOLD_NUM":"5000",
                "TOTAL_MARKET_CAP":"200000000000","TOTAL_A_SHARES":"190000000000",
                "HOLD_NOTICE_DATE":"2023-10-01T00:00:00","HOLDER_NUM":"120000",
                "PRE_HOLDER_NUM":"130000","HOLDER_NUM_CHANGE":"-10000",
                "HOLDER_NUM_RATIO":"-7.69","PRE_END_DATE":null,"f2":"10.5","f3":"1.2"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &GDHS_DETAIL_RENAME,
            &GDHS_DETAIL_SELECT,
            &GDHS_DETAIL_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&GDHS_DETAIL_DATE).unwrap();
        // 无 序号 列；quote f2/f3 不出现
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), GDHS_DETAIL_SELECT);
        let holder = df.inner().column("股东户数-本次").unwrap().f64().unwrap();
        assert_eq!(holder.get(0), Some(120000.0));
        let d = df
            .inner()
            .column("股东户数统计截止日")
            .unwrap()
            .str()
            .unwrap();
        assert_eq!(d.get(0), Some("2023-09-30"));
        // f2/f3 已丢弃
        assert!(df.inner().column("f2").is_err());
        assert!(df.inner().column("f3").is_err());
    }

    /// 离线验证股东协同（十大流通/十大股东）列契约：序号生成 + 协同次数数值化。
    #[test]
    fn gdfx_team_offline() {
        let rows = json!([
            {
                "HOLDER_NAME":"吕强","HOLDER_TYPE":"个人",
                "COOPERAT_HOLDER_NAME":"某协同股东","COOPERAT_HOLDER_TYPE":"基金",
                "COOPERAT_NUM":"5","PINGJIE":"个股A,个股B"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &GDFX_TEAM_RENAME,
            &GDFX_TEAM_SELECT,
            &GDFX_TEAM_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], GDFX_TEAM_SELECT);
        let c = df.inner().column("协同次数").unwrap().f64().unwrap();
        assert_eq!(c.get(0), Some(5.0));
        let name = df.inner().column("股东名称").unwrap().str().unwrap();
        assert_eq!(name.get(0), Some("吕强"));
    }

    /// 离线验证千股千评-用户关注指数列契约（无序号 + 数值化 + 日期截断）。
    #[test]
    fn comment_focus_offline() {
        let rows = json!([
            {
                "TRADE_DATE":"2026-04-10 00:00:00","MARKET_FOCUS":"88.5",
                "MARKET_FOCUS_RANK":"3","TOTAL_MARKET":"1000"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_FOCUS_RENAME,
            &COMMENT_FOCUS_SELECT,
            &COMMENT_FOCUS_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&COMMENT_FOCUS_DATE).unwrap();
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), COMMENT_FOCUS_SELECT);
        let f = df.inner().column("用户关注指数").unwrap().f64().unwrap();
        assert_eq!(f.get(0), Some(88.5));
        let d = df.inner().column("交易日").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证千股千评-市场参与意愿列契约（JSONP 剥离后；无序号 + 数值化 + 日期截断）。
    #[test]
    fn comment_desire_offline() {
        let rows = json!([
            {
                "SECURITY_INNER_CODE":"1000002165","SECURITY_CODE":"600000",
                "TRADE_DATE":"2026-04-10 00:00:00","PARTICIPATION_WISH":"75.3",
                "PARTICIPATION_WISH_5DAYS":"70.1","PARTICIPATION_WISH_CHANGE":"5.2",
                "PARTICIPATION_WISH_5DAYSCHANGE":"-2.1"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &COMMENT_DESIRE_RENAME,
            &COMMENT_DESIRE_SELECT,
            &COMMENT_DESIRE_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&COMMENT_DESIRE_DATE).unwrap();
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), COMMENT_DESIRE_SELECT);
        // 内部代码已丢弃
        assert!(df.inner().column("内部代码").is_err());
        let w = df.inner().column("参与意愿").unwrap().f64().unwrap();
        assert_eq!(w.get(0), Some(75.3));
        let d = df.inner().column("交易日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2026-04-10"));
    }

    /// 离线验证商誉-行业商誉列契约（无序号 + 数值化，无日期列）。
    #[test]
    fn sy_hy_offline() {
        let rows = json!([
            {
                "REPORT_DATE":"2024-09-30 00:00:00","INDUSTRY_NAME":"软件服务",
                "INDUSTRY_CODE":"1271","ORG_NUM":"300","GOODWILL":"5000000",
                "GOODWILL_CHANGE":"100000","SUMSHEQUITY":"80000000",
                "SUMSHEQUITY_RATIO":"6.25","SE_CHANGE_RATIO":"0.1",
                "PARENTNETPROFIT":"9000000","PNP_CHANGE_RATIO":"1.1"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df =
            finalize_report(&rows, &SY_HY_RENAME, &SY_HY_SELECT, &SY_HY_NUMERIC, None).unwrap();
        assert_ne!(df.column_names()[0], "序号");
        assert_eq!(df.column_names(), SY_HY_SELECT);
        let g = df.inner().column("商誉规模").unwrap().f64().unwrap();
        assert_eq!(g.get(0), Some(5000000.0));
        let ratio = df
            .inner()
            .column("商誉规模占净资产规模比例")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(ratio.get(0), Some(6.25));
        // 行业代码/商誉减值等被丢弃
        assert!(df.inner().column("行业代码").is_err());
    }

    /// 离线验证龙虎榜详情列契约（序号 + 数值化 + 日期截断）。
    #[test]
    fn lhb_detail_1j_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行",
                "TRADE_DATE":"2024-04-17 00:00:00","EXPLAIN":"换手率达20%","CLOSE_PRICE":"10.5",
                "CHANGE_RATE":"9.9","BILLBOARD_NET_AMT":"100.0","BILLBOARD_BUY_AMT":"200.0",
                "BILLBOARD_SELL_AMT":"100.0","BILLBOARD_DEAL_AMT":"300.0","ACCUM_AMOUNT":"5000.0",
                "DEAL_NET_RATIO":"0.02","DEAL_AMOUNT_RATIO":"0.06","TURNOVERRATE":"3.5",
                "FREE_MARKET_CAP":"2000000000","EXPLANATION":"日涨幅偏离值达7%","D1_CLOSE_ADJCHRATE":"1.1",
                "D2_CLOSE_ADJCHRATE":"2.2","D5_CLOSE_ADJCHRATE":"5.5","D10_CLOSE_ADJCHRATE":"10.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &LHB_DETAIL_RENAME,
            &LHB_DETAIL_SELECT,
            &LHB_DETAIL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&LHB_DETAIL_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_DETAIL_SELECT);
        let net = df.inner().column("龙虎榜净买额").unwrap().f64().unwrap();
        assert_eq!(net.get(0), Some(100.0));
        let date = df.inner().column("上榜日").unwrap().str().unwrap();
        assert_eq!(date.get(0), Some("2024-04-17"));
        assert_eq!(
            df.inner().column("解读").unwrap().str().unwrap().get(0),
            Some("换手率达20%")
        );
    }

    /// 离线验证机构席位追踪列契约（序号 + 全数值化）。
    #[test]
    fn lhb_jgstatistic_1j_offline() {
        let rows = json!([
            {
                "SECURITY_CODE":"000001","SECURITY_NAME_ABBR":"平安银行","CLOSE_PRICE":"10.5",
                "CHANGE_RATE":"9.9","AMOUNT":"300.0","ONLIST_TIMES":"5","BUY_AMT":"80.0",
                "BUY_TIMES":"2","SELL_AMT":"30.0","SELL_TIMES":"1","NET_BUY_AMT":"50.0",
                "M1_CLOSE_ADJCHRATE":"1.1","M3_CLOSE_ADJCHRATE":"3.3","M6_CLOSE_ADJCHRATE":"6.6",
                "Y1_CLOSE_ADJCHRATE":"12.0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &LHB_JGSTAT_RENAME,
            &LHB_JGSTAT_SELECT,
            &LHB_JGSTAT_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_JGSTAT_SELECT);
        let amt = df.inner().column("机构净买额").unwrap().f64().unwrap();
        assert_eq!(amt.get(0), Some(50.0));
        assert_eq!(df.column_names().len(), 16);
    }

    /// 离线验证每日活跃营业部列契约（positional 字段映射 + 数值化 + 日期截断）。
    #[test]
    fn lhb_hyyyb_1j_offline() {
        let rows = json!([
            {
                "OPERATEDEPT_NAME":"华泰证券深圳分公司","ONLIST_DATE":"2024-04-17 00:00:00",
                "BUYER_APPEAR_NUM":"3","SELLER_APPEAR_NUM":"2","TOTAL_BUYAMT":"900.0",
                "TOTAL_SELLAMT":"400.0","TOTAL_NETAMT":"500.0","BUY_STOCK":"平安银行",
                "OPERATEDEPT_CODE":"10188715","SECURITY_NAME_ABBR":"平安银行",
                "OPERATEDEPT_CODE_OLD":"0","ORG_NAME_ABBR":"华泰"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &LHB_HYYYB_RENAME,
            &LHB_HYYYB_SELECT,
            &LHB_HYYYB_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&LHB_HYYYB_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_HYYYB_SELECT);
        // 第 8/11/12 位占位 "-"（BUY_STOCK/OPERATEDEPT_CODE_OLD/ORG_NAME_ABBR）被丢弃
        assert!(df.inner().column("BUY_STOCK").is_err());
        assert!(df.inner().column("ORG_NAME_ABBR").is_err());
        assert_eq!(
            df.inner().column("买入股票").unwrap().str().unwrap().get(0),
            Some("平安银行")
        );
        assert_eq!(
            df.inner().column("上榜日").unwrap().str().unwrap().get(0),
            Some("2024-04-17")
        );
    }

    /// 离线验证营业部排行列契约（序号 + 15 指标全数值化）。
    #[test]
    fn lhb_yybph_1j_offline() {
        let rows = json!([
            {
                "OPERATEDEPT_NAME":"华泰证券深圳分公司",
                "TOTAL_BUYER_SALESTIMES_1DAY":"5","AVERAGE_INCREASE_1DAY":"1.1","RISE_PROBABILITY_1DAY":"0.6",
                "TOTAL_BUYER_SALESTIMES_2DAY":"4","AVERAGE_INCREASE_2DAY":"2.2","RISE_PROBABILITY_2DAY":"0.5",
                "TOTAL_BUYER_SALESTIMES_3DAY":"3","AVERAGE_INCREASE_3DAY":"3.3","RISE_PROBABILITY_3DAY":"0.4",
                "TOTAL_BUYER_SALESTIMES_5DAY":"2","AVERAGE_INCREASE_5DAY":"5.5","RISE_PROBABILITY_5DAY":"0.3",
                "TOTAL_BUYER_SALESTIMES_10DAY":"1","AVERAGE_INCREASE_10DAY":"10.0","RISE_PROBABILITY_10DAY":"0.2"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &LHB_YYBPH_RENAME,
            &LHB_YYBPH_SELECT,
            &LHB_YYBPH_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_YYBPH_SELECT);
        assert_eq!(df.column_names().len(), 17);
        let v = df
            .inner()
            .column("上榜后1天-买入次数")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(v.get(0), Some(5.0));
    }

    /// 离线验证营业部统计列契约（序号 + 数值化）。
    #[test]
    fn lhb_traderstatistic_1j_offline() {
        let rows = json!([
            {
                "OPERATEDEPT_NAME":"华泰证券深圳分公司","AMOUNT":"300.0","SALES_ONLIST_TIMES":"5",
                "ACT_BUY":"80.0","TOTAL_BUYER_SALESTIMES":"2","ACT_SELL":"30.0",
                "TOTAL_SELLER_SALESTIMES":"1","OPERATEDEPT_CODE_OLD":"0","ORG_NAME_ABBR":"华泰"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &LHB_TRADER_RENAME,
            &LHB_TRADER_SELECT,
            &LHB_TRADER_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_TRADER_SELECT);
        assert_eq!(
            df.inner()
                .column("龙虎榜成交金额")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(300.0)
        );
    }

    /// 离线验证个股龙虎榜详情-日期列契约（序号 + 日期截断 + 无数值列）。
    #[test]
    fn lhb_stock_detail_date_1j_offline() {
        let rows = json!([
            {"SECURITY_CODE":"600077","TRADE_DATE":"2007-04-16 00:00:00","TR_DATE":"2007-04-16"}
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &LHB_BOARDDATE_RENAME,
            &LHB_BOARDDATE_SELECT,
            &LHB_BOARDDATE_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&LHB_BOARDDATE_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_BOARDDATE_SELECT);
        // 第 4 位 TR_DATE 占位丢弃
        assert!(df.inner().column("TR_DATE").is_err());
        assert_eq!(
            df.inner().column("交易日").unwrap().str().unwrap().get(0),
            Some("2007-04-16")
        );
    }

    /// 离线验证个股龙虎榜详情列契约（买入/卖出双分支共用 RENAME；类型=EXPLANATION）。
    #[test]
    fn lhb_stock_detail_1j_offline() {
        let rows = json!([
            {
                "OPERATEDEPT_NAME":"华泰证券深圳分公司","EXPLANATION":"买入榜","BUY":"200.0",
                "SELL":"100.0","NET":"100.0","TOTAL_BUYRIO":"0.3","TOTAL_SELLRIO":"0.15",
                "CHANGE_TYPE":"买","TRADE_ID":"1"
            },
            {
                "OPERATEDEPT_NAME":"中信证券上海分公司","EXPLANATION":"卖出榜","BUY":"50.0",
                "SELL":"300.0","NET":"-250.0","TOTAL_BUYRIO":"0.1","TOTAL_SELLRIO":"0.45",
                "CHANGE_TYPE":"卖","TRADE_ID":"2"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let df = finalize_report(
            &rows,
            &LHB_DETAIL_STOCK_RENAME,
            &LHB_DETAIL_STOCK_SELECT,
            &LHB_DETAIL_STOCK_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_DETAIL_STOCK_SELECT);
        assert_eq!(df.column_names().len(), 8);
        let buy = df.inner().column("买入金额").unwrap().f64().unwrap();
        assert_eq!(buy.get(0), Some(200.0));
        assert_eq!(buy.get(1), Some(50.0));
        // 类型 由 EXPLANATION 映射（非 CHANGE_TYPE）
        let typ = df.inner().column("类型").unwrap().str().unwrap();
        assert_eq!(typ.get(0), Some("买入榜"));
        assert_eq!(typ.get(1), Some("卖出榜"));
        assert!(df.inner().column("CHANGE_TYPE").is_err());
    }

    /// 离线验证营业部历史交易明细列契约（序号 + 数值化 + 日期截断）。
    #[test]
    fn lhb_yyb_detail_1j_offline() {
        let rows = json!([
            {
                "OPERATEDEPT_CODE":"10188715","OPERATEDEPT_NAME":"华泰证券深圳分公司","ORG_NAME_ABBR":"华泰",
                "TRADE_DATE":"2022-03-15 00:00:00","SECURITY_CODE":"000788","SECURITY_NAME_ABBR":"北大医药",
                "CHANGE_RATE":"9.9","ACT_BUY":"200.0","ACT_SELL":"100.0","NET_AMT":"100.0",
                "EXPLANATION":"日涨幅偏离值达7%","D1_CLOSE_ADJCHRATE":"1.1","D2_CLOSE_ADJCHRATE":"2.2",
                "D3_CLOSE_ADJCHRATE":"3.3","D5_CLOSE_ADJCHRATE":"5.5","D10_CLOSE_ADJCHRATE":"10.0",
                "D20_CLOSE_ADJCHRATE":"20.0","D30_CLOSE_ADJCHRATE":"30.0","SECUCODE":"000788.SZ",
                "OPERATEDEPT_CODE_OLD":"0"
            }
        ]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &LHB_YYB_DETAIL_RENAME,
            &LHB_YYB_DETAIL_SELECT,
            &LHB_YYB_DETAIL_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&LHB_YYB_DETAIL_DATE).unwrap();
        assert_eq!(df.column_names()[0], "序号");
        assert_eq!(df.column_names()[1..], LHB_YYB_DETAIL_SELECT);
        assert_eq!(df.column_names().len(), 19);
        assert_eq!(
            df.inner().column("交易日期").unwrap().str().unwrap().get(0),
            Some("2022-03-15")
        );
        assert_eq!(
            df.inner().column("净额").unwrap().f64().unwrap().get(0),
            Some(100.0)
        );
        assert!(df.inner().column("SECUCODE").is_err());
    }

    #[test]
    #[ignore]
    fn live_columns_smoke() {
        match stock_cy_a_spot_em() {
            Ok(df) => {
                println!("CY cols={:?} rows={}", df.column_names(), df.height());
                assert_eq!(df.column_names(), SPOT_SELECT);
            }
            Err(e) => println!("CY spot network/blocked: {e}"),
        }
        match stock_zh_a_gdhs("最新") {
            Ok(df) => {
                println!("GDHS cols={:?} rows={}", df.column_names(), df.height());
                assert_eq!(df.column_names(), GDHS_SELECT);
            }
            Err(e) => println!("GDHS network/blocked: {e}"),
        }
    }

    /// 离线验证 北交所 新股申购列契约 + 计算列 最新价格-累计涨幅。
    #[test]
    fn xgsglb_neeq_offline() {
        let rows = json!([{
            "SECURITY_CODE":"920002","SECUCODE":"920002.BJ","SECURITY_NAME_ABBR":"测试股",
            "APPLY_CODE":"920002","EXPECT_ISSUE_NUM":"20000000","ISSUE_PRICE":"10.5",
            "ISSUE_PE_RATIO":"20.0","APPLY_DATE":"2024-01-02T00:00:00",
            "RESULT_NOTICE_DATE":"2024-01-03","SELECT_LISTING_DATE":"2024-01-10T00:00:00",
            "ONLINE_ISSUE_NUM":"16000000","APPLY_AMT_UPPER":"5000000","APPLY_NUM_UPPER":"16000",
            "ONLINE_PAY_DATE":"2024-01-05","ONLINE_REFUND_DATE":"2024-01-08",
            "ONLINE_ISSUE_LWR":"0.02","NEWEST_PRICE":25.0,"CLOSE_PRICE":30.0,
            "PER_SHARES_INCOME":"150.0","LD_CLOSE_CHANGE":"42.8","TURNOVERRATE":"0.5",
            "AMPLITUDE":"12.0","MAIN_BUSINESS":"主营","INDUSTRY_PE_RATIO":"18.0",
            "APPLY_AMT_100":"250000","TAKE_UP_TIME":"3","CAPTURE_PROFIT":"5.0",
            "APPLY_SHARE_100":"8000","AVERAGE_PRICE":"28.0","ORG_VAN":"5000","VA_AMT":"1000000"
        }]);
        let mut rows = rows.as_array().unwrap().clone();
        // 预计算 最新价格-累计涨幅 = 首日收盘价 / 最新价格-价格
        for row in &mut rows {
            let obj = row.as_object_mut().unwrap();
            let c = obj.get("CLOSE_PRICE").and_then(Value::as_f64).unwrap();
            let n = obj.get("NEWEST_PRICE").and_then(Value::as_f64).unwrap();
            obj.insert("COMPUTED_CUMCHG".into(), Value::String((c / n).to_string()));
        }
        let mut df = finalize_report(
            &rows,
            &XGSG_NEEQ_RENAME,
            &XGSG_NEEQ_SELECT,
            &XGSG_NEEQ_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&XGSG_NEEQ_DATE).unwrap();
        assert_eq!(df.column_names(), XGSG_NEEQ_SELECT);
        assert_eq!(df.height(), 1);
        // 累计涨幅 = 30 / 25 = 1.2
        let cum = df
            .inner()
            .column("最新价格-累计涨幅")
            .unwrap()
            .f64()
            .unwrap();
        assert_eq!(cum.get(0), Some(1.2));
        let px = df.inner().column("发行价格").unwrap().f64().unwrap();
        assert_eq!(px.get(0), Some(10.5));
        let d = df.inner().column("申购日").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-01-02"));
    }

    /// 离线验证 非北交所 新股申购列契约（24 列，无 序号）。
    #[test]
    fn xgsglb_ipo_offline() {
        let rows = json!([{
            "SECURITY_CODE":"001234","SECURITY_NAME":"测试新股","TRADE_MARKET":"深圳","MARKET_TYPE":"主板",
            "ISSUE_NUM":"50000000","ONLINE_ISSUE_NUM":"40000000","TOP_APPLY_MARKETCAP":"300000",
            "ONLINE_APPLY_UPPER":"40000","ISSUE_PRICE":"15.0","LATELY_PRICE":"28.0","CLOSE_PRICE":"32.0",
            "APPLY_DATE":"2024-02-01T00:00:00","BALLOT_NUM_DATE":"2024-02-03T00:00:00",
            "BALLOT_PAY_DATE":"2024-02-05T00:00:00","LISTING_DATE":"2024-02-10T00:00:00",
            "AFTER_ISSUE_PE":"22.0","ONLINE_ISSUE_LWR":"0.03","INITIAL_MULTIPLE":"1200.0",
            "INDUSTRY_PE_NEW":"19.0","OFFLINE_EP_OBJECT":"300","CONTINUOUS_1WORD_NUM":"3",
            "TOTAL_CHANGE":"114.0","PROFIT":"8500.0"
        }]);
        let rows = rows.as_array().unwrap().clone();
        let mut df = finalize_report(
            &rows,
            &XGSG_IPO_RENAME,
            &XGSG_IPO_SELECT,
            &XGSG_IPO_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&XGSG_IPO_DATE).unwrap();
        assert_eq!(df.column_names(), XGSG_IPO_SELECT);
        assert_eq!(df.height(), 1);
        let profit = df.inner().column("每中一签获利").unwrap().f64().unwrap();
        assert_eq!(profit.get(0), Some(8500.0));
        let listing = df.inner().column("上市日期").unwrap().str().unwrap();
        assert_eq!(listing.get(0), Some("2024-02-10"));
    }

    /// 离线验证 分析师排名动态列契约（year=2024，含动态 {year} 列 + 序号）。
    #[test]
    fn analyst_rank_offline() {
        let (rename, select, numeric, date) = analyst_rank_cols("2024");
        let rename_ref: Vec<(&str, &str)> = rename
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let select_ref: Vec<&str> = select.iter().map(|s| s.as_str()).collect();
        let numeric_ref: Vec<&str> = numeric.iter().map(|s| s.as_str()).collect();
        let date_ref: Vec<&str> = vec![date.as_str()];

        let rows = json!([{
            "ANALYST_CODE":"11000200926","ANALYST_NAME":"张三","TRADE_DATE":"2024-03-01T00:00:00",
            "YEAR":"2024","ORG_NAME":"某券商","ORG_CODE":"x","INDEX_VALUE":"1200.5",
            "YEAR_YIELD":"0.35","YIELD_3":"0.05","YIELD_6":"0.12","YIELD_12":"0.20",
            "SECURITY_COUNT":"5","SECURITY_NAME_ABBR":"股票A","SECUCODE":"000001",
            "SECURITY_CODE":"000001","NEWEST_STOCK_RATING":"买入","INDUSTRY_CODE":"C39","INDUSTRY_NAME":"医药"
        }]);
        let mut df = finalize_report(
            &rows.as_array().unwrap().clone(),
            &rename_ref,
            &select_ref,
            &numeric_ref,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&date_ref).unwrap();
        assert_eq!(
            df.column_names(),
            vec![
                "序号",
                "分析师名称",
                "分析师单位",
                "年度指数",
                "2024年收益率",
                "3个月收益率",
                "6个月收益率",
                "12个月收益率",
                "成分股个数",
                "2024最新个股评级-股票名称",
                "2024最新个股评级-股票代码",
                "分析师ID",
                "行业代码",
                "行业",
                "更新日期",
                "年度"
            ]
        );
        assert_eq!(df.height(), 1);
        let y = df.inner().column("2024年收益率").unwrap().f64().unwrap();
        assert_eq!(y.get(0), Some(0.35));
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
        let d = df.inner().column("更新日期").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-03-01"));
    }

    /// 离线验证 分析师详情-最新跟踪成分股 列契约（9 列含 序号）。
    #[test]
    fn analyst_detail_ntcs_offline() {
        let rows = json!([{
            "RATING_DATE":"2024-02-01T00:00:00","TRADE_MARKET_CODE":"1","ANALYST_CODE":"11000200926",
            "ANALYST_NAME":"张三","TRADE_DATE":"2024-01-01T00:00:00","SECURITY_CODE":"000001",
            "SECUCODE":"000001.SZ","SECURITY_NAME_ABBR":"平安银行","CHANGE_DATE":"2024-01-15T00:00:00",
            "RATING_NAME":"买入","CLOSE_FORWARD_ADJPRICE":"12.5","NEW_PRICE":"13.0","CURRENT_CHANGE":"4.0"
        }]);
        let mut df = finalize_report(
            &rows.as_array().unwrap().clone(),
            &ANALYST_NTCS_RENAME,
            &ANALYST_NTCS_SELECT,
            &ANALYST_NTCS_NUMERIC,
            Some("序号"),
        )
        .unwrap();
        df.cast_date(&ANALYST_NTCS_DATE).unwrap();
        assert_eq!(
            df.column_names(),
            vec![
                "序号",
                "股票代码",
                "股票名称",
                "调入日期",
                "最新评级日期",
                "当前评级名称",
                "成交价格(前复权)",
                "最新价格",
                "阶段涨跌幅"
            ]
        );
        assert_eq!(df.height(), 1);
        let p = df.inner().column("最新价格").unwrap().f64().unwrap();
        assert_eq!(p.get(0), Some(13.0));
        let idx = df.inner().column("序号").unwrap().f64().unwrap();
        assert_eq!(idx.get(0), Some(1.0));
    }

    /// 离线验证 分析师详情-历史指数 列契约（2 列 date/value，按 date 升序）。
    #[test]
    fn analyst_detail_hisidx_offline() {
        let mut rows = json!([
            {"TRADE_DATE":"2024-03-01T00:00:00","INDEX_HVALUE":"1100.5"},
            {"TRADE_DATE":"2024-01-01T00:00:00","INDEX_HVALUE":"1000.0"},
            {"TRADE_DATE":"2024-02-01T00:00:00","INDEX_HVALUE":"1050.0"},
        ])
        .as_array()
        .unwrap()
        .clone();
        rows.sort_by(|a, b| {
            let ka = a.get("TRADE_DATE").and_then(Value::as_str).unwrap_or("");
            let kb = b.get("TRADE_DATE").and_then(Value::as_str).unwrap_or("");
            ka.cmp(kb)
        });
        let mut df = finalize_report(
            &rows,
            &ANALYST_HISIDX_RENAME,
            &ANALYST_HISIDX_SELECT,
            &ANALYST_HISIDX_NUMERIC,
            None,
        )
        .unwrap();
        df.cast_date(&ANALYST_HISIDX_DATE).unwrap();
        assert_eq!(df.column_names(), vec!["date", "value"]);
        assert_eq!(df.height(), 3);
        let d = df.inner().column("date").unwrap().str().unwrap();
        assert_eq!(d.get(0), Some("2024-01-01"));
        assert_eq!(d.get(2), Some("2024-03-01"));
        let v = df.inner().column("value").unwrap().f64().unwrap();
        assert_eq!(v.get(0), Some(1000.0));
    }
}
