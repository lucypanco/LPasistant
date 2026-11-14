//! 指令调度:文本 → 意图 → 功能模块(从验证过的 Python bot.py 移植,规则 1:1)

use std::sync::LazyLock;

use chrono::{Duration, Local};
use regex::Regex;

use crate::api::Assistant;
use crate::store::CoreError2;
use crate::timeparse::{epoch_to_local, human_dt, parse_clock, parse_when, weekday_cn};
use crate::{news, poetry, weather};

pub const HELP_POINTER: &str = "📖 完整帮助已移至:右上角「设置 → 使用帮助」\n快捷词条在输入框上方,点按即用;\n长按任意消息可复制全部 / 选择文本";

static RE_DATE_Q: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:今天几号|今天日期|今天星期几|星期几|周几|日期)\??$").unwrap());
static RE_W_CITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^天气\s*([\p{Han}A-Za-z]{0,12})$").unwrap());
static RE_CITY_W: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\p{Han}]{2,10})天气$").unwrap());
static RE_CANCEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:取消提醒|删除提醒|关闭提醒)\s*([0-9a-fA-F]{1,8})$").unwrap());
static RE_NEWS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:新闻|要闻|资讯)\s*(\d{1,2})?$").unwrap());
static RE_INTL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:国际|国际新闻|国际要闻|国外|外媒)\s*(\d{1,2})?$").unwrap());
static RE_TECH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:科技|科技新闻|科技资讯|科技要闻)\s*(\d{1,2})?$").unwrap());
static RE_POLI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:时政|政治|时政新闻|国内要闻)\s*(\d{1,2})?$").unwrap());
static RE_POEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:诗词|古诗|今日诗词|推荐诗词|来首诗|来一首诗)\s*(.*)$").unwrap());
static RE_SUB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^订阅早报\s*(\S+)?\s*$").unwrap());
static RE_SETPOEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^设置诗词源\s*(https?://\S+)$").unwrap());
static RE_CLRPOEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^清除诗词源$").unwrap());

pub(crate) fn recur_label(recur: Option<&str>) -> &'static str {
    match recur {
        Some("daily") => "(每天)",
        Some(r) if r.starts_with("weekly:") => "(每周)",
        Some(r) if r.starts_with("monthly:") => "(每月)",
        _ => "",
    }
}

pub(crate) const USAGE: &str = "⏰ 提醒用法:\n\
提醒我 18:00 开会\n\
提醒我 30分钟后 喝水\n\
提醒我 晚上8点 跑步\n\
提醒我 明天 07:30 起床\n\
提醒我 每天 08:00 早读\n\
提醒我 每周五 18:00 交周报\n\
提醒我 每月15号 10:00 交房租";

fn friendly(r: Result<String, crate::api::CoreError>) -> String {
    match r {
        Ok(s) => s,
        Err(e) => e.msg().to_string(),
    }
}

/// "39.9042,116.4074" → (lat, lon),带范围校验
fn parse_coords(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once(',')?;
    let lat: f64 = a.trim().parse().ok()?;
    let lon: f64 = b.trim().parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some((lat, lon))
}

fn count(core: &Assistant, g: Option<regex::Match<'_>>) -> usize {
    match g.and_then(|m| m.as_str().parse::<usize>().ok()) {
        Some(n) => n.clamp(1, 30),
        None => core
            .store
            .lock()
            .ok()
            .and_then(|s| s.kv_get("news_count"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(8)
            .clamp(1, 30),
    }
}

/// 入口:规范化后分派;未识别返回引导语
pub fn dispatch(core: &Assistant, input: &str) -> String {
    let mut t = input.trim();
    while let Some(rest) = t.strip_prefix(['/', '!']) {
        t = rest.trim();
    }
    if t.is_empty() {
        return HELP_POINTER.to_string();
    }
    if let Some(reply) = try_dispatch(core, t) {
        reply
    } else if let Some(reply) = crate::llm::try_wide(core, t, Local::now().naive_local()) {
        // 宽兜底:规则没听懂的句子交给本地模型识别提醒意图(仅设置开启时生效)
        reply
    } else {
        format!("没太听懂「{t}」🤔\n发送「帮助」查看全部指令")
    }
}

fn try_dispatch(core: &Assistant, t: &str) -> Option<String> {
    if matches!(
        t.to_lowercase().as_str(),
        "帮助" | "菜单" | "功能" | "指令" | "指令列表" | "help" | "?" | "？"
    ) {
        return Some(HELP_POINTER.to_string());
    }

    // ---- 未完成的 LLM 追问:优先处理用户的补充回复(如「晚上八点」) ----
    if crate::llm::clarify_active(core) {
        if let Some(reply) = crate::llm::handle_clarify(core, t, Local::now().naive_local()) {
            return Some(reply);
        }
        // None = 用户切换了话题,追问状态已恢复,继续正常分派
    }

    // ---- LLM 兜底的二步确认:显式按键为主入口(CoreBridge.confirmPending 等);
    //      文字指令保留作为兼容入口 ----
    if t == "确认" {
        return Some(core.confirm_pending().unwrap_or_else(|e| format!("😅 {}", e.msg())));
    }
    if t == "取消" {
        return Some(core.cancel_pending());
    }

    // ---- 时间日期 ----
    if matches!(t, "时间" | "现在几点" | "几点了" | "几点" | "现在时间") || RE_DATE_Q.is_match(t) {
        let now = Local::now().naive_local();
        return Some(format!(
            "🕐 现在:{} {} {}",
            now.format("%Y年%m月%d日"),
            weekday_cn(&now),
            now.format("%H:%M:%S")
        ));
    }

    // ---- 天气 ----
    if let Some(c) = RE_W_CITY.captures(t) {
        let city = c.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        if city.is_empty() && core.geo_enabled() {
            // 自动定位:坐标直供 Open-Meteo,不经第三方逆地理编码
            if let Some(coords) = core.current_coords() {
                if let Some((lat, lon)) = parse_coords(&coords) {
                    return Some(friendly(weather::here(core, lat, lon)));
                }
            }
            return Some(
                "⚠️ 未能获取定位(未授权或暂无位置数据),已改用默认城市\n".to_string()
                    + &friendly(weather::get_weather(core, "")),
            );
        }
        return Some(friendly(weather::get_weather(core, city)));
    }
    if let Some(c) = RE_CITY_W.captures(t) {
        return Some(friendly(weather::get_weather(
            core,
            c.get(1).unwrap().as_str(),
        )));
    }

    // ---- 提醒:取消 ----
    if let Some(c) = RE_CANCEL.captures(t) {
        let mut store = core.store.lock().ok()?;
        return Some(match store.remove_reminder(c.get(1).unwrap().as_str()) {
            Some(r) => format!("🗑 已取消提醒「{}」", r.text),
            None => "没有找到这个编号,发送「我的提醒」查看列表".to_string(),
        });
    }

    // ---- 提醒:列表 ----
    if matches!(t, "我的提醒" | "提醒列表" | "查看提醒" | "提醒") {
        let store = core.store.lock().ok()?;
        let items = store.list_reminders();
        if items.is_empty() {
            return Some("📋 你还没有提醒。试试:提醒我 30分钟后 喝水".to_string());
        }
        let mut lines = vec![format!("📋 你的提醒(共{}条)", items.len())];
        for r in &items {
            let dt = epoch_to_local(r.next_run);
            lines.push(format!(
                "[{}] {} {} · {}",
                r.id,
                human_dt(&dt),
                recur_label(r.recur.as_deref()),
                r.text
            ));
        }
        lines.push("(取消:取消提醒 编号)".to_string());
        return Some(lines.join("\n"));
    }

    // ---- 提醒:新增 ----
    for p in ["提醒我", "定时提醒", "添加提醒", "设置提醒"] {
        if let Some(body) = t.strip_prefix(p) {
            return Some(add_reminder(core, body.trim()));
        }
    }

    // ---- 新闻(专业源:国际+科技+时政;已移除娱乐热搜类) ----
    if let Some(c) = RE_NEWS.captures(t) {
        return Some(friendly(news::news_all(core, count(core, c.get(1)))));
    }
    if let Some(c) = RE_INTL.captures(t) {
        return Some(friendly(news::news_intl(core, count(core, c.get(1)))));
    }
    if let Some(c) = RE_TECH.captures(t) {
        return Some(friendly(news::news_tech(core, count(core, c.get(1)))));
    }
    if let Some(c) = RE_POLI.captures(t) {
        return Some(friendly(news::news_politics(core, count(core, c.get(1)))));
    }
    if t == "早报" {
        return Some(friendly(crate::digest::compose(core)));
    }

    // ---- 诗词 ----
    if let Some(c) = RE_POEM.captures(t) {
        return Some(friendly(poetry::recommend(
            core,
            c.get(1).map(|m| m.as_str()).unwrap_or(""),
        )));
    }

    // ---- 早报订阅 ----
    if let Some(c) = RE_SUB.captures(t) {
        return Some(subscribe_digest(core, c.get(1).map(|m| m.as_str())));
    }
    if matches!(t, "取消订阅" | "取消订阅早报" | "退订早报") {
        let mut store = core.store.lock().ok()?;
        return Some(if store.remove_digests() > 0 {
            "🔔 早报订阅已取消".to_string()
        } else {
            "你还没有订阅早报".to_string()
        });
    }

    // ---- 电台 ----
    if t == "电台" {
        return Some(crate::radio::stations_text());
    }
    if let Some(rest) = t.strip_prefix("电台搜索").or_else(|| t.strip_prefix("电台 搜索")) {
        let q = rest.trim();
        if q.is_empty() {
            return Some("🔍 用法:电台 搜索 <关键词>\n例:电台 搜索 爵士 / 电台 搜索 中国".to_string());
        }
        return Some(match crate::radio::search(core, q, 8) {
            Ok(hits) if !hits.is_empty() => {
                let mut lines = vec![format!("🔍 电台搜索「{q}」")];
                for (i, h) in hits.iter().enumerate() {
                    lines.push(format!("{}. {} —— {}", i + 1, h.name, h.desc));
                }
                lines.push("点快捷词条「电台」的卡片可直接播放".to_string());
                lines.join("\n")
            }
            Ok(_) => format!("没搜到与「{q}」相关的电台,换个关键词试试"),
            Err(e) => format!("😅 {}", e.msg()),
        });
    }

    // ---- 设置 ----
    if let Some(c) = RE_SETPOEM.captures(t) {
        let url = c.get(1).unwrap().as_str();
        {
            let store = core.store.lock().ok()?;
            let mut cur = store.kv_get("poem_urls").unwrap_or_default();
            if !cur.lines().any(|l| l.trim() == url) {
                if !cur.is_empty() {
                    cur.push('\n');
                }
                cur.push_str(url);
                store.kv_set("poem_urls", &cur);
            }
        }
        return Some(match poetry::refresh_pool(core) {
            Ok(Some(n)) => format!("✅ 诗词源已添加,并预取 {n} 首入缓存"),
            Ok(None) => "✅ 诗词源已添加(联网拉取失败,之后会自动重试)".to_string(),
            Err(reason) => format!("⚠️ 诗词源已添加,但预取未完成:{reason}(之后会自动重试)"),
        });
    }
    if RE_CLRPOEM.is_match(t) {
        {
            let store = core.store.lock().ok()?;
            store.kv_set("poem_urls", "");
            store.kv_set("poem_pool_cache", "");
        }
        return Some("✅ 诗词源已清除".to_string());
    }

    None
}

fn add_reminder(core: &Assistant, body: &str) -> String {
    if body.is_empty() || body == "帮助" || body == "?" {
        return USAGE.to_string();
    }
    let now = Local::now().naive_local();
    let Some(parsed) = parse_when(body, now) else {
        // 规则没看懂 → 可选的本地 LLM 兜底(未开启/未就绪时返回 USAGE)
        return crate::llm::fallback(core, body, now);
    };
    let text = if parsed.rest.trim().is_empty() {
        "到时间啦"
    } else {
        parsed.rest.trim()
    };
    let mut store = match core.store.lock() {
        Ok(s) => s,
        Err(_) => return "😅 存储忙,稍后再试".to_string(),
    };
    match store.add_reminder(text, "text", parsed.when, parsed.recur.as_deref()) {
        Ok(rec) => format!(
            "✅ 提醒已设置 [{}]\n⏰ {} {}\n📝 {}\n到点我来喊你;取消发送「取消提醒 {}」",
            rec.id,
            human_dt(&parsed.when),
            recur_label(parsed.recur.as_deref()),
            text,
            rec.id
        ),
        Err(e) => format!("😅 保存提醒失败:{}", e.0),
    }
}

fn subscribe_digest(core: &Assistant, raw: Option<&str>) -> String {
    let (h, mi) = match raw {
        Some(raw) => match parse_clock(raw) {
            Some((h, mi, _)) => (h, mi),
            None => return "⚠️ 时间没看懂,示例:订阅早报 07:30".to_string(),
        },
        None => (8, 0),
    };
    let now = Local::now().naive_local();
    let mut dt = now.date().and_hms_opt(h, mi, 0).unwrap_or(now);
    if dt <= now {
        dt += Duration::days(1);
    }
    let mut store = match core.store.lock() {
        Ok(s) => s,
        Err(_) => return "😅 存储忙,稍后再试".to_string(),
    };
    store.remove_digests();
    match store.add_reminder("早报", "digest", dt, Some("daily")) {
        Ok(rec) => format!(
            "🔔 早报订阅成功,每天 {:02}:{:02} 推送(天气+要闻+今日诗词)\n[编号 {},取消发送「取消订阅」]",
            h, mi, rec.id
        ),
        Err(e) => format!("😅 保存订阅失败:{}", e.0),
    }
}

// 保证 CoreError2 在本模块可见(供 store 转换)
#[allow(dead_code)]
fn _touch(_e: CoreError2) {}
