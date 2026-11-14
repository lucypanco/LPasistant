package com.lenovo.lpasistant

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.core.content.ContextCompat
import com.lenovo.lpasistant.core.CoreBridge
import com.lenovo.lpasistant.sys.AlarmReceiver
import com.lenovo.lpasistant.ui.AssistantTheme
import com.lenovo.lpasistant.ui.ChatScreen
import com.lenovo.lpasistant.ui.HelpScreen
import com.lenovo.lpasistant.ui.ReminderListScreen
import com.lenovo.lpasistant.ui.SettingsScreen

class MainActivity : ComponentActivity() {

    private val notifPerm =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { /* 拒绝不影响聊天 */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // 边到边:系统条透明,背景即窗口底色 → 条框与主体同一条渐变时间线,无独立动画
        enableEdgeToEdge()
        CoreBridge.init(applicationContext)
        AlarmReceiver.ensureChannel(this)
        requestNotificationPermission()
        setContent {
            var themeMode by remember {
                mutableStateOf(CoreBridge.getSetting("theme_mode").ifBlank { "system" })
            }
            var themePalette by remember {
                mutableStateOf(CoreBridge.getSetting("theme_palette").ifBlank { "ink" })
            }
            var screen by remember { mutableStateOf("chat") }
            AssistantTheme(themeMode, themePalette) {
                // 根级 Surface:统一 LocalContentColor=onSurface(否则默认黑,暗色下图标/文字不可见)
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                // 系统返回键/手势:次级页回到上一级,而不是退出 App
                BackHandler(enabled = screen != "chat") {
                    screen = when (screen) {
                        "reminders", "help" -> "settings"
                        else -> "chat"
                    }
                }
                // safeDrawing 避让:状态栏/导航条/键盘(边到边模式下窗口不再自动缩放)
                Column(
                    Modifier
                        .fillMaxSize()
                        .windowInsetsPadding(WindowInsets.safeDrawing)
                ) {
                // 页面切换 = Apple式整页同时渐变:旧页淡出与新页淡入同速同步,
                // 各页底色与窗口底色同色 → 过程中无白闪;页内条框随页面一起渐变。
                AnimatedContent(
                    targetState = screen,
                    transitionSpec = {
                        fadeIn(tween(220, easing = FastOutSlowInEasing)) togetherWith
                            fadeOut(tween(220, easing = FastOutSlowInEasing))
                    },
                    modifier = Modifier.fillMaxSize(),
                    label = "screen"
                ) { s ->
                    when (s) {
                        "settings" -> SettingsScreen(
                            ctx = applicationContext,
                            themeMode = themeMode,
                            onThemeChanged = { themeMode = it },
                            themePalette = themePalette,
                            onPaletteChanged = { themePalette = it },
                            onBack = { screen = "chat" },
                            onOpenReminders = { screen = "reminders" },
                            onOpenHelp = { screen = "help" }
                        )
                        "reminders" -> ReminderListScreen(
                            ctx = applicationContext,
                            onBack = { screen = "settings" }
                        )
                        "help" -> HelpScreen(onBack = { screen = "settings" })
                        else -> ChatScreen(onOpenSettings = { screen = "settings" })
                    }
                }
                }
            }
        }
    }
    }

    override fun onResume() {
        super.onResume()
        // 回到前台时对齐一次闹钟(处理取消/新增/时钟变化)
        Thread {
            try { CoreBridge.syncReminders(applicationContext) } catch (_: Exception) {}
        }.start()
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= 33) {
            val p = Manifest.permission.POST_NOTIFICATIONS
            if (ContextCompat.checkSelfPermission(this, p) != PackageManager.PERMISSION_GRANTED) {
                notifPerm.launch(p)
            }
        }
    }
}
