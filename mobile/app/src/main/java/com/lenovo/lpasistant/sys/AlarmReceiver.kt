package com.lenovo.lpasistant.sys

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.RingtoneManager
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.lenovo.lpasistant.MainActivity
import com.lenovo.lpasistant.R
import com.lenovo.lpasistant.core.CoreBridge
import uniffi.lpasistant_core.FireOutcome

/** 提醒/早报到点:由系统闹钟拉起,按提醒方式呈现;循环提醒在此顺延下一次闹钟 */
class AlarmReceiver : BroadcastReceiver() {

    override fun onReceive(ctx: Context, intent: Intent) {
        val id = intent.getStringExtra(Reminders.EXTRA_ID) ?: return
        val result = goAsync()
        Thread {
            try {
                CoreBridge.init(ctx)
                when (val out = CoreBridge.fired(id)) {
                    is FireOutcome.Text -> {
                        if (out.mode == Reminders.MODE_ALARM) {
                            notifyAlarm(ctx, out.text)
                        } else {
                            notifyNotify(ctx, "⏰ 提醒", out.text)
                        }
                        if (out.nextRun > 0) Reminders.schedule(ctx, id, out.nextRun, out.mode)
                    }
                    is FireOutcome.Digest -> {
                        notifyNotify(ctx, "🌅 早报", CoreBridge.digest())
                        if (out.nextRun > 0) Reminders.schedule(ctx, id, out.nextRun, Reminders.MODE_NOTIFY)
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
        const val CHANNEL_ALARM = "reminders_alarm"

        fun ensureChannel(ctx: Context) {
            val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL_REMINDER, "提醒与早报", NotificationManager.IMPORTANCE_HIGH)
            )
            // 闹钟式通道:系统默认闹钟铃声、闹钟音量、免打扰穿透、锁屏公开
            val alarm = NotificationChannel(
                CHANNEL_ALARM, "闹钟提醒", NotificationManager.IMPORTANCE_HIGH
            ).apply {
                setSound(
                    RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM),
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_ALARM)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                        .build()
                )
                setBypassDnd(true)
                lockscreenVisibility = NotificationCompat.VISIBILITY_PUBLIC
            }
            nm.createNotificationChannel(alarm)
        }

        /** 通知式:高优先级 heads-up,无铃声 */
        private fun notifyNotify(ctx: Context, title: String, text: String) {
            ensureChannel(ctx)
            post(ctx, "⏰ 提醒", text, CHANNEL_REMINDER, fullScreen = false)
        }

        /** 闹钟式:闹钟铃声通道 + 锁屏全屏意图(未授权全屏权限时退回 heads-up) */
        private fun notifyAlarm(ctx: Context, text: String) {
            ensureChannel(ctx)
            post(ctx, "⏰ 闹钟", text, CHANNEL_ALARM, fullScreen = true)
        }

        private fun post(ctx: Context, title: String, text: String, channel: String, fullScreen: Boolean) {
            val nm = NotificationManagerCompat.from(ctx)
            if (!nm.areNotificationsEnabled()) return
            val pi = PendingIntent.getActivity(
                ctx, 0, Intent(ctx, MainActivity::class.java),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
            val b = NotificationCompat.Builder(ctx, channel)
                .setSmallIcon(R.drawable.ic_stat_notify)
                .setContentTitle(title)
                .setContentText(text)
                .setStyle(NotificationCompat.BigTextStyle().bigText(text))
                .setContentIntent(pi)
                .setAutoCancel(true)
                .setCategory(if (fullScreen) NotificationCompat.CATEGORY_ALARM else NotificationCompat.CATEGORY_REMINDER)
            if (fullScreen) b.setFullScreenIntent(pi, true)
            val n = b.build()
            try {
                nm.notify(text.hashCode(), n)
            } catch (_: SecurityException) {
            }
        }
    }
}
