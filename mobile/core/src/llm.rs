//! 受控本地 LLM 兜底(可选,默认关闭)—— 分割式架构(2026-11-15 按用户设计重构)
//!
//! 分层(特权/隔离,参考 CaMeL 与 Copilot Studio 插槽填充):
//!   模型(隔离层):只做「语句分割」——把句子切成 事项(event) + 时间短语(time),
//!                原样保留,不换算、不输出结构化字段(0.5B 复读枚举/幻觉日期,真机实证不可靠)
//!   规则(特权层):时间短语 → 精确时间戳(词表+算术,确定性可测试);
//!                缺槽依次追问(先事项后时间,≤2 轮),确认键落库
//!
//! 真机日志依据:模型分段质量可靠(路由/抽取全对),结构化输出不可靠。

use serde_json::Value;

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
use chrono::NaiveDateTime;

/// 追问状态:事项/时间槽依次补齐
#[derive(Debug, Clone)]
pub struct ClarifyState {
    /// 路由:"reminder"(payload=事项) | "weather"(payload 未用)
    pub route: String,
    /// 已确认的槽内容(事项)
    pub payload: String,
    /// 已追问轮数(上限2轮)
    pub round: u32,
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

// ---------- 分割 prompt(模型的唯一职责) ----------

pub fn build_segment_prompt(utterance: &str) -> String {
    format!(
        "你是语句分割器。把句子分成「事项」和「时间」两段,输出一行JSON,只输出JSON本身:\n\
         {{\"event\":\"把报告写完\",\"time\":\"下周末前\"}}\n\
         {{\"event\":\"跑步\",\"time\":\"明晚八点\"}}\n\
         {{\"event\":\"交房租\",\"time\":\"每月15号\"}}\n\
         {{\"event\":\"喝水\",\"time\":\"30分钟后\"}}\n\
         {{\"event\":\"部门例会\",\"time\":\"\"}}\n\
         规则:event=要做什么(把时间词去掉);time=什么时候(保留原词,不要换算成数字);句子没有时间就 time 为空串。\n\
         句子:{utterance}\nJSON:"
    )
}

pub struct Segmentation {
    pub event: String,
    pub time: String,
}

/// 解析分割输出(容忍围栏与废话;字段缺失视为空)
pub fn parse_segment(out: &str) -> Option<Segmentation> {
    let start = out.find('{')?;
    let end = out.rfind('}')?;
    if end < start {
        return None;
    }
    let v: Value = serde_json::from_str(&out[start..=end]).ok()?;
    let event = v["event"].as_str().unwrap_or("").trim().to_string();
    let time = v["time"].as_str().unwrap_or("").trim().to_string();
    Some(Segmentation { event, time })
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

pub(crate) fn store_clarify_reminder(core: &Assistant, event: &str) {
    if let Ok(mut g) = core.clarify.lock() {
        *g = Some(ClarifyState {
            route: "reminder".into(),
            payload: event.to_string(),
            round: 1,
        });
    }
}

/// 分割结果 → 规则解析时间 → 草稿或依次追问(模型的唯一消费路径)
fn reminder_via_segment(core: &Assistant, seg: &Segmentation, now: NaiveDateTime) -> String {
    // 两槽全空:没有可执行内容
    if seg.event.is_empty() && seg.time.is_empty() {
        return USAGE.to_string();
    }
    // 时刻在词表里 → 直接出草稿
    if !seg.time.is_empty() {
        if let Some(parsed) = parse_when(&seg.time, now) {
            let text = if seg.event.is_empty() { "到时间啦" } else { seg.event.as_str() };
            if let Ok(mut g) = core.pending.lock() {
                *g = Some(PendingReminder {
                    text: text.to_string(),
                    when: parsed.when,
                    recur: parsed.recur.clone(),
                });
            }
            return draft_reply(text, &parsed.when, parsed.recur.as_deref());
        }
    }
    // 缺时刻(或时刻不在词表):记下事项,依次追问时间(纯规则,模型不再参与)
    let event = if seg.event.is_empty() { seg.time.as_str() } else { seg.event.as_str() };
    store_clarify_reminder(core, event);
    if seg.time.is_empty() {
        format!("⏳ 想「{event}」——什么时候提醒你?\n直接回复如「明晚八点」「大后天下午三点」;回复「取消」放弃")
    } else {
        format!(
            "⏳ 收到,提醒你「{event}」,时间「{}」我没对上。\n直接回复具体时间,如「明晚八点」「大后天下午三点」;回复「取消」放弃",
            seg.time
        )
    }
}

// ---------- 窄兜底(「提醒我」前缀句) ----------

pub fn fallback(core: &Assistant, utterance: &str, now: NaiveDateTime) -> String {
    if !core.llm_enabled() {
        return USAGE.to_string();
    }
    // 疑问句预闸门(作用域控制):提醒句式里的提问不进模型
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
    reminder_via_segment(core, &seg, now)
}

// ---------- 追问状态机(纯规则第二阶段;模型不参与时刻换算) ----------

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
        // 提醒路由:规则优先解析补充的时间(确定性);不认识 → 继续追问(≤2轮)
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

// ---------- 宽兜底(全句路由;reminder 走分割) ----------

/// 路由解析结果(参数均已裁剪)
pub enum WideRoute {
    /// 提醒:事项 + 时间短语(原样,交规则换算)
    Reminder { event: String, time: String },
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

pub fn build_wide_prompt(utterance: &str) -> String {
    format!(
        "你是个人助手路由器。判断用户这句话想使用哪个功能,输出一行JSON,只输出JSON本身:\n\
         {{\"route\":\"reminder\",\"event\":\"把报告写完\",\"time\":\"下周末前\"}}\n\
         {{\"route\":\"weather\",\"city\":\"北京\"}}\n\
         {{\"route\":\"news\",\"kind\":\"all\"}}\n\
         {{\"route\":\"poetry\",\"query\":\"励志\"}}\n\
         {{\"route\":\"radio\",\"query\":\"爵士\"}}\n\
         {{\"route\":\"none\"}}\n\
         规则:\n\
         - reminder:要求在未来某时间做某事;event=要做什么(去掉时间词);time=什么时候(保留原词,不要换算成数字);没有时间就 time 为空串;\n\
         - weather/news/poetry/radio:query 填关键词;\n\
         - 与以上功能都无关(聊天/提问/其他)→ {{\"route\":\"none\"}};\n\
         句子:{utterance}\nJSON:"
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

/// 解析宽兜底路由输出(字段裁剪)
pub fn parse_wide(out: &str) -> Result<WideRoute, String> {
    let start = out.find('{').ok_or("输出里没有JSON")?;
    let end = out.rfind('}').ok_or("输出里没有JSON")?;
    if end < start {
        return Err("输出里没有JSON".into());
    }
    let v: Value = serde_json::from_str(&out[start..=end]).map_err(|e| format!("JSON非法:{e}"))?;
    let route = v["route"].as_str().ok_or("缺少route")?;
    let field = |k: &str| -> String { clip_field(v[k].as_str().unwrap_or(""), 40) };
    Ok(match route {
        "reminder" => WideRoute::Reminder {
            event: {
                let e = field("event");
                if e.is_empty() { field("text") } else { e }
            },
            time: field("time"),
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

pub(crate) fn wide_enabled(core: &Assistant) -> bool {
    core.llm_enabled()
        && core
            .store
            .lock()
            .ok()
            .map(|s| s.kv_get("llm_mode").as_deref() == Some("wide"))
            .unwrap_or(false)
}

/// 宽兜底:全句路由;reminder 走分割→规则换算→依次确认
pub fn try_wide(core: &Assistant, utterance: &str, now: NaiveDateTime) -> Option<String> {
    if !core.llm_enabled() || !wide_enabled(core) {
        return None;
    }
    // 疑问句不再拦截:路由器自行判断(如「明天北京天气怎么样」应路由 weather)
    if !core.llm_ready() {
        return Some(
            "🧩 AI兜底(宽模式)已开启,但本地模型未就绪(可在「设置」页查看模型状态)。".to_string(),
        );
    }
    let out = core.infer(&build_wide_prompt(utterance));
    let parsed = parse_wide(&out).ok()?;
    match parsed {
        WideRoute::None => None,
        WideRoute::Reminder { event, time } => Some(reminder_via_segment(
            core,
            &Segmentation { event, time },
            now,
        )),
        WideRoute::Weather { city } => {
            if city.is_empty() {
                if let Ok(mut g) = core.clarify.lock() {
                    *g = Some(ClarifyState {
                        route: "weather".into(),
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

// ---------- 测试 ----------

#[cfg(test)]
mod segment_tests {
    use super::*;
    use crate::api::LlmInfer;

    /// 模拟模型:路由→reminder 分割;分割器→event/time
    struct MockLlm;
    impl LlmInfer for MockLlm {
        fn infer(&self, prompt: String) -> String {
            if prompt.contains("路由器") {
                return r#"{"route":"reminder","event":"交周报","time":"周五18点"}"#.to_string();
            }
            if prompt.contains("分割器") {
                // 返回词表不认识的时间,以覆盖「依次追问」路径
                return r#"{"event":"交材料","time":"汇报会后"}"#.to_string();
            }
            String::new()
        }
    }

    #[test]
    fn 宽兜底分割后确认落库() {
        let db = std::env::temp_dir().join(format!("lp_seg_{}.db", uuid::Uuid::new_v4()));
        let core = Assistant::new(db.to_str().unwrap().to_string()).unwrap();
        core.set_setting("llm_enabled".to_string(), "1".to_string()).unwrap();
        core.set_setting("llm_mode".to_string(), "wide".to_string()).unwrap();
        core.attach_llm(Box::new(MockLlm));

        // 疑问句不再被宽模式拦截
        let r1 = crate::intent::dispatch(&core, "帮我安排一下交周报的事?");
        assert!(r1.contains("确认"), "分割后应出草稿: {r1}");
        let r2 = crate::intent::dispatch(&core, "确认");
        assert!(r2.contains("已设置"), "确认后落库: {r2}");
        assert_eq!(core.list_reminders().unwrap().len(), 1);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn 窄兜底分割_缺时间依次追问() {
        let db = std::env::temp_dir().join(format!("lp_seg2_{}.db", uuid::Uuid::new_v4()));
        let core = Assistant::new(db.to_str().unwrap().to_string()).unwrap();
        core.set_setting("llm_enabled".to_string(), "1".to_string()).unwrap();
        core.attach_llm(Box::new(MockLlm));

        // 规则不认识「汇报会后」→ 模型分割 → 时间词表不认识 → 依次追问(不再调模型)
        let r1 = crate::intent::dispatch(&core, "提醒我汇报会后交材料");
        assert!(r1.contains("时间"), "应追问时间: {r1}");
        // 补充可解析时间 → 草稿
        let r2 = crate::intent::dispatch(&core, "明天下午3点");
        assert!(r2.contains("确认"), "补充后应出草稿: {r2}");
        let r3 = crate::intent::dispatch(&core, "确认");
        assert!(r3.contains("已设置"));
        assert_eq!(core.list_reminders().unwrap()[0].text, "交材料");
        let _ = std::fs::remove_file(&db);
    }
}
