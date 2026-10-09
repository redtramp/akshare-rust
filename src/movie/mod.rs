//! movie 电影票房分类模块（批次 94 · 艺恩电影市场 `ys.endata.cn`）。
//!
//! 实现覆盖 akshare `movie` 分类（`movie_yien.py`）下全部 8 个公开函数：
//! - 实时票房（`movie_boxoffice_realtime`）
//! - 单日票房（`movie_boxoffice_daily`）
//! - 单月票房（`movie_boxoffice_monthly`）
//! - 年度票房（`movie_boxoffice_yearly`）
//! - 年度首周票房（`movie_boxoffice_yearly_first_week`）
//! - 影院日票房排行（`movie_boxoffice_cinema_daily`）
//! - 单周票房 / 影院周票房排行（`movie_boxoffice_weekly` /
//!   `movie_boxoffice_cinema_weekly`）
//!
//! 6 个列表接口走 `ys.endata.cn/enlib-api` 单一 form-POST 模板
//! （对应 `_post_endata_json` + `_fetch_endata_list` 分页），无需 JS 解密。
//! 两个周榜接口与 akshare 上游行为一致：公开周榜接口需要权限（匿名直接返回
//! 系统错误），akshare 版本直接抛 `APIError` 且不发起请求，本实现对齐该行为
//! （返回 [`AkshareError::Param`]，不发 HTTP）。列名与 akshare 逐字一致。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use chrono::{Datelike, NaiveDate};
use serde_json::{Map, Value};

const MOVIE_HEADERS: [(&str, &str); 5] = [
    ("Accept", "application/json, text/plain, */*"),
    ("Content-Type", "application/x-www-form-urlencoded"),
    ("Origin", "https://ys.endata.cn"),
    ("Referer", "https://ys.endata.cn/BoxOffice/Movie"),
    (
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
    ),
];

/// 影院接口页面路径不同（akshare `_get_endata_headers("/BoxOffice/Org")`）。
const CINEMA_HEADERS: [(&str, &str); 5] = [
    ("Accept", "application/json, text/plain, */*"),
    ("Content-Type", "application/x-www-form-urlencoded"),
    ("Origin", "https://ys.endata.cn"),
    ("Referer", "https://ys.endata.cn/BoxOffice/Org"),
    (
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
    ),
];

const MOVIE_DAY_URL: &str =
    "https://ys.endata.cn/enlib-api/api/movie/getMovie_BoxOffice_Day_List.do";
const MOVIE_MONTH_URL: &str =
    "https://ys.endata.cn/enlib-api/api/movie/getMovie_BoxOffice_Month_List.do";
const MOVIE_YEAR_URL: &str =
    "https://ys.endata.cn/enlib-api/api/movie/getMovie_BoxOffice_Year_List.do";
const CINEMA_DAY_URL: &str =
    "https://ys.endata.cn/enlib-api/api/cinema/getcinemaboxoffice_day_list.do";

/// 表单参数字典（保持插入顺序，与 akshare payload 键序一致）。
fn form(pairs: &[(&str, &str)]) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), Value::String(v.to_string()));
    }
    m
}

/// 对应 akshare `random.random()` 的 `r` 防缓存参数。
fn rand_r() -> String {
    format!("{}", rand::random_range(0.0..1.0))
}

/// 对应 akshare `_format_date`：`YYYYMMDD` → `YYYY-MM-DD`（非法输入抛 Param）。
fn format_date(date: &str) -> Result<String> {
    NaiveDate::parse_from_str(date, "%Y%m%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|e| AkshareError::Param(format!("日期参数 {date:?} 非法: {e}")))
}

/// 对应 akshare `_raise_week_permission_error`。
fn week_permission_error(interface: &str) -> AkshareError {
    AkshareError::Param(format!(
        "{interface} 上游艺恩公开周榜接口当前需要权限或直接返回系统错误, 暂无法匿名获取"
    ))
}

/// 对应 akshare `_post_endata_json`：POST 表单体并校验 `status == 1`。
fn post_endata(
    http: &HttpClient,
    url: &str,
    payload: &Map<String, Value>,
    headers: &[(&str, &str)],
) -> Result<Value> {
    let value = http.post_form(url, payload, headers)?;
    if value.get("status").and_then(Value::as_i64) != Some(1) {
        let des = value
            .get("des")
            .and_then(Value::as_str)
            .unwrap_or("艺恩接口返回异常");
        return Err(AkshareError::Param(des.to_string()));
    }
    Ok(value)
}

/// 单页 `data.table1` 行数组（缺失 → 空数组）。
fn page_rows(value: &Value, key: &str) -> Vec<Value> {
    value
        .get("data")
        .and_then(|d| d.get(key))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// 对应 akshare `_fetch_endata_list`：逐页拉取并拼接全部 `table1` 行。
fn fetch_endata_list(
    http: &HttpClient,
    url: &str,
    payload: Map<String, Value>,
    headers: &[(&str, &str)],
) -> Result<Vec<Value>> {
    let first = post_endata(http, url, &payload, headers)?;
    let mut rows = page_rows(&first, "table1");
    let total_page = first
        .get("data")
        .and_then(|d| d.get("table2"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|o| o.get("TotalPage"))
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1) as usize;
    for page in 2..=total_page {
        let mut next = payload.clone();
        next.insert("pageindex".into(), Value::String(page.to_string()));
        next.insert("r".into(), Value::String(rand_r()));
        let value = post_endata(http, url, &next, headers)?;
        rows.extend(page_rows(&value, "table1"));
    }
    Ok(rows)
}

/// JSON 标量 → 字符串单元格（`null`/缺失/对象 → 空单元格）。
fn cell(row: &Map<String, Value>, key: &str) -> Option<String> {
    match row.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// 按源字段顺序抽取为全字符串 `Df`（等价 akshare `temp_df[[...]]` 选择列）。
fn to_df(rows: &[Value], keys: &[&str]) -> Result<Df> {
    let mut mapped: Vec<Vec<Option<String>>> = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(obj) = row.as_object() else {
            return Err(AkshareError::json(
                "ys.endata.cn",
                format!("table1 行不是 JSON 对象: {row}"),
            ));
        };
        mapped.push(keys.iter().map(|k| cell(obj, k)).collect());
    }
    Df::from_string_rows(keys, &mapped)
}

/// 读字符串列全部值（列缺失 → 空表，对应 akshare 对缺失列的宽松路径）。
fn string_column(df: &Df, col: &str) -> Result<Vec<Option<String>>> {
    let inner = df.inner();
    let series = match inner.column(col) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };
    match series.str() {
        Ok(ca) => Ok(ca.iter().map(|v| v.map(str::to_string)).collect()),
        Err(_) => Ok(Vec::new()),
    }
}

/// 对应 akshare `.str.replace(" ", "", regex=False)`：去掉列内全部空格。
fn remove_spaces(df: &mut Df, col: &str) -> Result<()> {
    let values = string_column(df, col)?;
    if values.is_empty() {
        return Ok(());
    }
    let stripped: Vec<Option<String>> = values
        .into_iter()
        .map(|v| v.map(|s| s.replace(' ', "")))
        .collect();
    df.with_column(col, &stripped)?;
    Ok(())
}

/// 对应 akshare `_calc_first_week_days`：上映日期所在周剩余天数
/// （周一=7 … 周日=1；缺失/无法解析 → 空值，对应 `pd.NA`）。
fn first_week_days(df: &Df, date_col: &str) -> Result<Vec<Option<String>>> {
    let values = string_column(df, date_col)?;
    Ok(values
        .into_iter()
        .map(|v| {
            v.and_then(|s| {
                NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                    .ok()
                    .map(|d| (7 - d.weekday().num_days_from_monday() as i64).to_string())
            })
        })
        .collect())
}

/// 全空字符串列（对应 akshare `temp_df["口碑指数"] = pd.NA`）。
fn null_column(df: &mut Df, name: &str) -> Result<()> {
    let n = df.height();
    let values: Vec<Option<String>> = vec![None; n];
    df.with_column(name, &values)?;
    Ok(())
}

/// 电影票房-实时票房（对应 akshare `movie_boxoffice_realtime`）。
pub fn movie_boxoffice_realtime() -> Result<Df> {
    let http = HttpClient::default();
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let payload = form(&[
        ("r", &rand_r()),
        ("datetype", "Day"),
        ("date", &today),
        ("sdate", &today),
        ("edate", &today),
        ("bserviceprice", "1"),
        (
            "columnslist",
            "100,102,103,119,105,107,109,106,112,129,142,143,163,164,165",
        ),
        ("pageindex", "1"),
        ("pagesize", "20"),
        ("order", "103"),
        ("ordertype", "desc"),
    ]);
    let rows = fetch_endata_list(&http, MOVIE_DAY_URL, payload, &MOVIE_HEADERS)?;
    let mut df = to_df(
        &rows,
        &[
            "Irank",
            "MovieName",
            "BoxOffice",
            "BoxOfficePercent",
            "ReleaseDay",
            "TotalBoxOffice",
        ],
    )?;
    df.rename_columns(&[
        "排序",
        "影片名称",
        "实时票房",
        "票房占比",
        "上映天数",
        "累计票房",
    ])?;
    df.scale("实时票房", 10000.0)?;
    df.scale("累计票房", 10000.0)?;
    df.cast_numeric(&["票房占比"])?;
    df.cast_integer(&["上映天数", "排序"])?;
    df.sort_by_existing("排序", true)
}

/// 电影票房-单日票房（对应 akshare `movie_boxoffice_daily`）。
pub fn movie_boxoffice_daily(date: &str) -> Result<Df> {
    let http = HttpClient::default();
    let date_str = format_date(date)?;
    let payload = form(&[
        ("r", &rand_r()),
        ("datetype", "Day"),
        ("date", &date_str),
        ("sdate", &date_str),
        ("edate", &date_str),
        ("bserviceprice", "1"),
        ("columnslist", "100,102,103,146,105,111,113,112,119"),
        ("pageindex", "1"),
        ("pagesize", "500"),
        ("order", "103"),
        ("ordertype", "desc"),
    ]);
    let rows = fetch_endata_list(&http, MOVIE_DAY_URL, payload, &MOVIE_HEADERS)?;
    let mut df = to_df(
        &rows,
        &[
            "Irank",
            "MovieName",
            "BoxOffice",
            "BoxOfficeMoM",
            "TotalBoxOffice",
            "AvgBoxOffice",
            "AvgShowAudienceCount",
            "ReleaseDay",
        ],
    )?;
    df.rename_columns(&[
        "排序",
        "影片名称",
        "单日票房",
        "环比变化",
        "累计票房",
        "平均票价",
        "场均人次",
        "上映天数",
    ])?;
    // 口碑指数：akshare 以 pd.NA 全空列占位（object dtype），列位置在上映天数之前
    null_column(&mut df, "口碑指数")?;
    let mut df = df.select(&[
        "排序",
        "影片名称",
        "单日票房",
        "环比变化",
        "累计票房",
        "平均票价",
        "场均人次",
        "口碑指数",
        "上映天数",
    ])?;
    df.scale("单日票房", 10000.0)?;
    df.cast_numeric(&["环比变化", "平均票价", "场均人次"])?;
    df.scale("累计票房", 10000.0)?;
    df.cast_integer(&["上映天数", "排序"])?;
    df.sort_by_existing("排序", true)
}

/// 电影票房-单周票房（对应 akshare `movie_boxoffice_weekly`）。
///
/// akshare 上游周榜接口需要权限（匿名直接返回系统错误），akshare 版本直接抛
/// `APIError` 且不发起请求；本实现与之对齐。
pub fn movie_boxoffice_weekly(_date: &str) -> Result<Df> {
    Err(week_permission_error("movie_boxoffice_weekly"))
}

/// 电影票房-单月票房（对应 akshare `movie_boxoffice_monthly`）。
pub fn movie_boxoffice_monthly(date: &str) -> Result<Df> {
    let http = HttpClient::default();
    let date_obj = NaiveDate::parse_from_str(date, "%Y%m%d")
        .map_err(|e| AkshareError::Param(format!("日期参数 {date:?} 非法: {e}")))?;
    let year = date_obj.year();
    let month = date_obj.month();
    let month_start = NaiveDate::from_ymd_opt(year, month, 1)
        .expect("每月必有 1 日")
        .format("%Y-%m-%d")
        .to_string();
    // 月末：28 号 + 4 天必越入下月 → 下月 1 号再回退 1 天（对齐 akshare 算法）
    let next_first = NaiveDate::from_ymd_opt(year, month, 28).expect("每月必有 28 日")
        + chrono::Duration::days(4);
    let month_end = NaiveDate::from_ymd_opt(next_first.year(), next_first.month(), 1)
        .expect("每月必有 1 日")
        - chrono::Duration::days(1);
    let month_end = month_end.format("%Y-%m-%d").to_string();
    // month_id 基准：2026-01 = 241（akshare `month + (year - 2026) * 12 + 240`）
    let month_id = month as i64 + (year as i64 - 2026) * 12 + 240;
    let date_range = format!("{month_start},{month_end}");
    let payload = form(&[
        ("r", &rand_r()),
        ("datetype", "Month"),
        ("date", &date_range),
        ("sdate", &month_start),
        ("edate", &month_end),
        ("dateid", &month_id.to_string()),
        ("sdateid", &month_id.to_string()),
        ("edateid", &month_id.to_string()),
        ("bserviceprice", "1"),
        ("columnslist", "100,101,102,105,109,110,130,131"),
        ("pageindex", "1"),
        ("pagesize", "500"),
        ("order", "102"),
        ("ordertype", "desc"),
    ]);
    let rows = fetch_endata_list(&http, MOVIE_MONTH_URL, payload, &MOVIE_HEADERS)?;
    let mut df = to_df(
        &rows,
        &[
            "Irank",
            "MovieName",
            "BoxOffice",
            "BoxOfficePercent",
            "AvgBoxOffice",
            "AvgShowAudienceCount",
            "ReleaseDate",
            "ReleaseDay",
        ],
    )?;
    df.rename_columns(&[
        "排序",
        "影片名称",
        "单月票房",
        "月度占比",
        "平均票价",
        "场均人次",
        "上映日期",
        "月内天数",
    ])?;
    null_column(&mut df, "口碑指数")?;
    let mut df = df.select(&[
        "排序",
        "影片名称",
        "单月票房",
        "月度占比",
        "平均票价",
        "场均人次",
        "上映日期",
        "口碑指数",
        "月内天数",
    ])?;
    df.scale("单月票房", 10000.0)?;
    df.cast_numeric(&["月度占比", "平均票价", "场均人次", "月内天数"])?;
    df.cast_integer(&["排序"])?;
    df.cast_date(&["上映日期"])?;
    df.sort_by_existing("排序", true)
}

/// 电影票房-年度票房（对应 akshare `movie_boxoffice_yearly`）。
pub fn movie_boxoffice_yearly(date: &str) -> Result<Df> {
    let year = date
        .get(..4)
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| AkshareError::Param(format!("日期参数 {date:?} 缺少年份前缀")))?;
    let payload = form(&[
        ("r", &rand_r()),
        ("datetype", "Year"),
        ("date", &format!("{year}-01-01,{year}-12-31")),
        ("sdate", &format!("{year}-01-01")),
        ("edate", &format!("{year}-12-31")),
        ("dateid", &year.to_string()),
        ("sdateid", &year.to_string()),
        ("edateid", &year.to_string()),
        ("bserviceprice", "1"),
        ("columnslist", "100,101,108,115,105,106,109,107"),
        ("pageindex", "1"),
        ("pagesize", "500"),
        ("order", "115"),
        ("ordertype", "desc"),
    ]);
    let http = HttpClient::default();
    let rows = fetch_endata_list(&http, MOVIE_YEAR_URL, payload, &MOVIE_HEADERS)?;
    let mut df = to_df(
        &rows,
        &[
            "Irank",
            "MovieName",
            "GenreMain",
            "TotalBoxOffice",
            "AvgBoxOffice",
            "AvgShowAudienceCount",
            "Country",
            "ReleaseDate",
        ],
    )?;
    df.rename_columns(&[
        "排序",
        "影片名称",
        "类型",
        "总票房",
        "平均票价",
        "场均人次",
        "国家及地区",
        "上映日期",
    ])?;
    df.scale("总票房", 10000.0)?;
    df.cast_numeric(&["平均票价", "场均人次"])?;
    df.cast_integer(&["排序"])?;
    df.cast_date(&["上映日期"])?;
    remove_spaces(&mut df, "国家及地区")?;
    df.sort_by_existing("排序", true)
}

/// 电影票房-年度首周票房（对应 akshare `movie_boxoffice_yearly_first_week`）。
pub fn movie_boxoffice_yearly_first_week(date: &str) -> Result<Df> {
    let year = date
        .get(..4)
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| AkshareError::Param(format!("日期参数 {date:?} 缺少年份前缀")))?;
    let payload = form(&[
        ("r", &rand_r()),
        ("datetype", "Year"),
        ("date", &format!("{year}-01-01,{year}-12-31")),
        ("sdate", &format!("{year}-01-01")),
        ("edate", &format!("{year}-12-31")),
        ("dateid", &year.to_string()),
        ("sdateid", &year.to_string()),
        ("edateid", &year.to_string()),
        ("bserviceprice", "1"),
        ("columnslist", "100,101,108,118,119,106,109,107"),
        ("pageindex", "1"),
        ("pagesize", "500"),
        ("order", "118"),
        ("ordertype", "desc"),
    ]);
    let http = HttpClient::default();
    let rows = fetch_endata_list(&http, MOVIE_YEAR_URL, payload, &MOVIE_HEADERS)?;
    let mut df = to_df(
        &rows,
        &[
            "Irank",
            "MovieName",
            "GenreMain",
            "WeekBoxOffice",
            "WeekBoxPercent",
            "AvgShowAudienceCount",
            "Country",
            "ReleaseDate",
        ],
    )?;
    df.rename_columns(&[
        "排序",
        "影片名称",
        "类型",
        "首周票房",
        "占总票房比重",
        "场均人次",
        "国家及地区",
        "上映日期",
    ])?;
    // 首周天数：上映日期所在周剩余天数（周一=7 … 周日=1；缺失 → 空值）
    let days = first_week_days(&df, "上映日期")?;
    df.with_column("首周天数", &days)?;
    let mut df = df.select(&[
        "排序",
        "影片名称",
        "类型",
        "首周票房",
        "占总票房比重",
        "场均人次",
        "国家及地区",
        "上映日期",
        "首周天数",
    ])?;
    df.scale("首周票房", 10000.0)?;
    df.cast_numeric(&["占总票房比重", "场均人次"])?;
    df.cast_integer(&["排序", "首周天数"])?;
    df.cast_date(&["上映日期"])?;
    remove_spaces(&mut df, "国家及地区")?;
    df.sort_by_existing("排序", true)
}

/// 电影票房-影院票房-日票房排行（对应 akshare `movie_boxoffice_cinema_daily`）。
pub fn movie_boxoffice_cinema_daily(date: &str) -> Result<Df> {
    let http = HttpClient::default();
    let date_str = format_date(date)?;
    let payload = form(&[
        ("r", &rand_r()),
        ("bserviceprice", "0"),
        ("datetype", "Day"),
        ("date", &date_str),
        ("sdate", &date_str),
        ("edate", &date_str),
        ("citylevel", ""),
        ("lineid", ""),
        ("columnslist", "100,101,102,103,109,108,117"),
        ("pageindex", "1"),
        ("pagesize", "100"),
        ("order", "102"),
        ("ordertype", "desc"),
    ]);
    // akshare 此接口只取第一页（`_post_endata_json`，不翻页）
    let value = post_endata(&http, CINEMA_DAY_URL, &payload, &CINEMA_HEADERS)?;
    let rows = page_rows(&value, "table1");
    let mut df = to_df(
        &rows,
        &[
            "Irank",
            "CinemaName",
            "BoxOffice",
            "ShowCount",
            "AvgShowAudienceCount",
            "AvgBoxOffice",
            "Attendance",
        ],
    )?;
    df.rename_columns(&[
        "排序",
        "影院名称",
        "单日票房",
        "单日场次",
        "场均人次",
        "场均票价",
        "上座率",
    ])?;
    df.cast_numeric(&["单日票房", "场均人次", "场均票价", "上座率"])?;
    df.cast_integer(&["单日场次", "排序"])?;
    df.sort_by_existing("排序", true)
}

/// 电影票房-影院票房-周票房排行（对应 akshare `movie_boxoffice_cinema_weekly`）。
///
/// 与 [`movie_boxoffice_weekly`] 相同：上游周榜接口需要权限，akshare 直接抛
/// `APIError`，本实现对齐该行为。
pub fn movie_boxoffice_cinema_weekly(_date: &str) -> Result<Df> {
    Err(week_permission_error("movie_boxoffice_cinema_weekly"))
}
