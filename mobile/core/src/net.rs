//! 共享 HTTP 客户端(reqwest blocking + rustls,Android 可直接交叉编译)
//!
//! 全部出站均为 https(2026-11-14 实测:人民网/中国天气网d1/央广HLS 证书均有效)。
//! 响应体一律经 read_capped 限量读取:异常/恶意源返回超大响应时不会撑爆内存。

use std::io::Read;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/124.0 Mobile Safari/537.36";

/// 响应体大小上限(按用途分级;超出即停止下载,调用方据 truncated 给用户提示)
pub const CAP_FEED: usize = 2 * 1024 * 1024; // RSS 订阅源(人民网实测约 380KB)
pub const CAP_PAGE: usize = 3 * 1024 * 1024; // 新闻文章页
pub const CAP_JSON: usize = 2 * 1024 * 1024; // 各类 JSON API(天气/电台/诗词接口)
pub const CAP_DATASET: usize = 20 * 1024 * 1024; // 用户自配的诗词数据集

pub fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .expect("构建 HTTP 客户端失败")
}

/// 响应体限量读取:最多读 max_bytes 字节,超限立即停止(不再继续下载)。
/// 返回 (文本, 是否被截断);截断时文本为前 max_bytes 字节,
/// 调用方应向用户提示「内容可能不完整」。读取失败返回空串(false)。
pub fn read_capped(resp: reqwest::blocking::Response, max_bytes: usize) -> (String, bool) {
    let mut limited = resp.take(max_bytes as u64 + 1);
    let mut buf = Vec::new();
    if limited.read_to_end(&mut buf).is_err() {
        return (String::new(), false);
    }
    let truncated = buf.len() > max_bytes;
    if truncated {
        buf.truncate(max_bytes);
    }
    (String::from_utf8_lossy(&buf).into_owned(), truncated)
}
