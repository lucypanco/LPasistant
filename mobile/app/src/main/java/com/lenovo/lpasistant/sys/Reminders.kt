package com.lenovo.lpasistant.sys

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.AlarmManagerCompat
import com.lenovo.lpasistant.core.CoreBridge

/**
 * 提醒调度:把 SQLite 里的提醒映射到系统精确闹钟(AlarmManager)。
 *
 * 关键设计:App 平时不需要活着——到点由系统拉起 AlarmReceiver;
 * 重启后 BootReceiver 调 scheduleAll 恢复全部闹钟。
 */
object Reminders {

    const val EXTRA_ID = "id"

    /** 按存储当前内容重建闹钟(增量幂等,重复注册无副作用) */
    fun scheduleAll(ctx: Context) {
        val now = System.currentTimeMillis() / 1000
        CoreBridge.reminders()
            .filter { it.nextRun > now }
            .forEach { schedule(ctx, it.id, it.nextRun) }
    }

    fun schedule(ctx: Context, id: String, epochSec: Long) {
        val am = ctx.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val pi = pending(ctx, id)
        val triggerAt = epochSec * 1000
        if (Build.VERSION.SDK_INT >= 31 && !am.canScheduleExactAlarms()) {
            // 未授予“精确闹钟”权限时降级为窗口闹钟(±1 分钟)
            am.setWindow(AlarmManager.RTC_WAKEUP, triggerAt, 60_000L, pi)
        } else {
            AlarmManagerCompat.setExactAndAllowWhileIdle(am, AlarmManager.RTC_WAKEUP, triggerAt, pi)
        }
    }

    private fun pending(ctx: Context, id: String): PendingIntent =
        PendingIntent.getBroadcast(
            ctx,
            id.hashCode(),
            Intent(ctx, AlarmReceiver::class.java).putExtra(EXTRA_ID, id),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
}
