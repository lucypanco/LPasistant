package com.lenovo.lpasistant.core

import android.content.Context
import android.util.Log
import com.lenovo.lpasistant.sys.Reminders
import uniffi.lpasistant_core.Assistant
import uniffi.lpasistant_core.FireOutcome
import uniffi.lpasistant_core.NewsItemRecord
import uniffi.lpasistant_core.RadioStationRecord
import uniffi.lpasistant_core.ReminderDraft
import uniffi.lpasistant_core.ReminderRecord
import uniffi.lpasistant_core.WeatherChart

/**
 * Kotlin ↔ Rust 核心的唯一入口。
 *
 * Rust 侧暴露的都是同步函数,这里统一在 IO 线程调用(见 ui/ChatScreen.kt、sys/AlarmReceiver.kt)。
 * 绑定类 uniffi.lpasistant_core.* 由 `uniffi-bindgen generate` 生成(见 mobile/BUILD.md)。
 */
object CoreBridge {

    private const val TAG = "CoreBridge"

    @Volatile
    private var core: Assistant? = null

    fun init(ctx: Context) {
        if (core == null) {
            synchronized(this) {
                if (core == null) {
                    val db = ctx.getDatabasePath("lpasistant.db").absolutePath
                    val c = Assistant(db)
                    core = c
                    // 回调挂载(是否真正启用由 Rust 侧按设置 kv 决定,默认全关):
                    //  - LLM 兜底:仅本地模型,纯离线
                    //  - 定位:仅用于天气,坐标直供 Open-Meteo
                    try { c.attachLlm(LlmInferImpl(ctx)) } catch (_: Throwable) {}
                    try { c.attachLocation(LocationProviderImpl(ctx)) } catch (_: Throwable) {}
                }
            }
        }
    }

    fun getSetting(key: String): String = try {
        c().getSetting(key)
    } catch (_: Exception) {
        ""
    }

    fun setSetting(key: String, value: String) {
        try {
            c().setSetting(key, value)
        } catch (_: Exception) {
        }
    }

    fun pendingDraft(): ReminderDraft? = try {
        c().pendingDraft()
    } catch (_: Exception) {
        null
    }

    /** 表单保存(死规则直连):mode "notify"通知式 / "alarm"闹钟式;返回确认文案并同步系统闹钟 */
    fun saveReminderDirect(ctx: Context, text: String, epochSec: Long, recur: String, mode: String): String {
        val reply = try {
            c().saveReminderDirect(text, epochSec, recur, mode)
        } catch (e: Exception) {
            "😅 保存失败:${e.message}"
        }
        syncReminders(ctx)
        return reply
    }

    /** [确认] 键:落库待确认草稿并同步系统闹钟 */
    fun confirmPending(ctx: Context): String {
        val reply = try {
            c().confirmPending()
        } catch (e: Exception) {
            "😅 ${e.message}"
        }
        syncReminders(ctx)
        return reply
    }

    /** [取消] 键:放弃待确认草稿 */
    fun cancelPending(): String = try {
        c().cancelPending()
    } catch (e: Exception) {
        "😅 ${e.message}"
    }

    /** 提醒列表逐条 [取消] 键 */
    fun cancelReminder(ctx: Context, id: String): String {
        val reply = try {
            c().cancelReminder(id)
        } catch (e: Exception) {
            "😅 ${e.message}"
        }
        syncReminders(ctx)
        return reply
    }

    /** 结构化新闻列表(quick chips 用;点行获取详情) */
    fun newsList(kind: String, n: Int): List<NewsItemRecord> = try {
        c().newsList(kind, n.toUInt())
    } catch (e: Exception) {
        emptyList()
    }

    /** 天气曲线卡(播报文本+24h 曲线点);city 空=自动定位(设置开关),失败回退默认城市 */
    fun weatherChart(city: String): WeatherChart? = try {
        c().weatherChart(city)
    } catch (_: Exception) {
        null
    }

    /** 新闻详情:抓原文+提取正文;AI摘要由 Rust 侧按 AI兜底设置自动附加 */
    fun newsDetail(url: String, title: String): String = try {
        c().newsDetail(url, title)
    } catch (e: Exception) {
        "😅 详情获取失败:${e.message}\n可点原链接阅读:$url"
    }

    /** 精选电台(播放由 RadioController/Media3 执行) */
    fun radioStations(): List<RadioStationRecord> = try {
        c().radioStations()
    } catch (_: Exception) {
        emptyList()
    }

    fun radioSearch(q: String, n: Int): List<RadioStationRecord> = try {
        c().radioSearch(q, n.toUInt())
    } catch (_: Exception) {
        emptyList()
    }

    private fun c(): Assistant = core ?: error("CoreBridge 未初始化")

    fun handle(input: String): String = try {
        c().handleCommand(input)
    } catch (e: Exception) {
        "😅 核心出错:${e.message}"
    }

    fun reminders(): List<ReminderRecord> =
        try { c().listReminders() } catch (_: Exception) { emptyList() }

    fun fired(id: String): FireOutcome =
        try { c().reminderFired(id) } catch (_: Exception) { FireOutcome.Gone }

    // ---- 用户自定义源(保存即自检,勾选控制参与)----

    fun radioUserList(): List<uniffi.lpasistant_core.UserStation> = try {
        c().radioUserList()
    } catch (_: Exception) {
        emptyList()
    }

    fun radioUserAdd(name: String, url: String): uniffi.lpasistant_core.UserStation? = try {
        c().radioUserAdd(name, url)
    } catch (e: Exception) {
        Log.e(TAG, "radioUserAdd", e)
        null
    }

    fun radioUserSetEnabled(url: String, enabled: Boolean) = try {
        c().radioUserSetEnabled(url, enabled)
    } catch (_: Exception) {
    }

    fun radioUserRemove(url: String) = try {
        c().radioUserRemove(url)
    } catch (_: Exception) {
    }

    fun newsUserList(): List<uniffi.lpasistant_core.UserFeed> = try {
        c().newsUserList()
    } catch (_: Exception) {
        emptyList()
    }

    fun newsUserAdd(url: String): uniffi.lpasistant_core.UserFeed? = try {
        c().newsUserAdd(url)
    } catch (e: Exception) {
        Log.e(TAG, "newsUserAdd", e)
        null
    }

    fun newsUserSetEnabled(url: String, enabled: Boolean) = try {
        c().newsUserSetEnabled(url, enabled)
    } catch (_: Exception) {
    }

    fun newsUserRemove(url: String) = try {
        c().newsUserRemove(url)
    } catch (_: Exception) {
    }

    fun digest(): String = try {
        c().composeDigest()
    } catch (e: Exception) {
        "😅 早报组装失败:${e.message}"
    }

    /** 每次指令后调用:让系统闹钟与存储中的提醒保持一致 */
    fun syncReminders(ctx: Context) = Reminders.scheduleAll(ctx)
}
