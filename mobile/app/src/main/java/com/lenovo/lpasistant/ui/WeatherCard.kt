package com.lenovo.lpasistant.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.lpasistant_core.WeatherChart

/**
 * 天气曲线卡:播报文本 + 24 小时温度折线(主题色)+ 每 3 小时采样表。
 * 曲线下小柱 = 降水成员占比(≥40% 时加深,提示降水时段);设计语言:细线/黑白灰/主题点睛。
 */
@Composable
fun WeatherCard(chart: WeatherChart) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = RoundedCornerShape(14.dp),
        tonalElevation = 1.dp,
        modifier = Modifier
            .widthIn(max = 330.dp)
            .fillMaxWidth(0.86f)
    ) {
        Column(Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
            Text(
                chart.summary,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurface
            )
            Spacer(Modifier.height(10.dp))
            TempCurve(chart)
            Spacer(Modifier.height(10.dp))
            ChartTable(chart)
        }
    }
}

@Composable
private fun TempCurve(chart: WeatherChart) {
    val pts = chart.points
    if (pts.size < 2) return
    val primary = MaterialTheme.colorScheme.primary
    val labelColor = MaterialTheme.colorScheme.onSurfaceVariant
    val measurer = rememberTextMeasurer()
    val labelStyle = remember(labelColor) {
        TextStyle(fontSize = 9.sp, color = labelColor)
    }
    val hiIdx = pts.indexOfFirst { it.temp == chart.hi }.coerceAtLeast(0)
    val loIdx = pts.indexOfFirst { it.temp == chart.lo }.coerceAtLeast(0)

    Canvas(
        Modifier
            .fillMaxWidth()
            .height(118.dp)
    ) {
        val n = pts.size
        val w = size.width
        val h = size.height
        val xAt = { i: Int -> w * i / (n - 1).coerceAtLeast(1) }
        // 温度域上下各留 0.5° 余量;y 主区占 78%(底部留湿占比柱)
        val loV = (chart.lo - 0.5).toFloat()
        val hiV = (chart.hi + 0.5).toFloat()
        val plotH = h * 0.78f
        val yAt = { t: Double -> plotH * (1f - ((t.toFloat() - loV) / (hiV - loV)).coerceIn(0f, 1f)) }

        // 降水成员占比小柱(每根宽 = 槽宽 60%)
        val slot = w / n
        val barW = slot * 0.6f
        val barMax = h - plotH - 14.dp.toPx()
        pts.forEachIndexed { i, p ->
            if (p.wet > 0.03) {
                val bh = (p.wet * barMax).toFloat().coerceAtLeast(2.dp.toPx())
                drawRect(
                    color = if (p.wet >= 0.4) primary.copy(alpha = 0.55f) else primary.copy(alpha = 0.22f),
                    topLeft = Offset(xAt(i) - barW / 2, h - bh),
                    size = Size(barW, bh)
                )
            }
        }

        // 温度折线 + 下方渐变填充
        val line = Path()
        pts.forEachIndexed { i, p ->
            val px = xAt(i); val py = yAt(p.temp)
            if (i == 0) line.moveTo(px, py) else line.lineTo(px, py)
        }
        val fill = Path().apply {
            addPath(line)
            lineTo(w, plotH)
            lineTo(0f, plotH)
            close()
        }
        drawPath(
            fill,
            brush = Brush.verticalGradient(
                listOf(primary.copy(alpha = 0.16f), Color.Transparent),
                startY = 0f, endY = plotH
            )
        )
        drawPath(
            line,
            color = primary,
            style = Stroke(2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round)
        )

        // 最高/最低点标注
        drawCircle(primary, radius = 3.5.dp.toPx(), center = Offset(xAt(hiIdx), yAt(pts[hiIdx].temp)))
        drawCircle(Color.Transparent, radius = 3.5.dp.toPx(), center = Offset(xAt(loIdx), yAt(pts[loIdx].temp)))
        drawCircle(primary.copy(alpha = 0.45f), radius = 3.dp.toPx(), center = Offset(xAt(loIdx), yAt(pts[loIdx].temp)))
        drawText(
            measurer,
            "${"%.0f".format(chart.hi)}°",
            topLeft = Offset((xAt(hiIdx) - 30.dp.toPx()).coerceAtLeast(0f), (yAt(pts[hiIdx].temp) - 20.dp.toPx()).coerceAtLeast(0f)),
            style = labelStyle
        )
        drawText(
            measurer,
            "${"%.0f".format(chart.lo)}°",
            topLeft = Offset((xAt(loIdx) - 30.dp.toPx()).coerceAtLeast(0f), (yAt(pts[loIdx].temp) + 6.dp.toPx()).coerceAtMost(h - 12.dp.toPx())),
            style = labelStyle
        )

        // x 轴:每 4 小时一个「N时」标签
        pts.forEachIndexed { i, p ->
            if (i % 4 == 0 && p.hour.isNotBlank()) {
                drawText(
                    measurer,
                    "${p.hour}时",
                    topLeft = Offset((xAt(i) - 8.dp.toPx()).coerceAtLeast(0f), h - 12.dp.toPx()),
                    style = labelStyle
                )
            }
        }
    }
}

/** 逐 3 小时采样表:时刻 / 温度 / 降水概率;≥40% 降水概率用主题色提示 */
@Composable
private fun ChartTable(chart: WeatherChart) {
    val pts = chart.points
    if (pts.isEmpty()) return
    val sampled = pts.filterIndexed { i, _ -> i % 3 == 0 }
    val labelColor = MaterialTheme.colorScheme.onSurfaceVariant
    val valueColor = MaterialTheme.colorScheme.onSurface
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
            Text("时刻", style = MaterialTheme.typography.bodySmall, color = labelColor, modifier = Modifier.weight(1f))
            Text("温度", style = MaterialTheme.typography.bodySmall, color = labelColor, modifier = Modifier.weight(1f))
            Text("降水概率", style = MaterialTheme.typography.bodySmall, color = labelColor, modifier = Modifier.weight(1f))
        }
        HorizontalDivider(thickness = 0.5.dp, color = labelColor.copy(alpha = 0.25f))
        sampled.forEachIndexed { idx, p ->
            val wet = (p.wet * 100).toInt()
            Row(
                Modifier.fillMaxWidth().padding(vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("${p.hour}时", style = MaterialTheme.typography.bodySmall, color = valueColor, modifier = Modifier.weight(1f))
                Text("%.1f°C".format(p.temp), style = MaterialTheme.typography.bodySmall, color = valueColor, modifier = Modifier.weight(1f))
                val wetText = if (p.wet >= 0.4) "$wet%·雨" else "$wet%"
                Text(
                    wetText,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (p.wet >= 0.4) MaterialTheme.colorScheme.primary else valueColor,
                    modifier = Modifier.weight(1f)
                )
            }
            if (idx != sampled.lastIndex) {
                HorizontalDivider(thickness = 0.5.dp, color = labelColor.copy(alpha = 0.12f))
            }
        }
    }
}
