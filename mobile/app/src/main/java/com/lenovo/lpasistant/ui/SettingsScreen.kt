package com.lenovo.lpasistant.ui

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.location.Geocoder
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import java.util.Locale
import com.lenovo.lpasistant.R
import com.lenovo.lpasistant.core.CoreBridge
import com.lenovo.lpasistant.core.LlmController
import com.lenovo.lpasistant.core.LocationProviderImpl
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    ctx: Context,
    themeMode: String,
    onThemeChanged: (String) -> Unit,
    themePalette: String,
    onPaletteChanged: (String) -> Unit,
    onBack: () -> Unit,
    onOpenReminders: () -> Unit,
    onOpenHelp: () -> Unit
) {
    val scope = rememberCoroutineScope()
    var aiEnabled by remember { mutableStateOf(false) }
    var llmMode by remember { mutableStateOf("narrow") }
    var geoEnabled by remember { mutableStateOf(false) }
    var geoStatus by remember { mutableStateOf("") }
    var city by remember { mutableStateOf("北京") }
    var modelStatus by remember { mutableStateOf("…") }
    var reminderCount by remember { mutableStateOf(0) }

    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) {
            aiEnabled = CoreBridge.getSetting("llm_enabled") == "1"
            llmMode = CoreBridge.getSetting("llm_mode").ifBlank { "narrow" }
            geoEnabled = CoreBridge.getSetting("geo_enabled") == "1"
            city = CoreBridge.getSetting("default_city").ifBlank { "北京" }
            modelStatus = LlmController.modelStatus(ctx)
            reminderCount = CoreBridge.reminders().size
            if (geoEnabled) geoStatus = locateStatus(ctx)
        }
    }

    // 开关切换时刷新定位状态
    LaunchedEffect(geoEnabled) {
        if (geoEnabled) geoStatus = locateStatus(ctx)
    }


    val locationPerms = arrayOf(
        Manifest.permission.ACCESS_FINE_LOCATION,
        Manifest.permission.ACCESS_COARSE_LOCATION
    )
    fun hasLocationPermission(c: Context): Boolean =
        ContextCompat.checkSelfPermission(c, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
            ContextCompat.checkSelfPermission(c, Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED

    val permLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { grants ->
        if (grants.values.any { it }) {
            scope.launch(Dispatchers.IO) { CoreBridge.setSetting("geo_enabled", "1") }
            geoEnabled = true
        }
    }

    // 容器全部透明:顶栏直接透出根级背景(与主体同一条渐变进度线),
    // 规避 M3 TopAppBar 内部对容器色的二次弹簧动画(双重动画导致条框滞后于主体)
    Scaffold(
        containerColor = Color.Transparent,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent),
                title = { Text("设置") },
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
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            // ---- 基础设置:主题 ----
            Text("主题", style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("system" to "跟随系统", "light" to "亮色", "dark" to "暗色").forEach { (v, label) ->
                    FilterChip(
                        selected = themeMode == v,
                        onClick = {
                            onThemeChanged(v)
                            scope.launch(Dispatchers.IO) { CoreBridge.setSetting("theme_mode", v) }
                        },
                        label = { Text(label) }
                    )
                }
            }
            // ---- 主题色:柔和色板圆点选择 ----
            Text("主题色", style = MaterialTheme.typography.titleMedium)
            Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                PALETTES.forEach { p ->
                    val selected = p.key == themePalette
                    val press = rememberPress()
                    Box(
                        modifier = Modifier
                            .pressScale(press, pressedScale = 0.85f)
                            .size(34.dp)
                            .clip(CircleShape)
                            .background(Brush.horizontalGradient(listOf(p.swatchPrimary, p.swatchAccent)))
                            .border(
                                width = if (selected) 2.dp else 1.dp,
                                color = if (selected) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.outlineVariant,
                                shape = CircleShape
                            )
                            .clickable(
                                interactionSource = press,
                                indication = null
                            ) {
                                onPaletteChanged(p.key)
                                scope.launch(Dispatchers.IO) { CoreBridge.setSetting("theme_palette", p.key) }
                            },
                        contentAlignment = Alignment.Center
                    ) {
                        if (selected) {
                            Icon(
                                painterResource(R.drawable.ic_check),
                                contentDescription = "已选",
                                tint = Color.White,
                                modifier = Modifier.size(16.dp)
                            )
                        }
                    }
                }
            }
            Text(
                "当前:${PALETTES.find { it.key == themePalette }?.label ?: "墨灰"};圆点白勾 = 已选",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            HorizontalDivider()

            // ---- 提醒管理(次级页) ----
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    painterResource(R.drawable.ic_alarm),
                    contentDescription = null,
                    modifier = Modifier.size(22.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text("提醒管理", style = MaterialTheme.typography.titleMedium)
                    Text(
                        "已设置 $reminderCount 条;点进入查看全部提醒、取消或新建",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
                TextButton(onClick = onOpenReminders) { Text("进入") }
            }
            HorizontalDivider()

            // ---- 使用帮助(次级页) ----
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    painterResource(R.drawable.ic_info),
                    contentDescription = null,
                    modifier = Modifier.size(22.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text("使用帮助", style = MaterialTheme.typography.titleMedium)
                    Text(
                        "全部分类指令说明 + 数据来源与隐私说明",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
                TextButton(onClick = onOpenHelp) { Text("进入") }
            }
            HorizontalDivider()

            // ---- AI 兜底 ----
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(Modifier.weight(1f)) {
                    Text("AI 兜底(本地模型)", style = MaterialTheme.typography.titleMedium)
                    Text(
                        "规则没看懂提醒时由本地小模型理解,纯离线。\n模型状态:$modelStatus",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
                Switch(
                    checked = aiEnabled,
                    onCheckedChange = { on ->
                        aiEnabled = on
                        scope.launch(Dispatchers.IO) {
                            CoreBridge.setSetting("llm_enabled", if (on) "1" else "0")
                        }
                    }
                )
            }
            if (aiEnabled) {
                Text("兜底模式", style = MaterialTheme.typography.titleSmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(
                        selected = llmMode == "narrow",
                        onClick = {
                            llmMode = "narrow"
                            scope.launch(Dispatchers.IO) { CoreBridge.setSetting("llm_mode", "narrow") }
                        },
                        label = { Text("窄:仅「提醒」开头") }
                    )
                    FilterChip(
                        selected = llmMode == "wide",
                        onClick = {
                            llmMode = "wide"
                            scope.launch(Dispatchers.IO) { CoreBridge.setSetting("llm_mode", "wide") }
                        },
                        label = { Text("宽:全句识别") }
                    )
                }
                Text(
                    if (llmMode == "wide")
                        "任何没被规则听懂的句子都会尝试识别提醒意图(如「明早八点叫我起床」);每句最多2次本地推理,响应稍慢"
                    else
                        "只有「提醒我…」这类句子解析失败时才触发模型;其余输入永远不会进入模型",
                    style = MaterialTheme.typography.bodySmall
                )
            }
            Text(
                "模型文件:把 model.gguf(Qwen2.5-0.5B Q4,约400MB)放到\n/sdcard/Android/data/com.lenovo.lpasistant/files/ 下即可,首次使用自动加载",
                style = MaterialTheme.typography.bodySmall
            )
            HorizontalDivider()

            // ---- 定位天气 ----
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(Modifier.weight(1f)) {
                    Text("自动定位天气", style = MaterialTheme.typography.titleMedium)
                    Text(
                        "「天气」不带城市时用当前位置。坐标只发给 Open-Meteo(算天气所必需),不存历史、不经第三方反查、不发新闻。",
                        style = MaterialTheme.typography.bodySmall
                    )
                }
                Switch(
                    checked = geoEnabled,
                    onCheckedChange = { on ->
                        when {
                            !on -> {
                                geoEnabled = false
                                scope.launch(Dispatchers.IO) { CoreBridge.setSetting("geo_enabled", "0") }
                            }
                            hasLocationPermission(ctx) -> {
                                geoEnabled = true
                                scope.launch(Dispatchers.IO) { CoreBridge.setSetting("geo_enabled", "1") }
                            }
                            else -> permLauncher.launch(locationPerms)
                        }
                    }
                )
            }
            // 定位状态提示:成功(城市)/未授权/无数据
            if (geoEnabled && geoStatus.isNotBlank()) {
                Text(
                    geoStatus,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            HorizontalDivider()

            // ---- 天气默认城市 ----
            val context = LocalContext.current
            OutlinedTextField(
                value = city,
                onValueChange = { city = it },
                label = { Text("天气默认城市") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                supportingText = { Text("「天气」不带城市时使用;点保存立即生效") },
                trailingIcon = {
                    TextButton(onClick = {
                        scope.launch(Dispatchers.IO) { CoreBridge.setSetting("default_city", city) }
                        android.widget.Toast.makeText(context, "已保存", android.widget.Toast.LENGTH_SHORT).show()
                    }) { Text("保存") }
                }
            )

            Text("LPasistant v1.0", style = MaterialTheme.typography.bodySmall)
        }
    }
}

/** 定位状态:未授权/无数据/成功+城市名(系统 Geocoder 反查,不发给第三方网站) */
private suspend fun locateStatus(c: Context): String = withContext(Dispatchers.IO) {
    try {
        val granted = ContextCompat.checkSelfPermission(
            c, Manifest.permission.ACCESS_FINE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED || ContextCompat.checkSelfPermission(
            c, Manifest.permission.ACCESS_COARSE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
        if (!granted) return@withContext "✗ 尚未授予定位权限(开关打开后会弹授权)"
        val coords = LocationProviderImpl(c).currentCoords()
        if (coords.isNullOrBlank()) {
            return@withContext "✗ 暂无最近位置数据(打开地图/导航类应用后再试)"
        }
        val lat = coords.split(",").getOrNull(0)?.toDoubleOrNull()
        val lon = coords.split(",").getOrNull(1)?.toDoubleOrNull()
        if (lat == null || lon == null) return@withContext "✗ 坐标解析异常"
        if (!Geocoder.isPresent()) {
            return@withContext "✓ 已拿到坐标(${ "%.3f,%.3f".format(lat, lon) }),此设备不支持反查城市"
        }
        val addr = Geocoder(c, Locale.CHINA).getFromLocation(lat, lon, 1)?.firstOrNull()
        val city = addr?.locality ?: addr?.subAdminArea ?: addr?.adminArea ?: addr?.countryName ?: "未知区域"
        "✓ 定位成功:$city(${ "%.3f,%.3f".format(lat, lon) })"
    } catch (e: Exception) {
        "✗ 定位失败:${e.message}"
    }
}
