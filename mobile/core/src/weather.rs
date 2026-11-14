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

    let mut lines = vec![format!("🌤 {name} 实况(中国天气网·城市站)")];
    let weather = info["weather"].as_str().unwrap_or("");
    let wd = info["wd"].as_str().unwrap_or("");
    let ws = info["ws"].as_str().unwrap_or("");
    let mut now_line = String::new();
    if !weather.is_empty() {
        now_line.push_str(weather);
    }
    if !wd.is_empty() {
        now_line.push_str(&format!(",{wd}"));
        if !ws.is_empty() {
            now_line.push_str(&format!(" {ws}"));
        }
    }
    if let Some(t) = info["temp"].as_str() {
        if t != "999" && !t.is_empty() {
            now_line.push_str(&format!(",当前 {t}°C"));
        }
    }
    lines.push(now_line);
    if let Some(tn) = info["tempn"].as_str() {
        if !tn.is_empty() && tn != "999" {
            lines.push(format!("今夜低温 {tn}°C"));
        }
    }

    // 官方预警(气象台发布,站点级权威信息)
    if let Ok(alarm) = serde_json::from_str::<Value>(alarm_json) {
        if let Some(list) = alarm["w"].as_array() {
            let warns: Vec<String> = list
                .iter()
                .filter_map(|w| {
                    let area = w["w1"].as_str()?.trim();
                    let kind = w["w5"].as_str()?.trim();
                    let level = w["w7"].as_str()?.trim();
                    let time = w["w8"].as_str().unwrap_or("");
                    if kind.is_empty() {
                        return None;
                    }
                    let short_time = if time.len() >= 16 { &time[..16] } else { time };
                    Some(format!("⚠️ {area}·{kind}({level}) {short_time}发布"))
                })
                .collect();
            if !warns.is_empty() {
                lines.push(String::new());
                lines.extend(warns);
            }
        }
    } else {
        lines.push("无生效官方预警".to_string());
    }
    lines.push(String::new());
    lines.push("—— 数据:中国天气网(中央气象台),按城市站点发布".to_string());
    Ok(lines.join("\n"))
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

// ---------------- GFS 集合预报(30成员分歧度) ----------------

#[derive(Debug, Default)]
pub struct EnsembleStats {
    pub members: usize,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub precip_frac: f64,
}

pub fn ensemble_stats(core: &Assistant, lat: f64, lon: f64) -> Result<EnsembleStats, CoreError> {
    let resp = core
        .http
        .get(ENS_URL)
        .header("User-Agent", UA)
        .query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("hourly", "temperature_2m,precipitation".to_string()),
            ("models", "gfs_seamless".to_string()),
            ("forecast_hours", "1".to_string()),
            ("timezone", "auto".to_string()),
        ])
        .send()
        .map_err(|e| err(format!("集合预报请求失败:{e}")))?;
    let (body, _) = read_capped(resp, CAP_JSON);
    let v: Value = serde_json::from_str(&body).map_err(|e| err(format!("集合解析失败:{e}")))?;
    let temps = v["hourly"]["temperature_2m"].as_array().ok_or(err("集合数据异常"))?;
    let precs = v["hourly"]["precipitation"].as_array();

    let mut t: Vec<f64> = temps.iter().filter_map(|x| x.as_f64()).collect();
    if t.is_empty() {
        return Err(err("集合成员为空"));
    }
    t.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = t.len();
    let median = if n % 2 == 1 { t[n / 2] } else { (t[n / 2 - 1] + t[n / 2]) / 2.0 };
    let precip_frac = precs
        .map(|p| {
            let vals: Vec<f64> = p.iter().filter_map(|x| x.as_f64()).collect();
            if vals.is_empty() {
                0.0
            } else {
                let wet = vals.iter().filter(|&&x| x > 0.05).count();
                wet as f64 / vals.len() as f64
            }
        })
        .unwrap_or(0.0);
    Ok(EnsembleStats {
        members: n,
        median,
        min: t[0],
        max: t[n - 1],
        precip_frac,
    })
}

fn clothing_advice(temp: f64, precip_frac: f64) -> String {
    let mut tips = vec![];
    match temp {
        t if t < 0.0 => tips.push("严寒,羽绒服+保暖配饰"),
        t if t < 10.0 => tips.push("偏冷,厚外套"),
        t if t < 20.0 => tips.push("微凉,外套/薄毛衣"),
        t if t < 28.0 => tips.push("舒适,长袖即可"),
        _ => tips.push("偏热,短袖"),
    }
    if precip_frac > 0.4 {
        tips.push("降水概率偏高,建议带伞");
    } else if precip_frac > 0.15 {
        tips.push("降水可能性存在,可备伞");
    }
    tips.join(";")
}

// ---------------- 综合评价(定位) ----------------

pub fn comprehensive(core: &Assistant, lat: f64, lon: f64, place: &str) -> Result<String, CoreError> {
    // 两源并行
    let core_ref = &*core;
    let (met_res, ens_res) = std::thread::scope(|s| {
        let a = s.spawn(|| met_now(core_ref, lat, lon));
        let b = s.spawn(|| ensemble_stats(core_ref, lat, lon));
        (a.join(), b.join())
    });

    let mut lines = vec![format!("🌦 综合天气评价 · {place}")];
    let mut temp_for_advice: Option<f64> = None;
    let mut precip_frac = 0.0;
    let mut srcs = 0;

    if let Ok(Ok(m)) = met_res {
        srcs += 1;
        temp_for_advice = Some(m.temp);
        lines.push(format!(
            "🏛 挪威气象局(独立机构):{:.1}°C,湿度{:.0}%,{}风 {:.1}m/s{}",
            m.temp,
            m.humidity,
            wind_dir(m.wind_dir),
            m.wind_speed,
            m.precip_1h
                .map(|p| format!(",未来1h降水 {:.1}mm", p))
                .unwrap_or_default()
        ));
    }
    if let Ok(Ok(e)) = ens_res {
        srcs += 1;
        precip_frac = e.precip_frac;
        temp_for_advice.get_or_insert(e.median);
        let spread = e.max - e.min;
        let agree = if spread <= 1.0 {
            "高(各成员基本一致)"
        } else if spread <= 3.0 {
            "中(存在分歧)"
        } else {
            "低(分歧较大,预报不确定)"
        };
        lines.push(format!(
            "🌐 GFS集合预报({}个成员):中位 {:.1}°C,区间 {:.1}~{:.1}°C\n   一致性:{} | 降水成员占比 {:.0}%",
            e.members, e.median, e.min, e.max, agree, e.precip_frac * 100.0
        ));
    }
    if srcs == 0 {
        return Err(err("天气源暂时都连不上,稍后再试试~"));
    }
    if let Some(t) = temp_for_advice {
        lines.push(format!("🧭 综合:{:.1}°C左右;{}", t, clothing_advice(t, precip_frac)));
    }
    lines.push(String::new());
    lines.push(
        "—— 按坐标就地计算:MET挪威(独立机构)+ GFS集合30成员;非模糊城市级数据".to_string(),
    );
    Ok(lines.join("\n"))
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

/// 坐标实况(MET,简洁版)
pub fn met_basic(core: &Assistant, lat: f64, lon: f64, place: &str) -> Result<String, CoreError> {
    let m = met_now(core, lat, lon)?;
    Ok(format!(
        "📍 {} 实况(挪威气象局)\n{:.1}°C,湿度{:.0}%,{}风 {:.1}m/s",
        place,
        m.temp,
        m.humidity,
        wind_dir(m.wind_dir),
        m.wind_speed
    ))
}
