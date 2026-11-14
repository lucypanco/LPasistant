# LPasistant

**免 AI 的安卓个人助手**:定时提醒 / 天气 / 专业新闻 / 古诗词 / 网络电台,全部由本地规则驱动——Rust 核心 + Kotlin Jetpack Compose UI,数据不出手机。

> 设计理念:**能用确定性代码解决的事,不引入模型;引入模型的地方,只让它做受限翻译,永不给它决定权。**

---

## 特性

- **⏰ 定时提醒**——中文自然语言解析(`提醒我 明天 07:30 起床` / `提醒我 每周五 18:00 交周报` / `30分钟后 喝水`),由系统精确闹钟(AlarmManager)调度,App 平时不驻留、重启自动恢复
- **🌦 天气**——中国天气网城市站实况 + 中央气象台官方预警;开启定位后支持"当前位置"综合评价(挪威气象局 + GFS 集合 30 成员分歧度)
- **📰 新闻**——专业源白名单(国际 + 科技·科研 + 时政),Rust 并行抓取 + simhash 去重,国内外源对半交错;点标题抓原文正文
- **📖 古诗词**——今日诗词 API + 可配置数据集 + 内置 79 首兜底
- **📻 电台**——央广 HLS + 国际台精选,radio-browser 全球搜索
- **🌅 早报订阅**——每天定时推送一条:天气 + 要闻 + 今日诗词
- **🧩 可选本地 AI 兜底**(默认关闭)——规则没听懂时,由本地小模型(Qwen2.5-0.5B,纯离线)把句子"翻译"成受限 JSON 字段;字段校验、日期计算、二次确认全部在本地代码完成

## 安全与隐私设计

这个项目按"不留攻击接口、不危害手机系统"的标准做过一次全面审计:

- **零入站面**:无监听端口、无导出服务/Provider、无深链、无 WebView;导出组件仅启动 Activity 与系统保护广播接收器
- **权限最小化**:INTERNET / 通知 / 精确闹钟 / 开机广播 / 唤醒锁 / 定位——没有存储、无障碍、设备管理等任何危险权限
- **出站全白名单 + 全 HTTPS**:所有数据源硬编码,响应体限量读取(防恶意源撑爆内存),离线时回退最近缓存并标注保存时间
- **LLM 永不直接决定闹钟**:模型输出只进草稿,字段边界校验 + 二次自检 + 显式「确认」按键,四道闸 fail-closed;模型未开启时行为与纯规则版完全一致
- **数据全部本地**:SQLite 只存提醒与设置,坐标仅直供天气计算、不落库不转发;无账号、无统计、无追踪

## 架构

```
┌─ Kotlin (Jetpack Compose) ─────────────┐
│  聊天 UI / 系统闹钟调度 / 通知 / 电台播放 │
└──────────────┬─────────────────────────┘
               │ UniFFI(唯一桥接面)
┌──────────────┴─────────────────────────┐
│  Rust 核心 (lpasistant_core)            │
│  指令解析 / 中文时间解析 / SQLite 存储    │
│  HTTPS 抓取(限量)/ simhash / 缓存      │
└────────────────────────────────────────┘
```

- `mobile/core/` —— Rust 核心(crate `lpasistant_core`),含单元测试
- `mobile/app/` —— Android 工程(Kotlin + Compose)
- `scripts/` —— WSL 构建脚本(SDK 安装 / 交叉编译 / Gradle 构建)

## 构建

前置:WSL Ubuntu(rustup + JDK 17 + Android SDK/NDK),详见 [mobile/BUILD.md](mobile/BUILD.md)。

```bash
# 1) 验证核心逻辑 + 生成 UniFFI Kotlin 绑定(Windows 一键)
python wsl_test.py

# 2) 交叉编译 Rust → liblpasistant_core.so
wsl bash /mnt/c/qq-bot/scripts/crossbuild.sh    # 路径按实际仓库位置调整

# 3) Gradle 打包 release APK
wsl bash scripts/gradle_build.sh
```

或直接用 Android Studio 打开 `mobile/` 构建(需先完成第 1 步生成绑定)。

可选本地模型:把 `model.gguf`(Qwen2.5-0.5B-Instruct Q4,约 400MB)放到 `Android/data/com.lenovo.lpasistant/files/`,设置页开启「AI 兜底」即可;不放文件则完全不影响其余功能。

## 使用示例

```
提醒我 晚上8点 跑步
提醒我 每天 08:00 早读
天气          (设置页可开定位/改默认城市)
新闻 / 国际 / 科技 / 时政
诗词 / 诗词 励志
电台 / 电台 搜索 爵士
订阅早报 07:30
```

## 许可证

[MIT](LICENSE)
