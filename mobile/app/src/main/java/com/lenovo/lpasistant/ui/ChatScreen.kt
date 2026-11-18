package com.lenovo.lpasistant.ui

import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.lenovo.lpasistant.R
import com.lenovo.lpasistant.core.CoreBridge
import com.lenovo.lpasistant.core.RadioController
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.lpasistant_core.NewsItemRecord
import uniffi.lpasistant_core.RadioStationRecord
import uniffi.lpasistant_core.WeatherChart
import uniffi.lpasistant_core.ReminderDraft

// internal:MainActivity 提升消息列表时也要构造欢迎语与自增 id
internal const val WELCOME = """LPasistant(纯规则·不连AI)
点上方快捷词条或直接输入指令;
长按任意消息可复制;完整帮助在右上角「设置 → 使用帮助」"""

data class ChatAction(val label: String, val kind: String, val arg: String? = null, val iconRes: Int? = null)

data class ChatMsg(
    val id: Long,
    val isUser: Boolean,
    val text: String,
    val actions: List<ChatAction> = emptyList(),
    /** 非 null 时渲染为新闻卡片(点标题抓详情) */
    val news: List<NewsItemRecord>? = null,
    /** 非 null 时渲染为电台卡片(点 ▶ 播放) */
    val radio: List<RadioStationRecord>? = null,
    /** 非 null 时渲染为天气曲线卡(播报文本+24h 温度折线) */
    val weather: WeatherChart? = null
)

internal var msgSeq = 0L
internal fun nextMsgId() = ++msgSeq

private val QUICK_CHIPS = listOf(
    "提醒我",
    "天气",
    "新闻",
    "国际",
    "电台",
    "诗词",
    "喝水提醒",
    "工作提醒",
    "清空",
)

@Composable
fun ChatScreen(
    onOpenSettings: () -> Unit,
    onOpenReminders: () -> Unit,
    onOpenHelp: () -> Unit,
    // 消息列表由 MainActivity 持有:导航去设置/提醒管理/帮助回来不清,
    // 仅进程结束(用户手动结束或后台被系统回收)才随 Activity 重置
    msgs: SnapshotStateList<ChatMsg>,
) {
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    val focusManager = LocalFocusManager.current
    var input by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    val listState = rememberLazyListState()
    var showForm by remember { mutableStateOf(false) }
    var formDraft by remember { mutableStateOf<ReminderDraft?>(null) }
    // 长按菜单所属的消息 / 已进入“选择文本”模式的消息
    var menuMsgId by remember { mutableStateOf<Long?>(null) }
    var selectMsgId by remember { mutableStateOf<Long?>(null) }
    // 「清空」确认弹窗
    var showClear by remember { mutableStateOf(false) }

    fun append(text: String) = msgs.add(ChatMsg(nextMsgId(), false, text))

    fun copyAll(m: ChatMsg) {
        clipboard.setText(AnnotatedString(m.text))
        Toast.makeText(context, "已复制全部内容", Toast.LENGTH_SHORT).show()
        menuMsgId = null
    }

    fun startSelect(m: ChatMsg) {
        selectMsgId = m.id
        menuMsgId = null
        Toast.makeText(context, "已进入选择模式:长按文字拖动选区,可复制所选", Toast.LENGTH_LONG).show()
    }

    fun send(text: String) {
        val t = text.trim()
        if (t.isEmpty()) return
        msgs.add(ChatMsg(nextMsgId(), true, t))
        input = ""
        scope.launch {
            val (reply, actions) = withContext(Dispatchers.IO) {
                val r = CoreBridge.handle(t)
                val hasDraft = CoreBridge.pendingDraft() != null
                val acts = if (hasDraft) listOf(
                    ChatAction("确认", "confirm", iconRes = R.drawable.ic_check),
                    ChatAction("显示表单", "show_form", iconRes = R.drawable.ic_edit),
                    ChatAction("取消", "cancel_draft", iconRes = R.drawable.ic_close)
                ) else emptyList()
                r to acts
            }
            msgs.add(ChatMsg(nextMsgId(), false, reply, actions))
        }
    }

    fun onNewsDetail(item: NewsItemRecord) {
        msgs.add(ChatMsg(nextMsgId(), true, "详情:${item.title}"))
        scope.launch {
            append(withContext(Dispatchers.IO) { CoreBridge.newsDetail(item.url, item.title) })
        }
    }

    fun showNews(kind: String) {
        scope.launch {
            val list = withContext(Dispatchers.IO) { CoreBridge.newsList(kind, 8) }
            if (list.isEmpty()) {
                append("新闻源暂时都连不上,稍后再试试~")
            } else {
                msgs.add(ChatMsg(nextMsgId(), false, "", news = list))
            }
        }
    }

    fun showRadio() {
        scope.launch {
            val list = withContext(Dispatchers.IO) { CoreBridge.radioStations() }
            if (list.isEmpty()) {
                append("电台列表加载失败,稍后再试")
            } else {
                msgs.add(ChatMsg(nextMsgId(), false, "", radio = list))
            }
        }
    }

    /** 天气曲线卡:播报文本 + 24h 温度折线;city 空 = 自动定位(设置开关),失败回退默认城市 */
    fun showWeather(city: String = "") {
        scope.launch {
            val chart = withContext(Dispatchers.IO) { CoreBridge.weatherChart(city) }
            if (chart == null) {
                append("😅 天气获取失败,稍后再试")
            } else {
                msgs.add(ChatMsg(nextMsgId(), false, "", weather = chart))
            }
        }
    }

    fun onAction(a: ChatAction) {
        when (a.kind) {
            "confirm" -> scope.launch {
                append(withContext(Dispatchers.IO) { CoreBridge.confirmPending(context) })
            }
            "cancel_draft" -> scope.launch {
                append(withContext(Dispatchers.IO) { CoreBridge.cancelPending() })
            }
            "show_form" -> {
                formDraft = CoreBridge.pendingDraft()
                showForm = true
            }
            "delete_reminder" -> scope.launch {
                val id = a.arg ?: return@launch
                withContext(Dispatchers.IO) { CoreBridge.cancelReminder(context, id) }
                append("已取消,提醒管理里可查看剩余提醒")
            }
        }
    }

    LaunchedEffect(msgs.size) {
        if (msgs.isNotEmpty()) listState.animateScrollToItem(msgs.size - 1)
    }

    Column(Modifier.fillMaxSize()) {
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                "LPasistant",
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f)
            )
            Box {
                var navOpen by remember { mutableStateOf(false) }
                IconButton(onClick = { navOpen = !navOpen }) {
                    Icon(
                        painterResource(R.drawable.ic_tune),
                        contentDescription = "导航",
                        tint = MaterialTheme.colorScheme.onSurface
                    )
                }
                DropdownMenu(expanded = navOpen, onDismissRequest = { navOpen = false }) {
                    DropdownMenuItem(
                        text = { Text("⚙ 设置") },
                        onClick = { navOpen = false; onOpenSettings() }
                    )
                    DropdownMenuItem(
                        text = { Text("⏰ 提醒管理") },
                        onClick = { navOpen = false; onOpenReminders() }
                    )
                    DropdownMenuItem(
                        text = { Text("📖 使用帮助") },
                        onClick = { navOpen = false; onOpenHelp() }
                    )
                }
            }
        }
        HorizontalDivider()
        // 点消息区(空白处)收起键盘:输入框回落,再点输入框唤起
        val listPress = rememberPress()
        LazyColumn(
            state = listState,
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .clickable(interactionSource = listPress, indication = null) {
                    keyboard?.hide()
                    focusManager.clearFocus()
                },
            contentPadding = PaddingValues(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            items(msgs, key = { it.id }) { m ->
                when {
                    m.news != null -> NewsCard(m.news, onDetail = ::onNewsDetail)
                    m.radio != null -> RadioCard(m.radio)
                    m.weather != null -> WeatherCard(m.weather)
                    else -> Bubble(
                        m = m,
                        menuOpen = menuMsgId == m.id,
                        selectMode = selectMsgId == m.id,
                        onMenuOpen = { menuMsgId = m.id },
                        onMenuDismiss = { if (menuMsgId == m.id) menuMsgId = null },
                        onCopyAll = { copyAll(m) },
                        onSelectText = { startSelect(m) },
                        onAction = ::onAction
                    )
                }
            }
        }
        HorizontalDivider()
        // 迷你播放条(电台播放中显示,用辅助色点睛)
        RadioController.nowPlaying?.let { name ->
            Surface(
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    Modifier.padding(horizontal = 14.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        painterResource(R.drawable.ic_headphones),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.size(16.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        name,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.weight(1f)
                    )
                    TextButton(onClick = { RadioController.stop() }) {
                        Icon(
                            painterResource(R.drawable.ic_stop),
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSecondaryContainer,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(Modifier.width(4.dp))
                        Text("停止", color = MaterialTheme.colorScheme.onSecondaryContainer)
                    }
                }
            }
        }
        // 快捷词条:白底描边胶囊气泡(设计语言:白底黑标),按下缩放手感
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 10.dp, vertical = 4.dp)
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            QUICK_CHIPS.forEach { label ->
                val press = rememberPress()
                Surface(
                    shape = RoundedCornerShape(percent = 50),
                    color = MaterialTheme.colorScheme.surface,
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                    modifier = Modifier
                        .pressScale(press, pressedScale = 0.93f)
                        .clickable(interactionSource = press, indication = null) {
                            when (label) {
                                // 点「提醒我」直接进表单:填事项、选时间,死规则直连
                                "提醒我" -> {
                                    formDraft = null
                                    showForm = true
                                }
                                "新闻" -> showNews("all")
                                "国际" -> showNews("intl")
                                "电台" -> showRadio()
                                "天气" -> showWeather()
                                "清空" -> showClear = true
                                else -> send(label)
                            }
                        }
                ) {
                    Text(
                        label,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.padding(horizontal = 13.dp, vertical = 7.dp)
                    )
                }
            }
        }
        // 输入行:与快捷词条同款的描边胶囊(BasicTextField,提示/文字垂直居中)
        // 发送键 = 同款压扁胶囊(压扁圆),整体高度 42dp
        Row(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            BasicTextField(
                value = input,
                onValueChange = { input = it },
                modifier = Modifier
                    .weight(1f)
                    .height(42.dp)
                    .border(1.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(percent = 50))
                    .padding(horizontal = 16.dp),
                textStyle = MaterialTheme.typography.bodyMedium.copy(
                    color = MaterialTheme.colorScheme.onSurface
                ),
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                singleLine = true,
                decorationBox = { inner ->
                    Box(Modifier.fillMaxHeight(), contentAlignment = Alignment.CenterStart) {
                        if (input.isEmpty()) {
                            Text(
                                "输入指令,如:提醒我 30分钟后 喝水",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1
                            )
                        }
                        inner()
                    }
                }
            )
            Spacer(Modifier.width(8.dp))
            val sendPress = rememberPress()
            Surface(
                shape = RoundedCornerShape(percent = 50),
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .height(42.dp)
                    .pressScale(sendPress)
                    .clickable(interactionSource = sendPress, indication = null) { send(input) }
            ) {
                Box(
                    Modifier
                        .fillMaxHeight()
                        .padding(horizontal = 20.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text("发送", color = MaterialTheme.colorScheme.onPrimary)
                }
            }
        }
    }

    if (showForm) {
        ReminderFormDialog(
            prefill = formDraft,
            onDismiss = { showForm = false },
            onSave = { text, epochSec, recur ->
                showForm = false
                scope.launch {
                    append(withContext(Dispatchers.IO) {
                        CoreBridge.saveReminderDirect(context, text, epochSec, recur)
                    })
                }
            }
        )
    }

    if (showClear) {
        AlertDialog(
            onDismissRequest = { showClear = false },
            title = { Text("清空聊天记录?") },
            text = { Text("仅清空本页显示;已设置的提醒不受影响,到点仍会响。") },
            confirmButton = {
                TextButton(onClick = {
                    showClear = false
                    msgs.clear()
                    msgs.add(ChatMsg(nextMsgId(), false, WELCOME))
                    Toast.makeText(context, "已清空", Toast.LENGTH_SHORT).show()
                }) { Text("清空") }
            },
            dismissButton = {
                TextButton(onClick = { showClear = false }) { Text("取消") }
            }
        )
    }
}

/** 聊天气泡:长按 → 菜单[复制全部 / 选择文本];选择模式下支持拖动选区复制 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun Bubble(
    m: ChatMsg,
    menuOpen: Boolean,
    selectMode: Boolean,
    onMenuOpen: () -> Unit,
    onMenuDismiss: () -> Unit,
    onCopyAll: () -> Unit,
    onSelectText: () -> Unit,
    onAction: (ChatAction) -> Unit
) {
    val context = LocalContext.current
    val haptic = LocalHapticFeedback.current
    val press = rememberPress()
    // 聊天风格:气泡最大宽度 86%——软件输出靠左、右侧留白;用户消息靠右、左侧留白
    val maxBubbleWidth = (LocalConfiguration.current.screenWidthDp * 0.86f).dp
    Column(
        Modifier.fillMaxWidth(),
        horizontalAlignment = if (m.isUser) Alignment.End else Alignment.Start
    ) {
        Surface(
            color = if (m.isUser) MaterialTheme.colorScheme.primaryContainer
            else MaterialTheme.colorScheme.surfaceVariant,
            shape = RoundedCornerShape(14.dp),
            tonalElevation = 1.dp,
            modifier = Modifier
                .widthIn(max = maxBubbleWidth)
                .pressScale(press, pressedScale = 0.985f)
        ) {
            val textModifier = Modifier
                .padding(10.dp)
                .combinedClickable(
                    interactionSource = press,
                    indication = null,
                    onClick = {},
                    onLongClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        onMenuOpen()
                    }
                )
            if (selectMode) {
                // 选择文本模式:原生选区(长按拖动,工具栏可复制所选)
                SelectionContainer {
                    Text(m.text, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(10.dp))
                }
            } else {
                Text(
                    m.text,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = textModifier
                )
                DropdownMenu(expanded = menuOpen, onDismissRequest = onMenuDismiss) {
                    DropdownMenuItem(
                        text = { Text("复制全部") },
                        leadingIcon = { Icon(painterResource(R.drawable.ic_copy), contentDescription = null) },
                        onClick = {
                            clipboardCopy(context, m.text)
                            onMenuDismiss()
                        }
                    )
                    DropdownMenuItem(
                        text = { Text("选择文本") },
                        leadingIcon = { Icon(painterResource(R.drawable.ic_textselect), contentDescription = null) },
                        onClick = {
                            onMenuDismiss()
                            onSelectText()
                        }
                    )
                }
            }
        }
        if (m.actions.isNotEmpty()) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                modifier = Modifier.padding(top = 2.dp)
            ) {
                m.actions.forEach { a ->
                    TextButton(onClick = { onAction(a) }) {
                        a.iconRes?.let {
                            Icon(painterResource(it), contentDescription = null, modifier = Modifier.size(16.dp))
                            Spacer(Modifier.width(4.dp))
                        }
                        Text(a.label)
                    }
                }
            }
        }
    }
}

private fun clipboardCopy(context: Context, text: String) {
    val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
    cm.setPrimaryClip(android.content.ClipData.newPlainText("assistant", text))
    Toast.makeText(context, "已复制全部内容", Toast.LENGTH_SHORT).show()
}

/** 电台卡片:点 ▶ 播放(再点 ⏹ 停止);迷你播放条见上方 */
@Composable
private fun RadioCard(items: List<RadioStationRecord>) {
    val context = LocalContext.current
    Surface(
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = RoundedCornerShape(14.dp),
        tonalElevation = 1.dp,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(Modifier.padding(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    painterResource(R.drawable.ic_radio),
                    contentDescription = null,
                    modifier = Modifier.size(15.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.width(5.dp))
                Text("电台(点右侧键播放)", style = MaterialTheme.typography.labelLarge)
            }
            items.forEach { s ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(top = 8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(s.name, style = MaterialTheme.typography.bodyMedium)
                        Text(
                            "${s.desc} · ${s.region}",
                            style = MaterialTheme.typography.bodySmall
                        )
                    }
                    TextButton(onClick = { RadioController.toggle(context, s.name, s.url) }) {
                        val playing = RadioController.nowPlaying == s.name
                        Icon(
                            painterResource(if (playing) R.drawable.ic_stop else R.drawable.ic_play),
                            contentDescription = if (playing) "停止" else "播放",
                            tint = if (playing) MaterialTheme.colorScheme.secondary else Color.Unspecified,
                            modifier = Modifier.size(18.dp)
                        )
                    }
                }
            }
        }
    }
}

/** 新闻卡片:点标题 → 抓原文+提取正文 → AI摘要(可选)/规则要点;离线缓存时标题带时间 */
@Composable
private fun NewsCard(items: List<NewsItemRecord>, onDetail: (NewsItemRecord) -> Unit) {
    val cachedAt = items.firstOrNull()?.cachedAt ?: 0L
    val header = if (cachedAt > 0) {
        val t = java.time.Instant.ofEpochSecond(cachedAt)
            .atZone(java.time.ZoneId.systemDefault())
            .format(java.time.format.DateTimeFormatter.ofPattern("HH:mm"))
        "要闻(离线缓存 $t,点标题看详情)"
    } else {
        "要闻(点标题看详情)"
    }
    Surface(
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = RoundedCornerShape(14.dp),
        tonalElevation = 1.dp,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(Modifier.padding(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    painterResource(R.drawable.ic_news),
                    contentDescription = null,
                    modifier = Modifier.size(15.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.width(5.dp))
                Text(header, style = MaterialTheme.typography.labelLarge)
            }
            items.forEach { it2 ->
                val press = rememberPress()
                Column(
                    Modifier
                        .pressScale(press, pressedScale = 0.98f)
                        .fillMaxWidth()
                        .clickable(interactionSource = press, indication = null) { onDetail(it2) }
                        .padding(top = 8.dp)
                ) {
                    Text(
                        it2.source,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary
                    )
                    Text(it2.title, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}
