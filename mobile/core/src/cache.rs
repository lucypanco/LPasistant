//! 最近数据缓存:天气/新闻/诗词成功抓取后写入 kv;离线或源不可达时
//! 回退最近一次数据并标注保存时间,满足离线查看需求。
//! 存储格式:kv JSON {"t": 保存时间戳, "v": 内容}。

use crate::api::{Assistant, CoreError};
use chrono::Local;

/// 时间戳 → 「今天 14:30」/「11月13日 14:30」
fn label(ts: i64) -> String {
    match chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&Local)) {
        Some(d) if d.date_naive() == Local::now().date_naive() => d.format("今天 %H:%M").to_string(),
        Some(d) => d.format("%m月%d日 %H:%M").to_string(),
        None => String::new(),
    }
}

/// 保存最近一次成功数据
pub fn remember(core: &Assistant, key: &str, value: &str) {
    let txt = serde_json::json!({ "t": Local::now().timestamp(), "v": value }).to_string();
    if let Ok(s) = core.store.lock() {
        s.kv_set(key, &txt);
    }
}

/// 取回 (内容, 保存时间戳)
pub fn recall(core: &Assistant, key: &str) -> Option<(String, i64)> {
    let raw = core.store.lock().ok()?.kv_get(key)?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some((v["v"].as_str()?.to_string(), v["t"].as_i64().unwrap_or(0)))
}

/// 「何时保存」标注行(离线提示)
pub fn offline_note(ts: i64, what: &str) -> String {
    format!("📴 当前离线,以上为{}保存的{what}缓存", label(ts))
}

/// 文本类接口通用包装:成功→缓存一份;失败→回退缓存并附时间标注
pub fn cached_text(
    core: &Assistant,
    key: &str,
    r: Result<String, CoreError>,
    what: &str,
) -> Result<String, CoreError> {
    match r {
        Ok(text) => {
            remember(core, key, &text);
            Ok(text)
        }
        Err(e) => match recall(core, key) {
            Some((v, ts)) => Ok(format!("{v}\n\n{}", offline_note(ts, what))),
            None => Err(e),
        },
    }
}

/// 新闻列表专用:结构化缓存(cached_at 由调用方恢复到每条上)
pub fn remember_news(core: &Assistant, key: &str, items: &[crate::news::NewsItem]) {
    let arr: Vec<serde_json::Value> = items
        .iter()
        .map(|i| serde_json::json!({ "s": i.source, "h": i.title, "u": i.url }))
        .collect();
    let txt = serde_json::json!({ "t": Local::now().timestamp(), "v": arr }).to_string();
    if let Ok(s) = core.store.lock() {
        s.kv_set(key, &txt);
    }
}

pub fn recall_news(core: &Assistant, key: &str) -> Option<(Vec<crate::news::NewsItem>, i64)> {
    let raw = core.store.lock().ok()?.kv_get(key)?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let ts = v["t"].as_i64().unwrap_or(0);
    let arr = v["v"].as_array()?;
    let items = arr
        .iter()
        .filter_map(|i| {
            Some(crate::news::NewsItem {
                source: i["s"].as_str()?.to_string(),
                title: i["h"].as_str()?.to_string(),
                url: i["u"].as_str()?.to_string(),
                cached_at: ts,
            })
        })
        .collect();
    Some((items, ts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::err;

    #[test]
    fn text_roundtrip_and_offline_note() {
        let db = std::env::temp_dir().join(format!("lp_cache_{}.db", uuid::Uuid::new_v4()));
        let core = Assistant::new(db.to_str().unwrap().to_string()).unwrap();
        // 无缓存时失败原样透传
        assert!(cached_text(&core, "k", Err(err("源不可达")), "天气").is_err());
        // 成功即缓存
        cached_text(&core, "k", Ok("北京 晴 12°C".into()), "天气").unwrap();
        // 失败回退缓存 + 时间标注
        let out = cached_text(&core, "k", Err(err("源不可达")), "天气").unwrap();
        assert!(out.contains("北京 晴 12°C"));
        assert!(out.contains("离线") && out.contains("保存的天气缓存"));
        // 标注带「今天」或日期前缀(非空)
        let note = offline_note(Local::now().timestamp(), "天气");
        assert!(note.starts_with("📴 当前离线,以上为今天"));
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn news_roundtrip() {
        let db = std::env::temp_dir().join(format!("lp_cache_{}.db", uuid::Uuid::new_v4()));
        let core = Assistant::new(db.to_str().unwrap().to_string()).unwrap();
        assert!(recall_news(&core, "nk").is_none());
        let items = vec![crate::news::NewsItem {
            source: "CNBC".into(),
            title: "测试标题".into(),
            url: "https://x/1".into(),
            cached_at: 0,
        }];
        remember_news(&core, "nk", &items);
        let (back, ts) = recall_news(&core, "nk").unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].title, "测试标题");
        assert!(back[0].cached_at == ts && ts > 0);
        let _ = std::fs::remove_file(&db);
    }
}
