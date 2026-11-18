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
    // 二次元音乐(2026-11-15 radio-browser 实测可达;Gensokyo=东方Project,J1GOLD/Sakura=动画歌)
    Station {
        name: "二次元·Gensokyo",
        url: "https://stream.gensokyoradio.net/3/",
        desc: "东方Project同人·JP 256k",
        region: "二次元",
    },
    Station {
        name: "二次元·J1 GOLD",
        url: "https://jenny.torontocast.com:2000/stream/J1GOLD",
        desc: "J-Pop动画歌·CA 128k",
        region: "二次元",
    },
    Station {
        name: "二次元·SBS PopAsia",
        url: "https://sbs-ice.streamguys1.com/sbs-popasia-sbs-web",
        desc: "泛亚流行·AU 97k",
        region: "二次元",
    },
    Station {
        name: "二次元·J-Pop Sakura",
        url: "https://quincy.torontocast.com:2070/stream.mp3",
        desc: "亚洲DREAM动画歌·JP 128k",
        region: "二次元",
    },
    Station {
        name: "Anison动漫音乐台",
        url: "http://pool.anison.fm:9000/AniSonFM%28320%29?",
        desc: "中文动漫歌·CN 320k",
        region: "二次元",
    },
];

/// 拥有所有权的台站(内置 + 用户自定义)
pub struct StationOwned {
    pub name: String,
    pub url: String,
    pub desc: String,
}

/// 全部台站 = 内置精选 + 用户自定义(kv radio_user_stations,一行「名称|地址」,≤20个)
pub fn all_stations(core: &Assistant) -> Vec<StationOwned> {
    let mut v: Vec<StationOwned> = STATIONS
        .iter()
        .map(|s| StationOwned {
            name: s.name.to_string(),
            url: s.url.to_string(),
            desc: s.desc.to_string(),
        })
        .collect();
    for st in crate::sources::load_stations(core) {
        if !st.enabled {
            continue;
        }
        if v.iter().any(|x| x.url == st.url) || v.len() >= 20 {
            continue;
        }
        v.push(StationOwned {
            name: st.name,
            url: st.url,
            desc: if st.ok.unwrap_or(false) { "自定义·已验证".to_string() } else { "自定义".to_string() },
        });
    }
    v
}

pub fn stations_text(core: &Assistant) -> String {
    let mut lines = vec!["📻 电台(快捷词条卡片可点 ▶ 播放)".to_string()];
    for (i, s) in all_stations(core).into_iter().enumerate() {
        lines.push(format!("{}. {} —— {}({})", i + 1, s.name, s.desc, "内置".to_string()));
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
