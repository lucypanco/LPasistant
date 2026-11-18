//! 新闻:专业源白名单(国际 + 科技·科研 + 时政),统一 RSS/Atom/RDF 解析,
//! Rust 并行线程同时抓取(总延迟≈最慢单源),合并后 simhash 去重。
//! 按用户要求:不接娱乐热搜类源(60秒/头条/知乎/B站已移除)。
//!
//! 结构化输出:每条新闻保留 {来源, 标题, 链接},供 UI 点击获取详情。
//! 详情链路:抓取文章页 → 提取正文(纯代码)→ AI摘要(可选,若兜底开启)
//!          或规则抽取要点(新闻导语式文体,前几句即要点)。

use std::sync::LazyLock;

use regex::Regex;

use crate::api::{err, Assistant, CoreError};
use crate::net::{read_capped, CAP_FEED, CAP_PAGE, UA};
use crate::simhash;

// ---------------- 源白名单 ----------------

#[derive(Clone, Copy, PartialEq)]
pub enum Cat {
    General,
    Tech,
    Politics,
}

#[derive(Clone)]
pub struct Feed {
    pub label: &'static str,
    pub url: &'static str,
    pub cat: Cat,
    pub foreign: bool,
}

/// 默认源 = 本环境实测可达;被墙/不稳定的源以注释保留,网络可达时取消注释即可。
pub const FEEDS: &[Feed] = &[
    // ---- 国际综合 ----
    Feed { label: "CNBC", url: "https://www.cnbc.com/id/100003114/device/rss/rss.html", cat: Cat::General, foreign: true },
    // ---- 国外科技·科研 ----
    Feed { label: "Verge", url: "https://www.theverge.com/rss/index.xml", cat: Cat::Tech, foreign: true },
    Feed { label: "Ars", url: "https://feeds.arstechnica.com/arstechnica/index", cat: Cat::Tech, foreign: true },
    Feed { label: "TC", url: "https://techcrunch.com/feed/", cat: Cat::Tech, foreign: true },
    Feed { label: "HN", url: "https://hnrss.org/frontpage", cat: Cat::Tech, foreign: true },
    Feed { label: "Nature", url: "https://www.nature.com/nature.rss", cat: Cat::Tech, foreign: true },
    // ---- 国内科技 ----
    Feed { label: "Solidot", url: "https://www.solidot.org/index.rss", cat: Cat::Tech, foreign: false },
    Feed { label: "爱范儿", url: "https://www.ifanr.com/feed", cat: Cat::Tech, foreign: false },
    Feed { label: "少数派", url: "https://sspai.com/feed", cat: Cat::Tech, foreign: false },
    // ---- 国内时政 ----
    Feed { label: "人民网", url: "https://www.people.com.cn/rss/politics.xml", cat: Cat::Politics, foreign: false },
];

const PER_FEED: usize = 6;

/// 一条新闻(标题 + 原文链接,供点击详情)
/// cached_at > 0 表示来自离线缓存(值为缓存保存时间),UI 据此显示标注
#[derive(Debug, Clone)]
pub struct NewsItem {
    pub source: String,
    pub title: String,
    pub url: String,
    pub cached_at: i64,
}

// ---------------- RSS/Atom/RDF 解析 ----------------

static RE_ITEM_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<item[^>]*>(.*?)</item>").unwrap());
static RE_ENTRY_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<entry[^>]*>(.*?)</entry>").unwrap());
static RE_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>\s*(?:<!\[CDATA\[)?(.*?)\s*(?:\]\]>)?\s*</title>").unwrap());
static RE_LINK_HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<link[^>]*href="([^"]+)""#).unwrap());
static RE_LINK_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<link[^>]*>\s*(.*?)\s*</link>").unwrap());
// 注意:Rust regex 不支持反向引用,不能写成 <(script|style)>…</\1> —— 编译期即恐慌(曾致详情功能整体毒化)
static RE_SCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap());
static RE_STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap());
static RE_P: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap());
static RE_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]+>").unwrap());

pub(crate) fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

fn clean_text(s: &str) -> String {
    let t = unescape(&RE_TAGS.replace_all(s, " "));
    let mut out = String::new();
    let mut prev_space = true;
    for c in t.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn take_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}

fn clip(s: &str, n: usize) -> String {
    let s = clean_text(s);
    take_chars(&s, n)
}

/// 从条目块里取正文链接(RSS:<link>text;Atom:<link href>)
fn link_of(block: &str, feed_url: &str) -> String {
    let raw = RE_LINK_HREF
        .captures(block)
        .map(|c| c[1].trim().to_string())
        .or_else(|| {
            RE_LINK_TEXT.captures(block).map(|c| {
                let t = c[1].trim();
                RE_TAGS.replace_all(t, "").trim().to_string()
            })
        })
        .unwrap_or_default();
    // 相对链接 → 补全为绝对地址
    if raw.starts_with("//") {
        format!("https:{raw}")
    } else if raw.starts_with('/') {
        let origin = feed_url
            .split("://")
            .next()
            .map(|scheme| format!("{}://{}", scheme, feed_url.split("://").nth(1).unwrap_or("").split('/').next().unwrap_or("")))
            .unwrap_or_default();
        format!("{origin}{raw}")
    } else {
        raw
    }
}

/// 单源抓取:返回前 per 条(任何失败返回空)。
/// 响应体限量读取(CAP_FEED):截断时尾部残缺块不会匹配 <item>,自然少收几条。
fn fetch_feed(core: &Assistant, feed: &Feed, per: usize) -> Vec<NewsItem> {
    let Ok(resp) = core.http.get(feed.url).header("User-Agent", UA).send() else {
        return vec![];
    };
    let (text, _truncated) = read_capped(resp, CAP_FEED);
    let items: Vec<&str> = RE_ITEM_BLOCK.captures_iter(&text).map(|c| c.get(1).unwrap().as_str()).collect();
    let entries: Vec<&str> = RE_ENTRY_BLOCK.captures_iter(&text).map(|c| c.get(1).unwrap().as_str()).collect();
    let blocks = if entries.len() > items.len() { entries } else { items };
    blocks
        .iter()
        .filter_map(|b| {
            let title = RE_TITLE.captures(b).map(|c| clip(&c[1], 60))?;
            if title.is_empty() {
                return None;
            }
            let url = link_of(b, feed.url);
            Some(NewsItem {
                source: feed.label.to_string(),
                title,
                url,
                cached_at: 0,
            })
        })
        .take(per)
        .collect()
}

/// 并行抓取多个源(scoped threads;每个源独立超时,互不阻塞)
/// 返回与 feeds 等长的逐源列表(失败的源为空),供上层按国内外分组
fn fetch_many(core: &Assistant, feeds: &[&Feed]) -> Vec<Vec<NewsItem>> {
    std::thread::scope(|s| {
        let handles: Vec<_> = feeds
            .iter()
            .map(|f| s.spawn(move || fetch_feed(core, f, PER_FEED)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

fn dedup(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut kept: Vec<NewsItem> = Vec::new();
    let mut hs: Vec<u64> = Vec::new();
    for it in items {
        let h = simhash::hash(&it.title);
        if hs.iter().any(|k| simhash::hamming(*k, h) <= 3) {
            continue;
        }
        hs.push(h);
        kept.push(it);
    }
    kept
}

/// 两组轮流交错(f0,d0,f1,d1,…),多余的一方接到尾部
fn interleave(a: Vec<NewsItem>, b: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut ia = a.into_iter();
    let mut ib = b.into_iter();
    let mut out = Vec::new();
    loop {
        let x = ia.next();
        let y = ib.next();
        if x.is_none() && y.is_none() {
            break;
        }
        if let Some(v) = x {
            out.push(v);
        }
        if let Some(v) = y {
            out.push(v);
        }
    }
    out
}

/// 用户自定义源(kv news_user_feeds,一行一个 URL,最多 5 个)
fn user_feed_urls(core: &Assistant) -> Vec<String> {
    crate::sources::load_feeds(core)
        .into_iter()
        .filter(|f| f.enabled)
        .map(|f| f.url)
        .take(8)
        .collect()
}

/// 全部源 = 内置白名单 + 用户自定义(自定义源算国内综合类)
pub fn all_feeds(core: &Assistant) -> Vec<Feed> {
    let mut feeds: Vec<Feed> = FEEDS.to_vec();
    for (i, url) in user_feed_urls(core).into_iter().enumerate() {
        feeds.push(Feed {
            label: Box::leak(format!("自定义{}", i + 1).into_boxed_str()),
            url: Box::leak(url.into_boxed_str()),
            cat: Cat::General,
            foreign: false,
        });
    }
    feeds
}

/// 结构化新闻列表(kind: all/intl/tech/politics)
/// 「all/tech」同时含国内外源:两组各自 simhash 去重后对半交错,保证前 n 条国内外各半
pub fn news_items(core: &Assistant, kind: &str, n: usize) -> Result<Vec<NewsItem>, CoreError> {
    let all = all_feeds(core);
    let items: Vec<&Feed> = all.iter()
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|f| match kind {
            "intl" => f.foreign,
            "tech" => f.cat == Cat::Tech,
            "politics" => f.cat == Cat::Politics,
            _ => true,
        })
        .collect();
    if items.is_empty() {
        return Err(err("没有可用源"));
    }
    let lists = fetch_many(core, &items);
    let mut foreign: Vec<NewsItem> = Vec::new();
    let mut domestic: Vec<NewsItem> = Vec::new();
    for (list, feed) in lists.into_iter().zip(items.iter()) {
        if feed.foreign {
            foreign.extend(list);
        } else {
            domestic.extend(list);
        }
    }
    let merged = if foreign.is_empty() || domestic.is_empty() {
        // 一侧全部不可达:退化为另一侧,不让整个列表空掉
        let mut m = foreign;
        m.extend(domestic);
        dedup(m)
    } else {
        interleave(dedup(foreign), dedup(domestic))
    };
    if merged.is_empty() {
        // 全部源不可达(离线/网络故障):回退最近一次抓取的缓存,标注时间由 UI 呈现
        let cache_key = format!("cache:news:{kind}");
        if let Some((items, _ts)) = crate::cache::recall_news(core, &cache_key) {
            return Ok(items);
        }
        return Err(err("新闻源暂时都连不上,稍后再试试~"));
    }
    let picked: Vec<NewsItem> = merged.into_iter().take(n).collect();
    // 成功抓取 → 保留一份供离线查看
    crate::cache::remember_news(core, &format!("cache:news:{kind}"), &picked);
    Ok(picked)
}

fn header_of(kind: &str) -> &'static str {
    match kind {
        "intl" => "🌏 国际媒体",
        "tech" => "🔬 科技·科研",
        "politics" => "🏛 时政要闻",
        _ => "📰 要闻(国际+科技+时政)",
    }
}

fn format_items(items: &[NewsItem], header: &str) -> String {
    let body: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, it)| format!("{}. [{}] {}", i + 1, it.source, it.title))
        .collect();
    // 离线缓存回退时标注保存时间
    let cached_ts = items.first().map(|i| i.cached_at).unwrap_or(0);
    let note = if cached_ts > 0 {
        format!("\n\n{}", crate::cache::offline_note(cached_ts, "新闻"))
    } else {
        String::new()
    };
    format!("{header}\n{}{note}", body.join("\n"))
}

// ---------------- 文本指令入口(「新闻/国际/科技/时政」) ----------------

pub fn news_all(core: &Assistant, n: usize) -> Result<String, CoreError> {
    Ok(format_items(&news_items(core, "all", n)?, header_of("all")))
}

pub fn news_intl(core: &Assistant, n: usize) -> Result<String, CoreError> {
    Ok(format_items(&news_items(core, "intl", n)?, header_of("intl")))
}

pub fn news_tech(core: &Assistant, n: usize) -> Result<String, CoreError> {
    Ok(format_items(&news_items(core, "tech", n)?, header_of("tech")))
}

pub fn news_politics(core: &Assistant, n: usize) -> Result<String, CoreError> {
    Ok(format_items(&news_items(core, "politics", n)?, header_of("politics")))
}

// ---------------- 详情:抓原文 + 摘要 ----------------

/// 正文提取:优先 <p> 段落,兜底整页去标签
fn extract_body(html: &str, max_chars: usize) -> String {
    let no_script = RE_SCRIPT.replace_all(html, " ");
    let stripped = RE_STYLE.replace_all(&no_script, " ");
    let paras: Vec<String> = RE_P
        .captures_iter(&stripped)
        .map(|c| clean_text(&c[1]))
        .filter(|p| p.chars().count() > 20)
        .collect();
    let joined = if paras.is_empty() {
        clean_text(&stripped)
    } else {
        paras.join("\n")
    };
    take_chars(&joined, max_chars)
}

/// 规则抽取要点:导语式新闻前几句即重点(不依赖任何模型)
fn extractive_lead(body: &str) -> String {
    let mut out = String::new();
    for sent in body.split(['。', '!', '?', ';']) {
        let s = sent.trim();
        if s.chars().count() < 8 {
            continue;
        }
        if out.chars().count() + s.chars().count() > 150 {
            break;
        }
        out.push_str(s);
        out.push_str("。");
        if out.chars().count() >= 80 {
            break;
        }
    }
    if out.is_empty() {
        take_chars(body, 100)
    } else {
        out
    }
}

/// 新闻详情:抓文章页 → 正文提取 → (可选)本地AI摘要 → 组装。
/// 文章页限量读取(CAP_PAGE);截断时在结尾给用户分隔提示,便于判断内容完整度。
pub fn detail(core: &Assistant, url: &str, title: &str) -> Result<String, CoreError> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(err("链接无效"));
    }
    let resp = core
        .http
        .get(url)
        .header("User-Agent", UA)
        .send()
        .map_err(|e| err(format!("页面抓取失败:{e}")))?;
    let final_url = resp.url().to_string();
    let (html, truncated) = read_capped(resp, CAP_PAGE);
    let body = extract_body(&html, 1600);
    if body.chars().count() < 60 {
        return Err(err("该页面无法提取正文(多为动态渲染页),请点原链接阅读"));
    }

    // AI 摘要:仅在 AI兜底开启且模型就绪时;输入截断,防上下文溢出
    let ai_part = if core.llm_enabled() && core.llm_ready() {
        let input = take_chars(&body, 900);
        let prompt = format!("用不超过3句话概括这条新闻的要点,只输出概括本身:\n{input}");
        let s = core.infer(&prompt);
        let s = s.trim();
        if s.chars().count() > 10 {
            format!("🧩 AI摘要:\n{s}\n\n")
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // 截断分隔提示:原文超出上限被截断时,明确告诉用户「读到的是开头,可能不完整」
    let trunc_note = if truncated {
        format!(
            "\n\n✂️ ── 以上为原文前 {}MB,已截断 ──\n要点基于开头部分,可能不完整;完整内容请点上方链接",
            CAP_PAGE / 1024 / 1024
        )
    } else {
        String::new()
    };

    Ok(format!(
        "📰 {}\n🔗 {}\n\n{}📄 要点:{}\n\n—— 正文节选 ——\n{}\n…\n(完整内容点链接查看){}",
        title,
        final_url,
        ai_part,
        extractive_lead(&body),
        take_chars(&body, 400),
        trunc_note
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(source: &str, title: &str) -> NewsItem {
        NewsItem {
            source: source.to_string(),
            title: title.to_string(),
            url: String::new(),
            cached_at: 0,
        }
    }

    #[test]
    fn interleave_alternates_groups() {
        let foreign: Vec<NewsItem> = (0..3).map(|i| item("F", &format!("国外{i}"))).collect();
        let domestic: Vec<NewsItem> = (0..2).map(|i| item("D", &format!("国内{i}"))).collect();
        let m = interleave(foreign, domestic);
        let srcs: Vec<&str> = m.iter().map(|x| x.source.as_str()).collect();
        assert_eq!(srcs, vec!["F", "D", "F", "D", "F"], "应交错且多余一方接尾");
    }

    #[test]
    fn detail_regexes_compile_and_strip() {
        // 反向引用曾致编译恐慌 + LazyLock 毒化,这里锁死「能编译」这一底线
        let html = "<html><script>alert(1)</script><style>.x{}</style><p>正文段落,长度超过二十个字符的中文内容。</p></html>";
        let body = extract_body(html, 200);
        assert!(body.contains("正文段落"));
        assert!(!body.contains("alert"));
        assert!(!body.contains(".x{}"));
    }
}
