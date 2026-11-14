# LPasistant APK — 构建指南(Kotlin Compose UI + Rust 核心)

> 架构:App 平时**不驻留内存**,提醒由 Android 系统闹钟(AlarmManager)调度,
> 到点拉起通知;查询类功能(天气/新闻/诗词/时间)打开 App 即问即答。
> Rust 核心(`core/`)负责:规则解析、时间解析、SQLite 存储、HTTP 抓取、
> 新闻 simhash 去重、诗词网络源;Kotlin 负责聊天 UI 和系统能力(闹钟/通知/开机恢复)。

```
mobile/
├─ core/                 Rust 核心(lpasistant_core crate)
│  ├─ src/api.rs         UniFFI 导出面(Android 唯一入口)
│  ├─ src/timeparse.rs   中文时间解析(14 用例单测)
│  ├─ src/intent.rs      指令调度(与旧 Python 版规则 1:1)
│  ├─ src/store.rs       SQLite(提醒/设置/缓存)
│  ├─ src/weather.rs     天气:中国天气网(实况+官方预警)/挪威气象局/GFS集合30成员
│  ├─ src/news.rs        新闻专业源白名单(国际+科技+时政,并行抓取+simhash去重)
│  ├─ src/poetry.rs      今日诗词API + 可配置数据集 + 内置79首兜底
│  ├─ src/llm.rs         可选AI兜底:prompt构造+字段校验+二步确认(无推理依赖)
│  └─ src/simhash.rs     标题去重算法
└─ app/                  Android 工程
   └─ src/main/java/com/lenovo/assistant/
      ├─ MainActivity.kt / ui/      Compose 聊天界面
      ├─ core/CoreBridge.kt         Rust 桥
      └─ sys/                       闹钟/通知/开机恢复
```

## 一、环境准备(Linux,推荐;你已有 rustup)

```bash
# 1) Rust 附加目标(Android 交叉编译)
rustup target add aarch64-linux-android x86_64-linux-android

# 2) cargo-ndk(把 Rust 编到 Android .so 的标准工具)
cargo install cargo-ndk

# 3) Android SDK + NDK(两种方式任选)
#    A. Android Studio:SDK Manager 里装 "NDK (Side by side)" 和 "SDK Platform 34"
#    B. 纯命令行:下载 commandlinetools,然后
#       sdkmanager "platforms;android-34" "ndk;27.0.12077973" "build-tools;34.0.0"
# 4) JDK 17(Android Studio 自带;命令行需自备)
```

环境变量(按你的实际路径):
```bash
export ANDROID_HOME=~/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973
```

## 二、验证 Rust 核心(WSL Ubuntu 实测:11/11 通过、0 警告)

```bash
cd mobile/core
cargo test          # 逻辑单测:时间解析(20+例)/存储/simhash/词库/LLM校验器
```

Windows 上有 WSL 时,项目根目录一条命令完成「同步→测试→生成绑定」:

```powershell
python wsl_test.py
```

⚠️ **踩坑记录**:release profile 的 `strip = true/"symbols"` 会剥掉 uniffi 元数据符号,
导致 `uniffi-bindgen generate --library` **静默生成空目录**(exit 0 但无文件)。
Cargo.toml 已改为 `strip = "debuginfo"`;WSL 的 /tmp 不跨会话持久,生成物请直写 /mnt/c。

## 三、编译 .so 并生成 Kotlin 绑定

```bash
cd mobile/core

# 1) 交叉编译出各 ABI 的 liblpasistant_core.so,自动放进 app/src/main/jniLibs/
cargo ndk -t arm64-v8a -t x86_64 -o ../app/src/main/jniLibs build --release

# 2) 生成 Kotlin 绑定(uniffi/lpasistant_core/…;绑定内容与 ABI 无关,
#    用未完全 strip 的任意 .so 生成即可,本工程已提交一份)
cargo run --bin uniffi-bindgen -- generate \
    --library target/debug/liblpasistant_core.so \
    --language kotlin \
    --out-dir ../app/src/main/java
```

## 四、构建 APK(✅ 已实测打通)

**完整工具链均已装进 WSL Ubuntu(JDK17/SDK/NDK/cargo-ndk),构建产物 `LPasistant-debug.apk` 在项目根目录。**
一键重建(WSL 内):`bash ~/gradle_build.sh`;Windows 侧:`python scripts\*` + 见下方踩坑记录。

```bash
cd mobile/core
# 1) 交叉编译 arm64 jniLibs(NDK r26c)
export ANDROID_HOME=~/android ANDROID_NDK_HOME=~/android/ndk/26.3.11579264
cargo ndk -t arm64-v8a -o ../app/src/main/jniLibs build --release
# 2) 生成 Kotlin 绑定(绑定内容与 ABI 无关,工程已提交一份)
cargo run --bin uniffi-bindgen -- generate \
    --library target/debug/liblpasistant_core.so \
    --language kotlin --out-dir ../app/src/main/java
# 3) 出 APK(WSL 原生目录构建,/mnt/c 上太慢)
cd ~/qqbot/mobile && ./gradlew assembleDebug
```

**踩坑全记录(均已解决)**:
- `strip = true/"symbols"` 剥掉 uniffi 元数据 → bindgen 静默空输出 → 改 `strip = "debuginfo"`
- Gradle 报 SDK 许可未接受 → 手写 `~/android/licenses/android-sdk-license`(标准哈希)
- uniffi 生成的 Kotlin 接口方法为 **camelCase**(`currentCoords`),实现类必须一致
- Rust `u32` → Kotlin `UInt`(`newsList/radioSearch` 参数需 `.toUInt()`)
- error 字段名 `message` 与 Kotlin `Throwable.message` 重载歧义 → 改名 `detail`
- MediaPipe `LlmInferenceOptions` 无 `setTemperature`(已移除)
- `dl.google.com` 下载限速 75KB/s → SDK 组件改用腾讯镜像手动解包(`scripts/wsl_install_sdk.sh`)
- TUNA rustup 镜像对 rust-std(android target)403 → 官方源直连下载手动解包(`scripts/rust_targets.sh`)

## 四·五、装到手机 + 电脑看运行(GUI)

1. 手机:设置 → 关于手机 → 连点"版本号"7次开启开发者模式 → 开发者选项里开「USB 调试」
2. 数据线连接电脑,手机上弹出授权时点「允许」
3. 双击 **`release\安装到手机.bat`**(自动 adb install + 启动 App)
4. 电脑屏幕镜像手机:`release\scrcpy\scrcpy.exe`(窗口显示并操作手机,延迟低)
5. 以后改了代码重建 APK:`wsl bash ~/gradle_build.sh` → 新 APK 在 `release\` → 再双击安装脚本

(模拟器路线搁置:google_apis 系统镜像无国内镜像源且 dl.google 限速,SDK/模拟器二进制已装好,
待有代理时 `sdkmanager "system-images;android-34;google_apis;x86_64"` 补装即可;AVD 已预注册)

```bash
cd mobile
./gradlew assembleDebug     # 产物: app/build/outputs/apk/debug/app-debug.apk
# 或直接用 Android Studio 打开 mobile/ 目录点运行
```
adb 安装:`adb install app/build/outputs/apk/debug/app-debug.apk`

## 五、首次启动

1. 允许**通知**权限(弹窗);
2. 系统设置里给 App 打开"**闹钟和提醒**"(精确闹钟)权限——否则提醒降级为 ±1 分钟窗口;
3. 聊天框输入指令即可,提醒到点即使 App 已关闭也会弹通知;
4. 手机重启后提醒自动恢复(BootReceiver)。

## 六、新闻源与诗词数据集

**新闻源(`core/src/news.rs` 的 `FEEDS` 白名单,按"专业、无娱乐"原则筛选):**
- 国际综合:CNBC(SCMP/CNA/DW 等以注释保留,可达时启用)
- 国外科技·科研:The Verge、Ars Technica、TechCrunch、Hacker News、Nature
- 国内科技:Solidot、爱范儿、少数派
- 国内时政:人民网时政频道

**诗词数据集(可选):**

随机诗词默认走"今日诗词 API"(免Key)。想扩成 5 万+ 库:

```
设置诗词源 https://你可达的镜像/chinese-poetry/shijing.json
```
- 数据格式:JSON 数组,每项含 `title` + `content`(或 `paragraphs`/`para`)、可选 `author`/`dynasty`
- 成功后自动缓存 500 首进 SQLite,离线可用
- `清除诗词源` 恢复默认

## 七、可选:AI 兜底(纯离线本地模型,默认关闭;在「设置」页开启)

定位:**不是聊天机器人、不是 agent**。仅当规则解析没看懂提醒话术时,由本地小模型
把句子"翻译"成受限 JSON 字段;字段范围校验、日期计算全部由本地确定性代码把守——
**LLM 永远不直接决定闹钟时刻**。

- 开关位置:App 右上角「设置」→ AI 兜底(无需聊天指令)
- **Harness 四道闸 + 多轮追问(fail-closed,防小模型误触发)**:
  1. 作用域闸:只有「提醒我」等前缀句才可能进模型(宽模式:未匹配句),普通提问根本到不了 LLM
  2. 疑问句预闸门:句尾问号或含疑问词(什么/怎么/谁…)→ 纯规则直接拒绝,零推理
  3. Prompt 反例 + **ask 出口**:模型识别出"是提醒但缺时间"(如「提醒我晚上吃完饭写作业」)
     → 输出 ask + 自然语言追问,而不是硬编时间或拒绝
  4. 生成器→校验器两段式:草稿生成后第二遍推理自检"JSON 是否吻合原句意图",不是「是」一律丢弃
  - **多轮追问状态机**:ask 后挂起(记住事项,最多2轮)→ 用户回复时间 →
    规则解析优先合并(确定性)→ 草稿 + [确认/显示表单/取消] 按键;
    回复「取消」放弃;用户切换话题(天气/新闻等指令)自动保留追问状态不丢失;
    追问两轮仍未定 → 给出明确格式建议
  - 即使全漏,落库仍需「确认」或表单终审(人类终审)
- **两种兜底模式(设置页选择)**:
  - **窄(默认)**:只有「提醒我…」等前缀句解析失败才触发模型,其余输入永不进模型
  - **宽:全域路由器** —— 任何没被规则听懂的句子都尝试路由到五大能力:
    reminder(提醒,缺时间追问)/ weather(城市,缺城市追问)/ news / poetry / radio;
    只读查询直接执行,提醒仍走「生成→自检→确认按键」高危闸;每句最多 2 次本地推理
- **多轮追问状态机(两模式共用)**:缺时间/缺城市 → 追问(记住上下文,最多2轮)→
  用户补充 → 规则确定性解析优先合并 → 确认按键;「取消」放弃;切换话题状态保留
- 模型:Qwen2.5-0.5B-Instruct GGUF Q4_K_M(约400MB,ModelScope/HF 可下)
- 放置:`adb push model.gguf /sdcard/Android/data/com.lenovo.lpasistant/files/model.gguf`(即时生效)
- 流程:规则失败 → 本地推理 → 本地校验+计算 → 回显,气泡下附三个**显式按键**:
  [✅确认] 直接落库 / [📋显示表单] 查看修改 / [✖取消] 放弃(简单交互一律按键,复杂输入才回复)
- 「我的提醒」为结构化卡片,每条自带 [取消] 键
- 推理走 MediaPipe GenAI;不想要该能力可删掉此依赖与 `LlmController.kt`

**「显示表单」是死规则直连**:表单字段(事项/日期/时间/重复)就是最终值,
保存走 `save_reminder_direct`,不经过任何文本解析——即 LLM 只负责"起草",
用户可以在表单里纠正,再由确定性代码落库。

## 八、天气架构(2026-11-13 重构,数据源实测)

- **城市天气**:`d1.weather.com.cn`(中国天气网)——城市站点实况+风向风力+温度,
  附**中央气象台官方预警**(类型/级别/发布时间);免Key、国内直连
- **定位综合评价**(设置开启定位后):坐标直供
  `api.met.no`(挪威气象局实况:温/湿/风/1h降水)+ `ensemble-api.open-meteo.com`
  **GFS集合30成员**(中位温度/最低最高/一致性分级/降水成员占比)+ 穿衣建议
- 内置 38 主要城市代码表;未收录城市 → `geocoding-api.open-meteo.com` 反查坐标(✅可达)
- ⚠️ `api.open-meteo.com` 主域被墙(实测000)——已全部切离;其余子域可达
- 隐私:坐标仅发 MET挪威/集合API(算天气所必需);不存历史、不做逆地理编码

## 十、新闻详情(点标题 → 抓原文 + AI摘要)

- 快捷词条「新闻/国际」出**可点击**新闻卡片(链接来自白名单源 RSS)
- 点标题 → Rust 抓文章页 + 提取正文(`<p>` 段落优先,纯代码,无AI)
- **AI摘要**:仅当设置里 AI兜底开启且模型就绪时附加(0.5B,输入截断900字符,输出≤3句);
  未开启/失败自动降级为**规则抽取要点**(导语式新闻前几句即重点),永不阻塞
- 架构边界:本地AI不上网——抓网页是确定性代码,AI 只压缩信息;点击触发,不后台轮询

## 十一、电台(在线流媒体)

- 精选台(本环境实测可达):央广·中国之声/经济之声(国内HLS)、Radio Paradise、Nightride FM、KEXP(国际mp3)
- 快捷词条「电台」→ 电台卡片,点 ▶ 播放、⏹ 停止;播放中顶部输入框上方出现**迷你播放条**
- 文本指令:`电台` 看列表;`电台 搜索 <关键词>` → **radio-browser 开放API**(免Key,全球数万台,支持中文,逐镜像回退)
- 播放:Media3/ExoPlayer(`media3-exoplayer` + `media3-exoplayer-hls`),自动识别 HLS/Icecast
- MVP 边界:App 前台可听;**后台播放需 MediaSessionService 前台服务**,列为后续增强
- 国际台可达性因网络而异(本沙箱实测 SomaFM/FIP 受限,搜索可自行找可达台)

## 九、卸载与清理(根目录 uninstall.py)

```bash
python uninstall.py             # 清理本机构建产物(cargo-ndk产物/生成绑定/build/target)
python uninstall.py --adb       # 额外:卸载手机上的 App(adb uninstall,同时清其内部 SQLite)
python uninstall.py --data      # 额外:删除手机端模型目录(/sdcard/Android/data/<包名>)
```

- 源代码与文档永远保留;jniLibs 与 uniffi 绑定删后可用 `cargo ndk` / `uniffi-bindgen` 重新生成
- 项目根目录现仅含:`mobile/`(新项目)、`uninstall.py`、`模电知识总结.md`(个人笔记);
  旧 QQ 方案系列文件已于 2026-11-08 全部删除

## 常见问题

- **uniffi 绑定生成报错**:确认 Cargo.toml 里 uniffi 版本与 `setup_scaffolding!()` 一致(本项目 0.29);`cargo build` 必须在 bindgen 之前成功。
- **UnsatisfiedLinkError**:jniLibs 里没有对应 ABI 的 .so → 重跑第三章第 1 步。
- **cargo test 在 Windows 跑不了**:Windows 本地测试需要 MSVC Build Tools;Linux 无此问题。
- **新闻某条源失败**:多源并行抓取、单源失败自动跳过;要增删源编辑 `core/src/news.rs` 里的 `FEEDS` 白名单(注释里保留了 SCMP/CNA/DW 等备用源,网络可达时取消注释即可)。
- 旧 QQ 方案系列文件已于 2026-11-08 按用户要求全部删除,项目根目录只含 `mobile/`、`uninstall.py` 与个人笔记。
