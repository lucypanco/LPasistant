#!/usr/bin/env python
# SettingsScreen: 新闻/电台自定义源改为「添加即自检 + 勾选列表」UI
import io

p = r"C:/qq-bot/mobile/app/src/main/java/com/lenovo/lpasistant/ui/SettingsScreen.kt"
src = io.open(p, encoding="utf-8").read()

# 1. 状态区:替换 newsFeeds/radioFeeds 文本态为列表态 + 添加态
old = (
    '    var newsFeeds by remember { mutableStateOf("") }\n'
    '    var radioFeeds by remember { mutableStateOf("") }\n'
)
new = (
    "    val newsList = remember { mutableStateListOf<uniffi.lpasistant_core.UserFeed>() }\n"
    "    val radioList = remember { mutableStateListOf<uniffi.lpasistant_core.UserStation>() }\n"
    '    var newsUrl by remember { mutableStateOf("") }\n'
    '    var newsProbe by remember { mutableStateOf("") }\n'
    '    var radioName by remember { mutableStateOf("") }\n'
    '    var radioUrl by remember { mutableStateOf("") }\n'
    '    var radioProbe by remember { mutableStateOf("") }\n'
)
assert old in src, "状态锚点"
src = src.replace(old, new, 1)

# 2. LaunchedEffect 读 kv → 改读列表 API
old2 = '            newsFeeds = CoreBridge.getSetting("news_user_feeds")\n            radioFeeds = CoreBridge.getSetting("radio_user_stations")'
new2 = (
    "            newsList.clear(); newsList.addAll(CoreBridge.newsUserList())\n"
    "            radioList.clear(); radioList.addAll(CoreBridge.radioUserList())"
)
assert old2 in src, "load 锚点"
src = src.replace(old2, new2, 1)

# 3. 新闻区块:text field → 添加行+探针结果+列表
old3 = '''            // ---- 新闻自定义源 ----
            Text("新闻自定义源", style = MaterialTheme.typography.titleMedium)
            OutlinedTextField(
                value = newsFeeds,
                onValueChange = { newsFeeds = it },
                label = { Text("RSS 地址(每行一个,最多 5 个,与内置源合并抓取)") },
                minLines = 2,
                modifier = Modifier.fillMaxWidth(),
                trailingIcon = {
                    TextButton(onClick = {
                        CoreBridge.setSetting("news_user_feeds", newsFeeds)
                        Toast.makeText(context, "已保存", Toast.LENGTH_SHORT).show()
                    }) { Text("保存") }
                }
            )
            HorizontalDivider()'''
new3 = '''            // ---- 新闻自定义源(添加即自检;勾选参与抓取)----
            Text("新闻自定义源", style = MaterialTheme.typography.titleMedium)
            newsList.forEach { fd ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = fd.enabled, onCheckedChange = { en ->
                        scope.launch {
                            CoreBridge.newsUserSetEnabled(fd.url, en)
                            newsList.clear(); newsList.addAll(CoreBridge.newsUserList())
                        }
                    })
                    Column(Modifier.weight(1f)) {
                        Text(fd.title.ifBlank { fd.url }, style = MaterialTheme.typography.bodySmall, maxLines = 1)
                        Text(
                            when (fd.ok) {
                                true -> "✓ 已连通"
                                false -> "✗ ${fd.err}"
                                null -> "未测"
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = if (fd.ok == true) MaterialTheme.colorScheme.primary
                                    else MaterialTheme.colorScheme.error
                        )
                    }
                    TextButton(onClick = {
                        scope.launch {
                            CoreBridge.newsUserRemove(fd.url)
                            newsList.clear(); newsList.addAll(CoreBridge.newsUserList())
                        }
                    }) { Text("删") }
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = newsUrl,
                    onValueChange = { newsUrl = it },
                    label = { Text("RSS 地址(最多 5 个)") },
                    singleLine = true,
                    modifier = Modifier.weight(1f)
                )
                TextButton(onClick = {
                    if (newsUrl.isNotBlank()) {
                        scope.launch {
                            val fd = CoreBridge.newsUserAdd(newsUrl.trim())
                            if (fd != null) {
                                newsProbe = if (fd.ok == true) "✓ 已添加并连通" else "✗ ${fd.err}"
                                newsUrl = ""
                                newsList.clear(); newsList.addAll(CoreBridge.newsUserList())
                            } else {
                                newsProbe = "✗ 操作失败"
                            }
                        }
                    }
                }) { Text("添加") }
            }
            if (newsProbe.isNotBlank()) {
                Text(
                    newsProbe,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (newsProbe.startsWith("✓")) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.error
                )
            }
            HorizontalDivider()'''
assert old3 in src, "新闻区块锚点"
src = src.replace(old3, new3, 1)

# 4. 电台区块:同款列表 + 添加行(名称/地址)+ 探针结果
old4 = '''            // ---- 电台自定义源 ----
            Text("电台自定义源", style = MaterialTheme.typography.titleMedium)
            OutlinedTextField(
                value = radioFeeds,
                onValueChange = { radioFeeds = it },
                label = { Text("每行「名称|流地址」(追加在内置台之后,最多 20 个)") },
                minLines = 2,
                modifier = Modifier.fillMaxWidth(),
                trailingIcon = {
                    TextButton(onClick = {
                        CoreBridge.setSetting("radio_user_stations", radioFeeds)
                        Toast.makeText(context, "已保存", Toast.LENGTH_SHORT).show()
                    }) { Text("保存") }
                }
            )
            HorizontalDivider()'''
new4 = '''            // ---- 电台自定义源(添加即自检;勾选参与列表;点▶试听)----
            Text("电台自定义源", style = MaterialTheme.typography.titleMedium)
            radioList.forEach { st ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = st.enabled, onCheckedChange = { en ->
                        scope.launch {
                            CoreBridge.radioUserSetEnabled(st.url, en)
                            radioList.clear(); radioList.addAll(CoreBridge.radioUserList())
                        }
                    })
                    Column(Modifier.weight(1f)) {
                        Text(st.name, style = MaterialTheme.typography.bodySmall, maxLines = 1)
                        Text(
                            when (st.ok) {
                                true -> "✓ 已连通"
                                false -> "✗ ${st.err}"
                                null -> "未测"
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = if (st.ok == true) MaterialTheme.colorScheme.primary
                                    else MaterialTheme.colorScheme.error
                        )
                    }
                    IconButton(onClick = { RadioController.play(context, st.name, st.url) }) {
                        Icon(painterResource(R.drawable.ic_play), contentDescription = "试听")
                    }
                    TextButton(onClick = {
                        scope.launch {
                            CoreBridge.radioUserRemove(st.url)
                            radioList.clear(); radioList.addAll(CoreBridge.radioUserList())
                        }
                    }) { Text("删") }
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = radioName,
                    onValueChange = { radioName = it },
                    label = { Text("名称") },
                    singleLine = true,
                    modifier = Modifier.weight(1f)
                )
                Spacer(Modifier.width(6.dp))
                OutlinedTextField(
                    value = radioUrl,
                    onValueChange = { radioUrl = it },
                    label = { Text("流地址") },
                    singleLine = true,
                    modifier = Modifier.weight(1.2f)
                )
                TextButton(onClick = {
                    if (radioName.isNotBlank() && radioUrl.isNotBlank()) {
                        scope.launch {
                            val st = CoreBridge.radioUserAdd(radioName.trim(), radioUrl.trim())
                            if (st != null) {
                                radioProbe = if (st.ok == true) "✓ 已添加并连通" else "✗ ${st.err}"
                                radioName = ""; radioUrl = ""
                                radioList.clear(); radioList.addAll(CoreBridge.radioUserList())
                            } else {
                                radioProbe = "✗ 操作失败"
                            }
                        }
                    }
                }) { Text("添加") }
            }
            if (radioProbe.isNotBlank()) {
                Text(
                    radioProbe,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (radioProbe.startsWith("✓")) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.error
                )
            }
            HorizontalDivider()'''
assert old4 in src, "电台区块锚点"
src = src.replace(old4, new4, 1)

io.open(p, "w", encoding="utf-8", newline="").write(src)
print("SettingsScreen 源列表 UI 完成 ✓")
