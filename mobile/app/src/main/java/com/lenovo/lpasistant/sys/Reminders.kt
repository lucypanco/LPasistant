package com.lenovo.lpasistant.sys

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.AlarmManagerCompat
import com.lenovo.lpasistant.core.CoreBridge

/**
 * 提醒调度:把 SQLite 里的提醒映射到系统闹钟(AlarmManager)。
 *
 * 两种方式(mode):
 * - "alarm" 闹钟式:setAlarmClock——系统级闹钟标记(状态栏闹钟图标、Doze 完全豁免,
 *   无需与自带时钟 App 协调,到点由我们以闹钟铃声+锁屏强提醒呈现);
 * - "notify" 通知式:setExactAndAllowWhileIdle——到点发高优先级通知,不吵。
 *
 * 关键设计:App 平时不需要活着——到点由系统拉起 AlarmReceiver;
 * 重启后 BootReceiver 调 scheduleAll 恢复全部闹钟。
 */
object Reminders {

    const val EXTRA_ID = "id"
    const val MODE_ALARM = "alarm"
    const val MODE_NOTIFY = "notify"

    /** 按存储当前内容重建闹钟(增量幂等,重复注册无副作用) */
    fun scheduleAll(ctx: Context) {
        val now = System.currentTimeMillis() / 1000
        CoreBridge.reminders()
            .filter { it.nextRun > now }
            .forEach { schedule(ctx, it.id, it.nextRun, it.mode) }
    }

    fun schedule(ctx: Context, id: String, epochSec: Long, mode: String) {
        val am = ctx.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val pi = pending(ctx, id)
        val triggerAt = epochSec * 1000
        val exact = Build.VERSION.SDK_INT < 31 || am.canScheduleExactAlarms()
        if (mode == MODE_ALARM) {
            // 闹钟式:setAlarmClock(系统级标记;触发时仍拉起本 App 的 Receiver 响铃)
            val show = PendingIntent.getActivity(
                ctx, id.hashCode() + 1,
                Intent(ctx, com.lenovo.lpasistant.MainActivity::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
            am.setAlarmClock(AlarmManager.AlarmClockInfo(triggerAt, show), pi)
        } else if (exact) {
            AlarmManagerCompat.setExactAndAllowWhileIdle(am, AlarmManager.RTC_WAKEUP, triggerAt, pi)
        } else {
            // 未授予「精确闹钟」权限时降级为窗口闹钟(±1 分钟;Doze 下可能推迟)
            am.setWindow(AlarmManager.RTC_WAKEUP, triggerAt, 60_000L, pi)
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
