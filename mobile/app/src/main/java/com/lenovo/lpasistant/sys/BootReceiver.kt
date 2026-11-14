package com.lenovo.lpasistant.sys

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.lenovo.lpasistant.core.CoreBridge

/** 开机完成:恢复所有未来的提醒闹钟(SQLite 持久化,不怕重启) */
class BootReceiver : BroadcastReceiver() {

    override fun onReceive(ctx: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        try {
            CoreBridge.init(ctx)
            Reminders.scheduleAll(ctx)
        } catch (_: Exception) {
        }
    }
}
