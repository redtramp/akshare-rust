//! 胡润排行榜数据源（批次 5 长尾 · fortune）。
//!
//! 对应 akshare `fortune/fortune_hurun.py::hurun_rank`：胡润研究院各排行榜
//! （百富榜 / 全球富豪榜 / 独角兽榜 / 瞪羚榜 / Under30s / 500 强 / 艺术榜等）。
//!
//! 多步抓取（与 akshare 一致）：
//! 1. 首页下拉菜单取「榜单名 → 榜单页链接」映射；
//! 2. 榜单页 `select#exampleFormControlSelect1` 取「年份 → 编码」映射；
//! 3. `HsRankDetailsList` JSON 接口取列表（`num=年份编码, limit=20000`）。
//!
//! 其余 fortune 函数为财富媒体榜（Bloomberg / Forbes / 新财富 500 / fortune 500），
//! 上游页面结构已变或需反爬/订阅，本批次不可达，跳过。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use scraper::{Html, Selector};
use serde_json::{Map, Value};
use std::collections::HashMap;

const HURUN_HOME: &str = "https://www.hurun.net/zh-CN/Rank/HsRankDetails?pagetype=rich";
const HURUN_LIST_API: &str = "https://www.hurun.net/zh-CN/Rank/HsRankDetailsList";

/// 榜单名 → (输出列名, 源字段名) 映射，按输出列顺序排列。
type FieldMap = &'static [(&'static str, &'static str)];

fn indicator_fields(indicator: &str) -> Option<FieldMap> {
    match indicator {
        "胡润百富榜" => Some(&[
            ("排名", "hs_Rank_Rich_Ranking"),
            ("财富", "hs_Rank_Rich_Wealth"),
            ("姓名", "hs_Rank_Rich_ChaName_Cn"),
            ("企业", "hs_Rank_Rich_ComName_Cn"),
            ("行业", "hs_Rank_Rich_Industry_Cn"),
        ]),
        "胡润全球富豪榜" => Some(&[
            ("排名", "hs_Rank_Global_Ranking"),
            ("财富", "hs_Rank_Global_Wealth"),
            ("姓名", "hs_Rank_Global_ChaName_Cn"),
            ("企业", "hs_Rank_Global_ComName_Cn"),
            ("行业", "hs_Rank_Global_Industry_Cn"),
        ]),
        "胡润印度榜" => Some(&[
            ("排名", "hs_Rank_India_Ranking"),
            ("财富", "hs_Rank_India_Wealth"),
            ("姓名", "hs_Rank_India_ChaName_Cn"),
            ("企业", "hs_Rank_India_ComName_Cn"),
            ("行业", "hs_Rank_India_Industry_Cn"),
        ]),
        "胡润全球独角兽榜" => Some(&[
            ("排名", "hs_Rank_Unicorn_Ranking"),
            ("财富", "hs_Rank_Unicorn_Wealth"),
            ("姓名", "hs_Rank_Unicorn_ChaName_Cn"),
            ("企业", "hs_Rank_Unicorn_ComName_Cn"),
            ("行业", "hs_Rank_Unicorn_Industry_Cn"),
        ]),
        "中国瞪羚企业榜" => Some(&[
            ("企业信息", "hs_Rank_CGazelles_ComName_Cn"),
            ("掌门人/联合创始人", "hs_Rank_CGazelles_Name_Cn"),
            ("企业总部", "hs_Rank_CGazelles_ComHeadquarters_Cn"),
            ("行业", "hs_Rank_CGazelles_Industry_Cn"),
        ]),
        "全球瞪羚企业榜" => Some(&[
            ("企业信息", "hs_Rank_GGazelles_ComName_Cn"),
            ("掌门人/联合创始人", "hs_Rank_GGazelles_Name_Cn"),
            ("企业总部", "hs_Rank_GGazelles_ComHeadquarters_Cn"),
            ("行业", "hs_Rank_GGazelles_Industry_Cn"),
        ]),
        "胡润Under30s创业领袖榜" => Some(&[
            ("姓名", "hs_Rank_U30_ChaName_Cn"),
            ("企业信息", "hs_Rank_U30_ComName_Cn"),
            ("企业总部", "hs_Rank_U30_ComHeadquarters_Cn"),
            ("行业", "hs_Rank_U30_Industry_Cn"),
        ]),
        "胡润中国500强民营企业" => Some(&[
            ("排名", "hs_Rank_CTop500_Ranking"),
            ("排名变化", "hs_Rank_CTop500_Ranking_Change"),
            ("企业估值", "hs_Rank_CTop500_Wealth"),
            ("企业信息", "hs_Rank_CTop500_ComName_Cn"),
            ("CEO", "hs_Rank_CTop500_ChaName_Cn"),
            ("行业", "hs_Rank_CTop500_Industry_Cn"),
        ]),
        "胡润世界500强" => Some(&[
            ("排名", "hs_Rank_GTop500_Ranking"),
            ("排名变化", "hs_Rank_GTop500_Ranking_Change"),
            ("企业估值", "hs_Rank_GTop500_Wealth"),
            ("企业信息", "hs_Rank_GTop500_ComName_Cn"),
            ("CEO", "hs_Rank_GTop500_ChaName_Cn"),
            ("行业", "hs_Rank_GTop500_Industry_Cn"),
        ]),
        "胡润艺术榜" => Some(&[
            ("排名", "hs_Rank_Art_Ranking"),
            ("排名变化", "hs_Rank_Art_Ranking_Change"),
            ("成交额", "hs_Rank_Art_Turnover"),
            ("姓名", "hs_Rank_Art_Name_Cn"),
            ("年龄", "hs_Rank_Art_Age"),
            ("艺术类别", "hs_Rank_Art_ArtCategory_Cn"),
        ]),
        _ => None,
    }
}

/// 解析首页下拉菜单：`ul.dropdown-menu > a` → (链接文本, 完整链接)。
fn parse_dropdowns(html: &str) -> Result<HashMap<String, String>> {
    let doc = Html::parse_document(html);
    let ul_sel =
        Selector::parse("ul.dropdown-menu").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let a_sel = Selector::parse("a").map_err(|e| AkshareError::Empty(e.to_string()))?;

    let mut map = HashMap::new();
    for ul in doc.select(&ul_sel) {
        for a in ul.select(&a_sel) {
            let text = a.text().collect::<Vec<_>>().join("").trim().to_string();
            if text.is_empty() {
                continue;
            }
            if let Some(href) = a.value().attr("href") {
                let url = if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("https://www.hurun.net{href}")
                };
                map.insert(text, url);
            }
        }
    }
    Ok(map)
}

/// 解析榜单页年份下拉：`#exampleFormControlSelect1 option` → (年份, 编码)。
///
/// 编码取自 `option[value]` 以 `=` 切分后的第 3 段；年份取 option 文本首词。
fn parse_year_options(html: &str) -> Result<HashMap<String, String>> {
    let doc = Html::parse_document(html);
    let sel_sel = Selector::parse("#exampleFormControlSelect1")
        .map_err(|e| AkshareError::Empty(e.to_string()))?;
    let opt_sel = Selector::parse("option").map_err(|e| AkshareError::Empty(e.to_string()))?;

    let mut map = HashMap::new();
    if let Some(select) = doc.select(&sel_sel).next() {
        for opt in select.select(&opt_sel) {
            let value = opt.value().attr("value").unwrap_or("");
            let parts: Vec<&str> = value.split('=').collect();
            if parts.len() < 3 {
                continue;
            }
            let code = parts[2].to_string();
            let year = opt
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if !year.is_empty() {
                map.insert(year, code);
            }
        }
    }
    Ok(map)
}

/// 胡润排行榜（对应 akshare [`hurun_rank`]）。
///
/// # 参数
/// - `indicator`：榜单名，可选 `胡润百富榜` / `胡润全球富豪榜` / `胡润印度榜` /
///   `胡润全球独角兽榜` / `中国瞪羚企业榜` / `全球瞪羚企业榜` /
///   `胡润Under30s创业领袖榜` / `胡润中国500强民营企业` / `胡润世界500强` / `胡润艺术榜`。
/// - `year`：年份字符串（如 `2023`）。
///
/// # 返回列
/// 各榜单列名见 [`indicator_fields`]；默认 `胡润百富榜` 列：
/// `排名, 财富, 姓名, 企业, 行业`。
pub fn hurun_rank(indicator: &str, year: &str) -> Result<Df> {
    let fields = indicator_fields(indicator).ok_or_else(|| {
        AkshareError::Empty(format!(
            "不支持的胡润榜单: {indicator}（可选：胡润百富榜/胡润全球富豪榜/胡润印度榜/\
             胡润全球独角兽榜/中国瞪羚企业榜/全球瞪羚企业榜/胡润Under30s创业领袖榜/\
             胡润中国500强民营企业/胡润世界500强/胡润艺术榜）"
        ))
    })?;

    let http = HttpClient::default();

    // 1) 首页：下拉菜单 榜单名 → 链接
    let home_text = http.get_text(HURUN_HOME, &Map::new(), None)?;
    let name_url = parse_dropdowns(&home_text)?;
    let indicator_url = name_url
        .get(indicator)
        .ok_or_else(|| AkshareError::Empty(format!("胡润首页未找到榜单链接: {indicator}")))?;

    // 2) 榜单页：年份 → 编码
    let page_text = http.get_text(indicator_url, &Map::new(), None)?;
    let year_code = parse_year_options(&page_text)?;
    let num = year_code
        .get(year)
        .ok_or_else(|| AkshareError::Empty(format!("榜单 {indicator} 未找到年份: {year}")))?;

    // 3) JSON 列表
    let mut params = Map::new();
    params.insert("num".into(), Value::String(num.clone()));
    params.insert("search".into(), Value::String(String::new()));
    params.insert("offset".into(), Value::String("0".into()));
    params.insert("limit".into(), Value::String("20000".into()));

    let data = http.get_json(HURUN_LIST_API, &params, None)?;
    let rows = data
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| AkshareError::Empty("胡润接口未返回 rows".into()))?;

    // 4) 按输出列顺序抽取源字段
    let out_names: Vec<&str> = fields.iter().map(|(o, _)| *o).collect();
    let mut data_rows: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(obj) = row.as_object() else { continue };
        let rec: Vec<Option<String>> = fields
            .iter()
            .map(|(_, src)| match obj.get(*src) {
                Some(Value::String(s)) => Some(s.clone()),
                Some(Value::Null) => None,
                Some(other) => {
                    let s = other.to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                }
                None => None,
            })
            .collect();
        data_rows.push(rec);
    }

    let mut df = Df::from_string_rows(&out_names, &data_rows)?;

    // 数值列检测：仅当该列所有非空值均可解析为 f64 时才转数值
    // （与 pandas 的 dtype 推断一致：排名→int、财富→float，其余文本保持 str）。
    let numeric_cols: Vec<&str> = out_names
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            data_rows.iter().all(|r| match r.get(*i) {
                Some(Some(v)) => v.parse::<f64>().is_ok(),
                _ => true,
            })
        })
        .map(|(_, n)| *n)
        .collect();
    if !numeric_cols.is_empty() {
        df.cast_numeric(&numeric_cols)?;
    }

    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_year_option_value() {
        // option value 形如 "...?num=3YwKs889SRIm"，编码为第 3 段
        let value = "HsRankDetails?pagetype=rich&num=3YwKs889SRIm";
        let parts: Vec<&str> = value.split('=').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2], "3YwKs889SRIm");
    }

    #[test]
    fn indicator_field_count() {
        assert_eq!(indicator_fields("胡润百富榜").unwrap().len(), 5);
        assert_eq!(indicator_fields("胡润艺术榜").unwrap().len(), 6);
        assert!(indicator_fields("不存在的榜").is_none());
    }
}
