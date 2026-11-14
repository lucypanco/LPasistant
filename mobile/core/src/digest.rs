//! 早报:天气 + 要闻 + 今日诗词,组合成一条完整推送

use chrono::Local;

use crate::api::{Assistant, CoreError};
use crate::timeparse::weekday_cn;
use crate::{news, poetry, weather};

pub fn compose(core: &Assistant) -> Result<String, CoreError> {
    let now = Local::now().naive_local();
    let mut parts = vec![format!(
        "🌅 早报 | {} {}",
        now.format("%m月%d日"),
        weekday_cn(&now)
    )];

    match weather::get_weather(core, "") {
        Ok(w) => parts.push(w),
        Err(e) => parts.push(format!("🌤 天气获取失败({e})")),
    }

    match news::news_all(core, 5) {
        Ok(s) => parts.push(s),
        Err(e) => parts.push(format!("📰 新闻获取失败({e})")),
    }

    parts.push(poetry::recommend(core, "今日")?);
    Ok(parts.join("\n\n"))
}
