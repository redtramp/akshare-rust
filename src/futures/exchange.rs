//! 交易所官方数据接口（合约信息 / 仓单 / 交割 / 期转现 / 历史行情）。
//!
//! 对应 akshare：
//! - `futures_derivative/futures_contract_info_*.py`（中金所 / 郑商所 / 大商所 / 广期所 / 上期能源 / 上期所 合约信息）
//! - `futures/futures_warehouse_receipt.py`（仓单）
//! - `futures/futures_to_spot.py`（交割 / 期转现）
//! - `futures/futures_daily_bar.py`（中金所历史行情）
//!
//! 中金所 / 郑商所返回 XML，用本项目内置的扁平 XML 记录提取器解析；其余返回 JSON。
//! 列名 / 列序 / 数值列严格对齐 akshare（含 `pd.to_numeric` / `pd.to_datetime` 的类型约定）。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use crate::sources::eastmoney::finalize_report;
use calamine::{Data, Reader, Xls};
use serde_json::{Map, Value};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36";
/// 上期所/上能中心要求的老旧 UA（对应 akshare `cons.shfe_headers`）。
const UA_MSIE_SHFE: &str = "Mozilla/4.0 (compatible; MSIE 5.5; Windows NT)";

/// JSON 值 → Option<String>（数值走 `to_string`，与 akshare `pd.DataFrame` 后逐单元格 str 一致）。
fn cell(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

/// JSON 值 → Option<String>（仅字符串 / 数值，对应单元格的多种类型归一）。
fn cell_str(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

/// 内置扁平 XML 记录提取器（对应 akshare `xml.etree.ElementTree` 的 `findall`）。
///
/// 适用于「记录元素包含若干叶子子元素 `<TAG>text</TAG>`」的结构（中金所 `INDEX` /
/// 郑商所 `Contract`）。不依赖外部 XML 库，足以覆盖这两个端点的简单结构。
fn xml_records(body: &str, record_tag: &str) -> Result<Vec<Vec<(String, String)>>> {
    let open_pat = format!("<{record_tag}");
    let close_pat = format!("</{record_tag}>");
    let mut records: Vec<Vec<(String, String)>> = Vec::new();
    let mut i = 0;
    while let Some(rel) = body[i..].find(&open_pat) {
        let start = i + rel;
        let after = start + open_pat.len();
        let ok_open = matches!(
            body[after..].chars().next(),
            Some('>') | Some(' ') | Some('\n') | Some('\t')
        );
        if !ok_open {
            i = after;
            continue;
        }
        let open_end = match body[start..].find('>') {
            Some(p) => start + p,
            None => break,
        };
        let close_idx = match body[open_end..].find(&close_pat) {
            Some(p) => open_end + p,
            None => break,
        };
        let inner = &body[open_end + 1..close_idx];
        let mut rec: Vec<(String, String)> = Vec::new();
        let mut j = 0;
        while let Some(rel2) = inner[j..].find('<') {
            let lt = j + rel2;
            let gt = match inner[lt..].find('>') {
                Some(p) => lt + p,
                None => break,
            };
            let tag_full = &inner[lt + 1..gt];
            let tag_name = tag_full.split_whitespace().next().unwrap_or("");
            if tag_name.is_empty() || tag_name.starts_with('/') {
                j = gt + 1;
                continue;
            }
            let close_tag = format!("</{tag_name}>");
            let ce = match inner[gt + 1..].find(&close_tag) {
                Some(p) => gt + 1 + p,
                None => {
                    j = gt + 1;
                    continue;
                }
            };
            let val = unescape_xml(&inner[gt + 1..ce]);
            rec.push((tag_name.to_string(), val));
            j = ce + close_tag.len();
        }
        records.push(rec);
        i = close_idx + close_pat.len();
    }
    Ok(records)
}

/// 还原 5 个标准 XML 实体（对应 `ElementTree` 自动解码的字符引用）。
fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// `YYYYMMDD` -> `YYYY-MM-DD`（对应 akshare `pd.to_datetime(format="%Y%m%d").dt.date`）。
/// 非法 / 空值原样返回（由上层转为空单元格）。
fn fmt_ymd(s: &str) -> String {
    let s = s.trim();
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
    } else {
        s.to_string()
    }
}

/// 由「源键 JSON 对象数组」构建 DataFrame：按 `rename` 重命名、按 `select` 定列序、
/// 按 `numeric` 数值化，并对 `date_src` 中的源键做 `YYYYMMDD`->`YYYY-MM-DD` 归一。
fn rows_to_df(
    rows: &[Value],
    date_src: &[&str],
    rename: &[(&str, &str)],
    select: &[&str],
    numeric: &[&str],
) -> Result<Df> {
    let mut out: Vec<Value> = Vec::with_capacity(rows.len());
    for r in rows {
        let Some(obj) = r.as_object() else { continue };
        let mut m = Map::new();
        for (src, _) in rename {
            let val = match obj.get(*src) {
                Some(Value::String(s)) => {
                    let s = s.trim();
                    if s.is_empty() {
                        Value::Null
                    } else if date_src.contains(src) {
                        Value::String(fmt_ymd(s))
                    } else {
                        Value::String(s.to_string())
                    }
                }
                Some(Value::Number(n)) => Value::String(n.to_string()),
                Some(Value::Null) | None => Value::Null,
                Some(other) => Value::String(other.to_string()),
            };
            m.insert((*src).to_string(), val);
        }
        out.push(Value::Object(m));
    }
    finalize_report(&out, rename, select, numeric, None)
}

/// 由 XML 记录（`<TAG>text</TAG>` 扁平列表）构建 DataFrame。
fn xml_to_df(
    body: &str,
    record_tag: &str,
    date_src: &[&str],
    rename: &[(&str, &str)],
    select: &[&str],
    numeric: &[&str],
) -> Result<Df> {
    let records = xml_records(body, record_tag)?;
    let rows: Vec<Value> = records
        .into_iter()
        .map(|rec| {
            let mut m = Map::new();
            for (k, v) in rec {
                m.insert(k, Value::String(v));
            }
            Value::Object(m)
        })
        .collect();
    rows_to_df(&rows, date_src, rename, select, numeric)
}

// ───────────────────────────── 合约信息：中金所 (XML) ─────────────────────────────

/// 中国金融期货交易所-数据-交易参数（对应 akshare [`akshare.futures_contract_info_cffex`]）。
pub fn futures_contract_info_cffex(date: &str) -> Result<Df> {
    let url = format!(
        "http://www.cffex.com.cn/sj/jycs/{}/{}/index.xml",
        &date[0..6],
        &date[6..]
    );
    let http = HttpClient::default();
    let text = http.get_text_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let rename: &[(&str, &str)] = &[
        ("TRADING_DAY", "查询交易日"),
        ("PRODUCT_ID", "品种"),
        ("INSTRUMENT_ID", "合约代码"),
        ("INSTRUMENT_MONTH", "合约月份"),
        ("BASISPRICE", "挂盘基准价"),
        ("OPEN_DATE", "上市日"),
        ("END_TRADING_DAY", "最后交易日"),
        ("UPPER_VALUE", "涨停板幅度"),
        ("LOWER_VALUE", "跌停板幅度"),
        ("UPPERLIMITPRICE", "涨停板价位"),
        ("LOWERLIMITPRICE", "跌停板价位"),
        ("LONG_LIMIT", "持仓限额"),
    ];
    let select: &[&str] = &[
        "合约代码",
        "合约月份",
        "挂盘基准价",
        "上市日",
        "最后交易日",
        "涨停板幅度",
        "跌停板幅度",
        "涨停板价位",
        "跌停板价位",
        "持仓限额",
        "品种",
        "查询交易日",
    ];
    let numeric: &[&str] = &["挂盘基准价", "涨停板价位", "跌停板价位", "持仓限额"];
    let mut df = xml_to_df(&text, "INDEX", &["TRADING_DAY", "OPEN_DATE", "END_TRADING_DAY"], rename, select, numeric)?;
    df = df.sort_by("合约代码", false, false)?;
    Ok(df)
}

// ───────────────────────────── 合约信息：郑商所 (XML) ─────────────────────────────

/// 郑州商品交易所-交易数据-参考数据（对应 akshare [`akshare.futures_contract_info_czce`]）。
pub fn futures_contract_info_czce(date: &str) -> Result<Df> {
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Future/{}/{}/FutureDataReferenceData.xml",
        &date[0..4],
        date
    );
    let http = HttpClient::default();
    let headers = &[
        ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9"),
        ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/84.0.4147.89 Safari/537.36"),
        ("Host", "www.czce.com.cn"),
    ];
    let text = http.get_text_with_headers(&url, &Map::new(), headers, None)?;
    let rename: &[(&str, &str)] = &[
        ("Name", "产品名称"),
        ("CtrCd", "合约代码"),
        ("PrdCd", "产品代码"),
        ("PrdTp", "产品类型"),
        ("ExchCd", "交易所MIC编码"),
        ("SegTp", "交易场所"),
        ("TrdHrs", "交易时间节假日除外"),
        ("TrdCtyCd", "交易国家ISO编码"),
        ("TrdCcyCd", "交易币种ISO编码"),
        ("ClrngCcyCd", "结算币种ISO编码"),
        ("ExpiryTime", "到期时间待国家公布2025年节假日安排后进行调整"),
        ("SettleTp", "结算方式"),
        ("Duration", "挂牌频率"),
        ("TckSz", "最小变动价位"),
        ("TckVal", "最小变动价值"),
        ("CtrSz", "交易单位"),
        ("MsrmntUnt", "计量单位"),
        ("MaxOrdSz", "最大下单量"),
        ("MnthPosLmt", "日持仓限额期货公司会员不限仓"),
        ("MinBlckTrdSz", "大宗交易最小规模"),
        ("CesrEaaFl", "是否受CESR监管"),
        ("FlexElgblFl", "是否为灵活合约"),
        ("ListCy", "上市周期该产品的所有合约月份"),
        ("DlvryNtcDt", "交割通知日"),
        ("FrstTrdDt", "第一交易日"),
        ("LstTrdDt", "最后交易日待国家公布2025年节假日安排后进行调整"),
        ("DlvrySettleDt", "交割结算日"),
        ("MnthCd", "月份代码"),
        ("YrCd", "年份代码"),
        ("LstDlvryDt", "最后交割日"),
        ("LstDlvryDtBoard", "车（船）板最后交割日"),
        ("DlvryMnth", "合约交割月份本合约交割月份"),
        ("Margin", "交易保证金率"),
        ("PxLim", "涨跌停板"),
        ("FeeCcy", "费用币种ISO编码"),
        ("TrdFee", "交易手续费"),
        ("FeeCollectionType", "手续费收取方式"),
        ("DlvryFee", "交割手续费"),
        ("IntraDayTrdFee", "平今仓手续费"),
        ("TradingLimit", "交易限额"),
    ];
    let select: Vec<&str> = rename.iter().map(|(_, t)| *t).collect();
    let numeric: &[&str] = &["交易手续费", "交割手续费", "平今仓手续费", "交易限额"];
    xml_to_df(
        &text,
        "Contract",
        &["LstDlvryDtBoard"],
        rename,
        &select,
        numeric,
    )
}

// ───────────────────────────── 合约信息：大商所 (JSON) ─────────────────────────────

/// 大连商品交易所-业务数据-交易参数-合约信息（对应 akshare [`akshare.futures_contract_info_dce`]）。
pub fn futures_contract_info_dce() -> Result<Df> {
    let url = "http://www.dce.com.cn/dcereport/publicweb/tradepara/contractInfo";
    let http = HttpClient::default();
    let body = serde_json::json!({"lang": "zh", "tradeType": "1", "varietyId": "all"});
    let v = http.post_json_body(url, &body, &[])?;
    let rows = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename: &[(&str, &str)] = &[
        ("contractId", "合约"),
        ("variety", "品种名称"),
        ("varietyOrder", "品种代码"),
        ("unit", "交易单位"),
        ("tick", "最小变动价位"),
        ("startTradeDate", "开始交易日"),
        ("endTradeDate", "最后交易日"),
        ("endDeliveryDate", "最后交割日"),
        ("tradeType", ""),
    ];
    let select: &[&str] = &[
        "品种名称",
        "合约",
        "交易单位",
        "最小变动价位",
        "开始交易日",
        "最后交易日",
        "最后交割日",
    ];
    let numeric: &[&str] = &["交易单位", "最小变动价位"];
    rows_to_df(
        &rows,
        &["startTradeDate", "endTradeDate", "endDeliveryDate"],
        rename,
        select,
        numeric,
    )
}

// ───────────────────────────── 合约信息：广期所 (JSON) ─────────────────────────────

/// 广期所 POST 请求头（对应 akshare `gfex_headers`；表单体需要 Content-Length，
/// 裸 query 参数会被拒 411，故 `loadList` 用 `post_form` 发送）。
const GFEX_INFO_HEADERS: [(&str, &str); 6] = [
    ("Accept", "application/json, text/javascript, */*; q=0.01"),
    ("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8"),
    ("Origin", "http://www.gfex.com.cn"),
    ("Referer", "http://www.gfex.com.cn/gfex/hyxx/ywcs.shtml"),
    ("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36"),
    ("X-Requested-With", "XMLHttpRequest"),
];

/// 广州期货交易所-业务/服务-合约信息（对应 akshare [`akshare.futures_contract_info_gfex`]）。
pub fn futures_contract_info_gfex() -> Result<Df> {
    let url = "http://www.gfex.com.cn/u/interfacesWebTtQueryContractInfo/loadList";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("variety".into(), Value::String(String::new()));
    params.insert("trade_type".into(), Value::String("0".into()));
    let v = http.post_form(url, &params, &GFEX_INFO_HEADERS)?;
    let rows = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename: &[(&str, &str)] = &[
        ("tradeType", "-"),
        ("variety", "品种"),
        ("varietyOrder", "-"),
        ("contractId", "合约代码"),
        ("unit", "交易单位"),
        ("tick", "最小变动单位"),
        ("startTradeDate", "开始交易日"),
        ("endTradeDate", "最后交易日"),
        ("endDeliveryDate0", "最后交割日"),
    ];
    let select: &[&str] = &[
        "品种",
        "合约代码",
        "交易单位",
        "最小变动单位",
        "开始交易日",
        "最后交易日",
        "最后交割日",
    ];
    let numeric: &[&str] = &["交易单位", "最小变动单位"];
    rows_to_df(
        &rows,
        &["startTradeDate", "endTradeDate", "endDeliveryDate0"],
        rename,
        select,
        numeric,
    )
}

// ──────────────────── 合约信息：上期能源 / 上期所 (JSON .dat) ────────────────────

/// 上海国际能源交易中心-业务指南-交易参数汇总（对应 akshare [`akshare.futures_contract_info_ine`]）。
pub fn futures_contract_info_ine(date: &str) -> Result<Df> {
    let url = format!("https://www.ine.cn/data/busiparamdata/future/ContractBaseInfo{date}.dat");
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("rnd".into(), Value::String("0.8312696798757147".into()));
    let v = http.get_json_with_headers(&url, &params, &[("User-Agent", UA)], None)?;
    let rows = v
        .get("ContractBaseInfo")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    contract_info_shfe_like(&rows)
}

/// 上海期货交易所-交易所服务-业务数据-交易参数汇总查询（对应 akshare [`akshare.futures_contract_info_shfe`]）。
pub fn futures_contract_info_shfe(date: &str) -> Result<Df> {
    let url = format!("https://www.shfe.com.cn/data/busiparamdata/future/ContractBaseInfo{date}.dat");
    let http = HttpClient::default();
    let v = http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let rows = v
        .get("ContractBaseInfo")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut df = contract_info_shfe_like(&rows)?;
    if let Some(u) = v.get("update_date").and_then(Value::as_str) {
        let col: Vec<Option<String>> = (0..df.height()).map(|_| Some(u.to_string())).collect();
        df.with_column("更新时间", &col)?;
    }
    Ok(df)
}

/// `ine` / `shfe` 共用：列名 / 列序 / 数值列一致（`shfe` 额外带 `更新时间` 列）。
fn contract_info_shfe_like(rows: &[Value]) -> Result<Df> {
    let rename: &[(&str, &str)] = &[
        ("BASISPRICE", "挂牌基准价"),
        ("ENDDELIVDATE", "最后交割日"),
        ("EXPIREDATE", "到期日"),
        ("INSTRUMENTID", "合约代码"),
        ("OPENDATE", "上市日"),
        ("STARTDELIVDATE", "开始交割日"),
        ("TRADINGDAY", "交易日"),
    ];
    let select: &[&str] = &[
        "合约代码",
        "上市日",
        "到期日",
        "开始交割日",
        "最后交割日",
        "挂牌基准价",
        "交易日",
    ];
    let numeric: &[&str] = &["挂牌基准价"];
    rows_to_df(
        rows,
        &["OPENDATE", "EXPIREDATE", "STARTDELIVDATE", "ENDDELIVDATE", "TRADINGDAY"],
        rename,
        select,
        numeric,
    )
}

// ───────────────────────────── 仓单日报（warehouse_receipt） ─────────────────────────────

/// 空表（0 列 0 行）。
fn empty_df() -> Result<Df> {
    Df::from_string_rows(&[], &[])
}

/// 读取 xls/xlsx 首个工作表为字符串二维数组（对应 akshare `pd.read_excel`）。
///
/// calamine 直接解析 BIFF8（`.xls`）与原生 xlsx，无需 Python `xlrd`/`openpyxl`。
/// 单元格统一转为字符串：空 → `""`、数值 → 去尾零的十进制、其余 → 原值字符串。
fn xls_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    let cur = std::io::Cursor::new(bytes.to_vec());
    let mut wb = Xls::new(cur)
        .map_err(|e| AkshareError::Empty(format!("xls 解析失败: {e}")))?;
    let range = wb
        .worksheet_range_at(0)
        .ok_or_else(|| AkshareError::Empty("xls 无工作表".into()))?
        .map_err(|e| AkshareError::Empty(format!("读取 xls 工作表失败: {e}")))?;
    let mut rows = Vec::with_capacity(range.height());
    for r in range.rows() {
        let mut row = Vec::with_capacity(r.len());
        for c in r {
            row.push(match c {
                Data::Empty => String::new(),
                Data::String(s) => s.clone(),
                Data::Float(f) => {
                    let v = *f;
                    if v.fract() == 0.0 {
                        format!("{}", v as i64)
                    } else {
                        format!("{v}")
                    }
                }
                Data::Int(i) => i.to_string(),
                Data::Bool(b) => b.to_string(),
                Data::DateTime(_) => String::new(),
                Data::Error(e) => format!("{e:?}"),
                other => other.to_string(),
            });
        }
        rows.push(row);
    }
    Ok(rows)
}

/// 去掉数值字符串中的千分位逗号（对应 akshare `str.replace(",", "")`）。
fn strip_thousands(s: &str) -> String {
    s.replace(',', "")
}

/// 郑州商品交易所-仓单日报（对应 akshare [`akshare.futures_warehouse_receipt_czce`]）。
///
/// 上游返回 `dict`（品种 → DataFrame），无法与 Rust 单一 `Df` 对齐；本项目按「品种」
/// 分节切片后纵向合并为带 `品种` 列的单一 [`Df`]（同 sub-group B 中
/// `futures_foreign_commodity_subscribe_exchange_symbol` 的先例，不注册 parity）。
pub fn futures_warehouse_receipt_czce(date: &str) -> Result<Df> {
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Future/{}/{}/FutureDataWhsheet.xls",
        &date[..4],
        date
    );
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let rows = xls_rows(&bytes)?;
    // 分节行：首列以「品种」开头（如「品种：白糖SR ...」）
    let mut bounds: Vec<usize> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if r.first().map(|c| c.starts_with("品种")).unwrap_or(false) {
            bounds.push(i);
        }
    }
    if bounds.is_empty() {
        return empty_df();
    }
    bounds.push(rows.len());
    let mut out_rows: Vec<Vec<Option<String>>> = Vec::new();
    let mut out_cols: Vec<String> = Vec::new();
    for w in bounds.windows(2) {
        let (start, end) = (w[0], w[1]);
        // 品种代码：取分节标题中首个英文字母串（如「品种：白糖SR」→「SR」）
        let key = regex_first_alpha(&rows[start][0]);
        let header_row = &rows[start + 1];
        if out_cols.is_empty() {
            out_cols = header_row.to_vec();
        }
        for dr in &rows[start + 2..end] {
            if dr.iter().all(|c| c.is_empty()) {
                continue;
            }
            let mut row = vec![Some(key.clone())];
            for (j, h) in out_cols.iter().enumerate() {
                let _ = h;
                let v = dr.get(j).cloned().filter(|c| !c.is_empty());
                row.push(v);
            }
            out_rows.push(row);
        }
    }
    if out_cols.is_empty() {
        return empty_df();
    }
    let mut cols: Vec<&str> = vec!["品种"];
    cols.extend(out_cols.iter().map(String::as_str));
    Df::from_string_rows(&cols, &out_rows)
}

/// 取字符串中首个连续英文字母串（对应 akshare `re.findall(r"[a-zA-Z]+", s)[0]`）。
/// 注意使用字节偏移（`char_indices`）而非字符索引，避免中文前缀导致切片落在多字节字符内部。
fn regex_first_alpha(s: &str) -> String {
    let mut start: Option<usize> = None;
    let mut end: usize = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_alphabetic() {
            if start.is_none() {
                start = Some(i);
            }
            end = i + c.len_utf8();
        } else if start.is_some() {
            break;
        }
    }
    match start {
        Some(s0) => s[s0..end].to_string(),
        None => String::new(),
    }
}

/// 大连商品交易所-仓单日报（对应 akshare [`akshare.futures_warehouse_receipt_dce`]）。
///
/// `date`: 交易日 `YYYYMMDD`。数据源 `dcereport/publicweb/dailystat/wbillWeeklyQuotes`
/// （POST JSON）。注：大商所 `publicweb` 接口存在反爬（412），本环境难以实时校验，
/// 实现逻辑严格对齐 akshare，parity 在无 golden 时自动跳过。
pub fn futures_warehouse_receipt_dce(date: &str) -> Result<Df> {
    let url = "http://www.dce.com.cn/dcereport/publicweb/dailystat/wbillWeeklyQuotes";
    let http = HttpClient::default();
    let body = serde_json::json!({"tradeDate": date, "varietyId": "all"});
    let v = http.post_json_body(url, &body, &[])?;
    let rows = v
        .get("data")
        .and_then(|d| d.get("entityList"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rename: &[(&str, &str)] = &[
        ("variety", "品种名称"),
        ("whAbbr", "仓库/分库"),
        ("deliveryAbbr", "可选提货地点/分库-数量"),
        ("lastWbillQty", "昨日仓单量（手）"),
        ("wbillQty", "今日仓单量（手）"),
        ("diff", "增减（手）"),
        ("varietyOrder", "品种代码"),
    ];
    let select: &[&str] = &[
        "品种代码",
        "品种名称",
        "仓库/分库",
        "可选提货地点/分库-数量",
        "昨日仓单量（手）",
        "今日仓单量（手）",
        "增减（手）",
    ];
    let numeric: &[&str] = &["昨日仓单量（手）", "今日仓单量（手）", "增减（手）"];
    rows_to_df(&rows, &[], rename, select, numeric)
}

/// 上海期货交易所-仓单日报（对应 akshare [`akshare.futures_shfe_warehouse_receipt`]）。
///
/// 上游返回 `dict`（按 `VARNAME` 分品种），本项目合并为带 `品种` 列的单一 [`Df`]，
/// 不注册 parity（同 `futures_warehouse_receipt_czce` 先例）。
pub fn futures_shfe_warehouse_receipt(date: &str) -> Result<Df> {
    let url = format!(
        "https://www.shfe.com.cn/data/tradedata/future/dailydata/{}dailystock.dat",
        date
    );
    let http = HttpClient::default();
    let v = http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", UA_MSIE_SHFE)], None)?;
    let rows = v
        .get("o_cursor")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if rows.is_empty() {
        return empty_df();
    }
    // 列名取首个对象的键；并前置 `品种` 列（= VARNAME 的 `$` 前段）
    let first = rows[0].as_object().expect("o_cursor 元素应为对象");
    let mut cols: Vec<String> = vec!["品种".into()];
    for k in first.keys() {
        cols.push(k.clone());
    }
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        let mut row: Vec<Option<String>> = Vec::with_capacity(cols.len());
        let varname = obj
            .get("VARNAME")
            .and_then(Value::as_str)
            .unwrap_or("")
            .split('$')
            .next()
            .unwrap_or("")
            .to_string();
        row.push(Some(varname));
        for k in &cols[1..] {
            row.push(cell(obj.get(k)));
        }
        out.push(row);
    }
    let crefs: Vec<&str> = cols.iter().map(String::as_str).collect();
    Df::from_string_rows(&crefs, &out)
}

/// 广州期货交易所-仓单日报（对应 akshare [`akshare.futures_gfex_warehouse_receipt`]）。
///
/// 上游返回 `dict`（按 `varietyOrder` 分品种），本项目合并为带 `品种代码` 列的单一
/// [`Df`]，不注册 parity。
pub fn futures_gfex_warehouse_receipt(date: &str) -> Result<Df> {
    let url = "http://www.gfex.com.cn/u/interfacesWebTdWbillWeeklyQuotes/loadList";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("gen_date".into(), Value::String(date.to_string()));
    let v = http.post_form(url, &params, &GFEX_INFO_HEADERS)?;
    let rows = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // akshare：保留 whType 为数值且非空的行；合并后丢弃 whType，输出 6 列。
    let cols: [&str; 6] = [
        "品种代码",
        "品种",
        "仓库/分库",
        "昨日仓单量",
        "今日仓单量",
        "增减",
    ];
    let numeric: [&str; 3] = ["昨日仓单量", "今日仓单量", "增减"];
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        let obj = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        let variety_order = obj.get("varietyOrder").and_then(Value::as_str).unwrap_or("");
        if variety_order.is_empty() {
            continue;
        }
        // whType 须可解析为数值（对应 akshare dropna(subset=["whType"])）
        let wh_type = obj.get("whType").and_then(Value::as_str).unwrap_or("");
        if wh_type.parse::<f64>().is_err() {
            continue;
        }
        out.push(vec![
            Some(variety_order.to_string()),
            cell_str(obj.get("variety")),
            cell_str(obj.get("whAbbr")),
            Some(strip_thousands(obj.get("lastWbillQty").and_then(Value::as_str).unwrap_or(""))),
            Some(strip_thousands(obj.get("wbillQty").and_then(Value::as_str).unwrap_or(""))),
            Some(strip_thousands(obj.get("regWbillQty").and_then(Value::as_str).unwrap_or(""))),
        ]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

// ───────────────────────────── 期转现 / 交割（delivery / to_spot） ─────────────────────────────

/// 上海期货交易所-期转现（对应 akshare [`akshare.futures_to_spot_shfe`]）。
///
/// `date`: 年月 `YYYYMM`。数据源 `tsite.shfe.com.cn/.../ExchangeDelivery{date}.dat`
/// （JSON 数组，按位置取 `[日期, 合约, 交割量, 期转现量]`）。
/// 注：本环境 `tsite.shfe.com.cn` 域名无法解析，golden 无法生成，parity 自动跳过；
/// URL 与列序严格对齐 akshare，可在正常网络环境使用。
pub fn futures_to_spot_shfe(date: &str) -> Result<Df> {
    let url = format!(
        "https://tsite.shfe.com.cn/data/instrument/ExchangeDelivery{}.dat",
        date
    );
    let http = HttpClient::default();
    let v = http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", UA_MSIE_SHFE)], None)?;
    let rows = v
        .get("ExchangeDelivery")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cols = ["日期", "合约", "交割量", "期转现量"];
    let numeric = ["交割量", "期转现量"];
    let mut out: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for r in &rows {
        // 按位置：akshare 设列 [_, 日期, 交割量, _, 期转现量, 合约, _, _] 后取
        // [日期, 合约, 交割量, 期转现量] → 索引 [1, 5, 2, 4]
        match r {
            Value::Array(a) => {
                let get = |i: usize| cell(a.get(i));
                out.push(vec![get(1), get(5), get(2), get(4)]);
            }
            _ => continue,
        }
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 大连商品交易所-交割统计（对应 akshare [`akshare.futures_delivery_dce`]）。
///
/// `date`: 交割日期 `YYYYMM`。数据源 `publicweb/quotesdata/delivery.html`（POST，read_html）。
/// 注：大商所 `publicweb` 接口反爬（412），本环境难以实时校验，逻辑严格对齐 akshare。
pub fn futures_delivery_dce(date: &str) -> Result<Df> {
    let url = "http://www.dce.com.cn/publicweb/quotesdata/delivery.html";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("deliveryQuotes.variety".into(), Value::String("all".into()));
    params.insert("year".into(), Value::String(String::new()));
    params.insert("month".into(), Value::String(String::new()));
    params.insert("deliveryQuotes.begin_month".into(), Value::String(date.to_string()));
    params.insert(
        "deliveryQuotes.end_month".into(),
        Value::String((date.parse::<i64>().unwrap_or(0) + 1).to_string()),
    );
    let text = http.post_form_text(url, &params, &[])?;
    let tables = crate::core::html::read_html_tables(&text)?;
    let t = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("DCE 交割表无内容".into()))?;
    if t.len() < 2 {
        return empty_df();
    }
    let header = &t[0];
    let i_date = header.iter().position(|c| c == "交割日期");
    let i_var = header.iter().position(|c| c == "品种");
    let i_qty = header.iter().position(|c| c == "交割量");
    let i_amt = header.iter().position(|c| c == "交割金额");
    let cols = ["交割日期", "品种", "交割量", "交割金额"];
    let numeric = ["交割量", "交割金额"];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &t[1..] {
        // 过滤品种含「小计|总计」行
        if let Some(i) = i_var {
            if r.get(i).map(|c| c.contains("小计") || c.contains("总计")).unwrap_or(false) {
                continue;
            }
        }
        let date = i_date.and_then(|i| r.get(i).cloned()).map(|s| s.split('.').next().unwrap_or("").to_string());
        let var = i_var.and_then(|i| r.get(i).cloned()).filter(|c| !c.is_empty());
        let qty = i_qty.and_then(|i| r.get(i).cloned()).map(|s| strip_thousands(&s));
        let amt = i_amt.and_then(|i| r.get(i).cloned()).map(|s| strip_thousands(&s));
        out.push(vec![date, var, qty, amt]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 大连商品交易所-期转现（对应 akshare [`akshare.futures_to_spot_dce`]）。
///
/// `date`: 期转现日期 `YYYYMM`。数据源 `publicweb/quotesdata/ftsDeal.html`（POST，read_html）。
/// 注：大商所 `publicweb` 接口反爬（412），本环境难以实时校验，逻辑严格对齐 akshare。
pub fn futures_to_spot_dce(date: &str) -> Result<Df> {
    let url = "http://www.dce.com.cn/publicweb/quotesdata/ftsDeal.html";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("ftsDealQuotes.variety".into(), Value::String("all".into()));
    params.insert("year".into(), Value::String(String::new()));
    params.insert("month".into(), Value::String(String::new()));
    params.insert("ftsDealQuotes.begin_month".into(), Value::String(date.to_string()));
    params.insert("ftsDealQuotes.end_month".into(), Value::String(date.to_string()));
    let text = http.post_form_text(url, &params, &[])?;
    let tables = crate::core::html::read_html_tables(&text)?;
    let t = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("DCE 期转现表无内容".into()))?;
    if t.len() < 2 {
        return empty_df();
    }
    let header = &t[0];
    let i_date = header.iter().position(|c| c == "期转现发生日期");
    let i_sym = header.iter().position(|c| c == "合约代码");
    let i_qty = header.iter().position(|c| c == "期转现数量");
    let cols = ["期转现发生日期", "合约代码", "期转现数量"];
    let numeric = ["期转现数量"];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &t[1..] {
        if let Some(i) = i_sym {
            if r.get(i).map(|c| c.contains("小计") || c.contains("总计")).unwrap_or(false) {
                continue;
            }
        }
        let date = i_date.and_then(|i| r.get(i).cloned()).map(|s| s.split('.').next().unwrap_or("").to_string());
        let sym = i_sym.and_then(|i| r.get(i).cloned()).filter(|c| !c.is_empty());
        let qty = i_qty.and_then(|i| r.get(i).cloned()).map(|s| strip_thousands(&s));
        out.push(vec![date, sym, qty]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 大连商品交易所-交割配对表（对应 akshare [`akshare.futures_delivery_match_dce`]）。
///
/// `symbol`: 交割品种（如 `a`）。数据源 `publicweb/quotesdata/deliveryMatch.html`（POST，read_html）。
/// 注：大商所 `publicweb` 接口反爬（412），本环境难以实时校验，逻辑严格对齐 akshare。
pub fn futures_delivery_match_dce(symbol: &str) -> Result<Df> {
    let url = "http://www.dce.com.cn/publicweb/quotesdata/deliveryMatch.html";
    let http = HttpClient::default();
    let mut params = Map::new();
    params.insert("deliveryMatchQuotes.variety".into(), Value::String(symbol.to_string()));
    params.insert("contract.contract_id".into(), Value::String("all".into()));
    params.insert("contract.variety_id".into(), Value::String(symbol.to_string()));
    let text = http.post_form_text(url, &params, &[])?;
    let tables = crate::core::html::read_html_tables(&text)?;
    let t = tables
        .into_iter()
        .next()
        .ok_or_else(|| AkshareError::Empty("DCE 交割配对表无内容".into()))?;
    if t.len() < 2 {
        return empty_df();
    }
    // akshare：iloc[:-1] 丢弃末行
    let data = if t.len() > 2 { &t[1..t.len() - 1] } else { &t[1..] };
    let header = &t[0];
    let i_date = header.iter().position(|c| c == "配对日期");
    let i_lots = header.iter().position(|c| c == "配对手数");
    let i_settle = header.iter().position(|c| c == "交割结算价");
    let cols = ["配对日期", "配对手数", "交割结算价"];
    let numeric = ["配对手数", "交割结算价"];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in data {
        let date = i_date.and_then(|i| r.get(i).cloned()).map(|s| s.split('.').next().unwrap_or("").to_string());
        let lots = i_lots.and_then(|i| r.get(i).cloned()).map(|s| strip_thousands(&s));
        let settle = i_settle.and_then(|i| r.get(i).cloned()).map(|s| strip_thousands(&s));
        out.push(vec![date, lots, settle]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 郑州商品交易所-期转现统计（对应 akshare [`akshare.futures_to_spot_czce`]）。
///
/// `date`: 交易日 `YYYYMMDD`。数据源 `FutureDataTrdtrades.xls`（read_excel，skiprows=1）。
/// 列 [合约代码, 合约数量]，过滤「小计/合计」行，合约数量数值化。
pub fn futures_to_spot_czce(date: &str) -> Result<Df> {
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Future/{}/{}/FutureDataTrdtrades.xls",
        &date[..4],
        date
    );
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let rows = xls_rows(&bytes)?;
    // skiprows=1：跳过标题行，表头为 row1，数据从 row2 起
    if rows.len() < 3 {
        return empty_df();
    }
    let cols = ["合约代码", "合约数量"];
    let numeric = ["合约数量"];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &rows[2..] {
        let code = r.first().cloned().filter(|c| !c.is_empty());
        let Some(code) = code else { continue };
        if code.contains("小计") || code.contains("合计") {
            continue;
        }
        let qty = r.get(1).cloned().map(|s| strip_thousands(&s)).filter(|c| !c.is_empty());
        out.push(vec![Some(code), qty]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 郑州商品交易所-月度交割查询（对应 akshare [`akshare.futures_delivery_czce`]）。
///
/// `date`: 交易日 `YYYYMMDD`。数据源 `FutureDataSettlematched.xls`（read_excel，skiprows=1）。
/// 列 [品种, 交割数量, 交割额]，均数值化。
pub fn futures_delivery_czce(date: &str) -> Result<Df> {
    let url = format!(
        "http://www.czce.com.cn/cn/DFSStaticFiles/Future/{}/{}/FutureDataSettlematched.xls",
        &date[..4],
        date
    );
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let rows = xls_rows(&bytes)?;
    if rows.len() < 3 {
        return empty_df();
    }
    let cols = ["品种", "交割数量", "交割额"];
    let numeric = ["交割数量", "交割额"];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &rows[2..] {
        let var = r.first().cloned().filter(|c| !c.is_empty());
        let Some(var) = var else { continue };
        let qty = r.get(1).cloned().map(|s| strip_thousands(&s)).filter(|c| !c.is_empty());
        let amt = r.get(2).cloned().map(|s| strip_thousands(&s)).filter(|c| !c.is_empty());
        out.push(vec![Some(var), qty, amt]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// 上海期货交易所-交割情况表（对应 akshare [`akshare.futures_delivery_shfe`]）。
///
/// `date`: 年月 `YYYYMM`。数据源 `tsite.shfe.com.cn/.../{date}monthvarietystatistics.dat`
/// （JSON 数组，按位置取 `[品种, 交割量-本月, 交割量-比重, 交割量-本年累计, 交割量-累计同比]`）。
/// 注：本环境 `tsite.shfe.com.cn` 域名无法解析，golden 无法生成，parity 自动跳过。
pub fn futures_delivery_shfe(date: &str) -> Result<Df> {
    let url = format!(
        "https://tsite.shfe.com.cn/data/dailydata/{}monthvarietystatistics.dat",
        date
    );
    let http = HttpClient::default();
    let v = http.get_json_with_headers(&url, &Map::new(), &[("User-Agent", UA_MSIE_SHFE)], None)?;
    let rows = v
        .get("o_curdelivery")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cols = [
        "品种",
        "交割量-本月",
        "交割量-比重",
        "交割量-本年累计",
        "交割量-累计同比",
    ];
    let numeric = ["交割量-本月", "交割量-比重", "交割量-本年累计", "交割量-累计同比"];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for r in &rows {
        // 按位置：akshare 设列 [品种, 品种代码, _, 交割量-本月, 交割量-比重,
        // 交割量-本年累计, 交割量-累计同比] 后取前 5 → 索引 [0,3,4,5,6]
        match r {
            Value::Array(a) => {
                let get = |i: usize| cell(a.get(i));
                out.push(vec![get(0), get(3), get(4), get(5), get(6)]);
            }
            _ => continue,
        }
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

// ───────────────────────────── 中金所历史行情（futures_daily_bar） ─────────────────────────────

/// 中国金融期货交易所-历史日线（对应 akshare [`akshare.futures_hist_daily_cffex`]）。
///
/// `date`: 交易日 `YYYYMMDD`。数据源 `cffex.com.cn/sj/hqsj/rtj/{YYYYMM}/{DD}/{date}_1.csv`
/// （GBK 编码），按位置映射列并过滤「小计/合计/IO/MO/HO」。
pub fn futures_hist_daily_cffex(date: &str) -> Result<Df> {
    let url = format!(
        "http://www.cffex.com.cn/sj/hqsj/rtj/{}/{}/{}_1.csv",
        &date[..6],
        &date[6..],
        date
    );
    let http = HttpClient::default();
    let bytes = http.get_bytes_with_headers(&url, &Map::new(), &[("User-Agent", UA)], None)?;
    let text = decode_gbk(&bytes);
    let mut lines = text.lines();
    let _ = lines.next(); // 跳过 CSV 表头行（akshare read_csv 用其作列名，但本项目按位置重命名）
    let cols = [
        "symbol",
        "date",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "open_interest",
        "turnover",
        "settle",
        "pre_settle",
        "variety",
    ];
    let numeric = [
        "open",
        "high",
        "low",
        "close",
        "volume",
        "open_interest",
        "turnover",
        "settle",
        "pre_settle",
    ];
    let mut out: Vec<Vec<Option<String>>> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 14 {
            continue;
        }
        // 位置映射（akshare 列序）：0=合约代码,1=今开盘,2=最高价,3=最低价,4=成交量,
        // 5=成交金额,6=持仓量,8=今收盘,9=今结算,10=前结算
        let symbol = f[0].trim();
        if symbol.is_empty() || symbol == "小计" || symbol == "合计" {
            continue;
        }
        if symbol.contains("IO") || symbol.contains("MO") || symbol.contains("HO") {
            continue;
        }
        let variety = regex_first_alpha(symbol);
        out.push(vec![
            Some(symbol.to_string()),
            Some(date.to_string()),
            Some(f[1].trim().to_string()),
            Some(f[2].trim().to_string()),
            Some(f[3].trim().to_string()),
            Some(f[8].trim().to_string()),
            Some(f[4].trim().to_string()),
            Some(f[6].trim().to_string()),
            Some(f[5].trim().to_string()),
            Some(f[9].trim().to_string()),
            Some(f[10].trim().to_string()),
            Some(variety),
        ]);
    }
    let mut df = Df::from_string_rows(&cols, &out)?;
    df.cast_numeric(&numeric)?;
    Ok(df)
}

/// GBK/GB2312 解码（对应 akshare `read_csv(encoding="gbk")`）。
fn decode_gbk(bytes: &[u8]) -> String {
    use encoding_rs::{Encoding, GBK, UTF_8};
    if let Some((enc, _)) = Encoding::for_bom(bytes) {
        if enc == UTF_8 {
            return String::from_utf8_lossy(bytes).to_string();
        }
    }
    let (cow, _, _) = GBK.decode(bytes);
    cow.into_owned()
}
