//! 电台:精选在线电台(本环境实测可达,2026-11-14 起全部 https)+ radio-browser 开放API搜索(免Key)
//!
//! 架构:Rust 只负责"台标数据与搜索";音频播放由 Kotlin 侧 Media3/ExoPlayer 完成。
//! 安全边界:targetSdk 34 平台层默认禁明文流量,因此搜索结果只收 https 台
//! (http 台放进来也播不了,只会让用户点了没反应);精选台实测 https 证书有效。

use serde_json::Value;

use crate::api::{err, Assistant, CoreError};
use crate::net::{read_capped, CAP_JSON, UA};

pub struct Station {
    pub name: &'static str,
    pub url: &'static str,
    pub desc: &'static str,
    pub region: &'static str,
}

pub const STATIONS: &[Station] = &[
    Station {
        name: "央广·中国之声",
        url: "https://ngcdn001.cnr.cn/live/zgzs/index.m3u8",
        desc: "国家电台·新闻综合",
        region: "国内",
    },
    Station {
        name: "央广·经济之声",
        url: "https://ngcdn002.cnr.cn/live/jjzs/index.m3u8",
        desc: "财经·经济",
        region: "国内",
    },
    Station {
        name: "Radio Paradise",
        url: "https://stream.radioparadise.com/mp3-128",
        desc: "自由风格摇滚/混合",
        region: "国际",
    },
    Station {
        name: "Nightride FM",
        url: "https://stream.nightride.fm/nightride.mp3",
        desc: "合成器浪潮/电子",
        region: "国际",
    },
    Station {
        name: "KEXP",
        url: "https://kexp-mp3-128.streamguys1.com/kexp128.mp3",
        desc: "西雅图独立/另类",
        region: "国际",
    },
];

pub fn stations_text() -> String {
    let mut lines = vec!["📻 电台(快捷词条卡片可点 ▶ 播放)".to_string()];
    for (i, s) in STATIONS.iter().enumerate() {
        lines.push(format!("{}. {} —— {}({})", i + 1, s.name, s.desc, s.region));
    }
    lines.push("🔍 搜索全球数万台:电台 搜索 <关键词>(radio-browser 开放API)".to_string());
    lines.join("\n")
}

pub struct StationHit {
    pub name: String,
    pub url: String,
    pub desc: String,
    pub region: String,
}

const RB_MIRRORS: [&str; 2] = [
    "https://de1.api.radio-browser.info",
    "https://de2.api.radio-browser.info",
];

/// radio-browser 开放API搜索(免Key,支持中文台名);逐镜像回退
pub fn search(core: &Assistant, query: &str, n: usize) -> Result<Vec<StationHit>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Err(err("请输入关键词,如:电台 搜索 爵士"));
    }
    let mut last_err = String::new();
    for base in RB_MIRRORS {
        let resp = core.http.get(format!("{base}/json/stations/search"))
            .header("User-Agent", UA)
            .query(&[
                ("name", q.to_string()),
                ("limit", n.to_string()),
                ("hidebroken", "true".to_string()),
            ])
            .send();
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        let (body, _truncated) = read_capped(resp, CAP_JSON);
        let v: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        let Some(arr) = v.as_array() else {
            last_err = "返回结构异常".to_string();
            continue;
        };
        let hits: Vec<StationHit> = arr
            .iter()
            .filter_map(|s| {
                let name = s["name"].as_str()?.trim().to_string();
                let url = s["url_resolved"]
                    .as_str()
                    .or_else(|| s["url"].as_str())?
                    .to_string();
                // 仅收 https:平台层禁明文,http 台根本播不了;顺带挡掉 file:// 等奇异 scheme
                if name.is_empty() || !url.starts_with("https://") {
                    return None;
                }
                let country = s["country"].as_str().unwrap_or("");
                let tags = s["tags"].as_str().unwrap_or("");
                let desc = if tags.is_empty() {
                    country.to_string()
                } else {
                    format!("{country} · {tags}")
                };
                Some(StationHit {
                    name,
                    url,
                    desc,
                    region: "搜索".to_string(),
                })
            })
            .take(n)
            .collect();
        if !hits.is_empty() {
            return Ok(hits);
        }
        last_err = "无结果".to_string();
    }
    Err(err(format!("电台搜索服务暂不可达({last_err}),稍后再试")))
}
