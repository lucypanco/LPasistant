//! LPasistant核心引擎(Rust)
//!
//! 设计:App 平时不驻留,提醒由 Android AlarmManager 调度;
//! 本引擎是"命令处理器 + 本地存储",接收文本指令返回回复,
//! 并把提醒的解析/存储/顺延逻辑收敛在这里(跨平台可测)。

mod api;
mod cache;
mod digest;
mod intent;
mod llm;
mod net;
mod news;
mod poetry;
mod radio;
mod simhash;
mod sources;
mod store;
mod timeparse;
mod weather;

uniffi::setup_scaffolding!();
