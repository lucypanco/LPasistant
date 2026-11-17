//! 受控本地 LLM 兜底(可选,默认关闭)—— v3:分割+语义数字转换+依次确认
//!
//! 分层(特权/隔离):
//!   模型(隔离层):①语句分割(事项/时间短语,原样);②时间短语→数字规格
//!     (days/hour/minute/recur)。prompt 内含「今天日期+对照表」,模型做查表
//!     而非算术;输出数字由规则层校验(越界即无效)。
//!   规则(特权层):时间短语先过确定性解析(词表命中→规则赢);未命中才用
//!     模型数字换算;缺槽依次追问(时间→≤2轮;确认键落库)。
//!
//! 真机日志依据(2026-11-15):模型分段/路由可靠;结构化字段输出会复读枚举
//! 与幻觉日期 → 用「prompt 对照表+数字输出+规则校验」三级兜住。

use serde_json::Value;
use chrono::{Datelike, Duration, NaiveDateTime, Timelike, Weekday};

use crate::api::Assistant;
use crate::intent::{recur_label, USAGE};
use crate::timeparse::{human_dt, parse_when};

/// 待用户「确认」的提醒草稿(确认键/表单消费;存于 Assistant.pending)
#[derive(Debug, Clone)]
pub struct PendingReminder {
    pub text: String,
    pub when: NaiveDateTime,
    pub recur: Option<String>,
}

/// 追问状态:事项/时间槽依次补齐
#[derive(Debug, Clone)]
pub struct ClarifyState {
    /// 路由:"reminder"(payload=事项) | "weather"(payload 未用)
    pub route: String,
    /// 已确认的槽内容(事项)
    pub payload: String,
    /// 已追问轮数(上限2轮)
    pub round: u32,
    /// 已确定的相对天数(模型转换结果,规则校验后)
    pub days: Option<u32>,
    /// 已确定的循环(daily/weekly:X/monthly:X;空=单次)
    pub recur: String,
}

/// 疑问句预闸门(纯规则,零推理):拦住"用提醒句式问问题"的干扰输入。
pub(crate) fn looks_like_question(t: &str) -> bool {
    let t = t.trim();
    if t.ends_with('?') || t.ends_with('?') {
        return true;
    }
    const MARKERS: [&str; 10] = [
        "什么", "怎么", "为什么", "为啥", "哪些", "哪个", "谁", "多少", "请问", "介绍一下",
    ];
    MARKERS.iter().any(|m| t.contains(m))
}

// ---------- prompt(模型:分割 + 查表转数字) ----------

fn today_line() -> String {
    let now = chrono::Local::now();
    let wd = match now.weekday() {
        Weekday::Mon => "周一",
        Weekday::Tue => "周二",
        Weekday::Wed => "周三",
        Weekday::Thu => "周四",
        Weekday::Fri => "周五",
        Weekday::Sat => "周六",
        Weekday::Sun => "周日",
    };
    format!(
        "{}({wd}) {:02}:{:02}",
        now.format("%Y-%m-%d"),
        now.hour(),
        now.minute()
    )
}

/// 对照表段(两个 prompt 共用;模型按表查数,不做算术)
const TIME_TABLE: &str = "对照表:今天=0天,明天=1天,后天=2天,大后天=3天;每天 recur=daily;每周X recur=weekly:X(X:周一=0,周二=1,…,周日=6);每月X号 recur=monthly:X。";

pub fn build_segment_prompt(utterance: &str) -> String {
    format!(
        "你是语句分割器。今天是{}。把句子分成「事项」和「时间」,输出一行JSON,只输出JSON本身:\n\
         {{\"event\":\"把报告写完\",\"time\":\"下周末前\",\"days\":7,\"hour\":9,\"minute\":0,\"recur\":\"\"}}\n\
         {{\"event\":\"开会\",\"time\":\"大后天下午三点\",\"days\":3,\"hour\":15,\"minute\":0,\"recur\":\"\"}}\n\
         {{\"event\":\"交房租\",\"time\":\"每月15号上午十点\",\"days\":15,\"hour\":10,\"minute\":0,\"recur\":\"monthly:15\"}}\n\
         {{\"event\":\"部门例会\",\"time\":\"\",\"days\":null,\"hour\":null,\"minute\":null,\"recur\":\"\"}}\n\
         规则:event=要做什么(时间词移入 time);time=时间短语原词;days/hour/minute 按对照表转成数字;没有的时刻填 null;每天用 recur=daily;句子没有时间就 days/hour/minute/recur 全空。\n\
        {TIME_TABLE}\n\
         句子:{utterance}\nJSON:",
        today_line()
    )
}

pub fn build_wide_prompt(utterance: &str) -> String {
    format!(
        "你是个人助手路由器。今天是{}。{}判断这句话用哪个功能,输出一行JSON,只输出JSON本身:\n\
         {{\"route\":\"reminder\",\"event\":\"把报告写完\",\"time\":\"下周末前\",\"days\":7,\"hour\":9,\"minute\":0,\"recur\":\"\"}}\n\
         {{\"route\":\"weather\",\"city\":\"北京\"}}\n\
         {{\"route\":\"news\",\"kind\":\"all\"}}\n\
         {{\"route\":\"poetry\",\"query\":\"励志\"}}\n\
         {{\"route\":\"radio\",\"query\":\"爵士\"}}\n\
         {{\"route\":\"none\"}}\n\
         规则:\n\
         - reminder:event=要做什么(去掉时间词);time=时间短语原词;days/hour/minute/recur 按对照表转成数字;没有的时刻填 null;没有时间全空;\n\
         - weather/news/poetry/radio:query 填关键词;\n\
         - 与以上功能都无关 → {{\"route\":\"none\"}};\n\
         句子:{utterance}\nJSON:",
        today_line(),
        TIME_TABLE
    )
}

/// 追问时刻的第二阶段(模型转换用户补充的时间;规则校验)
pub fn build_reply_spec_prompt(reply: &str) -> String {
    format!(
        "今天是{}。把用户补充的提醒时间转成一行JSON,只输出JSON本身:\n\
         {{\"days\":1,\"hour\":15,\"minute\":0,\"recur\":\"\"}}\n\
         例:「大后天的下午三点」→ {{\"days\":3,\"hour\":15,\"minute\":0,\"recur\":\"\"}};「每天早上八点」→ {{\"days\":0,\"hour\":8,\"minute\":0,\"recur\":\"daily\"}}。\n\
         没把握就全部填 null。\n\
         用户补充:{reply}\nJSON:",
        today_line()
    )
}

// ---------- 解析(宽容:字段缺失/类型错都容忍) ----------

#[derive(Debug, Default, Clone)]
pub struct Segmentation {
    pub event: String,
    pub time: String,
    pub days: Option<u32>,
    pub hour: Option<u32>,
    pub minute: Option<u32>,
    pub recur: String,
}

#[derive(Debug, Default, Clone)]
pub struct TimeSpec {
    pub days: Option<u32>,
    pub hour: Option<u32>,
    pub minute: Option<u32>,
    pub recur: String,
}

fn json_u32(v: &Value, key: &str) -> Option<u32> {
    match &v[key] {
        Value::Number(n) => n.as_u64().map(|x| x as u32),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// 分割输出解析(容忍围栏与废话)
pub fn parse_segment(out: &str) -> Option<Segmentation> {
    let start = out.find('{')?;
    let end = out.rfind('}')?;
    if end < start {
        return None;
    }
    let v: Value = serde_json::from_str(&out[start..=end]).ok()?;
    Some(Segmentation {
        event: v["event"].as_str().unwrap_or("").trim().to_string(),
        time: v["time"].as_str().unwrap_or("").trim().to_string(),
        days: json_u32(&v, "days"),
        hour: json_u32(&v, "hour"),
        minute: json_u32(&v, "minute"),
        recur: v["recur"].as_str().unwrap_or("").trim().to_string(),
    })
}

/// 解析宽兜底路由输出
pub fn parse_wide(out: &str) -> Result<WideRoute, String> {
    let start = out.find('{').ok_or("输出里没有JSON")?;
    let end = out.rfind('}').ok_or("输出里没有JSON")?;
    if end < start {
        return Err("输出里没有JSON".into());
    }
    let v: Value = serde_json::from_str(&out[start..=end]).map_err(|e| format!("JSON非法:{e}"))?;
    let route = v["route"].as_str().ok_or("缺少route")?;
    let field = |k: &str| -> String { clip_field(v[k].as_str().unwrap_or(""), 40) };
    let num = |k: &str| json_u32(&v, k);
    Ok(match route {
        "reminder" => WideRoute::Reminder {
            event: {
                let e = field("event");
                if e.is_empty() { field("text") } else { e }
            },
            time: field("time"),
            days: num("days"),
            hour: num("hour"),
            minute: num("minute"),
            recur: field("recur"),
        },
        "weather" => WideRoute::Weather { city: field("city") },
        "news" => {
            let k = field("kind");
            WideRoute::News {
                kind: if matches!(k.as_str(), "intl" | "tech" | "politics") { k } else { "all".to_string() },
            }
        }
        "poetry" => WideRoute::Poetry { query: field("query") },
        "radio" => WideRoute::Radio { query: field("query") },
        "none" => WideRoute::None,
        other => return Err(format!("未知route:{other}")),
    })
}

/// 追问第二阶段的数字规格解析(容忍围栏与废话)
pub fn parse_reply_spec(out: &str) -> Option<TimeSpec> {
    let start = out.find('{')?;
    let end = out.rfind('}')?;
    if end < start {
        return None;
    }
    let v: Value = serde_json::from_str(&out[start..=end]).ok()?;
    Some(TimeSpec {
        days: json_u32(&v, "days"),
        hour: json_u32(&v, "hour"),
        minute: json_u32(&v, "minute"),
        recur: v["recur"].as_str().unwrap_or("").trim().to_string(),
    })
}

#[derive(Debug, Clone)]
pub enum WideRoute {
    Reminder {
        event: String,
        time: String,
        days: Option<u32>,
        hour: Option<u32>,
        minute: Option<u32>,
        recur: String,
    },
    Weather { city: String },
    News { kind: String },
    Poetry { query: String },
    Radio { query: String },
    None,
}

fn clip_field(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() > n {
        t.chars().take(n).collect()
    } else {
        t.to_string()
    }
}

// ---------- 规则层:校验与换算 ----------

fn valid_recur(r: &str) -> bool {
    r.is_empty()
        || r == "daily"
        || (r.starts_with("weekly:") && r[7..].parse::<u32>().map(|x| x <= 6).unwrap_or(false))
        || (r.starts_with("monthly:") && r[8..].parse::<u32>().map(|x| (1..=31).contains(&x)).unwrap_or(false))
}

/// 数字规格 → 具体时刻(纯算术;全部校验;返回 (时刻, recur))
pub fn spec_to_when(
    days: u32,
    hour: u32,
    minute: u32,
    recur: &str,
    now: NaiveDateTime,
) -> Option<(NaiveDateTime, Option<String>)> {
    if days > 60 || hour > 23 || minute > 59 || !valid_recur(recur) {
        return None;
    }
    let at = |base: chrono::NaiveDate| base.and_hms_opt(hour, minute, 0);
    match recur {
        "" => {
            let mut dt = at(now.date() + Duration::days(days as i64))?;
            if dt <= now {
                dt += Duration::days(1); // 已过 → 顺延一天(单次提醒不落过去)
            }
            Some((dt, None))
        }
        "daily" => {
            let mut dt = at(now.date())?;
            if dt <= now {
                dt += Duration::days(1);
            }
            Some((dt, Some("daily".into())))
        }
        r if r.starts_with("weekly:") => {
            let wd: u32 = r[7..].parse().ok()?;
            let target = now.date()
                + Duration::days((wd as i64 - now.weekday().num_days_from_monday() as i64).rem_euclid(7));
            let mut dt = at(target)?;
            if dt <= now {
                dt += Duration::days(7);
            }
            Some((dt, Some(format!("weekly:{wd}"))))
        }
        r if r.starts_with("monthly:") => {
            let day: u32 = r[8..].parse().ok()?;
            if !(1..=31).contains(&day) {
                return None;
            }
            let mut y = now.year();
            let mut m = now.month();
            loop {
                if let Some(d) = chrono::NaiveDate::from_ymd_opt(y, m, day) {
                    if let Some(dt) = d.and_hms_opt(hour, minute, 0) {
                        if dt > now {
                            return Some((dt, Some(format!("monthly:{day}"))));
                        }
                    }
                }
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
                if (y, m) > (now.year() + 2, now.month()) {
                    return None;
                }
            }
        }
        _ => None,
    }
}

// ---------- 草稿与追问 ----------

fn draft_reply(text: &str, when: &NaiveDateTime, recur: Option<&str>) -> String {
    format!(
        "🧩 我理解你想设置:\n⏰ {} {}\n📝 {}\n\n下方按键:确认生效 / 取消放弃 / 查看修改表单",
        human_dt(when),
        recur_label(recur),
        text
    )
}

pub(crate) fn store_clarify_reminder(
    core: &Assistant,
    event: &str,
    days: Option<u32>,
    recur: &str,
) {
    if let Ok(mut g) = core.clarify.lock() {
        *g = Some(ClarifyState {
            route: "reminder".into(),
            payload: event.to_string(),
            round: 1,
            days,
            recur: recur.to_string(),
        });
    }
}

/// 分割/规格 → 草稿或依次追问(模型数字的唯一消费路径;全部经规则校验)
fn reminder_via_spec(
    core: &Assistant,
    event: &str,
    days: Option<u32>,
    hour: Option<u32>,
    minute: Option<u32>,
    recur: &str,
    now: NaiveDateTime,
) -> String {
    let event_t = event.trim();
    // 规则层校验:天数/时刻越界 → 无效
    let days_v = days.filter(|d| (0..=60).contains(d));
    let hour_v = hour.filter(|h| (0..=23).contains(h));
    let min_v = minute.filter(|m| (0..=59).contains(m));
    let recur_v = if valid_recur(recur) { recur.to_string() } else { String::new() };

    // 时刻齐了 → 直接出草稿(存 pending 待确认)
    if let (Some(d), Some(h), Some(mi)) = (days_v, hour_v, min_v) {
        if let Some((when, recur_v)) = spec_to_when(d, h, mi, &recur_v, now) {
            if let Ok(mut g) = core.pending.lock() {
                *g = Some(PendingReminder {
                    text: event_t.to_string(),
                    when,
                    recur: recur_v.clone(),
                });
            }
            return draft_reply(event_t, &when, recur_v.as_deref());
        }
    }

    // 缺槽 → 依次追问(纯规则;模型不再参与第二轮)
    let event = if event_t.is_empty() { "这件事" } else { event_t };
    store_clarify_reminder(core, event, days_v, &recur_v);
    if hour.is_none() {
        format!("⏳ 想「{event}」——什么时候提醒你?\n直接回复如「明晚八点」「大后天下午三点」;回复「取消」放弃")
    } else {
        format!("⏳ 收到,提醒你「{event}」。\n补充相对天数,回复如「3天后」「下周一」;回复「取消」放弃")
    }
}

// ---------- 窄兜底(「提醒我」前缀句) ----------

pub fn fallback(core: &Assistant, utterance: &str, now: NaiveDateTime) -> String {
    if !core.llm_enabled() {
        return USAGE.to_string();
    }
    if looks_like_question(utterance) {
        return "🤔 这句话更像提问而不是设置提醒,为避免误会我没有处理。\n\
                若确实是提醒,请换个不含疑问词的说法(发送「帮助」看示例)"
            .to_string();
    }
    if !core.llm_ready() {
        return "🧩 AI兜底已开启,但本地模型未就绪(可在「设置」页查看模型状态)。".to_string();
    }
    let out = core.infer(&build_segment_prompt(utterance));
    let Some(seg) = parse_segment(&out) else {
        return "😅 这句话我没分割开,试试明确格式:\n提醒我 明天下午3点 交报告(发送「帮助」看示例)".to_string();
    };
    reminder_via_spec(core, &seg.event, seg.days, seg.hour, seg.minute, &seg.recur, now)
}

// ---------- 追问状态机(规则优先;未命中用模型数字转换) ----------

pub(crate) fn clarify_active(core: &Assistant) -> bool {
    core.clarify.lock().map(|g| g.is_some()).unwrap_or(false)
}

fn looks_like_command(t: &str) -> bool {
    const P: [&str; 12] = [
        "天气", "新闻", "国际", "科技", "时政", "诗词", "电台", "早报", "帮助", "设置城市",
        "订阅", "时间",
    ];
    P.iter().any(|p| t.starts_with(p)) || t == "时间" || t == "帮助"
}

pub fn handle_clarify(core: &Assistant, input: &str, now: NaiveDateTime) -> Option<String> {
    let state = core.clarify.lock().ok()?.take()?;

    if input == "取消" || input == "不设了" || input == "算了" {
        return Some("好的,已放弃".to_string());
    }
    if input == "确认" {
        if let Ok(mut g) = core.clarify.lock() {
            *g = Some(state);
        }
        return Some("当前在等你补充信息:直接回复内容,或回复「取消」放弃".to_string());
    }
    if looks_like_command(input) {
        if let Ok(mut g) = core.clarify.lock() {
            *g = Some(state);
        }
        return None;
    }

    match state.route.as_str() {
        // 提醒路由:规则优先(词表:明天/大后天/每周X/…)→ 未命中才用模型数字转换 → 仍无效 → 继续追问
        "reminder" => {
            if let Some(parsed) = parse_when(input, now) {
                if let Ok(mut g) = core.pending.lock() {
                    *g = Some(PendingReminder {
                        text: state.payload.clone(),
                        when: parsed.when,
                        recur: parsed.recur.clone(),
                    });
                }
                return Some(draft_reply(
                    &state.payload,
                    &parsed.when,
                    parsed.recur.as_deref(),
                ));
            }
            // 模型转换补充时间(语义→数字;规则校验)
            if core.llm_ready() {
                let raw = core.infer(&build_reply_spec_prompt(input));
                if let Some(spec) = parse_reply_spec(&raw) {
                    if let (Some(d), Some(h), Some(mi)) = (
                        spec.days,
                        spec.hour,
                        spec.minute.filter(|m| (0..=59).contains(m)),
                    ) {
                        if let Some((when, recur_v)) = spec_to_when(d, h, mi, &spec.recur, now) {
                            if let Ok(mut g) = core.pending.lock() {
                                *g = Some(PendingReminder {
                                    text: state.payload.clone(),
                                    when,
                                    recur: recur_v.clone(),
                                });
                            }
                            return Some(draft_reply(&state.payload, &when, recur_v.as_deref()));
                        }
                    }
                }
            }
            if state.round >= 2 {
                return Some(
                    "😅 两轮没对上时间。建议用明确格式:\n「明天下午3点 交报告」或右上角表单新建"
                        .to_string(),
                );
            }
            if let Ok(mut g) = core.clarify.lock() {
                *g = Some(ClarifyState {
                    route: "reminder".into(),
                    payload: state.payload.clone(),
                    round: state.round + 1,
                    days: state.days,
                    recur: state.recur.clone(),
                });
            }
            Some("⏳ 时间没对上。直接回复具体时间,如「明天下午3点」「大后天上午10点」;回复「取消」放弃".to_string())
        }
        // 天气路由:用户补充的回复即城市名,直接查询
        "weather" => {
            let city = input.trim();
            if city.is_empty() {
                return Some("⚠️ 城市名为空,请重新回复城市名".to_string());
            }
            Some(match crate::weather::get_weather(core, city) {
                Ok(w) => w,
                Err(e) => format!("😅 {}", e.msg()),
            })
        }
        _ => Some("⚠️ 未知的追问类型,已取消".to_string()),
    }
}

// ---------- 宽兜底(全句路由;reminder 走分割+规格) ----------

/// 路由解析结果(参数均已裁剪)
pub(crate) fn wide_enabled(core: &Assistant) -> bool {
    core.llm_enabled()
        && core
            .store
            .lock()
            .ok()
            .map(|s| s.kv_get("llm_mode").as_deref() == Some("wide"))
            .unwrap_or(false)
}

/// 宽兜底:全句路由;reminder 走分割→规格→规则校验→依次确认
pub fn try_wide(core: &Assistant, utterance: &str, now: NaiveDateTime) -> Option<String> {
    if !core.llm_enabled() || !wide_enabled(core) {
        return None;
    }
    if !core.llm_ready() {
        return Some(
            "🧩 AI兜底(宽模式)已开启,但本地模型未就绪(可在「设置」页查看模型状态)。".to_string(),
        );
    }
    let out = core.infer(&build_wide_prompt(utterance));
    let parsed = parse_wide(&out).ok()?;
    match parsed {
        WideRoute::None => None,
        WideRoute::Reminder { event, time, days, hour, minute, recur } => Some(reminder_via_spec(
            core,
            &event,
            days,
            hour,
            minute,
            &recur,
            now,
        )),
        WideRoute::Weather { city } => {
            if city.is_empty() {
                if let Ok(mut g) = core.clarify.lock() {
                    *g = Some(ClarifyState {
                        route: "weather".into(),
                        payload: String::new(),
                        round: 1,
                        days: None,
                        recur: String::new(),
                    });
                }
                Some("🌤 想查哪个城市的天气?直接回复城市名(如「上海」)".to_string())
            } else {
                Some(match crate::weather::get_weather(core, &city) {
                    Ok(w) => w,
                    Err(e) => format!("😅 {}", e.msg()),
                })
            }
        }
        WideRoute::News { kind } => Some(match crate::news::news_items(core, &kind, 8) {
            Ok(items) => {
                let body: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(i, it)| format!("{}. [{}] {}", i + 1, it.source, it.title))
                    .collect();
                format!(
                    "📰 新闻(点不动这里,请在聊天页快捷词条查看可点击版)\n{}",
                    body.join("\n")
                )
            }
            Err(e) => format!("😅 {}", e.msg()),
        }),
        WideRoute::Poetry { query } => Some(match crate::poetry::recommend(core, &query) {
            Ok(s) => s,
            Err(e) => format!("😅 {}", e.msg()),
        }),
        WideRoute::Radio { query } => {
            if query.trim().is_empty() {
                Some(crate::radio::stations_text(core))
            } else {
                Some(match crate::radio::search(core, &query, 8) {
                    Ok(hits) if !hits.is_empty() => {
                        let lines: Vec<String> = hits
                            .iter()
                            .enumerate()
                            .map(|(i, h)| format!("{}. {} —— {}", i + 1, h.name, h.desc))
                            .collect();
                        format!("🔍 电台搜索「{query}」\n{}", lines.join("\n"))
                    }
                    _ => format!("没搜到与「{query}」相关的电台"),
                })
            }
        }
    }
}
