//! 古诗词:今日诗词 API(主源,免Key) + 可配置远程数据集(拉取后缓存 SQLite) + 内置79首兜底
//!
//! 「今日」:每天固定一首(先查当日缓存,未命中则联网,失败用内置库按日期种子确定)
//! 「随机」:今日诗词 API → 远程数据集缓存 → 内置库
//! 「<关键词>」:在内置库按 标签/作者/标题/朝代 过滤,无匹配回退随机

use std::sync::LazyLock;

use chrono::{Datelike, Local};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{err, Assistant, CoreError};
use crate::net::{read_capped, CAP_DATASET, CAP_JSON, UA};

const JINRISHICI_URL: &str = "https://v1.jinrishici.com/all.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poem {
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub dynasty: String,
    #[serde(rename = "content", default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub note: String,
}

static BUNDLED: LazyLock<Vec<Poem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("poems.json")).expect("内置词库 poems.json 解析失败")
});

pub fn format_poem(p: &Poem) -> String {
    let dynasty = if p.dynasty.trim().is_empty() { "佚名" } else { p.dynasty.trim() };
    let author = if p.author.trim().is_empty() { "佚名" } else { p.author.trim() };
    let mut lines = vec![
        format!("📖 「{}」", p.title),
        format!("{dynasty} · {author}"),
        String::new(),
    ];
    lines.extend(p.lines.iter().cloned());
    lines.push(String::new());
    if !p.tags.is_empty() {
        lines.push(p.tags.iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" "));
    }
    if !p.note.is_empty() {
        lines.push(format!("☕ {}", p.note));
    }
    lines.join("\n")
}

// ---------------- 今日诗词 API ----------------

fn fetch_jinrishici(core: &Assistant) -> Option<Poem> {
    let resp = core
        .http
        .get(JINRISHICI_URL)
        .header("User-Agent", UA)
        .send()
        .ok()?;
    let (body, _) = read_capped(resp, CAP_JSON);
    let v: Value = serde_json::from_str(&body).ok()?;
    let content = v["content"].as_str()?.trim().to_string();
    if content.is_empty() {
        return None;
    }
    Some(Poem {
        title: v["origin"].as_str().unwrap_or("今日诗词").trim().to_string(),
        author: v["author"].as_str().unwrap_or("").trim().to_string(),
        dynasty: String::new(),
        lines: vec![content],
        tags: v["category"]
            .as_str()
            .map(|c| vec![c.trim().to_string()])
            .unwrap_or_default(),
        note: "来自今日诗词 API".to_string(),
    })
}

// ---------------- 远程数据集(可选,配置 poem_urls 后生效) ----------------

fn poem_from_value(v: &Value) -> Option<Poem> {
    let title = v["title"]
        .as_str()
        .or_else(|| v["name"].as_str())?
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let as_lines = |x: &Value| -> Vec<String> {
        if let Some(a) = x.as_array() {
            a.iter()
                .filter_map(|i| i.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else if let Some(s) = x.as_str() {
            if s.trim().is_empty() { vec![] } else { vec![s.trim().to_string()] }
        } else {
            vec![]
        }
    };
    let mut lines = as_lines(&v["content"]);
    if lines.is_empty() {
        lines = as_lines(&v["paragraphs"]);
    }
    if lines.is_empty() {
        lines = as_lines(&v["para"]);
    }
    if lines.is_empty() {
        return None;
    }
    Some(Poem {
        title,
        author: v["author"]
            .as_str()
            .or_else(|| v["poet"].as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        dynasty: v["dynasty"].as_str().unwrap_or("").trim().to_string(),
        lines,
        tags: vec![],
        note: String::new(),
    })
}

/// 按 kv 中配置的 URL 列表拉取数据集(任意 chinese-poetry 风格的 JSON 数组),
/// 成功后缓存前 500 首到 SQLite,此后离线也可用。
/// 响应限量读取(CAP_DATASET);数据集超限时明确报原因,不与普通失败混淆。
pub fn refresh_pool(core: &Assistant) -> Result<Option<usize>, String> {
    let urls: Vec<String> = {
        let s = core
            .store
            .lock()
            .map_err(|_| "内部存储忙,暂无法预取".to_string())?;
        s.kv_get("poem_urls")
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("https://") || l.starts_with("http://"))
            .map(str::to_string)
            .collect()
    };
    let mut too_big = false;
    for url in urls {
        let Ok(resp) = core.http.get(&url).header("User-Agent", UA).send() else { continue };
        let (body, truncated) = read_capped(resp, CAP_DATASET);
        if truncated {
            too_big = true; // 该数据集超过上限,跳过它试下一个源
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&body) else { continue };
        let Some(arr) = v.as_array() else { continue };
        let poems: Vec<Poem> = arr.iter().filter_map(poem_from_value).collect();
        if !poems.is_empty() {
            let n = poems.len();
            let sample: Vec<Poem> = poems.into_iter().take(500).collect();
            if let Ok(s) = core.store.lock() {
                if let Ok(txt) = serde_json::to_string(&sample) {
                    s.kv_set("poem_pool_cache", &txt);
                }
            }
            return Ok(Some(n));
        }
    }
    if too_big {
        Err(format!(
            "数据集过大(超过 {}MB 上限),请改用更小的数据文件",
            CAP_DATASET / 1024 / 1024
        ))
    } else {
        Ok(None)
    }
}

fn random_from_pool_cache(core: &Assistant) -> Option<Poem> {
    let raw = {
        let s = core.store.lock().ok()?;
        s.kv_get("poem_pool_cache")?
    };
    let pool: Vec<Poem> = serde_json::from_str(&raw).ok()?;
    let mut rng = rand::thread_rng();
    Some(pool[rng.gen_range(0..pool.len())].clone())
}

// ---------------- 缓存工具(按天固定) ----------------

fn poem_cache_get(core: &Assistant, key: &str) -> Option<Poem> {
    let raw = {
        let s = core.store.lock().ok()?;
        s.kv_get(key)?
    };
    serde_json::from_str(&raw).ok()
}

fn poem_cache_set(core: &Assistant, key: &str, poem: &Poem) {
    if let Ok(s) = core.store.lock() {
        if let Ok(txt) = serde_json::to_string(poem) {
            s.kv_set(key, &txt);
        }
    }
}

fn bundled_daily() -> Option<Poem> {
    let now = Local::now();
    let seed = (now.year() as u64) * 10000 + (now.month() as u64) * 100 + now.day() as u64;
    let mut rng = StdRng::seed_from_u64(seed ^ 0x700E4D);
    let i = rng.gen_range(0..BUNDLED.len());
    Some(BUNDLED[i].clone())
}

fn bundled_random() -> Result<String, CoreError> {
    let mut rng = rand::thread_rng();
    let i = rng.gen_range(0..BUNDLED.len());
    Ok(format_poem(&BUNDLED[i]))
}

// ---------------- 对外入口 ----------------

pub fn recommend(core: &Assistant, query: &str) -> Result<String, CoreError> {
    let q = query.trim();
    if q == "今日" {
        let key = format!("daily_poem:{}", Local::now().format("%Y%m%d"));
        if let Some(p) = poem_cache_get(core, &key) {
            return Ok(format_poem(&p));
        }
        match fetch_jinrishici(core) {
            Some(p) => {
                poem_cache_set(core, &key, &p);
                crate::cache::remember(core, "cache:poem", &format_poem(&p));
                Ok(format_poem(&p))
            }
            None => {
                // 离线:优先回退最近联网取到的诗(带保存时间),否则内置库按日期兜底
                if let Some((v, ts)) = crate::cache::recall(core, "cache:poem") {
                    return Ok(format!("{v}\n\n{}", crate::cache::offline_note(ts, "诗词")));
                }
                let poem = bundled_daily().ok_or_else(|| err("今日诗词获取失败,稍后再试"))?;
                poem_cache_set(core, &key, &poem);
                Ok(format_poem(&poem))
            }
        }
    } else if q.is_empty() || q == "随机" {
        if let Some(p) = fetch_jinrishici(core) {
            crate::cache::remember(core, "cache:poem", &format_poem(&p));
            return Ok(format_poem(&p));
        }
        // 离线:优先回退最近联网取到的诗(带保存时间)
        if let Some((v, ts)) = crate::cache::recall(core, "cache:poem") {
            return Ok(format!("{v}\n\n{}", crate::cache::offline_note(ts, "诗词")));
        }
        if let Some(p) = random_from_pool_cache(core) {
            return Ok(format_poem(&p));
        }
        bundled_random()
    } else {
        let pool: Vec<&Poem> = BUNDLED
            .iter()
            .filter(|p| {
                p.tags.iter().any(|t| t == q)
                    || p.author.contains(q)
                    || p.title.contains(q)
                    || p.dynasty.contains(q)
            })
            .collect();
        if !pool.is_empty() {
            let mut rng = rand::thread_rng();
            let i = rng.gen_range(0..pool.len());
            return Ok(format_poem(pool[i]));
        }
        let hint = format!("词库里没有与「{q}」相关的,随机来一首吧~\n\n");
        if let Some(p) = fetch_jinrishici(core) {
            return Ok(format!("{hint}{}", format_poem(&p)));
        }
        bundled_random().map(|s| format!("{hint}{s}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_loads_and_filters() {
        assert!(BUNDLED.len() >= 70, "内置词库应≥70首,实际 {}", BUNDLED.len());
        let pool: Vec<&Poem> = BUNDLED
            .iter()
            .filter(|p| p.tags.iter().any(|t| t == "励志"))
            .collect();
        assert!(!pool.is_empty());
        let libai: Vec<&Poem> = BUNDLED.iter().filter(|p| p.author.contains("李白")).collect();
        assert!(libai.len() >= 5);
    }

    #[test]
    fn format_looks_right() {
        let p = &BUNDLED[0];
        let s = format_poem(p);
        assert!(s.contains(&format!("📖 「{}」", p.title)));
        assert!(s.contains("·"));
    }
}
