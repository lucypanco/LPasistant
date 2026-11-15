#!/usr/bin/env python
import io

# 1. llm.rs: parse_segment 恢复 Option ?(它返回 Option,? 是对的)
p = r"C:/qq-bot/mobile/core/src/llm.rs"
src = io.open(p, encoding="utf-8").read()

old1 = (
    "pub fn parse_segment(out: &str) -> Option<Segmentation> {\n"
    "    let start = out.find('{').ok_or(\"输出里没有JSON\")?;\n"
    "    let end = out.rfind('}').ok_or(\"输出里没有JSON\")?;"
)
new1 = (
    "pub fn parse_segment(out: &str) -> Option<Segmentation> {\n"
    "    let start = out.find('{')?;\n"
    "    let end = out.rfind('}')?;"
)
if old1 in src:
    src = src.replace(old1, new1)
    print("parse_segment 恢复 ✓")

# 2. llm.rs: 恢复 PendingReminder 结构
old3 = "use crate::timeparse::{human_dt, parse_when};"
new3 = (
    "use crate::timeparse::{human_dt, parse_when};\n"
    "\n"
    "/// 待用户「确认」的提醒草稿(确认键/表单消费;存于 Assistant.pending)\n"
    "#[derive(Debug, Clone)]\n"
    "pub struct PendingReminder {\n"
    "    pub text: String,\n"
    "    pub when: NaiveDateTime,\n"
    "    pub recur: Option<String>,\n"
    "}"
)
if "pub struct PendingReminder" not in src:
    assert old3 in src, "E3"
    src = src.replace(old3, new3)
    print("PendingReminder 恢复 ✓")

# 3. llm.rs: via_segment 落 pending
old4 = (
    "        if let Some(parsed) = parse_when(&seg.time, now) {\n"
    "            let text = if seg.event.is_empty() { \"到时间啦\" } else { seg.event.as_str() };\n"
    "            return draft_reply(text, &parsed.when, parsed.recur.as_deref());\n"
    "        }"
)
new4 = (
    "        if let Some(parsed) = parse_when(&seg.time, now) {\n"
    "            let text = if seg.event.is_empty() { \"到时间啦\" } else { seg.event.as_str() };\n"
    "            if let Ok(mut g) = core.pending.lock() {\n"
    "                *g = Some(PendingReminder {\n"
    "                    text: text.to_string(),\n"
    "                    when: parsed.when,\n"
    "                    recur: parsed.recur,\n"
    "                });\n"
    "            }\n"
    "            return draft_reply(text, &parsed.when, parsed.recur.as_deref());\n"
    "        }"
)
if old4 in src:
    src = src.replace(old4, new4)
    print("via_segment 落 pending ✓")

io.open(p, "w", encoding="utf-8", newline="").write(src)

# 4. api.rs: 恢复 pending 字段
p2 = r"C:/qq-bot/mobile/core/src/api.rs"
src2 = io.open(p2, encoding="utf-8").read()
old5 = "    pub(crate) llm_cb: Mutex<Option<Box<dyn LlmInfer>>>,"
new5 = (
    "    pub(crate) pending: Mutex<Option<crate::llm::PendingReminder>>,\n"
    "    pub(crate) llm_cb: Mutex<Option<Box<dyn LlmInfer>>>,"
)
if old5 in src2 and "pending: Mutex" not in src2:
    src2 = src2.replace(old5, new5)
old6 = "            llm_cb: Mutex::new(None),"
if old6 in src2 and "pending: Mutex::new(None)" not in src2:
    src2 = src2.replace(old6, "            pending: Mutex::new(None),\n" + old6)
io.open(p2, "w", encoding="utf-8", newline="").write(src2)
print("api.rs pending 字段 ✓")
