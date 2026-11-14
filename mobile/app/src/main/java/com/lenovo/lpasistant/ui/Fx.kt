package com.lenovo.lpasistant.ui

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback

/** 记得创建 interactionSource 的快捷方式 */
@Composable
fun rememberPress(): MutableInteractionSource = remember { MutableInteractionSource() }

/**
 * 按压缩放反馈(替代大面积水波纹,风格更「黑标」)。
 * 用法:val ia = rememberPress(); Modifier.pressScale(ia).clickable(interactionSource = ia, indication = null) { … }
 */
fun Modifier.pressScale(
    interactionSource: MutableInteractionSource,
    pressedScale: Float = 0.97f
): Modifier = composed {
    val pressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed) pressedScale else 1f,
        animationSpec = spring(dampingRatio = 0.55f, stiffness = Spring.StiffnessMediumLow),
        label = "pressScale"
    )
    graphicsLayer { scaleX = scale; scaleY = scale }
}

/** 长按触觉 */
@Composable
fun rememberHaptic() = LocalHapticFeedback.current
