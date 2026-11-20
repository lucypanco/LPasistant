package com.lenovo.lpasistant.ui

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.lenovo.lpasistant.R
import com.lenovo.lpasistant.core.CoreBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.lpasistant_core.ReminderDraft
import uniffi.lpasistant_core.ReminderRecord
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale

private fun recurLabel(recur: String): String = when {
    recur == "daily" -> "(每天)"
    recur.startsWith("weekly:") -> "(每周)"
    recur.startsWith("monthly:") -> "(每月)"
    else -> ""
}

private fun fmtTime(epochSec: Long): String {
    val cal = Calendar.getInstance().apply { timeInMillis = epochSec * 1000 }
    return SimpleDateFormat("MM月dd日(E) HH:mm", Locale.CHINESE).format(cal.time)
}

/** 次级页:所有已设置提醒集中在此查看/取消/新建 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReminderListScreen(ctx: Context, onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<ReminderRecord>>(emptyList()) }
    var showForm by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) { items = CoreBridge.reminders() }
    }

    // 容器透明:顶栏透出根级背景,与主体同一条渐变进度线(同 SettingsScreen)
    Scaffold(
        containerColor = Color.Transparent,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent),
                title = { Text("提醒管理") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(painterResource(R.drawable.ic_back), contentDescription = "返回")
                    }
                }
            )
        }
    ) { pad ->
        Column(
            Modifier
                .padding(pad)
                .fillMaxSize()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("已设置 ${items.size} 条", style = MaterialTheme.typography.titleMedium)
                TextButton(onClick = { showForm = true }) {
                    Icon(
                        painterResource(R.drawable.ic_add),
                        contentDescription = null,
                        modifier = Modifier.size(16.dp)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text("新建")
                }
            }

            if (items.isEmpty()) {
                Text(
                    "还没有提醒。\n· 在聊天页发送指令,如「提醒我 30分钟后 喝水」\n· 或点右上「新建」用表单直接创建",
                    style = MaterialTheme.typography.bodySmall
                )
            }

            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(items) { r ->
                    Surface(
                        color = MaterialTheme.colorScheme.surfaceVariant,
                        shape = MaterialTheme.shapes.medium,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .padding(10.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(
                                    "[${r.id}] ${fmtTime(r.nextRun)} ${recurLabel(r.recur)}" +
                                        if (r.mode == "alarm") " ⏰闹钟" else "",
                                    style = MaterialTheme.typography.bodySmall
                                )
                                Text(r.text, style = MaterialTheme.typography.bodyMedium)
                            }
                            TextButton(onClick = {
                                scope.launch {
                                    withContext(Dispatchers.IO) { CoreBridge.cancelReminder(ctx, r.id) }
                                    withContext(Dispatchers.IO) { items = CoreBridge.reminders() }
                                }
                            }) { Text("取消") }
                        }
                    }
                }
            }
        }
    }

    if (showForm) {
        ReminderFormDialog(
            prefill = null as ReminderDraft?,
            onDismiss = { showForm = false },
            onSave = { text, epochSec, recur, mode ->
                showForm = false
                scope.launch {
                    withContext(Dispatchers.IO) { CoreBridge.saveReminderDirect(ctx, text, epochSec, recur, mode) }
                    withContext(Dispatchers.IO) { items = CoreBridge.reminders() }
                }
            }
        )
    }
}
