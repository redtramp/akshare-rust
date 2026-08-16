//! 新闻联播文字稿数据源（批次 5 长尾 · news）。
//!
//! 对应 akshare `news/news_cctv.py`：抓取央视网「新闻联播」每日文字稿列表，
//! 逐条进入子页提取标题与正文。本实现覆盖 `date > 20160203` 分支
//! （即 `https://tv.cctv.com/lm/xwlb/day/{date}.shtml`），其余历史分支日期
//! 结构不同，按需可后续扩展。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use scraper::{Html, Selector};

const CCTV_LIST_URL: &str = "https://tv.cctv.com/lm/xwlb/day";

/// 人民日报式请求头（对应 akshare `news_cctv` 的 headers）。
const CCTV_HEADERS: &[(&str, &str)] = &[
    (
        "user-agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/92.0.4515.159 Safari/537.36",
    ),
    ("host", "tv.cctv.com"),
];

/// 清理标题：去掉 `[视频]` 前缀并压缩空白。
fn clean_title(raw: &str) -> String {
    raw.trim_start_matches("[视频]")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 清理正文：去掉常见的「央视网消息(新闻联播)：」前缀并压缩空白。
fn clean_content(raw: &str) -> String {
    let stripped = raw
        .trim_start_matches("央视网消息(新闻联播)：")
        .trim_start_matches("央视网消息（新闻联播）：")
        .trim_start_matches("(新闻联播)：");
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 新闻联播文字稿（对应 akshare [`news_cctv`]）。
///
/// # 参数
/// `date`：日期 `YYYYMMDD`（需 `> 20160203`）。
///
/// # 返回列
/// `date, title, content`
pub fn news_cctv(date: &str) -> Result<Df> {
    if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
        return Err(AkshareError::Empty(format!(
            "新闻联播日期需为 YYYYMMDD，收到: {date}"
        )));
    }
    if date <= "20160203" {
        return Err(AkshareError::Empty(format!(
            "news_cctv 仅支持 date > 20160203，收到: {date}"
        )));
    }

    let http = HttpClient::default();
    let list_url = format!("{CCTV_LIST_URL}/{date}.shtml");
    let list_text =
        http.get_text_with_headers(&list_url, &Default::default(), CCTV_HEADERS, None)?;

    let doc = Html::parse_document(&list_text);
    let li_sel = Selector::parse("li").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let a_sel = Selector::parse("a").map_err(|e| AkshareError::Empty(e.to_string()))?;

    let mut links: Vec<String> = Vec::new();
    for (i, li) in doc.select(&li_sel).enumerate() {
        if i == 0 {
            continue; // akshare 跳过第一个 li
        }
        if let Some(a) = li.select(&a_sel).next() {
            if let Some(href) = a.value().attr("href") {
                links.push(href.to_string());
            }
        }
    }

    let h3_sel = Selector::parse("h3").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let tit_sel = Selector::parse("div.tit").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let cnt_sel = Selector::parse("div.cnt_bd").map_err(|e| AkshareError::Empty(e.to_string()))?;
    let area_sel =
        Selector::parse("div.content_area").map_err(|e| AkshareError::Empty(e.to_string()))?;

    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(links.len());
    for link in &links {
        let sub_text =
            match http.get_text_with_headers(link, &Default::default(), CCTV_HEADERS, None) {
                Ok(t) => t,
                Err(_) => continue,
            };
        let sub = Html::parse_document(&sub_text);
        let title = sub
            .select(&h3_sel)
            .next()
            .or_else(|| sub.select(&tit_sel).next())
            .map(|e| e.text().collect::<Vec<_>>().join(""));
        let content = sub
            .select(&cnt_sel)
            .next()
            .or_else(|| sub.select(&area_sel).next())
            .map(|e| e.text().collect::<Vec<_>>().join(""));
        let (title, content) = match (title, content) {
            (Some(t), Some(c)) => (t, c),
            _ => continue,
        };
        rows.push(vec![
            Some(date.to_string()),
            Some(clean_title(&title)),
            Some(clean_content(&content)),
        ]);
    }

    Df::from_string_rows(&["date", "title", "content"], &rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_title_trims_video_tag() {
        assert_eq!(clean_title("[视频] 国内联播快讯"), "国内联播快讯");
    }

    #[test]
    fn clean_content_strips_prefix() {
        let c = "央视网消息(新闻联播)：今日要闻。\n 详情如下。";
        assert_eq!(clean_content(c), "今日要闻。 详情如下。");
    }
}
