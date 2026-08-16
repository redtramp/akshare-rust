//! 同花顺-数据中心-资金流向（`data.10jqka.com.cn/funds/*`）。
//!
//! 对应 akshare `stock_feature/stock_fund_flow.py`：个股/概念/行业资金流与大单追踪。
//! 数据源为同花顺数据中心 HTML 表格（`table.m-table.J-ajax-table`），经 `sources::ths`
//! 的 `fetch_ths_rank`（携带 `Cookie: v={hexin-v}` 令牌 + 自动分页）抓取，再用
//! `parse_ths_table` 提取每行 `td` 文本。
//!
//! 列名与 akshare 逐字对齐：akshare 在 `pd.read_html` 后按固定数组重命名（剥离
//! `(元)`/`(亿)`/`↓` 等后缀、`del 序号` 后以 1 基序号重排），本模块以相同的硬编码
//! 列名数组还原。数值列（同 `pd.read_html` 自动推断）通过 `cast_numeric` 转为浮点，
//! 概念/行业「即时」的 `行业-涨跌幅`/`领涨股-涨跌幅` 先 `strip_suffix("%")` 再数值化。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::sources::ths::fetch_ths_rank;

/// 同花顺资金流向数据中心根域。
const FF_HOST: &str = "http://data.10jqka.com.cn/funds";

// === 个股资金流（ggzjl）列契约 ===
const FF_INDIVIDUAL_IMM_SELECT: [&str; 10] = [
    "序号",
    "股票代码",
    "股票简称",
    "最新价",
    "涨跌幅",
    "换手率",
    "流入资金",
    "流出资金",
    "净额",
    "成交额",
];
const FF_INDIVIDUAL_IMM_NUMERIC: [&str; 3] = ["序号", "股票代码", "最新价"];
const FF_INDIVIDUAL_PER_SELECT: [&str; 7] = [
    "序号",
    "股票代码",
    "股票简称",
    "最新价",
    "阶段涨跌幅",
    "连续换手率",
    "资金流入净额",
];
const FF_INDIVIDUAL_PER_NUMERIC: [&str; 3] = ["序号", "股票代码", "最新价"];

// === 概念/行业资金流（gnzjl / hyzjl）列契约（两者结构一致） ===
const FF_SECTOR_IMM_SELECT: [&str; 11] = [
    "序号",
    "行业",
    "行业指数",
    "行业-涨跌幅",
    "流入资金",
    "流出资金",
    "净额",
    "公司家数",
    "领涨股",
    "领涨股-涨跌幅",
    "当前价",
];
const FF_SECTOR_IMM_NUMERIC: [&str; 9] = [
    "序号",
    "行业指数",
    "行业-涨跌幅",
    "流入资金",
    "流出资金",
    "净额",
    "公司家数",
    "领涨股-涨跌幅",
    "当前价",
];
const FF_SECTOR_PER_SELECT: [&str; 8] = [
    "序号",
    "行业",
    "公司家数",
    "行业指数",
    "阶段涨跌幅",
    "流入资金",
    "流出资金",
    "净额",
];
const FF_SECTOR_PER_NUMERIC: [&str; 6] = [
    "序号",
    "公司家数",
    "行业指数",
    "流入资金",
    "流出资金",
    "净额",
];
/// 概念/行业「即时」需先剥 `%` 再数值化的涨跌幅列。
const FF_SECTOR_PCT_COLS: [&str; 2] = ["行业-涨跌幅", "领涨股-涨跌幅"];

// === 大单追踪（ddzz）列契约 ===
const FF_BIG_DEAL_SELECT: [&str; 9] = [
    "成交时间",
    "股票代码",
    "股票简称",
    "成交价格",
    "成交量",
    "成交额",
    "大单性质",
    "涨跌幅",
    "涨跌额",
];
const FF_BIG_DEAL_NUMERIC: [&str; 5] = ["股票代码", "成交价格", "成交量", "成交额", "涨跌额"];

/// 资金流周期参数 → 板块编号（对应 akshare 的 `board/{n}` 路径段）。
fn board_segment(symbol: &str) -> Result<Option<&'static str>> {
    match symbol {
        "即时" => Ok(None),
        "3日排行" => Ok(Some("3")),
        "5日排行" => Ok(Some("5")),
        "10日排行" => Ok(Some("10")),
        "20日排行" => Ok(Some("20")),
        other => Err(AkshareError::Param(format!("未知资金流周期: {other}"))),
    }
}

/// 由已抓取的原始行（每行为 `td` 文本，首列为页面序号或成交时间）构建资金流表。
///
/// `with_seq=true`（个股/概念/行业）：丢弃首列页面序号，前置 1 基 `序号`；
/// `with_seq=false`（大单追踪）：丢弃末列 `详细`。随后对 `strip_pct` 剥 `%`、
/// 对 `numeric` 列数值化。I/O 与构建分离，便于离线单测。
fn build_fund_flow(
    raw: &[Vec<String>],
    select: &[&str],
    numeric: &[&str],
    with_seq: bool,
    strip_pct: &[&str],
) -> Result<Df> {
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(raw.len());
    for (i, r) in raw.iter().enumerate() {
        if with_seq {
            let mut row = Vec::with_capacity(select.len());
            row.push(Some((i + 1).to_string()));
            let tail: &[String] = if r.len() > 1 { &r[1..] } else { &[] };
            for v in tail {
                row.push(Some(v.clone()));
            }
            rows.push(row);
        } else {
            // 大单追踪：丢弃末列 详细
            let end = r.len().saturating_sub(1);
            let mut row = Vec::with_capacity(select.len());
            for v in &r[..end] {
                row.push(Some(v.clone()));
            }
            rows.push(row);
        }
    }
    let mut df = Df::from_string_rows(select, &rows)?;
    if !strip_pct.is_empty() {
        df.strip_suffix(strip_pct, "%")?;
    }
    df.cast_numeric(numeric)?;
    Ok(df)
}

/// 同花顺-数据中心-资金流向-个股资金流（`stock_fund_flow_individual`）。
///
/// `symbol`：`即时` / `3日排行` / `5日排行` / `10日排行` / `20日排行`。
pub fn stock_fund_flow_individual(symbol: &str) -> Result<Df> {
    let board = board_segment(symbol)?;
    let base = match board {
        None => format!("{FF_HOST}/ggzjl/field/zdf/order/desc"),
        Some(b) => format!("{FF_HOST}/ggzjl/board/{b}/field/zdf/order/desc"),
    };
    let (select, numeric) = if symbol == "即时" {
        (
            &FF_INDIVIDUAL_IMM_SELECT[..],
            &FF_INDIVIDUAL_IMM_NUMERIC[..],
        )
    } else {
        (
            &FF_INDIVIDUAL_PER_SELECT[..],
            &FF_INDIVIDUAL_PER_NUMERIC[..],
        )
    };
    let url_for_page = move |page: u32| format!("{base}/page/{page}/ajax/1/free/1/");
    let raw = fetch_ths_rank(&url_for_page)?;
    build_fund_flow(&raw, select, numeric, true, &[])
}

/// 同花顺-数据中心-资金流向-概念资金流（`stock_fund_flow_concept`）。
pub fn stock_fund_flow_concept(symbol: &str) -> Result<Df> {
    let board = board_segment(symbol)?;
    let base = match board {
        None => format!("{FF_HOST}/gnzjl/field/tradezdf/order/desc"),
        Some(b) => format!("{FF_HOST}/gnzjl/board/{b}/field/tradezdf/order/desc"),
    };
    let (select, numeric, strip_pct) = if symbol == "即时" {
        (
            &FF_SECTOR_IMM_SELECT[..],
            &FF_SECTOR_IMM_NUMERIC[..],
            &FF_SECTOR_PCT_COLS[..],
        )
    } else {
        (
            &FF_SECTOR_PER_SELECT[..],
            &FF_SECTOR_PER_NUMERIC[..],
            &[][..],
        )
    };
    let url_for_page = move |page: u32| format!("{base}/page/{page}/ajax/1/free/1/");
    let raw = fetch_ths_rank(&url_for_page)?;
    build_fund_flow(&raw, select, numeric, true, strip_pct)
}

/// 同花顺-数据中心-资金流向-行业资金流（`stock_fund_flow_industry`）。
pub fn stock_fund_flow_industry(symbol: &str) -> Result<Df> {
    let board = board_segment(symbol)?;
    let base = match board {
        None => format!("{FF_HOST}/hyzjl/field/tradezdf/order/desc"),
        Some(b) => format!("{FF_HOST}/hyzjl/board/{b}/field/tradezdf/order/desc"),
    };
    let (select, numeric, strip_pct) = if symbol == "即时" {
        (
            &FF_SECTOR_IMM_SELECT[..],
            &FF_SECTOR_IMM_NUMERIC[..],
            &FF_SECTOR_PCT_COLS[..],
        )
    } else {
        (
            &FF_SECTOR_PER_SELECT[..],
            &FF_SECTOR_PER_NUMERIC[..],
            &[][..],
        )
    };
    let url_for_page = move |page: u32| format!("{base}/page/{page}/ajax/1/free/1/");
    let raw = fetch_ths_rank(&url_for_page)?;
    build_fund_flow(&raw, select, numeric, true, strip_pct)
}

/// 同花顺-数据中心-资金流向-大单追踪（`stock_fund_flow_big_deal`）。
pub fn stock_fund_flow_big_deal() -> Result<Df> {
    let base = format!("{FF_HOST}/ddzz/order/desc");
    let url_for_page = move |page: u32| format!("{base}/page/{page}/ajax/1/free/1/");
    let raw = fetch_ths_rank(&url_for_page)?;
    build_fund_flow(
        &raw,
        &FF_BIG_DEAL_SELECT[..],
        &FF_BIG_DEAL_NUMERIC[..],
        false,
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fund_flow_individual_immediate_build_offline() {
        let raw = vec![vec![
            "1".to_string(),
            "688485".to_string(),
            "九州一轨".to_string(),
            "72.10".to_string(),
            "20.01%".to_string(),
            "12.25%".to_string(),
            "6.60亿".to_string(),
            "6.18亿".to_string(),
            "4216.11万".to_string(),
            "12.79亿".to_string(),
        ]];
        let df = build_fund_flow(
            &raw,
            &FF_INDIVIDUAL_IMM_SELECT,
            &FF_INDIVIDUAL_IMM_NUMERIC,
            true,
            &[],
        )
        .unwrap();
        assert_eq!(df.column_names(), FF_INDIVIDUAL_IMM_SELECT.to_vec());
        // 序号/股票代码/最新价 数值化（对应 pd.read_html 自动推断）
        assert_eq!(
            df.inner().column("序号").unwrap().f64().unwrap().get(0),
            Some(1.0)
        );
        assert_eq!(
            df.inner().column("股票代码").unwrap().f64().unwrap().get(0),
            Some(688485.0)
        );
        assert_eq!(
            df.inner().column("最新价").unwrap().f64().unwrap().get(0),
            Some(72.10)
        );
        // 涨跌幅/换手率/流入资金 等含 %/亿，保持字符串
        assert_eq!(
            df.inner().column("涨跌幅").unwrap().str().unwrap().get(0),
            Some("20.01%")
        );
        assert_eq!(
            df.inner().column("流入资金").unwrap().str().unwrap().get(0),
            Some("6.60亿")
        );
    }

    #[test]
    fn fund_flow_individual_period_build_offline() {
        let raw = vec![vec![
            "1".to_string(),
            "301717".to_string(),
            "超纯应材".to_string(),
            "538.00".to_string(),
            "715.28%".to_string(),
            "202.34%".to_string(),
            "11.56亿".to_string(),
        ]];
        let df = build_fund_flow(
            &raw,
            &FF_INDIVIDUAL_PER_SELECT,
            &FF_INDIVIDUAL_PER_NUMERIC,
            true,
            &[],
        )
        .unwrap();
        assert_eq!(df.column_names(), FF_INDIVIDUAL_PER_SELECT.to_vec());
        assert_eq!(
            df.inner().column("最新价").unwrap().f64().unwrap().get(0),
            Some(538.00)
        );
        // 阶段涨跌幅 含 % 保持字符串
        assert_eq!(
            df.inner()
                .column("阶段涨跌幅")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("715.28%")
        );
    }

    #[test]
    fn fund_flow_concept_immediate_build_offline() {
        let raw = vec![vec![
            "1".to_string(),
            "共封装光学(CPO)".to_string(),
            "5833.17".to_string(),
            "2.94%".to_string(),
            "1582.63".to_string(),
            "1449.87".to_string(),
            "132.76".to_string(),
            "205".to_string(),
            "金戈新材".to_string(),
            "29.98%".to_string(),
            "40.89".to_string(),
        ]];
        let df = build_fund_flow(
            &raw,
            &FF_SECTOR_IMM_SELECT,
            &FF_SECTOR_IMM_NUMERIC,
            true,
            &FF_SECTOR_PCT_COLS,
        )
        .unwrap();
        assert_eq!(df.column_names(), FF_SECTOR_IMM_SELECT.to_vec());
        // 涨跌幅列先剥 % 再数值化
        assert_eq!(
            df.inner()
                .column("行业-涨跌幅")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(2.94)
        );
        assert_eq!(
            df.inner()
                .column("领涨股-涨跌幅")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(29.98)
        );
        // 流入资金/行业指数/公司家数/当前价 纯数字数值化
        assert_eq!(
            df.inner().column("流入资金").unwrap().f64().unwrap().get(0),
            Some(1582.63)
        );
        assert_eq!(
            df.inner().column("行业指数").unwrap().f64().unwrap().get(0),
            Some(5833.17)
        );
        // 行业/领涨股 保持字符串
        assert_eq!(
            df.inner().column("行业").unwrap().str().unwrap().get(0),
            Some("共封装光学(CPO)")
        );
        assert_eq!(
            df.inner().column("领涨股").unwrap().str().unwrap().get(0),
            Some("金戈新材")
        );
    }

    #[test]
    fn fund_flow_concept_period_build_offline() {
        let raw = vec![vec![
            "1".to_string(),
            "共封装光学(CPO)".to_string(),
            "205".to_string(),
            "5833.17".to_string(),
            "715.28%".to_string(),
            "1582.63".to_string(),
            "1449.87".to_string(),
            "132.76".to_string(),
        ]];
        let df = build_fund_flow(
            &raw,
            &FF_SECTOR_PER_SELECT,
            &FF_SECTOR_PER_NUMERIC,
            true,
            &[],
        )
        .unwrap();
        assert_eq!(df.column_names(), FF_SECTOR_PER_SELECT.to_vec());
        assert_eq!(
            df.inner().column("行业指数").unwrap().f64().unwrap().get(0),
            Some(5833.17)
        );
        assert_eq!(
            df.inner().column("流入资金").unwrap().f64().unwrap().get(0),
            Some(1582.63)
        );
        // 阶段涨跌幅 含 % 保持字符串
        assert_eq!(
            df.inner()
                .column("阶段涨跌幅")
                .unwrap()
                .str()
                .unwrap()
                .get(0),
            Some("715.28%")
        );
    }

    #[test]
    fn fund_flow_industry_immediate_same_as_concept() {
        // 行业「即时」与概念「即时」列契约完全一致
        let raw = vec![vec![
            "1".to_string(),
            "电子化学品".to_string(),
            "91970.8".to_string(),
            "4.07%".to_string(),
            "171.56".to_string(),
            "155.37".to_string(),
            "16.19".to_string(),
            "43".to_string(),
            "中石科技".to_string(),
            "20.01%".to_string(),
            "67.36".to_string(),
        ]];
        let df = build_fund_flow(
            &raw,
            &FF_SECTOR_IMM_SELECT,
            &FF_SECTOR_IMM_NUMERIC,
            true,
            &FF_SECTOR_PCT_COLS,
        )
        .unwrap();
        assert_eq!(df.column_names(), FF_SECTOR_IMM_SELECT.to_vec());
        assert_eq!(
            df.inner()
                .column("行业-涨跌幅")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(4.07)
        );
        assert_eq!(
            df.inner()
                .column("领涨股-涨跌幅")
                .unwrap()
                .f64()
                .unwrap()
                .get(0),
            Some(20.01)
        );
    }

    #[test]
    fn fund_flow_big_deal_build_offline() {
        let raw = vec![vec![
            "2026-08-14 15:00:01".to_string(),
            "689009".to_string(),
            "九号公司".to_string(),
            "44.48".to_string(),
            "14457".to_string(),
            "64.30".to_string(),
            "卖盘".to_string(),
            "-1.02%".to_string(),
            "-0.46".to_string(),
            "详细".to_string(),
        ]];
        let df =
            build_fund_flow(&raw, &FF_BIG_DEAL_SELECT, &FF_BIG_DEAL_NUMERIC, false, &[]).unwrap();
        // 末列 详细 被丢弃
        assert_eq!(df.column_names(), FF_BIG_DEAL_SELECT.to_vec());
        assert_eq!(df.height(), 1);
        // 成交时间 保持字符串，编号/价格/量/额 数值化
        assert_eq!(
            df.inner().column("成交时间").unwrap().str().unwrap().get(0),
            Some("2026-08-14 15:00:01")
        );
        assert_eq!(
            df.inner().column("股票代码").unwrap().f64().unwrap().get(0),
            Some(689009.0)
        );
        assert_eq!(
            df.inner().column("成交价格").unwrap().f64().unwrap().get(0),
            Some(44.48)
        );
        assert_eq!(
            df.inner().column("成交量").unwrap().f64().unwrap().get(0),
            Some(14457.0)
        );
        assert_eq!(
            df.inner().column("涨跌额").unwrap().f64().unwrap().get(0),
            Some(-0.46)
        );
        // 涨跌幅 含 % 保持字符串
        assert_eq!(
            df.inner().column("涨跌幅").unwrap().str().unwrap().get(0),
            Some("-1.02%")
        );
    }
}
