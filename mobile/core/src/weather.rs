//! 天气:国内优先(中国天气网·城市站实况+官方预警)+ 定位综合评价
//! (挪威气象局实况 + GFS集合30成员分歧度)+ Open-Meteo 兜底
//!
//! 数据源可达性(用户网络实测):
//! - d1.weather.com.cn(中国天气网)✅ 站点级实况 + 官方预警
//! - api.met.no ✅ / ensemble-api.open-meteo.com ✅ / geocoding-api.open-meteo.com ✅
//! - api.open-meteo.com(主域)✗ 被墙 → 仅作最后兜底

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::api::{err, Assistant, CoreError};
use crate::net::{read_capped, CAP_JSON, UA};

const GEO_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const CN_D1: &str = "https://d1.weather.com.cn/weather_index";
const MET_NO: &str = "https://api.met.no/weatherapi/locationforecast/2.0/compact";
const ENS_URL: &str = "https://ensemble-api.open-meteo.com/v1/ensemble";

pub fn wind_dir(deg: f64) -> &'static str {
    const DIRS: [&str; 8] = ["北", "东北", "东", "东南", "南", "西南", "西", "西北"];
    DIRS[((deg.rem_euclid(360.0)) / 45.0) as usize % 8]
}

pub fn default_city() -> &'static str {
    "北京"
}

/// 内置城市 → 中国天气网城市代码(站点级数据;不在表内的城市走坐标方案)
pub const CN_CITY_CODES: &[(&str, &str)] = &[
    ("北京", "101010100"), ("上海", "101020100"), ("天津", "101030100"),
    ("重庆", "101040100"), ("广州", "101280101"), ("深圳", "101280601"),
    ("杭州", "101210101"), ("南京", "101190101"), ("武汉", "101200101"),
    ("成都", "101270101"), ("西安", "101110101"), ("沈阳", "101070101"),
    ("大连", "101070201"), ("哈尔滨", "101050101"), ("长春", "101060101"),
    ("济南", "101120101"), ("青岛", "101120201"), ("郑州", "101180101"),
    ("合肥", "101220101"), ("福州", "101230101"), ("厦门", "101230201"),
    ("长沙", "101250101"), ("南宁", "101300101"), ("海口", "101310101"),
    ("三亚", "101310201"), ("石家庄", "101090101"), ("太原", "101100101"),
    ("呼和浩特", "101080101"), ("银川", "101170101"), ("西宁", "101150101"),
    ("兰州", "101160101"), ("乌鲁木齐", "101130101"), ("拉萨", "101140101"),
    ("昆明", "101290101"), ("贵阳", "101260101"), ("香港", "101320101"),
    ("澳门", "101330101"), ("台北", "101340101"),
];

pub fn cn_code(city: &str) -> Option<(&'static str, &'static str)> {
    CN_CITY_CODES
        .iter()
        .find(|(name, _)| *name == city)
        .map(|(n, c)| (*n, *c))
}

/// 城市 → 坐标(geocoding-api ✅ 用户网络可达;结果持久缓存)
pub(crate) fn geocode_raw(core: &Assistant, city: &str) -> Result<(f64, f64, String, String), CoreError> {
    let cache_key = format!("geo:{city}");
    if let Some(hit) = core.store.lock().map_err(|_| err("存储忙"))?.kv_get(&cache_key) {
        if let Ok(v) = serde_json::from_str::<Value>(&hit) {
            return Ok((
                v["lat"].as_f64().unwrap_or(0.0),
                v["lon"].as_f64().unwrap_or(0.0),
                v["name"].as_str().unwrap_or(city).to_string(),
                v["admin"].as_str().unwrap_or("").to_string(),
            ));
        }
    }
    let resp = core
        .http
        .get(GEO_URL)
        .header("User-Agent", UA)
        .query(&[("name", city), ("count", "1"), ("language", "zh"), ("format", "json")])
        .send()
        .map_err(|e| err(format!("地理编码请求失败:{e}")))?;
    let (body, _) = read_capped(resp, CAP_JSON);
    let v: Value = serde_json::from_str(&body).map_err(|e| err(format!("地理编码解析失败:{e}")))?;
    let r = v["results"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| err(format!("⚠️ 没找到城市「{city}」")))?;
    let info = serde_json::json!({
        "lat": r["latitude"].as_f64().unwrap_or(0.0),
        "lon": r["longitude"].as_f64().unwrap_or(0.0),
        "name": r["name"].as_str().unwrap_or(city),
        "admin": r["admin1"].as_str().unwrap_or(""),
    });
    core.store.lock().map_err(|_| err("存储忙"))?.kv_set(&cache_key, &info.to_string());
    Ok((
        info["lat"].as_f64().unwrap(),
        info["lon"].as_f64().unwrap(),
        info["name"].as_str().unwrap_or(city).to_string(),
        info["admin"].as_str().unwrap_or("").to_string(),
    ))
}

/// 定位综合评价入口(坐标来自手机定位)
/// 成功 → 缓存最近一份;失败(离线)→ 回退缓存并标注保存时间
pub fn here(core: &Assistant, lat: f64, lon: f64) -> Result<String, CoreError> {
    let r = comprehensive(core, lat, lon, "我当前位置(自动定位)");
    crate::cache::cached_text(core, "cache:weather", r, "天气")
}

// ---------------- 中国天气网(国内主源) ----------------

static RE_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)var\s+(\w+)\s*=\s*(\{.*?\});").unwrap());

/// 城市实况 + 官方预警(站点级)
pub fn cn_city(core: &Assistant, code: &str, name: &str) -> Result<String, CoreError> {
    let url = format!("{CN_D1}/{code}.html");
    let resp = core
        .http
        .get(&url)
        .header("User-Agent", UA)
        .header("Referer", "https://www.weather.com.cn/")
        .send()
        .map_err(|e| err(format!("中国天气网请求失败:{e}")))?;
    // 限量读取:读取失败返回空串,后续 city_json 为空走「未返回数据」错误文案
    let (body, _truncated) = read_capped(resp, CAP_JSON);

    // 提取 var cityDZ = {...}; var alarmDZ = {...};
    let mut city_json = "";
    let mut alarm_json = "";
    for cap in RE_VAR.captures_iter(&body) {
        match &cap[1] {
            "cityDZ" => city_json = cap.get(2).unwrap().as_str(),
            "alarmDZ" => alarm_json = cap.get(2).unwrap().as_str(),
            _ => {}
        }
    }
    if city_json.is_empty() {
        return Err(err("中国天气网未返回该城市数据(暂只收录主要城市,其他城市可尝试定位天气)"));
    }
    let dz: Value = serde_json::from_str(city_json).map_err(|e| err(format!("实况解析失败:{e}")))?;
    let info = &dz["weatherinfo"];
    let weather = info["weather"].as_str().unwrap_or("").trim().to_string();
    let wd = info["wd"].as_str().unwrap_or("").trim().to_string();
    let ws = info["ws"].as_str().unwrap_or("").trim().to_string();
    let temp_raw = info["temp"].as_str().unwrap_or("").trim().to_string();
    let tempn_raw = info["tempn"].as_str().unwrap_or("").trim().to_string();
    let temp_v: Option<f64> = if temp_raw != "999" && !temp_raw.is_empty() {
        temp_raw.parse().ok()
    } else {
        None
    };
    let tonight = if !tempn_raw.is_empty() && tempn_raw != "999" {
        Some(format!("今夜最低{tempn_raw}°C"))
    } else {
        None
    };

    // 现在的描述:天气 + 气温 + 风向风力(ws 数字则补「级」)
    let mut now_desc = String::new();
    if !weather.is_empty() {
        now_desc.push_str(&weather);
    }
    if let Some(t) = temp_v {
        now_desc.push_str(&format!("{:.0}°C", t));
    }
    if !wd.is_empty() {
        match ws.parse::<f64>() {
            Ok(v) => now_desc.push_str(&format!(",{wd}风{v:.0}级")),
            _ => {
                if !ws.is_empty() {
                    now_desc.push_str(&format!(",{wd}风{ws}"))
                }
            }
        }
    }
    let mut now_desc_opt = if now_desc.is_empty() { None } else { Some(now_desc) };

    // 湿度 + 未来24小时:城市名→坐标→MET/GFS集合(并行;失败则省略对应句)
    let mut humidity = None;
    let mut f24 = None;
    if let Ok(g) = geocode(core, name) {
        let (met_res, ens_res) = std::thread::scope(|s| {
            let a = s.spawn(|| met_now(core, g.lat, g.lon).ok());
            let b = s.spawn(|| ensemble_24h(core, g.lat, g.lon).ok());
            (a.join().unwrap_or(None), b.join().unwrap_or(None))
        });
        // 实况温度缺失时用 MET 温度补
        if let Some(m) = &met_res {
            if temp_v.is_none() {
                if let Some(d) = now_desc_opt.as_mut() {
                    *d = format!("{d},{:.0}°C", m.temp);
                }
            }
            humidity = Some(m.humidity as u32);
        }
        f24 = ens_res;
    }

    // 官方预警(气象台发布,站点级权威信息)
    let mut warns: Vec<String> = Vec::new();
    if let Ok(alarm) = serde_json::from_str::<Value>(alarm_json) {
        if let Some(list) = alarm["w"].as_array() {
            for w in list {
                let Some(area) = w["w1"].as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                    continue;
                };
                let Some(kind) = w["w5"].as_str().map(str::trim).filter(|s| !s.is_empty()) else {
                    continue;
                };
                let level = w["w7"].as_str().unwrap_or("").trim();
                let time = w["w8"].as_str().unwrap_or("");
                let short_time = if time.len() >= 16 { &time[..16] } else { time };
                warns.push(format!("⚠️ {area}·{kind}({level}) {short_time}发布"));
            }
        }
    }

    Ok(broadcast_text(
        name,
        now_desc_opt.as_deref(),
        humidity,
        tonight.as_deref(),
        f24,
        &warns,
        "数据:中国天气网(中央气象台)·挪威气象局·GFS集合预报",
    ))
}

// ---------------- MET 挪威(定位实况,独立机构) ----------------

#[derive(Debug, Default)]
pub struct MetNow {
    pub temp: f64,
    pub wind_speed: f64,
    pub wind_dir: f64,
    pub humidity: f64,
    pub precip_1h: Option<f64>,
}

pub fn met_now(core: &Assistant, lat: f64, lon: f64) -> Result<MetNow, CoreError> {
    let resp = core
        .http
        .get(MET_NO)
        .header("User-Agent", UA)
        .query(&[("lat", lat.to_string()), ("lon", lon.to_string())])
        .send()
        .map_err(|e| err(format!("挪威气象局请求失败:{e}")))?;
    let (body, _) = read_capped(resp, CAP_JSON);
    let v: Value = serde_json::from_str(&body).map_err(|e| err(format!("MET解析失败:{e}")))?;
    let ts = &v["properties"]["timeseries"];
    let first = ts
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| err("MET无数据"))?;
    let inst = &first["data"]["instant"]["details"];
    let out = MetNow {
        temp: inst["air_temperature"].as_f64().unwrap_or(0.0),
        wind_speed: inst["wind_speed"].as_f64().unwrap_or(0.0),
        wind_dir: inst["wind_from_direction"].as_f64().unwrap_or(0.0),
        humidity: inst["relative_humidity"].as_f64().unwrap_or(0.0),
        precip_1h: first["data"]["next_1_hours"]["details"]["precipitation_amount"]
            .as_f64(),
    };
    Ok(out)
}

// ---------------- GFS 集合预报(未来24小时) ----------------

/// 未来24小时变化(集合预报):最高/最低、前后半程中位温差(趋势)、
/// 逐小时聚合点(集合成员中位温度+湿占比)
#[derive(Debug, Default, Clone)]
pub struct HourStat {
    pub median_temp: f64,
    pub wet_frac: f64,
    /// 当地时间「时」标签(如「14」;解析失败为空)
    pub hour_label: String,
}

/// 逐小时降水成员占比(雨窗判断用)
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Forecast24 {
    /// 各小时的当地时间「时」标签(用于雨窗播报)
    pub hours: Vec<String>,
    pub hi: f64,
    pub lo: f64,
    /// 后半程中位温度 - 前半程中位温度(正=升温)
    pub trend: f64,
    pub precip_frac: f64,
    /// 每小时降水成员占比(0.0-1.0;index 0 = 未来第1小时)
    pub wet_by_hour: Vec<f64>,
}

fn median_of(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

pub fn ensemble_24h(core: &Assistant, lat: f64, lon: f64) -> Result<Forecast24, CoreError> {
    let resp = core
        .http
        .get(ENS_URL)
        .header("User-Agent", UA)
        .query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("hourly", "temperature_2m,precipitation".to_string()),
            ("models", "gfs_seamless".to_string()),
            ("forecast_hours", "24".to_string()),
            ("timezone", "auto".to_string()),
        ])
        .send()
        .map_err(|e| err(format!("集合预报请求失败:{e}")))?;
    let (body, _) = read_capped(resp, CAP_JSON);
    let v: Value = serde_json::from_str(&body).map_err(|e| err(format!("集合解析失败:{e}")))?;
    let temps = v["hourly"]["temperature_2m"].as_array().ok_or(err("集合数据异常"))?;
    let precs = v["hourly"]["precipitation"].as_array();

    // 逐小时聚合:集合成员按小时分块(30成员/小时),取中位温度与湿占比
    let times = v["hourly"]["time"]
        .as_array()
        .and_then(|a| {
            let items: Vec<String> = a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            (!items.is_empty()).then_some(items)
        })
        .unwrap_or_default();
    let n_hours = if times.is_empty() { 24 } else { times.len() };
    if temps.len() < n_hours || n_hours == 0 {
        return Err(err("集合成员为空"));
    }
    let per = temps.len() / n_hours; // 每小时成员数
    let temp_at = |h: usize| -> Vec<f64> {
        temps[h * per..(h + 1) * per]
            .iter()
            .filter_map(|x| x.as_f64())
            .collect()
    };
    let wet_frac_at = |h: usize| -> f64 {
        match precs {
            Some(p) if p.len() >= (h + 1) * per => {
                let vals: Vec<f64> = p[h * per..(h + 1) * per].iter().filter_map(|x| x.as_f64()).collect();
                if vals.is_empty() {
                    0.0
                } else {
                    vals.iter().filter(|&&x| x > 0.1).count() as f64 / vals.len() as f64
                }
            }
            _ => 0.0,
        }
    };

    let points: Vec<HourStat> = (0..n_hours)
        .map(|h| HourStat {
            median_temp: median_of(temp_at(h)),
            wet_frac: wet_frac_at(h),
            hour_label: if h < times.len() {
                times[h].get(11..13).unwrap_or("").to_string()
            } else {
                String::new()
            },
        })
        .collect();

    let t: Vec<f64> = points.iter().map(|p| p.median_temp).collect();
    let half = t.len() / 2;
    let trend = median_of(t[half..].to_vec()) - median_of(t[..half].to_vec());
    let precip_frac = points.iter().map(|p| p.wet_frac).sum::<f64>() / points.len() as f64;
    Ok(Forecast24 {
        hi: t.iter().cloned().fold(f64::MIN, f64::max),
        lo: t.iter().cloned().fold(f64::MAX, f64::min),
        trend,
        precip_frac,
        wet_by_hour: points.iter().map(|p| p.wet_frac).collect(),
        hours: points.iter().map(|p| p.hour_label.clone()).collect(),
    })
}

/// 风速 m/s → 蒲福风级
pub fn wind_level(ms: f64) -> u32 {
    const THRESH: [(f64, u32); 13] = [
        (0.2, 0), (1.5, 1), (3.3, 2), (5.4, 3), (7.9, 4), (10.7, 5), (13.8, 6),
        (17.1, 7), (20.7, 8), (24.4, 9), (28.4, 10), (32.6, 11), (f64::MAX, 12),
    ];
    for (lim, lv) in THRESH {
        if ms <= lim {
            return lv;
        }
    }
    12
}

/// 播报式文本组装(所有天气出口共用;纯函数,可单测)
pub fn broadcast_text(
    place: &str,
    now_desc: Option<&str>,
    humidity: Option<u32>,
    tonight: Option<&str>,
    f24: Option<Forecast24>,
    warns: &[String],
    source: &str,
) -> String {
    let mut lines = vec![format!("🌤 {place}天气播报:")];
    let mut now_parts: Vec<String> = Vec::new();
    if let Some(d) = now_desc {
        if !d.is_empty() {
            now_parts.push(d.to_string());
        }
    }
    if let Some(h) = humidity {
        if h > 0 {
            now_parts.push(format!("湿度{h}%"));
        }
    }
    if !now_parts.is_empty() {
        lines.push(format!("现在{}。", now_parts.join("、")));
    }
    if let Some(t) = tonight {
        if !t.is_empty() {
            lines.push(format!("{t}。"));
        }
    }
    if let Some(f) = f24 {
        let trend_txt = if f.trend >= 0.8 {
            format!("气温上升约{:.0}°C", f.trend)
        } else if f.trend <= -0.8 {
            format!("气温下降约{:.0}°C", -f.trend)
        } else {
            "气温变化不大".to_string()
        };
        // 雨窗时段化:连续≥2小时降水成员≥40% 视为一个降雨时段
        let mut wins: Vec<String> = Vec::new();
        let mut i = 0;
        while i < f.wet_by_hour.len() {
            if f.wet_by_hour[i] >= 0.4 {
                let s = i;
                while i < f.wet_by_hour.len() && f.wet_by_hour[i] >= 0.4 {
                    i += 1;
                }
                if i - s >= 2 {
                    let s_lab = f.hours.get(s).cloned().unwrap_or_default();
                    let e_lab = f.hours.get(i - 1).cloned().unwrap_or_default();
                    let maxf = f.wet_by_hour[s..i].iter().cloned().fold(0.0, f64::max);
                    wins.push(format!("{}时-{}时(成员约{:.0}%)", s_lab, e_lab, maxf * 100.0));
                }
            } else {
                i += 1;
            }
        }
        let rain = if wins.is_empty() {
            "未来24小时各时段降水成员占比低,以晴到多云为主".to_string()
        } else {
            format!("降水时段:{};其余时段降水少", wins.join(";"))
        };
        lines.push(format!(
            "未来24小时:最高{:.1}°C、最低{:.1}°C,{};{}。",
            f.hi, f.lo, trend_txt, rain
        ));
    }
    lines.extend(warns.iter().cloned());
    lines.push(String::new());
    lines.push(format!("—— {source}"));
    lines.join("\n")
}

// ---------------- 综合评价(定位) ----------------

pub fn comprehensive(core: &Assistant, lat: f64, lon: f64, place: &str) -> Result<String, CoreError> {
    // 两源并行
    let core_ref = &*core;
    let (met_res, ens_res) = std::thread::scope(|s| {
        let a = s.spawn(|| met_now(core_ref, lat, lon).ok());
        let b = s.spawn(|| ensemble_24h(core_ref, lat, lon).ok());
        (a.join().unwrap_or(None), b.join().unwrap_or(None))
    });
    if met_res.is_none() && ens_res.is_none() {
        return Err(err("天气源暂时都连不上,稍后再试试~"));
    }
    let now_desc = met_res
        .as_ref()
        .map(|m| format!("{:.1}°C,{}风{}级", m.temp, wind_dir(m.wind_dir), wind_level(m.wind_speed)));
    let humidity = met_res.as_ref().map(|m| m.humidity as u32);
    Ok(broadcast_text(
        place,
        now_desc.as_deref(),
        humidity,
        None,
        ens_res,
        &[],
        "数据:挪威气象局·GFS集合预报(按坐标就地计算)",
    ))
}

// ---------------- 城市天气(兼容旧入口) ----------------

/// 城市天气(兼容旧入口)
/// 成功 → 缓存最近一份;失败(离线)→ 回退缓存并标注保存时间
pub fn get_weather(core: &Assistant, city: &str) -> Result<String, CoreError> {
    let r = get_weather_inner(core, city);
    crate::cache::cached_text(core, "cache:weather", r, "天气")
}

fn get_weather_inner(core: &Assistant, city: &str) -> Result<String, CoreError> {
    let city = if city.trim().is_empty() {
        core.store
            .lock()
            .map_err(|_| err("存储忙"))?
            .kv_get("default_city")
            .unwrap_or_else(|| default_city().to_string())
    } else {
        city.trim().to_string()
    };
    // 国内主源
    if let Some((name, code)) = cn_code(&city) {
        return cn_city(core, code, name);
    }
    // 非内置城市:geocoding(✅可用)→ MET 坐标实况
    let loc = geocode(core, &city)?;
    met_basic(core, loc.lat, loc.lon, &loc.label)
}

pub struct Geo {
    pub lat: f64,
    pub lon: f64,
    pub label: String,
}

pub fn geocode(core: &Assistant, city: &str) -> Result<Geo, CoreError> {
    let v = crate::weather::geocode_raw(core, city)?;
    Ok(Geo {
        lat: v.0,
        lon: v.1,
        label: v.2,
    })
}

/// 坐标实况(MET,播报式)
pub fn met_basic(core: &Assistant, lat: f64, lon: f64, place: &str) -> Result<String, CoreError> {
    let core_ref = &*core;
    let (met_res, ens_res) = std::thread::scope(|s| {
        let a = s.spawn(|| met_now(core_ref, lat, lon));
        let b = s.spawn(|| ensemble_24h(core_ref, lat, lon).ok());
        (a.join().unwrap_or(Err(err("MET无数据"))), b.join().unwrap_or(None))
    });
    let m = met_res?;
    let now_desc = format!(
        "{:.1}°C,{}风{}级",
        m.temp,
        wind_dir(m.wind_dir),
        wind_level(m.wind_speed)
    );
    Ok(broadcast_text(
        place,
        Some(&now_desc),
        Some(m.humidity as u32),
        None,
        ens_res,
        &[],
        "数据:挪威气象局·GFS集合预报",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_text_shapes_sentences() {
        let f = Forecast24 {
            hi: 29.0,
            lo: 21.0,
            trend: 2.5,
            precip_frac: 0.35,
            hours: vec!["14".to_string(), "15".to_string(), "16".to_string()],
            wet_by_hour: vec![0.1, 0.6, 0.7],
        };
        let out = broadcast_text(
            "北京",
            Some("晴,26°C,南风2级"),
            Some(65),
            Some("今夜最低22°C"),
            Some(f),
            &["⚠️ 北京·暴雨(蓝色) 07-14 08:00发布".to_string()],
            "数据:测试",
        );
        assert!(out.contains("北京天气播报:"));
        assert!(out.contains("湿度65%"));
        assert!(out.contains("今夜最低22°C。"));
        assert!(out.contains("未来24小时:最高29.0°C、最低21.0°C"));
        assert!(out.contains("气温上升约2°C"));
        // 时段化雨窗:连续两小时湿占比≥0.4 → 「15时-16时」,最大占比 70%
        assert!(out.contains("降水时段:15时-16时(成员约70%)"), "实际: {out}");
        assert!(out.contains("⚠️ 北京·暴雨"));
        assert!(out.ends_with("—— 数据:测试"));
    }

    #[test]
    fn broadcast_text_tolerates_missing_sources() {
        // MET/集合/预警全缺:只有地名+来源,不应出现空句
        let out = broadcast_text("某地", None, None, None, None, &[], "数据:测试");
        assert!(out.contains("某地天气播报:"));
        assert!(!out.contains("现在"));
        assert!(!out.contains("未来24小时"));
    }

    #[test]
    fn wind_level_matches_beaufort() {
        assert_eq!(wind_level(0.1), 0);
        assert_eq!(wind_level(2.0), 2);
        assert_eq!(wind_level(6.0), 4);
        assert_eq!(wind_level(14.0), 7);
        assert_eq!(wind_level(40.0), 12);
    }
}
