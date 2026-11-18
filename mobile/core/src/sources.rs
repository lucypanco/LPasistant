// 用户自定义源管理(radio + news):JSON 模型 + 连通性自检 + 启停
//
// kv 格式(均为 JSON 数组):
//   radio_user_stations: [{"name":"..","url":"..","enabled":true,"ok":null,"err":""}]
//   news_user_feeds:     [{"url":"..","enabled":true,"ok":null,"err":"","title":""}]
// ok: null=未测 / true=最近一次自检连通 / false=失败(err 带原因)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct UserStation {
    pub name: String,
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub ok: Option<bool>,
    #[serde(default)]
    pub err: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct UserFeed {
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub ok: Option<bool>,
    #[serde(default)]
    pub err: String,
    #[serde(default)]
    pub title: String,
}

fn default_true() -> bool {
    true
}

fn default_enabled() -> bool {
    true
}

// ---------- 读写 ----------

pub fn load_stations(core: &crate::api::Assistant) -> Vec<UserStation> {
    let raw = core
        .store
        .lock()
        .ok()
        .and_then(|s| s.kv_get("radio_user_stations"))
        .unwrap_or_default();
    parse_or_migrate_stations(&raw)
}

fn parse_or_migrate_stations(raw: &str) -> Vec<UserStation> {
    let t = raw.trim();
    if t.starts_with('[') {
        return serde_json::from_str(t).unwrap_or_default();
    }
    // 旧格式迁移:「名称|URL」行
    t.lines()
        .filter_map(|l| l.split_once('|'))
        .map(|(n, u)| UserStation {
            name: n.trim().to_string(),
            url: u.trim().to_string(),
            enabled: true,
            ok: None,
            err: String::new(),
        })
        .collect()
}

pub fn save_stations(core: &crate::api::Assistant, list: &[UserStation]) {
    if let Ok(txt) = serde_json::to_string(list) {
        if let Ok(s) = core.store.lock() {
            s.kv_set("radio_user_stations", &txt);
        }
    }
}

pub fn load_feeds(core: &crate::api::Assistant) -> Vec<UserFeed> {
    let raw = core
        .store
        .lock()
        .ok()
        .and_then(|s| s.kv_get("news_user_feeds"))
        .unwrap_or_default();
    parse_or_migrate_feeds(&raw)
}

fn parse_or_migrate_feeds(raw: &str) -> Vec<UserFeed> {
    let t = raw.trim();
    if t.starts_with('[') {
        return serde_json::from_str(t).unwrap_or_default();
    }
    t.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("http"))
        .map(|u| UserFeed {
            url: u.to_string(),
            enabled: true,
            ok: None,
            err: String::new(),
            title: String::new(),
        })
        .collect()
}

pub fn save_feeds(core: &crate::api::Assistant, list: &[UserFeed]) {
    if let Ok(txt) = serde_json::to_string(list) {
        if let Ok(s) = core.store.lock() {
            s.kv_set("news_user_feeds", &txt);
        }
    }
}

// ---------- 连通性自检 ----------

/// 电台流自检:GET 取响应头与首块,期望 2xx + 音频/流媒体特征
pub fn probe_radio(core: &crate::api::Assistant, url: &str) -> (bool, String) {
    let client = core.http.clone();
    let resp = match client
        .get(url)
        .header("User-Agent", crate::net::UA)
        .header("Icy-MetaData", "1")
        .timeout(std::time::Duration::from_secs(8))
        .send()
    {
        Ok(r) => r,
        Err(e) => return (false, format!("连接失败:{e}")),
    };
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return (false, format!("HTTP {status}"));
    }
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    // 流媒体常见:audio/*、视频容器也可播;HTML 大概率是错误页
    if ct.starts_with("text/html") {
        return (false, format!("返回网页而非音频({ct})"));
    }
    (true, ct)
}

/// 新闻源自检:GET(限量 64KB),期望 XML/Feed 特征
pub fn probe_news(core: &crate::api::Assistant, url: &str) -> (bool, String, String) {
    let client = core.http.clone();
    let resp = match client
        .get(url)
        .header("User-Agent", crate::net::UA)
        .timeout(std::time::Duration::from_secs(10))
        .send()
    {
        Ok(r) => r,
        Err(e) => return (false, String::new(), format!("连接失败:{e}")),
    };
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return (false, String::new(), format!("HTTP {status}"));
    }
    let body = match resp.bytes() {
        Ok(b) => {
            let cut = b.len().min(65536);
            String::from_utf8_lossy(&b[..cut]).to_string()
        }
        Err(e) => return (false, String::new(), format!("读取失败:{e}")),
    };
    let low = body.to_lowercase();
    let is_feed = low.contains("<item") || low.contains("<entry") || low.contains("<channel");
    if !is_feed {
        return (false, String::new(), "响应不是 RSS/Atom 格式".to_string());
    }
    // 抽标题
    let title = body
        .find("<title>")
        .and_then(|i| body[i + 7..].find("</title>").map(|j| body[i + 7..i + 7 + j].to_string()))
        .unwrap_or_default();
    (true, title, String::new())
}
