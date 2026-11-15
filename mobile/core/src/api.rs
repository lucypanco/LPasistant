//! UniFFI 导出的 API 面:Android(Kotlin)唯一接触点。
//!
//! 约定:
//! - 全部为同步函数(Kotlin 侧在 Dispatchers.IO 调用),网络功能内部使用
//!   reqwest blocking 客户端,避免跨 FFI 异步的复杂度。
//! - 提醒到点的"调度"由 Android 侧 AlarmManager 负责;引擎只负责
//!   解析、存储、到点时给出文本并顺延循环提醒(reminder_fired)。

use std::sync::Mutex;

use crate::store::Store;

// ---------- 导出类型 ----------

#[derive(Debug, uniffi::Error)]
pub enum CoreError {
    Failure { detail: String },
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::Failure { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl CoreError {
    /// 统一取错误文案(intent 等模块友好展示用)
    pub fn msg(&self) -> &str {
        match self {
            CoreError::Failure { detail } => detail.as_str(),
        }
    }
}

pub(crate) fn err(msg: impl Into<String>) -> CoreError {
    CoreError::Failure { detail: msg.into() }
}

/// 提醒条目(列表/重调度用)
#[derive(Debug, uniffi::Record)]
pub struct ReminderRecord {
    pub id: String,
    pub text: String,
    /// "text" 普通提醒 | "digest" 早报订阅
    pub kind: String,
    /// 下次触发的 Unix 秒级时间戳
    pub next_run: i64,
    /// "" 单次 | "daily" | "weekly:<0-6>"(0=周一)
    pub recur: String,
}

/// 提醒到点的处理结果
#[derive(Debug, uniffi::Enum)]
pub enum FireOutcome {
    /// 普通提醒:text 为要通知的内容;next_run>0 表示循环提醒已顺延,需要重新调度闹钟
    Text { text: String, next_run: i64 },
    /// 早报订阅到点:调用 compose_digest() 组装内容;next_run 同上
    Digest { next_run: i64 },
    /// 提醒不存在(已取消/已触发过)
    Gone,
}

/// 宿主实现的位置回调:返回 "lat,lon" 字符串;未授权/失败返回空串。
/// 坐标仅用于天气直供 Open-Meteo,不落库、不转发第三方。
#[uniffi::export(callback_interface)]
pub trait LocationProvider: Send + Sync {
    fn current_coords(&self) -> String;
}

/// 新闻条目(结构化,UI 渲染可点击行 → 点了走 news_detail)
/// cached_at > 0 = 来自离线缓存(值为缓存保存的 Unix 秒),UI 显示「离线缓存」标注
#[derive(Debug, uniffi::Record)]
pub struct NewsItemRecord {
    pub source: String,
    pub title: String,
    pub url: String,
    pub cached_at: i64,
}

/// 电台条目(播放由 Kotlin Media3 执行)
#[derive(Debug, uniffi::Record)]
pub struct RadioStationRecord {
    pub name: String,
    pub url: String,
    pub desc: String,
    pub region: String,
}

/// 提醒表单草稿(LLM 兜底回显用;表单保存走 save_reminder_direct,死规则直连)
#[derive(Debug, uniffi::Record)]
pub struct ReminderDraft {
    pub text: String,
    /// Unix 秒
    pub next_run: i64,
    /// "" 单次 | "daily" | "weekly:<0-6>" | "monthly:<1-31>"
    pub recur: String,
}

/// 宿主实现的本地推理回调:Android 侧加载小模型(如 Qwen2.5-0.5B GGUF),
/// Rust 核心零推理依赖;模型缺席时兜底整体关闭,行为与纯规则版一致。
#[uniffi::export(callback_interface)]
pub trait LlmInfer: Send + Sync {
    fn infer(&self, prompt: String) -> String;
}

// ---------- 核心对象 ----------

#[derive(uniffi::Object)]
pub struct Assistant {
    pub(crate) store: Mutex<Store>,
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) clarify: Mutex<Option<crate::llm::ClarifyState>>,
    pub(crate) pending: Mutex<Option<crate::llm::PendingReminder>>,
    pub(crate) llm_cb: Mutex<Option<Box<dyn LlmInfer>>>,
    pub(crate) location_cb: Mutex<Option<Box<dyn LocationProvider>>>,
}

#[uniffi::export]
impl Assistant {
    /// db_path:Android 侧传 context.getDatabasePath("assistant.db") 的绝对路径
    #[uniffi::constructor]
    pub fn new(db_path: String) -> Result<Self, CoreError> {
        let store = Store::open(&db_path).map_err(|e| err(format!("数据库初始化失败:{e}")))?;
        Ok(Self {
            store: Mutex::new(store),
            http: crate::net::client(),
            clarify: Mutex::new(None),
            pending: Mutex::new(None),
            llm_cb: Mutex::new(None),
            location_cb: Mutex::new(None),
        })
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// 处理一条文本指令,返回给用户的回复(可能含网络请求,放 IO 线程调用)
    pub fn handle_command(&self, input: String) -> Result<String, CoreError> {
        Ok(crate::intent::dispatch(self, &input))
    }

    /// 全部提醒(Kotlin 用它来注册/恢复闹钟;每次指令后重新同步即可)
    pub fn list_reminders(&self) -> Result<Vec<ReminderRecord>, CoreError> {
        let store = self.store.lock().map_err(|_| err("存储忙"))?;
        Ok(store
            .list_reminders()
            .into_iter()
            .map(|r| ReminderRecord {
                id: r.id,
                text: r.text,
                kind: r.kind,
                next_run: r.next_run,
                recur: r.recur.unwrap_or_default(),
            })
            .collect())
    }

    /// 某条提醒到点了:返回要通知的内容,并把循环提醒顺延(单次的直接删除)
    pub fn reminder_fired(&self, id: String) -> Result<FireOutcome, CoreError> {
        let mut store = self.store.lock().map_err(|_| err("存储忙"))?;
        match store.fire(&id, chrono::Local::now().naive_local())? {
            crate::store::Fired::Text { text, next_run } => {
                Ok(FireOutcome::Text { text, next_run })
            }
            crate::store::Fired::Digest { next_run } => Ok(FireOutcome::Digest { next_run }),
            crate::store::Fired::Gone => Ok(FireOutcome::Gone),
        }
    }

    /// 组装完整早报(天气+要闻+今日诗词;网络请求,放 IO 线程)
    pub fn compose_digest(&self) -> Result<String, CoreError> {
        crate::digest::compose(self)
    }

    // ---- 简单设置读写(Kotlin 设置页可用) ----
    pub fn get_setting(&self, key: String) -> Result<String, CoreError> {
        let store = self.store.lock().map_err(|_| err("存储忙"))?;
        Ok(store.kv_get(&key).unwrap_or_default())
    }

    pub fn set_setting(&self, key: String, value: String) -> Result<(), CoreError> {
        let store = self.store.lock().map_err(|_| err("存储忙"))?;
        store.kv_set(&key, &value);
        Ok(())
    }

    // ---- 可选本地 LLM 兜底 ----
    pub fn attach_llm(&self, cb: Box<dyn LlmInfer>) {
        if let Ok(mut g) = self.llm_cb.lock() {
            *g = Some(cb);
        }
    }

    pub fn detach_llm(&self) {
        if let Ok(mut g) = self.llm_cb.lock() {
            *g = None;
        }
    }

    pub fn llm_ready(&self) -> bool {
        self.llm_cb.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    // ---- 定位(仅用于天气;设置中开启并授权后才生效) ----
    pub fn attach_location(&self, cb: Box<dyn LocationProvider>) {
        if let Ok(mut g) = self.location_cb.lock() {
            *g = Some(cb);
        }
    }

    pub fn geo_enabled(&self) -> bool {
        self.store
            .lock()
            .ok()
            .map(|s| s.kv_get("geo_enabled").as_deref() == Some("1"))
            .unwrap_or(false)
    }

    pub fn weather_here(&self, lat: f64, lon: f64) -> Result<String, CoreError> {
        crate::weather::here(self, lat, lon)
    }

    // ---- 新闻(结构化:UI 渲染可点击行) ----
    /// kind: "all" | "intl" | "tech" | "politics"
    pub fn news_list(&self, kind: String, n: u32) -> Result<Vec<NewsItemRecord>, CoreError> {
        Ok(crate::news::news_items(self, &kind, n as usize)?
            .into_iter()
            .map(|it| NewsItemRecord {
                source: it.source,
                title: it.title,
                url: it.url,
                cached_at: it.cached_at,
            })
            .collect())
    }

    /// 新闻详情:抓文章页+提取正文;AI摘要仅在 AI兜底开启且模型就绪时附加
    pub fn news_detail(&self, url: String, title: String) -> Result<String, CoreError> {
        crate::news::detail(self, &url, &title)
    }

    // ---- 电台(数据/搜索在 Rust;播放由 Kotlin Media3 完成) ----
    pub fn radio_stations(&self) -> Vec<RadioStationRecord> {
        crate::radio::all_stations(self)
            .into_iter()
            .map(|s| RadioStationRecord {
                name: s.name,
                url: s.url,
                desc: s.desc,
                region: "内置".to_string(),
            })
            .collect()
    }

    pub fn radio_search(&self, query: String, n: u32) -> Result<Vec<RadioStationRecord>, CoreError> {
        Ok(crate::radio::search(self, &query, n as usize)?
            .into_iter()
            .map(|h| RadioStationRecord {
                name: h.name,
                url: h.url,
                desc: h.desc,
                region: h.region,
            })
            .collect())
    }

    // ---- 提醒表单(死规则直连:绕过一切解析,字段即最终值) ----
    /// 当前待确认的 LLM 草稿(有则聊天回复可附「显示表单」按钮)
    pub fn pending_draft(&self) -> Option<ReminderDraft> {
        let g = self.pending.lock().ok()?;
        g.as_ref().map(|d| ReminderDraft {
            text: d.text.clone(),
            next_run: crate::timeparse::local_to_epoch(d.when),
            recur: d.recur.clone().unwrap_or_default(),
        })
    }

    /// 表单保存:字段即最终值,不做任何解析;同时清除待确认草稿
    pub fn save_reminder_direct(
        &self,
        text: String,
        next_run: i64,
        recur: String,
    ) -> Result<String, CoreError> {
        if let Ok(mut g) = self.pending.lock() {
            *g = None;
        }
        let when = crate::timeparse::epoch_to_local(next_run);
        let mut store = self.store.lock().map_err(|_| err("存储忙"))?;
        let recur_opt = if recur.is_empty() { None } else { Some(recur.as_str()) };
        let rec = store
            .add_reminder(&text, "text", when, recur_opt)
            .map_err(|e| err(format!("保存失败:{}", e.0)))?;
        Ok(format!(
            "✅ 提醒已设置 [{}]\n⏰ {} {}\n📝 {}\n取消发送「取消提醒 {}」",
            rec.id,
            crate::timeparse::human_dt(&when),
            crate::intent::recur_label(recur_opt),
            text,
            rec.id
        ))
    }

    // ---- 显式按键交互(简单交互=按键,复杂输入=回复) ----
    /// [确认] 键:落库待确认草稿
    pub fn confirm_pending(&self) -> Result<String, CoreError> {
        let draft = self.pending.lock().map_err(|_| err("存储忙"))?.take();
        let Some(d) = draft else {
            return Ok("没有待确认的提醒".to_string());
        };
        let mut store = self.store.lock().map_err(|_| err("存储忙"))?;
        match store.add_reminder(&d.text, "text", d.when, d.recur.as_deref()) {
            Ok(rec) => Ok(format!(
                "✅ 提醒已设置 [{}]\n⏰ {} {}\n📝 {}\n到点我来喊你;取消发送「取消提醒 {}」",
                rec.id,
                crate::timeparse::human_dt(&d.when),
                crate::intent::recur_label(d.recur.as_deref()),
                d.text,
                rec.id
            )),
            Err(e) => Err(err(format!("保存提醒失败:{}", e.0))),
        }
    }

    /// [取消] 键:放弃待确认草稿
    pub fn cancel_pending(&self) -> String {
        let had = self
            .pending
            .lock()
            .map(|mut g| g.take().is_some())
            .unwrap_or(false);
        if had {
            "好的,已放弃这条提醒".to_string()
        } else {
            "没有待确认的提醒".to_string()
        }
    }

    /// 提醒列表逐条 [取消] 键:按完整 id 删除
    pub fn cancel_reminder(&self, id: String) -> Result<String, CoreError> {
        let mut store = self.store.lock().map_err(|_| err("存储忙"))?;
        match store.remove_reminder(&id) {
            Some(r) => Ok(format!("🗑 已取消提醒「{}」", r.text)),
            None => Ok("没有找到这个编号".to_string()),
        }
    }
}

// 非导出的内部方法
impl Assistant {
    pub(crate) fn infer(&self, prompt: &str) -> String {
        match self.llm_cb.lock() {
            Ok(g) => match g.as_ref() {
                Some(c) => c.infer(prompt.to_string()),
                None => String::new(),
            },
            Err(_) => String::new(),
        }
    }

    pub(crate) fn llm_enabled(&self) -> bool {
        self.store
            .lock()
            .ok()
            .map(|s| s.kv_get("llm_enabled").as_deref() == Some("1"))
            .unwrap_or(false)
    }

    pub(crate) fn current_coords(&self) -> Option<String> {
        let g = self.location_cb.lock().ok()?;
        g.as_ref()
            .map(|c| c.current_coords())
            .filter(|s| !s.trim().is_empty())
    }
}
