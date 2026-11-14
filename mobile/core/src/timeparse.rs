//! 中文时间表达解析(从经过验证的 Python 版 parse_time.py 移植,语义 1:1)
//!
//! 支持:18:00 / 18点 / 18点半 / 30分钟后 / 半分钟后 / 半小时后 /
//! 今天 12:00 / 明天 7:30 / 周五 20:00 / 每天 08:00 / 每周五 18:00 / 5月1日 09:00

use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use regex::Regex;

pub const WEEK_CN: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

static RE_CLOCK_HM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,2})\s*[:：]\s*(\d{1,2})\s*").unwrap());
static RE_CLOCK_HALF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,2})\s*点\s*半\s*").unwrap());
static RE_CLOCK_DIAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,2})\s*点\s*(\d{1,2})?\s*分?\s*").unwrap());
static RE_CLOCK_SHI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,2})\s*时\s*(\d{1,2})?\s*分?\s*").unwrap());
static RE_REL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(半|\d+(?:\.\d+)?)\s*个?\s*(小时|分钟|分|天|日|周|星期)\s*(?:之|以)?后\s*").unwrap());
static RE_DAILY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(每天|每日)\s*").unwrap());
static RE_WEEKLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^每\s*周\s*([一二三四五六日天])\s*").unwrap());
static RE_BAREWEEK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^周\s*([一二三四五六日天])\s*").unwrap());
static RE_DAYOFFSET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(今天|明天|后天)\s*").unwrap());
static RE_DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})\s*月\s*(\d{1,2})\s*[日号]\s*").unwrap());
// 时段前缀:晚上8点 → 20:00,中午12点 → 12:00,下午3点半 → 15:30
static RE_PERIOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(凌晨|早上|上午|清晨|中午|下午|傍晚|晚上|夜里|深夜)\s*").unwrap());
// 每月15号 10:00 交房租(recur=monthly:15)
static RE_MONTHLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^每月\s*(\d{1,2})\s*[日号]\s*").unwrap());
// 下周一 9:00 例会(严格"下一周"的那一天,一次性)
static RE_NEXTWEEK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^下周\s*([一二三四五六日天])\s*").unwrap());
// 月底 复盘(本月最后一天)
static RE_MONTHEND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^月底\s*").unwrap());
// 15号 交房租(本月该日,已过则下月)
static RE_BAREDAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})\s*[日号]\s*").unwrap());

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedWhen {
    pub when: NaiveDateTime,
    /// None=单次 | daily | weekly:<0-6>
    pub recur: Option<String>,
    /// 时间短语之后的剩余文本(即提醒事项)
    pub rest: String,
}

pub fn weekday_cn(dt: &NaiveDateTime) -> &'static str {
    WEEK_CN[dt.weekday().num_days_from_monday() as usize]
}

fn weekday_idx(s: &str) -> Option<u32> {
    match s {
        "一" => Some(0),
        "二" => Some(1),
        "三" => Some(2),
        "四" => Some(3),
        "五" => Some(4),
        "六" => Some(5),
        "日" | "天" => Some(6),
        _ => None,
    }
}

fn unit_seconds(unit: &str) -> Option<f64> {
    match unit {
        "小时" => Some(3600.0),
        "分钟" | "分" => Some(60.0),
        "天" | "日" => Some(86400.0),
        "周" | "星期" => Some(604800.0),
        _ => None,
    }
}

/// 时段前缀对小时数的修正(全部确定性规则,无任何猜测)
fn adjust_hour(period: Option<&str>, h: u32) -> u32 {
    match period {
        Some("中午") => {
            if (1..=6).contains(&h) { h + 12 } else { h } // 中午2点=14:00,中午12点=12:00
        }
        Some("下午" | "傍晚") => {
            if h < 12 { h + 12 } else { h } // 下午3点=15:00,下午12点=12:00
        }
        Some("晚上" | "夜里" | "深夜") => {
            if h < 12 { h + 12 } else if h == 12 { 0 } else { h } // 晚上8点=20:00,晚上12点=午夜0点
        }
        _ => h, // 凌晨/早上/上午/清晨:原样
    }
}

/// 从文本开头解析「时刻」,返回 (时, 分, 剩余文本);支持时段前缀
pub fn parse_clock(text: &str) -> Option<(u32, u32, &str)> {
    let (period, text) = match RE_PERIOD.captures(text) {
        Some(c) => (
            Some(c.get(1).unwrap().as_str()),
            &text[c.get(0).unwrap().end()..],
        ),
        None => (None, text),
    };
    let patterns: [&Regex; 4] = [&RE_CLOCK_HM, &RE_CLOCK_HALF, &RE_CLOCK_DIAN, &RE_CLOCK_SHI];
    for (i, re) in patterns.iter().enumerate() {
        if let Some(caps) = re.captures(text) {
            let h: u32 = caps.get(1)?.as_str().parse().ok()?;
            let minute: u32 = if i == 0 {
                caps.get(2)?.as_str().parse().ok()?
            } else if i == 1 {
                30
            } else {
                caps.get(2).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0)
            };
            let h = adjust_hour(period, h);
            if h > 23 || minute > 59 {
                return None;
            }
            return Some((h, minute, &text[caps.get(0).unwrap().end()..]));
        }
    }
    None
}

fn next_weekday_date(today: NaiveDate, wd: u32) -> NaiveDate {
    let delta = (wd + 7 - today.weekday().num_days_from_monday()) % 7;
    today + Duration::days(delta as i64)
}

pub(crate) fn next_recurring_dt(now: NaiveDateTime, recur: &str, h: u32, minute: u32) -> NaiveDateTime {
    if recur == "daily" {
        let mut dt = now.date().and_hms_opt(h, minute, 0).unwrap_or_else(|| now);
        if dt <= now {
            dt += Duration::days(1);
        }
        dt
    } else if let Some(wd) = recur.strip_prefix("weekly:").and_then(|s| s.parse::<u32>().ok()) {
        let d = next_weekday_date(now.date(), wd);
        let mut dt = d.and_hms_opt(h, minute, 0).unwrap_or_else(|| now);
        if dt <= now {
            dt += Duration::days(7);
        }
        dt
    } else {
        now
    }
}

// ---------------- 日期/月历工具(全部确定性) ----------------

fn last_day(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn next_month(y: i32, m: u32) -> (i32, u32) {
    if m == 12 {
        (y + 1, 1)
    } else {
        (y, m + 1)
    }
}

/// 「每月D号」首次触发:当月无此日(如2月30/31)则跳过该月;当月已过 → 下个月
pub(crate) fn first_monthly(now: NaiveDateTime, d: u32, h: u32, mi: u32) -> Option<NaiveDateTime> {
    if d == 0 || d > 31 {
        return None;
    }
    let (mut y, mut m) = (now.year(), now.month());
    for _ in 0..48 {
        if d <= last_day(y, m) {
            if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                let cand = date.and_hms_opt(h, mi, 0)?;
                if cand > now {
                    return Some(cand);
                }
            }
        }
        (y, m) = next_month(y, m);
    }
    None
}

/// 「下周X」:严格落在下一个自然周(周一为一周之始)
fn week_monday(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

fn next_week_exclusive(today: NaiveDate, wd: u32) -> NaiveDate {
    let mut d = next_weekday_date(today, wd);
    if week_monday(d) == week_monday(today) {
        d += Duration::days(7);
    }
    d
}

/// 「15号」:本月该日,已过则顺延下月;日不存在(如2月30)则继续向后找
fn bare_day_date(now: NaiveDateTime, d: u32) -> Option<NaiveDate> {
    if d == 0 || d > 31 {
        return None;
    }
    let (mut y, mut m) = (now.year(), now.month());
    for _ in 0..24 {
        if d <= last_day(y, m) {
            let cand = NaiveDate::from_ymd_opt(y, m, d)?;
            if cand >= now.date() {
                return Some(cand);
            }
        }
        (y, m) = next_month(y, m);
    }
    None
}

/// 解析提醒时间表达;now 由调用方传入(便于测试)
pub fn parse_when(text: &str, now: NaiveDateTime) -> Option<ParsedWhen> {
    let original = text.trim();
    if original.is_empty() {
        return None;
    }
    let mut rest = original;
    let mut base_date: Option<NaiveDate> = None;
    let mut recur: Option<String> = None;

    if let Some(caps) = RE_DAILY.captures(rest) {
        recur = Some("daily".into());
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_WEEKLY.captures(rest) {
        let wd = weekday_idx(caps.get(1)?.as_str())?;
        recur = Some(format!("weekly:{wd}"));
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_MONTHLY.captures(rest) {
        let d: u32 = caps.get(1)?.as_str().parse().ok()?;
        let after = &rest[caps.get(0).unwrap().end()..];
        // 时刻可选:缺省与“只有日期”规则一致,默认 09:00
        let (h, mi, rest2) = match parse_clock(after) {
            Some((h, mi, r2)) => (h, mi, r2),
            None => (9, 0, after),
        };
        let when = first_monthly(now, d, h, mi)?;
        return Some(ParsedWhen {
            when,
            recur: Some(format!("monthly:{d}")),
            rest: rest2.to_string(),
        });
    } else if let Some(caps) = RE_NEXTWEEK.captures(rest) {
        let wd = weekday_idx(caps.get(1)?.as_str())?;
        base_date = Some(next_week_exclusive(now.date(), wd));
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_DAYOFFSET.captures(rest) {
        let off = match caps.get(1).unwrap().as_str() {
            "今天" => 0,
            "明天" => 1,
            _ => 2,
        };
        base_date = Some((now + Duration::days(off)).date());
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_BAREWEEK.captures(rest) {
        let wd = weekday_idx(caps.get(1)?.as_str())?;
        base_date = Some(next_weekday_date(now.date(), wd));
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_DATE.captures(rest) {
        let mo: u32 = caps.get(1)?.as_str().parse().ok()?;
        let dy: u32 = caps.get(2)?.as_str().parse().ok()?;
        let mut d = NaiveDate::from_ymd_opt(now.year(), mo, dy)?;
        if d < now.date() {
            d = NaiveDate::from_ymd_opt(now.year() + 1, mo, dy)?;
        }
        base_date = Some(d);
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_MONTHEND.captures(rest) {
        base_date = Some(
            NaiveDate::from_ymd_opt(now.year(), now.month(), last_day(now.year(), now.month()))?,
        );
        rest = &rest[caps.get(0).unwrap().end()..];
    } else if let Some(caps) = RE_BAREDAY.captures(rest) {
        let end = caps.get(0).unwrap().end();
        // "3日后"是相对时间,不能被"3日"劫持:紧跟"后"时放弃本分支
        if !rest[end..].starts_with('后') {
            let d: u32 = caps.get(1)?.as_str().parse().ok()?;
            base_date = Some(bare_day_date(now, d)?);
            rest = &rest[end..];
        }
    }

    // 循环提醒:必须带时刻
    if let Some(r) = recur.clone() {
        let (h, mi, rest2) = parse_clock(rest)?;
        let when = next_recurring_dt(now, &r, h, mi);
        return Some(ParsedWhen {
            when,
            recur: Some(r),
            rest: rest2.to_string(),
        });
    }

    // 相对时间(仅在未指定日期时生效)
    if base_date.is_none() {
        if let Some(caps) = RE_REL.captures(rest) {
            let val: f64 = if caps.get(1).unwrap().as_str() == "半" {
                0.5
            } else {
                caps.get(1)?.as_str().parse().ok()?
            };
            let unit = caps.get(2)?.as_str();
            let secs = (val * unit_seconds(unit)?).round() as i64;
            if secs <= 0 {
                return None;
            }
            return Some(ParsedWhen {
                when: now + Duration::seconds(secs),
                recur: None,
                rest: rest[caps.get(0).unwrap().end()..].to_string(),
            });
        }
    }

    // 绝对时刻
    if let Some((h, mi, rest2)) = parse_clock(rest) {
        let d = base_date.unwrap_or_else(|| now.date());
        let mut when = d.and_hms_opt(h, mi, 0)?;
        if base_date.is_none() && when <= now {
            when += Duration::days(1); // 今天的时刻已过 → 明天
        }
        return Some(ParsedWhen {
            when,
            recur: None,
            rest: rest2.to_string(),
        });
    }

    // 只有日期没有时刻 → 默认 09:00
    if let Some(d) = base_date {
        return Some(ParsedWhen {
            when: d.and_hms_opt(9, 0, 0)?,
            recur: None,
            rest: rest.to_string(),
        });
    }
    None
}

/// 循环提醒触发后计算下一次;单次返回 None
pub fn next_after(prev: NaiveDateTime, recur: &str, now: NaiveDateTime) -> Option<NaiveDateTime> {
    match recur {
        "daily" => {
            let mut nxt = prev + Duration::days(1);
            while nxt <= now {
                nxt += Duration::days(1);
            }
            Some(nxt)
        }
        r if r.starts_with("weekly:") => {
            let mut nxt = prev + Duration::days(7);
            while nxt <= now {
                nxt += Duration::days(7);
            }
            Some(nxt)
        }
        r if r.starts_with("monthly:") => {
            let d: u32 = r["monthly:".len()..].parse().ok()?;
            let (h, mi) = (prev.hour(), prev.minute());
            // 从 prev 的下个月起逐月找(当月无此日则跳过,如2月无31号),直到 > now
            let (mut y, mut m) = (prev.year(), prev.month());
            for _ in 0..48 {
                (y, m) = next_month(y, m);
                if d <= last_day(y, m) {
                    if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                        let cand = date.and_hms_opt(h, mi, 0)?;
                        if cand > now {
                            return Some(cand);
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// 本地 NaiveDateTime → Unix 秒(按本地时区)
pub fn local_to_epoch(ndt: NaiveDateTime) -> i64 {
    use chrono::TimeZone;
    chrono::Local
        .from_local_datetime(&ndt)
        .single()
        .map(|d| d.timestamp())
        .unwrap_or_else(|| ndt.and_utc().timestamp())
}

/// Unix 秒 → 本地 NaiveDateTime
pub fn epoch_to_local(epoch: i64) -> NaiveDateTime {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|d| d.naive_local())
        .unwrap_or_else(|| chrono::Utc.timestamp_opt(epoch, 0).single().unwrap().naive_utc())
}

/// "11月04日(周二) 08:00"
pub fn human_dt(dt: &NaiveDateTime) -> String {
    format!("{}({}) {:02}:{:02}", dt.format("%m月%d日"), weekday_cn(dt), dt.hour(), dt.minute())
}

// ---------------- 测试(与 Python 版 selftest 同一批用例) ----------------

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 3).unwrap().and_hms_opt(10, 0, 0).unwrap() // 周二 10:00
    }

    fn ndt(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    #[test]
    fn relative_and_clock() {
        let b = base();
        for (text, when, recur, rest) in [
            ("30分钟后 喝水", b + Duration::minutes(30), None, "喝水"),
            ("半小时后 吃药", b + Duration::minutes(30), None, "吃药"),
            ("半分钟后 测试", b + Duration::seconds(30), None, "测试"),
            ("2小时后 开会", b + Duration::hours(2), None, "开会"),
            ("18:00 开会", ndt(2026, 11, 3, 18, 0), None, "开会"),
            ("18点半 吃饭", ndt(2026, 11, 3, 18, 30), None, "吃饭"),
            ("今天 12:00 午休", ndt(2026, 11, 3, 12, 0), None, "午休"),
            ("明天 7:30 起床", ndt(2026, 11, 4, 7, 30), None, "起床"),
            ("每天 08:00 早读", ndt(2026, 11, 4, 8, 0), Some("daily".into()), "早读"),
            ("每周五 18:00 交周报", ndt(2026, 11, 6, 18, 0), Some("weekly:4".into()), "交周报"),
            ("周五 20:00 看电影", ndt(2026, 11, 6, 20, 0), None, "看电影"),
            ("5月1日 09:00 出发", ndt(2027, 5, 1, 9, 0), None, "出发"),
            ("08:00 晨会", ndt(2026, 11, 4, 8, 0), None, "晨会"), // 已过 → 明天
            // ---- 时段前缀 ----
            ("晚上8点 跑步", ndt(2026, 11, 3, 20, 0), None, "跑步"),
            ("下午3点半 开会", ndt(2026, 11, 3, 15, 30), None, "开会"),
            ("中午12点 吃饭", ndt(2026, 11, 3, 12, 0), None, "吃饭"),
            ("中午2点 午休", ndt(2026, 11, 3, 14, 0), None, "午休"),
            ("晚上12点 睡觉", ndt(2026, 11, 4, 0, 0), None, "睡觉"), // 午夜0点,已过→明天
            // ---- 下周X / 每月X号 / X号 / 月底 ----
            ("下周一 9:00 例会", ndt(2026, 11, 9, 9, 0), None, "例会"),
            ("每月15号 10:00 交房租", ndt(2026, 11, 15, 10, 0), Some("monthly:15".into()), "交房租"),
            ("15号 交房租", ndt(2026, 11, 15, 9, 0), None, "交房租"),
            ("31号 聚会", ndt(2026, 12, 31, 9, 0), None, "聚会"), // 11月无31日 → 12月
            ("月底 复盘", ndt(2026, 11, 30, 9, 0), None, "复盘"),
            ("每月31号 存档", ndt(2026, 12, 31, 9, 0), Some("monthly:31".into()), "存档"),
            ("每周一 早上7点 晨读", ndt(2026, 11, 9, 7, 0), Some("weekly:0".into()), "晨读"),
        ] {
            let got = parse_when(text, b).unwrap_or_else(|| panic!("解析失败:{text}"));
            assert_eq!(got.when, when, "时间不符:{text} → {:?}", got.when);
            assert_eq!(got.recur, recur, "recur 不符:{text}");
            assert_eq!(got.rest.trim(), rest, "剩余文本不符:{text}");
        }
    }

    #[test]
    fn recurring_advance() {
        let b = base();
        let fired = ndt(2026, 11, 3, 8, 0);
        assert_eq!(next_after(fired, "daily", b).unwrap(), ndt(2026, 11, 4, 8, 0));
        let wk = ndt(2026, 11, 6, 18, 0);
        assert_eq!(next_after(wk, "weekly:4", b).unwrap(), ndt(2026, 11, 13, 18, 0));
        let mo = ndt(2026, 11, 15, 10, 0);
        assert_eq!(
            next_after(mo, "monthly:15", b).unwrap(),
            ndt(2026, 12, 15, 10, 0)
        );
        // 1月31 → 2月无31日,跳过 → 3月31(2027非闰年)
        let jan31 = ndt(2027, 1, 31, 9, 0);
        assert_eq!(
            next_after(jan31, "monthly:31", ndt(2027, 2, 1, 0, 0)).unwrap(),
            ndt(2027, 3, 31, 9, 0)
        );
        assert_eq!(next_after(fired, "", b), None);
    }

    #[test]
    fn human_readable() {
        assert_eq!(human_dt(&ndt(2026, 11, 4, 8, 5)), "11月04日(周三) 08:05");
    }
}
