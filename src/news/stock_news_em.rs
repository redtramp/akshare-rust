//! 东财个股新闻数据源（news 分类）。
//!
//! 对应 akshare `news/news_stock.py`：
//! - [`stock_news_em`]：个股新闻（最近 100 条），`search-api-web.eastmoney.com/search/jsonp`

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{json, Map, Value};

const SEARCH_URL: &str = "https://search-api-web.eastmoney.com/search/jsonp";

/// 东财-个股新闻-最近 100 条（对应 akshare [`akshare.stock_news_em`]）。
///
/// - `symbol`: 股票代码，如 `"603777"`。
///
/// # 返回列
/// `关键词, 新闻标题, 新闻内容, 发布时间, 文章来源, 新闻链接`
pub fn stock_news_em(symbol: &str) -> Result<Df> {
    let inner_param = json!({
        "uid": "",
        "keyword": symbol,
        "type": ["cmsArticleWebOld"],
        "client": "web",
        "clientType": "web",
        "clientVersion": "curr",
        "param": {
            "cmsArticleWebOld": {
                "searchScope": "default",
                "sort": "default",
                "pageIndex": 1,
                "pageSize": 10,
                "preTag": "<em>",
                "postTag": "</em>",
            }
        },
    });
    let inner_str = serde_json::to_string(&inner_param)
        .map_err(|e| AkshareError::json(SEARCH_URL, e.to_string()))?;

    let params = json!({
        "cb": "jQuery35101792940631092459_1764599530165",
        "param": inner_str,
        "_": "1764599530176",
    });
    let params: Map<String, Value> = params.as_object().cloned().unwrap_or_default();

    let http = HttpClient::default();
    let text = http.get_text(SEARCH_URL, &params, Some("https://so.eastmoney.com/"))?;

    // 剥离 JSONP 外壳 `jQuery...(...)`
    let trimmed = text.trim();
    let inner = trimmed
        .strip_prefix("jQuery35101792940631092459_1764599530165(")
        .unwrap_or(trimmed)
        .strip_suffix(')')
        .unwrap_or(trimmed);
    let value: Value =
        serde_json::from_str(inner).map_err(|e| AkshareError::json(SEARCH_URL, e.to_string()))?;

    let articles = value
        .get("result")
        .and_then(|r| r.get("cmsArticleWebOld"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(articles.len());
    for art in &articles {
        let get = |k: &str| -> Option<String> {
            art.get(k).and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Null => None,
                other => Some(other.to_string()),
            })
        };
        let title_raw = get("title").unwrap_or_default();
        let content_raw = get("content").unwrap_or_default();
        let code = get("code").unwrap_or_default();
        let url = format!("http://finance.eastmoney.com/a/{code}.html");

        // 清洗 <em>/</em>/(<em> 等高亮标签（对应 akshare 多次 str.replace）
        let strip_em = |s: &str| {
            s.replace("(<em>", "")
                .replace("</em>)", "")
                .replace("<em>", "")
                .replace("</em>", "")
        };
        let title = strip_em(&title_raw);
        let content = strip_em(&content_raw)
            .replace('\u{3000}', "")
            .replace("\r\n", " ");

        rows.push(vec![
            Some(symbol.to_string()),
            Some(title),
            Some(content),
            get("date"),
            get("mediaName"),
            Some(url),
        ]);
    }

    let df = Df::from_string_rows(
        &[
            "关键词",
            "新闻标题",
            "新闻内容",
            "发布时间",
            "文章来源",
            "新闻链接",
        ],
        &rows,
    )?;
    // 发布时间为字符串（与 akshare 一致，不做日期化）
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonp_strip_offline() {
        let text = r#"jQuery35101792940631092459_1764599530165({"code":0,"result":{}})"#;
        let trimmed = text.trim();
        let inner = trimmed
            .strip_prefix("jQuery35101792940631092459_1764599530165(")
            .unwrap_or(trimmed)
            .strip_suffix(')')
            .unwrap_or(trimmed);
        let v: Value = serde_json::from_str(inner).unwrap();
        assert_eq!(v["code"], 0);
    }

    #[test]
    fn em_tag_clean_offline() {
        let s = "15.<em>03</em>亿元(<em>食品</em>)流入";
        let cleaned = s
            .replace("(<em>", "")
            .replace("</em>)", "")
            .replace("<em>", "")
            .replace("</em>", "");
        assert_eq!(cleaned, "15.03亿元食品流入");
    }
}
