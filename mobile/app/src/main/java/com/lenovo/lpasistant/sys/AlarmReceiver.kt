package com.lenovo.lpasistant.sys

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.lenovo.lpasistant.MainActivity
import com.lenovo.lpasistant.R
import com.lenovo.lpasistant.core.CoreBridge
import uniffi.lpasistant_core.FireOutcome

/** 提醒/早报到点:由系统闹钟拉起,通知用户;循环提醒在此顺延下一次闹钟 */
class AlarmReceiver : BroadcastReceiver() {

    override fun onReceive(ctx: Context, intent: Intent) {
        val id = intent.getStringExtra(Reminders.EXTRA_ID) ?: return
        val result = goAsync()
        Thread {
            try {
                CoreBridge.init(ctx)
                when (val out = CoreBridge.fired(id)) {
                    is FireOutcome.Text -> {
                        notify(ctx, "⏰ 提醒", out.text)
                        if (out.nextRun > 0) Reminders.schedule(ctx, id, out.nextRun)
                    }
                    is FireOutcome.Digest -> {
                        notify(ctx, "🌅 早报", CoreBridge.digest())
                        if (out.nextRun > 0) Reminders.schedule(ctx, id, out.nextRun)
                    }
                    else -> {} // 已取消/不存在
                }
            } catch (_: Exception) {
            } finally {
                result.finish()
            }
        }.start()
    }

    companion object {
        const val CHANNEL_REMINDER = "reminders"

        fun ensureChannel(ctx: Context) {
            val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL_REMINDER, "提醒与早报", NotificationManager.IMPORTANCE_HIGH)
            )
        }

        private fun notify(ctx: Context, title: String, text: String) {
            ensureChannel(ctx)
            val nm = NotificationManagerCompat.from(ctx)
            if (!nm.areNotificationsEnabled()) return
            val pi = PendingIntent.getActivity(
                ctx, 0, Intent(ctx, MainActivity::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
            val n = NotificationCompat.Builder(ctx, CHANNEL_REMINDER)
                .setSmallIcon(R.drawable.ic_stat_notify)
                .setContentTitle(title)
                .setContentText(text)
                .setStyle(NotificationCompat.BigTextStyle().bigText(text))
                .setContentIntent(pi)
                .setAutoCancel(true)
                .build()
            try {
                nm.notify(text.hashCode(), n)
            } catch (_: SecurityException) {
            }
        }
    }
}
