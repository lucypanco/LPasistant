package com.lenovo.lpasistant.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.lenovo.lpasistant.R

/** App 式使用帮助(次级页):分类卡片,替代终端式文本 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HelpScreen(onBack: () -> Unit) {
    // 容器透明:顶栏透出根级背景,与主体同一条渐变进度线(同 SettingsScreen)
    Scaffold(
        containerColor = Color.Transparent,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent),
                title = { Text("使用帮助") },
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
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Card(R.drawable.ic_alarm, "提醒", listOf(
                "提醒我 18:00 开会 —— 今天该时刻,已过则明天",
                "提醒我 30分钟后 喝水 / 半小时后 吃药",
                "提醒我 明天 07:30 起床 / 后天 9点",
                "提醒我 每天 08:00 早读(每天循环)",
                "提醒我 每周五 18:00 交周报(每周循环)",
                "提醒我 每月15号 10:00 交房租(每月循环)",
                "提醒我 晚上8点 跑步 / 下周一 9:00 例会 / 月底 复盘",
                "我的提醒 → 已移至「设置 → 提醒管理」,可逐条取消",
                "到点会弹系统通知;MIUI 请允许自启动,避免拦截"
            ))
            Card(R.drawable.ic_weather, "天气与定位", listOf(
                "天气 —— 默认城市实况 + 官方气象预警(中国天气网)",
                "天气 上海 —— 指定城市",
                "设置里开「自动定位」→ 天气 变为:挪威气象局实况 +",
                "  GFS集合30成员分歧度 + 降水概率 + 穿衣建议",
                "设置城市 上海 —— 修改默认城市",
                "注:非主要城市按坐标计算,数据为模式预报而非站点观测"
            ))
            Card(R.drawable.ic_news, "新闻", listOf(
                "新闻 —— 专业源(人民网时政+国际科技媒体)",
                "国际 / 科技 / 时政 —— 分类查看",
                "新闻 5 —— 控制条数;点标题抓原文,可附AI摘要",
                "数据源不含娱乐热搜;点卡片标题即抓取原文详情"
            ))
            Card(R.drawable.ic_book, "诗词", listOf(
                "诗词 —— 随机一首(在线词库,离线兜底内置79首)",
                "诗词 今日 —— 每天固定一首",
                "诗词 励志 / 思乡 / 送别 —— 按主题",
                "诗词 李白 / 宋 —— 按作者 / 朝代"
            ))
            Card(R.drawable.ic_radio, "电台", listOf(
                "电台 —— 精选台(央广中国之声/经济之声+国际台)",
                "电台 搜索 爵士 —— 全球电台搜索(radio-browser)",
                "播放中底部出现迷你播放条,点停止键结束"
            ))
            Card(R.drawable.ic_bell, "订阅与杂项", listOf(
                "订阅早报 07:30 —— 每天:天气+要闻+今日诗词",
                "取消订阅",
                "设置城市 上海 —— 修改天气默认城市"
            ))
            Card(R.drawable.ic_chip, "AI 兜底(可选,默认关)", listOf(
                "设置 → AI 兜底 开启;模型:model.gguf 放入 App 外部目录",
                "规则没看懂时,本地小模型理解提醒意图;缺时间会追问",
                "宽模式:任何句子都尝试识别提醒;纯离线,无云端"
            ))
            Card(R.drawable.ic_info, "关于", listOf(
                "纯规则解析,不连接任何云端AI(可选本地模型兜底)",
                "提醒用系统精确闹钟,重启自动恢复",
                "隐私:天气坐标仅发 Open-Meteo/挪威气象局;新闻抓取不含个人信息"
            ))
        }
    }
}

@Composable
private fun Card(iconRes: Int, title: String, lines: List<String>) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = MaterialTheme.shapes.medium,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    painterResource(iconRes),
                    contentDescription = null,
                    modifier = Modifier.size(17.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.width(7.dp))
                Text(title, style = MaterialTheme.typography.titleSmall)
            }
            HorizontalDivider(Modifier.padding(vertical = 6.dp))
            lines.forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
        }
    }
}
