//! 受控本地 LLM 兜底(可选,默认关闭)
//!
//! 两层 schema:
//! - 窄兜底(「提醒我」前缀句):提醒专用 schema,不变;
//! - 宽兜底(设置开启):**全域路由器** —— 把任何规则没听懂的句子路由到五大能力
//!   (提醒/天气/新闻/诗词/电台),缺参数(如天气没城市)进入追问状态机。
//!
//! 风险分层:
//! - 提醒(写闹钟,高危):生成→verifier 自检→「确认/显示表单/取消」显式按键,LLM 永不直接决定闹钟;
//! - 天气/新闻/诗词/电台(只读,低危):参数超界/none 直接回「没太听懂」,不阻塞;
//! - 所有日期计算都在本地确定性代码完成,LLM 只输出受限字段与原文短语。

use serde_json::Value;

use crate::api::Assistant;
use crate::intent::{recur_label, USAGE};
use crate::timeparse::{human_dt, next_recurring_dt, parse_when};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

// ---------------- 窄兜底:提醒专用 schema ----------------

/// 待用户「确认」的提醒草稿
#[derive(Debug, Clone)]
pub struct PendingReminder {
    pub text: String,
    pub when: NaiveDateTime,
    pub recur: Option<String>,
}

/// 追问状态:用户已确认“要设提醒/查天气”但关键参数未定
#[derive(Debug, Clone)]
pub struct ClarifyState {
    /// 路由:"reminder"(payload=事项文本) | "weather"(payload 未用)
    pub route: String,
    /// 路由所需载荷
    pub payload: String,
    /// 已追问轮数(上限2轮)
    pub round: u32,
}

/// prompt:把 schema 钉死,模型只做"翻译",不做推理;反例防误触发
pub fn build_prompt(utterance: &str) -> String {
    format!(
        "你是提醒时间解析器。把用户的话转成一行JSON,只能输出JSON本身,字段只能是以下之一:\n\
{{\"kind\":\"rel\",\"value\":30,\"unit\":\"分钟\",\"text\":\"喝水\"}}\n\
{{\"kind\":\"clock\",\"h\":20,\"m\":0,\"text\":\"跑步\"}}\n\
{{\"kind\":\"daily\",\"h\":8,\"m\":0,\"text\":\"早读\"}}\n\
{{\"kind\":\"weekly\",\"wd\":4,\"h\":18,\"m\":0,\"text\":\"交周报\"}}\n\
{{\"kind\":\"monthly\",\"day\":15,\"h\":10,\"m\":0,\"text\":\"交房租\"}}\n\
{{\"kind\":\"dayoffset\",\"days\":1,\"h\":7,\"m\":30,\"text\":\"起床\"}}\n\
{{\"kind\":\"date\",\"month\":5,\"day\":1,\"h\":9,\"m\":0,\"text\":\"出发\"}}\n\
{{\"kind\":\"ask\",\"question\":\"晚上具体几点提醒你?\"}}\n\
{{\"kind\":\"none\"}}\n\
规则:wd 0=周一…6=周日;unit 只能是 分钟/小时/天;时间用24小时制(晚上8点→h=20,下午3点半→h=15,m=30);\
\"每天\"用daily,\"每周X\"用weekly,\"每月X号\"用monthly,今天/明天/后天用dayoffset,具体某月某日用date;\
rel 用于\"N个单位后\"。\n\
只有当这句话明确要求\"在未来某个时间做某事\"时才输出非none;提问、聊天、查询一律输出 {{\"kind\":\"none\"}}。\n\
如果明确是要设提醒、但缺少具体时间(如「晚上」「吃完饭后」这种模糊时间),输出 ask,question 用一句自然的话追问具体时间。\n\
示例:「晚上有学校活动」→ {{\"kind\":\"ask\",\"question\":\"晚上几点提醒你「学校活动」?\"}}\n\
反例:「提醒我一下你是谁」→ {{\"kind\":\"none\"}}\n\
反例:「提醒我 明天天气怎么样」→ {{\"kind\":\"none\"}}\n\
用户:{utterance}\nJSON:"
    )
}

/// 校验器 prompt:generator→verifier 两段式 harness 的第二段
fn build_verify_prompt(utterance: &str, draft_json: &str) -> String {
    format!(
        "你是校验器。判断下面的JSON是否准确表达了原句的提醒意图(触发时间和事项都要对得上)。只回答一个字:是 或 否。\n原句:{utterance}\nJSON:{draft_json}\n答案:"
    )
}

/// 自检:fail-closed——模型没答「是」(含空输出/拒答/含糊)一律视为不通过
fn verifier_ok(core: &Assistant, utterance: &str, draft_json: &str) -> bool {
    let ans = core.infer(&build_verify_prompt(utterance, draft_json));
    let a = ans.trim();
    !a.is_empty() && a.contains('是') && !a.contains('否') && !a.contains('不')
}

/// 解析模型输出:none→Err;ask→Ask(追问);其余→校验后的草稿
pub enum DraftOutcome {
    Draft(PendingReminder),
    Ask(String),
}

/// 从模型输出中提取第一个 JSON 对象(容忍 ```json 围栏与前后废话)
fn extract_json(out: &str) -> Option<&str> {
    let start = out.find('{')?;
    let end = out.rfind('}')?;
    if end < start {
        return None;
    }
    Some(&out[start..=end])
}

/// 校验并计算:LLM 输出 → 草稿或追问(此处绝不信任模型,一切超界即拒)
pub fn parse_draft(out: &str, now: NaiveDateTime) -> Result<DraftOutcome, String> {
    let raw = extract_json(out).ok_or("输出里没有JSON")?;
    let v: Value = serde_json::from_str(raw).map_err(|e| format!("JSON非法:{e}"))?;
    let kind = v["kind"].as_str().ok_or("缺少kind")?;
    if kind == "none" {
        return Err("模型判断这不是设置提醒".to_string());
    }
    if kind == "ask" {
        let q = v["question"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("具体几点提醒你?")
            .to_string();
        return Ok(DraftOutcome::Ask(q));
    }
    let text = {
        let t = v["text"].as_str().unwrap_or("").trim();
        if t.is_empty() {
            "到时间啦"
        } else {
            t
        }
    };
    let get_u32 = |k: &str| -> Result<u32, String> {
        v[k].as_u64()
            .filter(|x| *x <= u32::MAX as u64)
            .map(|x| x as u32)
            .ok_or(format!("字段{k}缺失或非法"))
    };

    let (when, recur) = match kind {
        "rel" => {
            let value = v["value"].as_f64().ok_or("value非法")?;
            if !(0.0..1e6).contains(&value) || value <= 0.0 {
                return Err("value超界".to_string());
            }
            let unit = v["unit"].as_str().ok_or("unit非法")?;
            let secs = match unit {
                "分钟" => value * 60.0,
                "小时" => value * 3600.0,
                "天" => value * 86400.0,
                _ => return Err("unit只能是 分钟/小时/天".to_string()),
            };
            (now + Duration::seconds(secs.round() as i64), None)
        }
        "clock" => {
            let (h, m) = (get_u32("h")?, get_u32("m")?);
            if h > 23 || m > 59 {
                return Err("时刻超界".to_string());
            }
            let mut when = now.date().and_hms_opt(h, m, 0).ok_or("时刻非法")?;
            if when <= now {
                when += Duration::days(1);
            }
            (when, None)
        }
        "daily" => {
            let (h, m) = (get_u32("h")?, get_u32("m")?);
            if h > 23 || m > 59 {
                return Err("时刻超界".to_string());
            }
            (next_recurring_dt(now, "daily", h, m), Some("daily".to_string()))
        }
        "weekly" => {
            let wd = get_u32("wd")?;
            let (h, m) = (get_u32("h")?, get_u32("m")?);
            if wd > 6 || h > 23 || m > 59 {
                return Err("weekly字段超界".to_string());
            }
            (
                next_recurring_dt(now, &format!("weekly:{wd}"), h, m),
                Some(format!("weekly:{wd}")),
            )
        }
        "monthly" => {
            let day = get_u32("day")?;
            let (h, m) = (get_u32("h")?, get_u32("m")?);
            if h > 23 || m > 59 {
                return Err("时刻超界".to_string());
            }
            (crate::timeparse::first_monthly(now, day, h, m).ok_or("日期计算失败")?, Some(format!("monthly:{day}")))
        }
        "dayoffset" => {
            let days = get_u32("days")?;
            let (h, m) = (get_u32("h")?, get_u32("m")?);
            if days > 60 || h > 23 || m > 59 {
                return Err("dayoffset字段超界".to_string());
            }
            let mut when = (now + Duration::days(days as i64))
                .date()
                .and_hms_opt(h, m, 0)
                .ok_or("时刻非法")?;
            if when <= now {
                when += Duration::days(1);
            }
            (when, None)
        }
        "date" => {
            let mo = get_u32("month")?;
            let dy = get_u32("day")?;
            let (h, m) = (get_u32("h")?, get_u32("m")?);
            if mo == 0 || mo > 12 || dy == 0 || dy > 31 || h > 23 || m > 59 {
                return Err("date字段超界".to_string());
            }
            let mut d = NaiveDate::from_ymd_opt(now.year(), mo, dy).ok_or("日期不存在")?;
            if d < now.date() {
                d = NaiveDate::from_ymd_opt(now.year() + 1, mo, dy).ok_or("日期不存在")?;
            }
            (d.and_hms_opt(h, m, 0).ok_or("时刻非法")?, None)
        }
        other => return Err(format!("未知kind:{other}")),
    };
    Ok(DraftOutcome::Draft(PendingReminder {
        text: text.to_string(),
        when,
        recur,
    }))
}

/// 疑问句预闸门(纯规则,零推理):拦住"用提醒句式问问题"的干扰输入。
/// 只用于窄兜底(提醒作用域);宽兜底可路由天气等问题类查询。
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

/// 路由解析结果(参数均已裁剪/校验)
pub enum WideRoute {
    /// 提醒(when 为原句时间短语,交由本地 parse_when 确定性计算)
    Reminder { text: String, when: String },
    /// 天气(city 可为空 → 追问)
    Weather { city: String },
    /// 新闻(all/intl/tech/politics)
    News { kind: String },
    /// 诗词
    Poetry { query: String },
    /// 电台(query 可空 → 精选台)
    Radio { query: String },
    /// 与能力无关
    None,
}

/// 宽兜底路由 prompt
pub fn build_wide_prompt(utterance: &str) -> String {
    format!(
        "你是个人助手路由器。判断用户这句话想使用哪个功能,输出一行JSON,只输出JSON本身:\n\
{{\"route\":\"reminder\",\"text\":\"起床\",\"when\":\"明早八点\"}}\n\
{{\"route\":\"weather\",\"city\":\"北京\"}}\n\
{{\"route\":\"news\",\"kind\":\"all\"}}\n\
{{\"route\":\"poetry\",\"query\":\"励志\"}}\n\
{{\"route\":\"radio\",\"query\":\"爵士\"}}\n\
{{\"route\":\"none\"}}\n\
规则:\n\
- reminder:要求在未来某时间做某事;when 填原句里的时间短语(如「晚上八点」「明早」「30分钟后」),没有就填空串;\n\
- weather:查天气;city 填城市名,没提到城市就填空串;\n\
- news:kind 只能 all/intl/tech/politics;poetry/radio 的 query 填关键词,没有就填空串;\n\
- 与以上功能都无关(聊天/提问/其他)→ {{\"route\":\"none\"}};\n\
- 时间短语保持中文原样,不要换算成数字。\n\
用户:{utterance}\nJSON:"
    )
}

fn clip_field(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() > n {
        t.chars().take(n).collect()
    } else {
        t.to_string()
    }
}

/// 解析宽兜底路由输出(字段裁剪 + 枚举校验;参数的语义校验在调用方)
pub fn parse_wide(out: &str) -> Result<WideRoute, String> {
    let raw = extract_json(out).ok_or("输出里没有JSON")?;
    let v: Value = serde_json::from_str(raw).map_err(|e| format!("JSON非法:{e}"))?;
    let route = v["route"].as_str().ok_or("缺少route")?;
    let field = |k: &str| -> String { clip_field(v[k].as_str().unwrap_or(""), 40) };
    Ok(match route {
        "reminder" => WideRoute::Reminder {
            text: {
                let t = field("text");
                if t.is_empty() { "到时间啦".to_string() } else { t }
            },
            when: field("when"),
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

// ---------------- 追问(多轮澄清)状态机 ----------------

pub(crate) fn clarify_active(core: &Assistant) -> bool {
    core.clarify.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// 判断是否是“切换话题”的普通指令(是→恢复追问状态,让正常分派处理)
fn looks_like_command(t: &str) -> bool {
    const P: [&str; 12] = [
        "天气", "新闻", "国际", "科技", "时政", "诗词", "电台", "早报", "帮助", "设置城市",
        "订阅", "时间",
    ];
    P.iter().any(|p| t.starts_with(p)) || t == "时间" || t == "帮助"
}

/// 第二轮:结合原句与用户补充,再解析一次(仅提醒路由使用)
fn build_clarify_prompt(text: &str, reply: &str) -> String {
    format!(
        "你是提醒时间解析器。用户想设置提醒:「{text}」,并补充了时间信息:「{reply}」。\
把合并后的提醒转成一行JSON,kind 只能是 rel/clock/daily/weekly/monthly/dayoffset/date;\
时间用24小时制;时间仍不明确就输出 {{\"kind\":\"ask\",\"question\":\"...\"}}。只输出JSON。\nJSON:"
    )
}

/// 有未完成追问时,对每条用户输入先行处理。
/// 返回 Some(回复文案)=本条已消费;None=用户切换了话题,恢复正常分派。
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
        // 提醒路由:规则解析时间(确定性优先)→ 合并草稿
        "reminder" => {
            if let Some(parsed) = parse_when(input, now) {
                return Some(draft_reply(
                    core,
                    PendingReminder {
                        text: state.payload.clone(),
                        when: parsed.when,
                        recur: parsed.recur,
                    },
                ));
            }
            if state.round >= 2 {
                return Some(
                    "😅 追问两轮仍未确定时间。建议用明确格式:\n提醒我 晚上8点 写作业\n\
                     (时间不定的场景,可用「显示表单」手动选时间)"
                        .to_string(),
                );
            }
            let out = core.infer(&build_clarify_prompt(&state.payload, input));
            match parse_draft(&out, now) {
                Ok(DraftOutcome::Draft(d)) => Some(draft_reply(core, d)),
                Ok(DraftOutcome::Ask(q)) => {
                    if let Ok(mut g) = core.clarify.lock() {
                        *g = Some(ClarifyState {
                            route: "reminder".to_string(),
                            payload: state.payload.clone(),
                            round: state.round + 1,
                        });
                    }
                    Some(format!(
                        "⏳ {q}\n(直接回复时间即可,如「晚上八点」;回复「取消」放弃)"
                    ))
                }
                Err(_) => Some(
                    "😅 还是没能确定时间。建议直接用明确格式:\n提醒我 晚上8点 写作业".to_string(),
                ),
            }
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

// ---------------- harness 公共流程 ----------------

/// 窄兜底 harness:疑问闸 → 提醒schema生成 → 解析 → 自检
enum HarnessOutcome {
    Draft(PendingReminder),
    Ask(String),
    NotAReminder,
    Question,
    NotReady,
}

fn run_harness(core: &Assistant, utterance: &str, now: NaiveDateTime) -> HarnessOutcome {
    if looks_like_question(utterance) {
        return HarnessOutcome::Question;
    }
    if !core.llm_ready() {
        return HarnessOutcome::NotReady;
    }
    let out = core.infer(&build_prompt(utterance));
    match parse_draft(&out, now) {
        Ok(DraftOutcome::Draft(d)) => {
            let json = extract_json(&out).unwrap_or("");
            if !verifier_ok(core, utterance, json) {
                return HarnessOutcome::NotAReminder;
            }
            HarnessOutcome::Draft(d)
        }
        Ok(DraftOutcome::Ask(q)) => HarnessOutcome::Ask(q),
        Err(_) => HarnessOutcome::NotAReminder,
    }
}

fn draft_reply(core: &Assistant, draft: PendingReminder) -> String {
    let label = recur_label(draft.recur.as_deref());
    let when = draft.when;
    let text = draft.text.clone();
    if let Ok(mut g) = core.pending.lock() {
        *g = Some(draft);
    }
    format!(
        "🧩 我理解你想设置:\n⏰ {} {}\n📝 {}\n\n下方按键:确认生效 / 取消放弃 / 查看修改表单",
        human_dt(&when),
        label,
        text
    )
}

pub(crate) fn store_clarify_reminder(core: &Assistant, utterance: &str) {
    if let Ok(mut g) = core.clarify.lock() {
        *g = Some(ClarifyState {
            route: "reminder".to_string(),
            payload: utterance.to_string(),
            round: 1,
        });
    }
}

/// 窄兜底:「提醒」前缀句解析失败时调用;任何结果都转成给用户的文案
pub fn fallback(core: &Assistant, utterance: &str, now: NaiveDateTime) -> String {
    if !core.llm_enabled() {
        return USAGE.to_string();
    }
    match run_harness(core, utterance, now) {
        HarnessOutcome::Draft(d) => draft_reply(core, d),
        HarnessOutcome::Ask(q) => {
            store_clarify_reminder(core, utterance);
            format!("⏳ {q}\n(直接回复时间即可,如「晚上八点」;回复「取消」放弃)")
        }
        HarnessOutcome::Question => {
            "🤔 这句话更像提问而不是设置提醒,为避免误会我没有处理。\n\
             若确实是提醒,请换个不含疑问词的说法(发送「帮助」看示例)"
                .to_string()
        }
        HarnessOutcome::NotReady => {
            "🧩 AI兜底已开启,但本地模型未就绪(可在「设置」页查看模型状态)。\n\
             请把 GGUF 模型放到:/sdcard/Android/data/com.lenovo.lpasistant/files/model.gguf"
                .to_string()
        }
        HarnessOutcome::NotAReminder => {
            "😅 这句话我没看懂,本地模型也没能给出有效结果。\n\
             换个更明确的说法试试,如:提醒我 晚上8点 跑步(发送「帮助」看示例)"
                .to_string()
        }
    }
}

/// 宽兜底:全句路由到五大能力;None = 与能力无关(调用方回退「没太听懂」)
pub fn try_wide(core: &Assistant, utterance: &str, now: NaiveDateTime) -> Option<String> {
    if !core.llm_enabled() || !wide_enabled(core) {
        return None;
    }
    // 宽模式不拦疑问句:路由器自行判断(如「明天北京天气怎么样」应路由 weather);
    // reminder 路由仍有 parse_when+verifier+确认闸兜底,误入的疑问句会被校验拒掉。
    if !core.llm_ready() {
        return Some(
            "🧩 AI兜底(宽模式)已开启,但本地模型未就绪(可在「设置」页查看)。".to_string(),
        );
    }
    let out = core.infer(&build_wide_prompt(utterance));
    let parsed = parse_wide(&out).ok()?;
    match parsed {
        WideRoute::None => None,
        WideRoute::Reminder { text, when } => {
            match parse_when(&when, now) {
                // 时间短语可被本地确定性解析 → 仍走自检 + 确认按键
                Some(parsed) => {
                    let json = extract_json(&out).unwrap_or("");
                    if !verifier_ok(core, utterance, json) {
                        return None;
                    }
                    Some(draft_reply(
                        core,
                        PendingReminder {
                            text,
                            when: parsed.when,
                            recur: parsed.recur,
                        },
                    ))
                }
                // 时间短语规则解析失败 → 进入追问状态机(记住事项,追问时间)
                None => {
                    store_clarify_reminder(core, &text);
                    Some(
                        "⏳ 想在什么时间提醒你「{text}」?\n直接回复时间(如「晚上八点」「30分钟后」),回复「取消」放弃"
                            .replace("{text}", &text),
                    )
                }
            }
        }
        WideRoute::Weather { city } => {
            if city.is_empty() {
                if let Ok(mut g) = core.clarify.lock() {
                    *g = Some(ClarifyState {
                        route: "weather".to_string(),
                        payload: String::new(),
                        round: 1,
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
                let note = items
                    .first()
                    .filter(|i| i.cached_at > 0)
                    .map(|i| format!("\n\n{}", crate::cache::offline_note(i.cached_at, "新闻")))
                    .unwrap_or_default();
                format!(
                    "📰 新闻(点不动这里,请在聊天页快捷词条查看可点击版)\n{}{note}",
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
                Some(crate::radio::stations_text())
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

pub(crate) fn wide_enabled(core: &Assistant) -> bool {
    core.llm_enabled()
        && core
            .store
            .lock()
            .ok()
            .map(|s| s.kv_get("llm_mode").as_deref() == Some("wide"))
            .unwrap_or(false)
}

#[cfg(test)]
mod wide_tests {
    use crate::api::{Assistant, LlmInfer};

    /// 模拟模型:校验器问「是」;路由/解析一律返回合法 reminder JSON
    struct MockLlm;
    impl LlmInfer for MockLlm {
        fn infer(&self, prompt: String) -> String {
            if prompt.contains("校验器") {
                return "是".to_string();
            }
            r#"{"route":"reminder","text":"交周报","when":"周五18点"}"#.to_string()
        }
    }

    /// 全链路(模拟模型):开兜底+宽模式 → 规则未命中句 → 路由 → 草稿确认
    #[test]
    fn wide_routes_reminder_and_confirms() {
        let db = std::env::temp_dir().join(format!("lp_wide_{}.db", uuid::Uuid::new_v4()));
        let core = Assistant::new(db.to_str().unwrap().to_string()).unwrap();
        core.set_setting("llm_enabled".to_string(), "1".to_string()).unwrap();
        core.set_setting("llm_mode".to_string(), "wide".to_string()).unwrap();
        core.attach_llm(Box::new(MockLlm));

        // 疑问句不再被宽模式拦截(路由器自行判断)
        let r1 = crate::intent::dispatch(&core, "帮我安排一下交周报的事?");
        assert!(r1.contains("确认"), "疑问句也应路由进提醒草稿: {r1}");

        let r2 = crate::intent::dispatch(&core, "确认");
        assert!(r2.contains("已设置"), "确认后落库: {r2}");
        let list = core.list_reminders().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text, "交周报");
        let _ = std::fs::remove_file(&db);
    }
}
