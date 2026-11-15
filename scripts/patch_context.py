#!/usr/bin/env python
import io

p = r"C:/qq-bot/mobile/app/src/main/java/com/lenovo/lpasistant/ui/SettingsScreen.kt"
src = io.open(p, encoding="utf-8").read()

# 1. 删除原位置的 context 声明
old = (
    "            // ---- 天气默认城市 ----\n"
    "            val context = LocalContext.current\n"
    "            OutlinedTextField("
)
new = "            // ---- 天气默认城市 ----\n            OutlinedTextField("
assert old in src, "old ctx anchor"
src = src.replace(old, new, 1)

# 2. 提到状态声明区
old2 = (
    '    var newsFeeds by remember { mutableStateOf("") }\n'
    '    var radioFeeds by remember { mutableStateOf("") }'
)
new2 = (
    '    var newsFeeds by remember { mutableStateOf("") }\n'
    '    var radioFeeds by remember { mutableStateOf("") }\n'
    "    val context = LocalContext.current"
)
assert old2 in src, "new ctx anchor"
src = src.replace(old2, new2, 1)

io.open(p, "w", encoding="utf-8", newline="").write(src)
print("context moved OK")
