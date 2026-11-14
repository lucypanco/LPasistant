package com.lenovo.lpasistant.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.lpasistant_core.ReminderDraft
import java.util.Calendar

/**
 * 提醒表单:「死规则直连」——字段就是最终值,保存走 save_reminder_direct,
 * 不经过任何文本解析;日期算术只发生在本地日历代码里。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReminderFormDialog(
    prefill: ReminderDraft?,
    onDismiss: () -> Unit,
    onSave: (text: String, epochSec: Long, recur: String) -> Unit
) {
    val cal = Calendar.getInstance().apply {
        prefill?.let { timeInMillis = it.nextRun * 1000 }
    }
    var text by remember { mutableStateOf(prefill?.text ?: "") }
    var year by remember { mutableStateOf(cal.get(Calendar.YEAR)) }
    var month by remember { mutableStateOf(cal.get(Calendar.MONTH)) } // 0-based
    var day by remember { mutableStateOf(cal.get(Calendar.DAY_OF_MONTH)) }
    var hour by remember { mutableStateOf(cal.get(Calendar.HOUR_OF_DAY)) }
    var minute by remember { mutableStateOf(cal.get(Calendar.MINUTE)) }

    val initialMode = when {
        prefill?.recur?.startsWith("weekly") == true -> "每周"
        prefill?.recur?.startsWith("monthly") == true -> "每月"
        prefill?.recur == "daily" -> "每天"
        else -> "单次"
    }
    var mode by remember { mutableStateOf(initialMode) }
    var weekday by remember {
        mutableStateOf((cal.get(Calendar.DAY_OF_WEEK) + 5) % 7) // 0=周一
    }

    var showDate by remember { mutableStateOf(false) }
    var showTime by remember { mutableStateOf(false) }
    var repeatExpanded by remember { mutableStateOf(false) }
    var weekdayExpanded by remember { mutableStateOf(false) }

    val weekdays = listOf("周一", "周二", "周三", "周四", "周五", "周六", "周日")
    val dateStr = "%04d-%02d-%02d".format(year, month + 1, day)
    val timeStr = "%02d:%02d".format(hour, minute)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("提醒表单") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    label = { Text("事项(留空则为「到时间啦」)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { showDate = true }, modifier = Modifier.weight(1f)) {
                        Text(dateStr)
                    }
                    OutlinedButton(onClick = { showTime = true }, modifier = Modifier.weight(1f)) {
                        Text(timeStr)
                    }
                }
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("重复", style = MaterialTheme.typography.labelLarge)
                    Box {
                        OutlinedButton(onClick = { repeatExpanded = true }) { Text(mode) }
                        DropdownMenu(
                            expanded = repeatExpanded,
                            onDismissRequest = { repeatExpanded = false }
                        ) {
                            listOf("单次", "每天", "每周", "每月").forEach { m ->
                                DropdownMenuItem(
                                    text = { Text(m) },
                                    onClick = { mode = m; repeatExpanded = false }
                                )
                            }
                        }
                    }
                    if (mode == "每周") {
                        Box {
                            OutlinedButton(onClick = { weekdayExpanded = true }) {
                                Text(weekdays[weekday])
                            }
                            DropdownMenu(
                                expanded = weekdayExpanded,
                                onDismissRequest = { weekdayExpanded = false }
                            ) {
                                weekdays.forEachIndexed { i, w ->
                                    DropdownMenuItem(
                                        text = { Text(w) },
                                        onClick = { weekday = i; weekdayExpanded = false }
                                    )
                                }
                            }
                        }
                    }
                }
                if (mode == "每月") {
                    Text(
                        "按所选日期(${day}号)每月循环,大小月自动钳制",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val c2 = Calendar.getInstance().apply { set(year, month, day, hour, minute, 0) }
                val recur = when (mode) {
                    "每天" -> "daily"
                    "每周" -> "weekly:$weekday"
                    "每月" -> "monthly:$day"
                    else -> ""
                }
                onSave(text.ifBlank { "到时间啦" }, c2.timeInMillis / 1000, recur)
            }) { Text("保存") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("取消") }
        }
    )

    if (showDate) {
        val state = rememberDatePickerState(
            initialSelectedDateMillis = Calendar.getInstance()
                .apply { set(year, month, day) }.timeInMillis
        )
        DatePickerDialog(
            onDismissRequest = { showDate = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let {
                        val c3 = Calendar.getInstance().apply { timeInMillis = it }
                        year = c3.get(Calendar.YEAR)
                        month = c3.get(Calendar.MONTH)
                        day = c3.get(Calendar.DAY_OF_MONTH)
                    }
                    showDate = false
                }) { Text("确定") }
            }
        ) {
            DatePicker(state = state)
        }
    }

    if (showTime) {
        val state = rememberTimePickerState(
            initialHour = hour,
            initialMinute = minute,
            is24Hour = true
        )
        AlertDialog(
            onDismissRequest = { showTime = false },
            confirmButton = {
                TextButton(onClick = {
                    hour = state.hour
                    minute = state.minute
                    showTime = false
                }) { Text("确定") }
            },
            dismissButton = { TextButton(onClick = { showTime = false }) { Text("取消") } },
            text = { TimePicker(state = state) }
        )
    }
}
