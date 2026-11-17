#!/usr/bin/env python
import io

p = r"C:/qq-bot/mobile/core/src/llm.rs"
src = io.open(p, encoding="utf-8").read()

# 找到第二次出现的重复块起点(宽兜底区内的旧定义)
marker = "// ---------- 宽兜底(全句路由;reminder 走分割+规格) ----------"
i1 = src.find(marker)
assert i1 >= 0, "宽兜底区未找到"
dup_start = src.find("/// 分割输出解析(容忍围栏与废话)", i1)
assert dup_start >= 0, "重复块起点未找到"
dup_end = src.find("pub(crate) fn wide_enabled", dup_start)
assert dup_end >= 0, "重复块终点未找到"
removed = src[dup_start:dup_end]
src = src[:dup_start] + src[dup_end:]

# 校验:每个重复名只出现一次定义
for name in ("pub struct Segmentation", "pub enum WideRoute", "pub fn build_wide_prompt", "pub fn parse_segment", "fn clip_field", "fn json_u32"):
    cnt = src.count(name)
    assert cnt == 1, f"{name} 出现 {cnt} 次"
io.open(p, "w", encoding="utf-8", newline="").write(src)
print(f"重复块已删({len(removed)} 字符),全文件各定义唯一 ✓")
