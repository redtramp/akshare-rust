//! 雪球（xueqiu）数据源。
//!
//! 对应 akshare `stock_feature/stock_hot_xq.py`：
//! - 先访问 `https://xueqiu.com/` 建立会话 cookie（否则 API 拒绝）
//! - 再请求 `service/v5/stock/screener/screen` 分页拉取热度排行
//! - 响应 `data.count` 决定总页数（每页 200 条）
//!
//! 当前实现无需登录态（v1.0 无浏览器）；若站点将来要求登录，
//! [`crate::core::error::AkshareError::AuthRequired`] 会被触发。

use crate::core::df::Df;
use crate::core::error::{AkshareError, Result};
use crate::core::http::HttpClient;
use serde_json::{json, Map, Value};

const HOME: &str = "https://xueqiu.com/";
const SCREEN_URL: &str = "https://xueqiu.com/service/v5/stock/screener/screen";

/// 雪球热榜公共抓取：分页拉取 `order_by` 排序的热门股票。
///
/// `follow_col`: 输出列中的"关注"来源（`follow` 或 `follow7d`）。
fn hot_rank(order_by: &str, follow_col: &str) -> Result<Df> {
    let http = HttpClient::default();
    // 1) 建立会话 cookie（首页可能返回 WAF 页，仅需 cookie，跳过内容检测）
    let _ = http.get_text_allow_blocked(HOME, &Map::new(), None)?;

    // 2) 首页确定总数
    let params = json!({
        "category": "CN",
        "size": "200",
        "order": "desc",
        "order_by": order_by,
        "only_count": "0",
        "page": "1",
    });
    let first = http.get_json(
        SCREEN_URL,
        params.as_object().expect("静态参数"),
        Some("https://xueqiu.com/hq"),
    )?;
    let data = first
        .get("data")
        .ok_or_else(|| AkshareError::Empty("雪球响应缺少 data".into()))?;
    let total = data.get("count").and_then(Value::as_u64).unwrap_or(0);
    let total_pages = (total as usize).div_ceil(200).max(1);

    let mut all: Vec<Value> = Vec::new();
    for page in 1..=total_pages {
        let mut p = params.as_object().expect("静态参数").clone();
        p.insert("page".to_string(), Value::from(page));
        let resp = http.get_json(SCREEN_URL, &p, Some("https://xueqiu.com/hq"))?;
        if let Some(list) = resp
            .get("data")
            .and_then(|d| d.get("list"))
            .and_then(Value::as_array)
        {
            all.extend(list.iter().cloned());
        }
        if page < total_pages {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }

    if all.is_empty() {
        return Df::from_string_rows(&["股票代码", "股票简称", "关注", "最新价"], &[]);
    }

    let df = Df::from_json_rows(&all)?;
    let mut out = df.select(&["symbol", "name", follow_col, "current"])?;
    out.rename_columns(&["股票代码", "股票简称", "关注", "最新价"])?;
    out.cast_numeric(&["关注", "最新价"])?;
    Ok(out)
}

/// 雪球-沪深股市-热度排行榜-关注排行榜（对应 akshare [`akshare.stock_hot_follow_xq`]）。
///
/// `symbol`: `"本周新增"/"最热门"`。
///
/// # 返回列
/// `股票代码, 股票简称, 关注, 最新价`
pub fn stock_hot_follow_xq(symbol: &str) -> Result<Df> {
    match symbol {
        "本周新增" => hot_rank("follow7d", "follow7d"),
        "最热门" => hot_rank("follow", "follow"),
        _ => Err(AkshareError::Param(format!(
            "无效 symbol: {symbol}（应为 本周新增/最热门）"
        ))),
    }
}

/// 雪球-沪深股市-热度排行榜-讨论排行榜（对应 akshare [`akshare.stock_hot_tweet_xq`]）。
///
/// `symbol`: `"本周新增"/"最热门"`。
///
/// # 返回列
/// `股票代码, 股票简称, 讨论, 最新价`
pub fn stock_hot_tweet_xq(symbol: &str) -> Result<Df> {
    // 讨论榜同样走 screen 接口（order_by=tweet/tweet7d）
    match symbol {
        "本周新增" => hot_rank("tweet7d", "tweet7d"),
        "最热门" => hot_rank("tweet", "tweet"),
        _ => Err(AkshareError::Param(format!(
            "无效 symbol: {symbol}（应为 本周新增/最热门）"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_validation() {
        assert!(stock_hot_follow_xq("无效参数").is_err());
        assert!(stock_hot_tweet_xq("无效参数").is_err());
    }
}
