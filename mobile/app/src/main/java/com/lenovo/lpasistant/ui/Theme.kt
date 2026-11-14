package com.lenovo.lpasistant.ui

import android.app.Activity
import android.content.ContextWrapper
import android.graphics.drawable.ColorDrawable
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat

/*
 * 主题设计:延续启动图标「白底黑标」的定位,提供 6 套柔和色板(设置页可切换)。
 *  每套色板 = 主色对(primary,骨架:按钮/气泡/链接)+ 撞色辅助对(accent,点睛:
 *  选中芯片 / 播放条 / 正在播放标识),两色明度、饱和度错开,柔和但有层次。
 *  - 全部组合经过 WCAG 对比度校验:主色上白字 ≥5.6:1,主色作文字 ≥4.85:1,
 *    辅助色容器上文字 ≥10:1,深色模式 ≥5~7:1 —— 字体功能区分度优先于风格
 *  - 字体不打包(跟随各 ROM 中文字体),只收紧字号/字重层次
 *  - 全局圆角统一:小 10 / 中 14 / 大 20
 */

/** 单一模式的完整用色(经校验的固定值,不做运行时推导) */
private data class HueSet(
    val primary: Color, val onPrimary: Color,
    val container: Color, val onContainer: Color,
    val accent: Color, val onAccent: Color,
    val accentContainer: Color, val onAccentContainer: Color,
    val surface: Color, val surfaceVariant: Color, val onSurfaceVariant: Color,
    val onSurface: Color, val outline: Color, val outlineVariant: Color
)

/** 一套色板:浅色 + 深色;swatch 双色供设置页圆点渐变显示 */
data class PaletteSpec(
    val key: String,
    val label: String,
    val swatchPrimary: Color,
    val swatchAccent: Color,
    val light: ColorScheme,
    val dark: ColorScheme
)

private fun makeColors(h: HueSet, dark: Boolean): ColorScheme {
    val base = if (dark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = h.primary, onPrimary = h.onPrimary,
        primaryContainer = h.container, onPrimaryContainer = h.onContainer,
        secondary = h.accent, onSecondary = h.onAccent,
        secondaryContainer = h.accentContainer, onSecondaryContainer = h.onAccentContainer,
        tertiary = h.accent, onTertiary = h.onAccent,
        tertiaryContainer = h.accentContainer, onTertiaryContainer = h.onAccentContainer,
        background = h.surface, onBackground = h.onSurface,
        surface = h.surface, onSurface = h.onSurface,
        surfaceVariant = h.surfaceVariant, onSurfaceVariant = h.onSurfaceVariant,
        outline = h.outline, outlineVariant = h.outlineVariant
    )
}

private val InkL = Color(0xFF1B1B1F)   // 亮色正文近黑
private val InkD = Color(0xFFE5E5E9)   // 暗色正文近白

private val PALETTE_DEFS: List<PaletteSpec> = listOf(
    PaletteSpec(
        "ink", "墨灰", Color(0xFF7A7A80), Color(0xFF8A6D45),
        makeColors(HueSet(
            primary = Color(0xFF1B1B1F), onPrimary = Color.White,
            container = Color(0xFFE6E6E9), onContainer = Color(0xFF101012),
            accent = Color(0xFF8A6D45), onAccent = Color.White,               // 琥珀铜
            accentContainer = Color(0xFFE8E2DA), onAccentContainer = Color(0xFF302618),
            surface = Color(0xFFFAFAFA), surfaceVariant = Color(0xFFEFEFF1),
            onSurfaceVariant = Color(0xFF44464A), onSurface = InkL,
            outline = Color(0xFF77757F), outlineVariant = Color(0xFFE1E1E4)
        ), dark = false),
        makeColors(HueSet(
            primary = Color(0xFFB1B1B3), onPrimary = InkL,
            container = Color(0xFF313133), onContainer = Color(0xFFE5E5E9),
            accent = Color(0xFFAB977B), onAccent = InkL,
            accentContainer = Color(0xFF37332C), onAccentContainer = Color(0xFFE7E7E6),
            surface = Color(0xFF131316), surfaceVariant = Color(0xFF26262B),
            onSurfaceVariant = Color(0xFFC6C6CC), onSurface = InkD,
            outline = Color(0xFF8E8E95), outlineVariant = Color(0xFF4C4C54)
        ), dark = true)
    ),
    PaletteSpec(
        "violet", "雾紫", Color(0xFF685BA0), Color(0xFF96742F),
        makeColors(HueSet(
            primary = Color(0xFF685BA0), onPrimary = Color.White,
            container = Color(0xFFE7E3F6), onContainer = Color(0xFF221E33),
            accent = Color(0xFF96742F), onAccent = Color.White,               // 杏金
            accentContainer = Color(0xFFEAE3D5), onAccentContainer = Color(0xFF352910),
            surface = Color(0xFFFDFCFF), surfaceVariant = Color(0xFFEFEDF6),
            onSurfaceVariant = Color(0xFF4A4658), onSurface = InkL,
            outline = Color(0xFF8B8798), outlineVariant = Color(0xFFE2DFEC)
        ), dark = false),
        makeColors(HueSet(
            primary = Color(0xFFB7B1D2), onPrimary = InkL,
            container = Color(0xFF34313A), onContainer = Color(0xFFE7E3F6),
            accent = Color(0xFFAD925D), onAccent = InkL,
            accentContainer = Color(0xFF363126), onAccentContainer = Color(0xFFEAE3D5),
            surface = Color(0xFF131316), surfaceVariant = Color(0xFF26262B),
            onSurfaceVariant = Color(0xFFC8C5D2), onSurface = InkD,
            outline = Color(0xFF9692A5), outlineVariant = Color(0xFF4B4859)
        ), dark = true)
    ),
    PaletteSpec(
        "blue", "黛蓝", Color(0xFF4B6A88), Color(0xFFA2674F),
        makeColors(HueSet(
            primary = Color(0xFF4B6A88), onPrimary = Color.White,
            container = Color(0xFFDCE6EF), onContainer = Color(0xFF17222E),
            accent = Color(0xFFA2674F), onAccent = Color.White,               // 陶橙
            accentContainer = Color(0xFFECE1DC), onAccentContainer = Color(0xFF392420),
            surface = Color(0xFFFBFCFE), surfaceVariant = Color(0xFFEBF0F5),
            onSurfaceVariant = Color(0xFF3F4A56), onSurface = InkL,
            outline = Color(0xFF7D8894), outlineVariant = Color(0xFFDDE3E9)
        ), dark = false),
        makeColors(HueSet(
            primary = Color(0xFFA2B3C2), onPrimary = InkL,
            container = Color(0xFF303437), onContainer = Color(0xFFDCE6EF),
            accent = Color(0xFFBC9281), onAccent = InkL,
            accentContainer = Color(0xFF3B312E), onAccentContainer = Color(0xFFECE1DC),
            surface = Color(0xFF131316), surfaceVariant = Color(0xFF26262B),
            onSurfaceVariant = Color(0xFFC2C9D1), onSurface = InkD,
            outline = Color(0xFF8B95A1), outlineVariant = Color(0xFF495158)
        ), dark = true)
    ),
    PaletteSpec(
        "sage", "苔绿", Color(0xFF516C54), Color(0xFFA0654E),
        makeColors(HueSet(
            primary = Color(0xFF516C54), onPrimary = Color.White,
            container = Color(0xFFDCE8DC), onContainer = Color(0xFF16241A),
            accent = Color(0xFFA0654E), onAccent = Color.White,               // 暖陶
            accentContainer = Color(0xFFECE0DC), onAccentContainer = Color(0xFF38231B),
            surface = Color(0xFFFAFCFA), surfaceVariant = Color(0xFFE9EFE8),
            onSurfaceVariant = Color(0xFF3E4A40), onSurface = InkL,
            outline = Color(0xFF7A887C), outlineVariant = Color(0xFFDDE4DA)
        ), dark = false),
        makeColors(HueSet(
            primary = Color(0xFFADBAB0), onPrimary = InkL,
            container = Color(0xFF313431), onContainer = Color(0xFFDCE8DC),
            accent = Color(0xFFBB9080), onAccent = InkL,
            accentContainer = Color(0xFF3B312E), onAccentContainer = Color(0xFFECE0DC),
            surface = Color(0xFF131316), surfaceVariant = Color(0xFF26262B),
            onSurfaceVariant = Color(0xFFC1C9C2), onSurface = InkD,
            outline = Color(0xFF89938A), outlineVariant = Color(0xFF49514B)
        ), dark = true)
    ),
    PaletteSpec(
        "clay", "陶土", Color(0xFF8A5848), Color(0xFF5B7285),
        makeColors(HueSet(
            primary = Color(0xFF8A5848), onPrimary = Color.White,
            container = Color(0xFFF4E2DA), onContainer = Color(0xFF2A1B15),
            accent = Color(0xFF5B7285), onAccent = Color.White,               // 雾蓝
            accentContainer = Color(0xFFDEE3E7), onAccentContainer = Color(0xFF20282F),
            surface = Color(0xFFFFFBF8), surfaceVariant = Color(0xFFF5EDE8),
            onSurfaceVariant = Color(0xFF52443C), onSurface = InkL,
            outline = Color(0xFF857468), outlineVariant = Color(0xFFEDDFD7)
        ), dark = false),
        makeColors(HueSet(
            primary = Color(0xFFC8B0A9), onPrimary = InkL,
            container = Color(0xFF37312F), onContainer = Color(0xFFF4E2DA),
            accent = Color(0xFF899AA8), onAccent = InkL,
            accentContainer = Color(0xFF313437), onAccentContainer = Color(0xFFDEE3E7),
            surface = Color(0xFF131316), surfaceVariant = Color(0xFF26262B),
            onSurfaceVariant = Color(0xFFD7C4BA), onSurface = InkD,
            outline = Color(0xFFA08D81), outlineVariant = Color(0xFF574A42)
        ), dark = true)
    ),
    PaletteSpec(
        "rose", "暮粉", Color(0xFF845C69), Color(0xFF6E7F63),
        makeColors(HueSet(
            primary = Color(0xFF845C69), onPrimary = Color.White,
            container = Color(0xFFF5E3E9), onContainer = Color(0xFF2E1B23),
            accent = Color(0xFF6E7F63), onAccent = Color.White,               // 橄榄
            accentContainer = Color(0xFFE2E5E0), onAccentContainer = Color(0xFF272C23),
            surface = Color(0xFFFFFBFC), surfaceVariant = Color(0xFFF5EBEF),
            onSurfaceVariant = Color(0xFF4F424A), onSurface = InkL,
            outline = Color(0xFF86737D), outlineVariant = Color(0xFFEDDFE5)
        ), dark = false),
        makeColors(HueSet(
            primary = Color(0xFFC5B2B8), onPrimary = InkL,
            container = Color(0xFF373134), onContainer = Color(0xFFF5E3E9),
            accent = Color(0xFF8F9B86), onAccent = InkL,
            accentContainer = Color(0xFF30332E), onAccentContainer = Color(0xFFE2E5E0),
            surface = Color(0xFF131316), surfaceVariant = Color(0xFF26262B),
            onSurfaceVariant = Color(0xFFD5C3CC), onSurface = InkD,
            outline = Color(0xFF9F8B95), outlineVariant = Color(0xFF55444B)
        ), dark = true)
    )
)

/** 注册表:设置页与主题都从这里取 */
val PALETTES: List<PaletteSpec> = PALETTE_DEFS

fun paletteFor(key: String): PaletteSpec = PALETTES.find { it.key == key } ?: PALETTES.first()

/** ColorScheme 渐变(全区域同时渐变的实现):所有常用色沿同一进度线插值,未列色取目标值 */
private fun lerpScheme(from: ColorScheme, to: ColorScheme, t: Float): ColorScheme = to.copy(
    primary = lerp(from.primary, to.primary, t),
    onPrimary = lerp(from.onPrimary, to.onPrimary, t),
    primaryContainer = lerp(from.primaryContainer, to.primaryContainer, t),
    onPrimaryContainer = lerp(from.onPrimaryContainer, to.onPrimaryContainer, t),
    secondary = lerp(from.secondary, to.secondary, t),
    onSecondary = lerp(from.onSecondary, to.onSecondary, t),
    secondaryContainer = lerp(from.secondaryContainer, to.secondaryContainer, t),
    onSecondaryContainer = lerp(from.onSecondaryContainer, to.onSecondaryContainer, t),
    tertiary = lerp(from.tertiary, to.tertiary, t),
    onTertiary = lerp(from.onTertiary, to.onTertiary, t),
    tertiaryContainer = lerp(from.tertiaryContainer, to.tertiaryContainer, t),
    onTertiaryContainer = lerp(from.onTertiaryContainer, to.onTertiaryContainer, t),
    background = lerp(from.background, to.background, t),
    onBackground = lerp(from.onBackground, to.onBackground, t),
    surface = lerp(from.surface, to.surface, t),
    onSurface = lerp(from.onSurface, to.onSurface, t),
    surfaceVariant = lerp(from.surfaceVariant, to.surfaceVariant, t),
    onSurfaceVariant = lerp(from.onSurfaceVariant, to.onSurfaceVariant, t),
    outline = lerp(from.outline, to.outline, t),
    outlineVariant = lerp(from.outlineVariant, to.outlineVariant, t),
    error = lerp(from.error, to.error, t),
    onError = lerp(from.onError, to.onError, t),
    errorContainer = lerp(from.errorContainer, to.errorContainer, t),
    onErrorContainer = lerp(from.onErrorContainer, to.onErrorContainer, t),
    scrim = lerp(from.scrim, to.scrim, t),
    inverseSurface = lerp(from.inverseSurface, to.inverseSurface, t),
    inverseOnSurface = lerp(from.inverseOnSurface, to.inverseOnSurface, t),
    inversePrimary = lerp(from.inversePrimary, to.inversePrimary, t),
    surfaceContainer = lerp(from.surfaceContainer, to.surfaceContainer, t),
    surfaceContainerHigh = lerp(from.surfaceContainerHigh, to.surfaceContainerHigh, t),
    surfaceContainerHighest = lerp(from.surfaceContainerHighest, to.surfaceContainerHighest, t),
    surfaceContainerLow = lerp(from.surfaceContainerLow, to.surfaceContainerLow, t),
    surfaceContainerLowest = lerp(from.surfaceContainerLowest, to.surfaceContainerLowest, t),
    surfaceDim = lerp(from.surfaceDim, to.surfaceDim, t),
    surfaceBright = lerp(from.surfaceBright, to.surfaceBright, t)
)

private val AppTypography = Typography().run {
    copy(
        // 标题统一加粗一档,拉齐「标题 / 正文 / 标签」层次
        titleLarge = titleLarge.copy(fontWeight = FontWeight.SemiBold),
        titleMedium = titleMedium.copy(fontWeight = FontWeight.SemiBold, fontSize = 16.sp, lineHeight = 22.sp),
        titleSmall = titleSmall.copy(fontWeight = FontWeight.SemiBold),
        // 正文行高微松,长文本(帮助/新闻)更易读
        bodyLarge = bodyLarge.copy(lineHeight = 24.sp),
        bodyMedium = bodyMedium.copy(lineHeight = 21.sp),
        labelLarge = labelLarge.copy(letterSpacing = 0.1.sp)
    )
}

private val AppShapes = Shapes(
    extraSmall = RoundedCornerShape(6.dp),
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(14.dp),
    large = RoundedCornerShape(20.dp),
    extraLarge = RoundedCornerShape(26.dp)
)

/** mode: "system" | "light" | "dark";paletteKey: PALETTES 里的 key(设置页可选) */
@Composable
fun AssistantTheme(mode: String, paletteKey: String, content: @Composable () -> Unit) {
    val dark = when (mode) {
        "dark" -> true
        "light" -> false
        else -> isSystemInDarkTheme()
    }
    val spec = paletteFor(paletteKey)
    val target = if (dark) spec.dark else spec.light

    // Apple式全区域同时渐变:主体与窗口底色沿同一条进度线。
    // 关键:进度必须在「组合期」随 target 重建(remember(target)),第一帧即 0% ——
    // 若放到 LaunchedEffect 里 snapTo(0),会晚一帧:先闪一帧新色、再回跳旧色(闪两次)。
    val lastTarget = remember { mutableStateOf(target) }
    val fade = remember(target) {
        val from = lastTarget.value
        lastTarget.value = target
        from to Animatable(0f)
    }
    LaunchedEffect(target) {
        fade.second.animateTo(1f, tween(250, easing = FastOutSlowInEasing))
    }
    val colors = if (fade.second.value < 1f && fade.first != target) {
        lerpScheme(fade.first, target, fade.second.value)
    } else {
        target
    }

    MaterialTheme(
        colorScheme = colors,
        typography = AppTypography,
        shapes = AppShapes,
        content = content
    )
    // 窗口底色与主体同值同帧;系统条透明(edge-to-edge),看到的就是这个底色;
    // 状态栏图标明暗跟随背景实际亮度,渐变过半自然翻转,不再瞬间跳变。
    val view = LocalView.current
    SideEffect {
        var ctx = view.context
        while (ctx is ContextWrapper && ctx !is Activity) ctx = ctx.baseContext
        val window = (ctx as? Activity)?.window ?: return@SideEffect
        window.setBackgroundDrawable(ColorDrawable(colors.background.toArgb()))
        WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars =
            colors.background.luminance() > 0.5f
    }
}
