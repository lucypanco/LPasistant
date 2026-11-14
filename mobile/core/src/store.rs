//! 本地存储:SQLite(rusqlite bundled,自带 C 实现无需系统依赖)
//!
//! 表:
//! - reminders:提醒(id/text/kind/next_run(epoch 秒)/recur)
//! - kv:键值设置(默认城市、早报缓存、地理编码缓存等)

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, OptionalExtension};

use crate::api::err;
use crate::timeparse::{epoch_to_local, local_to_epoch, next_after};

pub struct Store {
    conn: Connection,
}

/// 到点处理结果
pub enum Fired {
    Text { text: String, next_run: i64 },
    Digest { next_run: i64 },
    Gone,
}

#[derive(Debug, Clone)]
pub struct ReminderRow {
    pub id: String,
    pub text: String,
    pub kind: String,
    pub next_run: i64,
    pub recur: Option<String>,
}

impl Store {
    pub fn open(path: &str) -> rusqlite::Result<Store> {
        if let Some(dir) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS reminders(
                id       TEXT PRIMARY KEY,
                text     TEXT NOT NULL,
                kind     TEXT NOT NULL DEFAULT 'text',
                next_run INTEGER NOT NULL,
                recur    TEXT,
                created  INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kv(k TEXT PRIMARY KEY, v TEXT NOT NULL);",
        )?;
        Ok(Store { conn })
    }

    // ---------- kv ----------
    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT v FROM kv WHERE k=?1", params![key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn kv_set(&self, key: &str, value: &str) {
        let _ = self.conn.execute(
            "INSERT INTO kv(k,v) VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=?2",
            params![key, value],
        );
    }

    // ---------- 提醒 ----------
    pub fn add_reminder(
        &mut self,
        text: &str,
        kind: &str,
        next_run: NaiveDateTime,
        recur: Option<&str>,
    ) -> Result<ReminderRow, CoreError2> {
        let id = uuid::Uuid::new_v4().simple().to_string()[..6].to_string();
        let epoch = local_to_epoch(next_run);
        self.conn
            .execute(
                "INSERT INTO reminders(id,text,kind,next_run,recur,created) VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    id,
                    text,
                    kind,
                    epoch,
                    recur,
                    local_to_epoch(chrono::Local::now().naive_local())
                ],
            )
            .map_err(|e| CoreError2(e.to_string()))?;
        Ok(ReminderRow {
            id,
            text: text.to_string(),
            kind: kind.to_string(),
            next_run: epoch,
            recur: recur.map(|s| s.to_string()),
        })
    }

    pub fn list_reminders(&self) -> Vec<ReminderRow> {
        let mut stmt = match self
            .conn
            .prepare("SELECT id,text,kind,next_run,recur FROM reminders ORDER BY next_run")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let map = |r: &rusqlite::Row| -> rusqlite::Result<ReminderRow> {
            Ok(ReminderRow {
                id: r.get(0)?,
                text: r.get(1)?,
                kind: r.get(2)?,
                next_run: r.get(3)?,
                recur: r.get(4)?,
            })
        };
        let rows = match stmt.query_map([], map) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        };
        rows
    }

    pub fn remove_reminder(&mut self, id_prefix: &str) -> Option<ReminderRow> {
        let found = self.list_reminders().into_iter().find(|r| r.id.starts_with(id_prefix))?;
        self.conn
            .execute("DELETE FROM reminders WHERE id=?1", params![found.id])
            .ok()?;
        Some(found)
    }

    pub fn remove_digests(&mut self) -> usize {
        let n = self
            .conn
            .execute("DELETE FROM reminders WHERE kind='digest'", [])
            .unwrap_or(0);
        n
    }

    /// 提醒到点:单次删除,循环顺延;kind=digest 时返回 Digest
    pub fn fire(&mut self, id: &str, now: NaiveDateTime) -> Result<Fired, CoreError2> {
        let row = self
            .conn
            .query_row(
                "SELECT id,text,kind,next_run,recur FROM reminders WHERE id=?1",
                params![id],
                |r| {
                    Ok(ReminderRow {
                        id: r.get(0)?,
                        text: r.get(1)?,
                        kind: r.get(2)?,
                        next_run: r.get(3)?,
                        recur: r.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|e| CoreError2(e.to_string()))?;
        let Some(row) = row else {
            return Ok(Fired::Gone);
        };
        let prev = epoch_to_local(row.next_run);
        let next = row.recur.as_deref().and_then(|r| next_after(prev, r, now));
        match next {
            Some(nxt) => {
                self.conn
                    .execute(
                        "UPDATE reminders SET next_run=?1 WHERE id=?2",
                        params![local_to_epoch(nxt), row.id],
                    )
                    .map_err(|e| CoreError2(e.to_string()))?;
            }
            None => {
                self.conn
                    .execute("DELETE FROM reminders WHERE id=?1", params![row.id])
                    .map_err(|e| CoreError2(e.to_string()))?;
            }
        }
        let next_run = next.map(|n| local_to_epoch(n)).unwrap_or(0);
        // 已过期的循环提醒顺延后可能仍在过去(极端时钟漂移),交给调度方兜底
        if row.kind == "digest" {
            Ok(Fired::Digest { next_run })
        } else {
            Ok(Fired::Text { text: row.text, next_run })
        }
    }
}

/// 存储层内部错误(避免循环依赖 api 模块)
pub struct CoreError2(pub String);

impl From<rusqlite::Error> for CoreError2 {
    fn from(e: rusqlite::Error) -> Self {
        CoreError2(e.to_string())
    }
}

impl From<CoreError2> for crate::api::CoreError {
    fn from(e: CoreError2) -> Self {
        err(e.0)
    }
}
