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
    fetch_clist, fetch_datacenter_pages, finalize_report, finalize_spot, push2_urls,
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
}
