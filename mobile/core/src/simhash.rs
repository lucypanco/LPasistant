//! 轻量 simhash:新闻跨源去重(中文按字符二元组切特征,无外部依赖)

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// simhash 主函数:中文按字符二元组切特征
pub fn hash(text: &str) -> u64 {
    let chars: Vec<char> = text
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let mut freq: HashMap<u64, u32> = HashMap::new();
    if chars.len() == 1 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        chars[0].hash(&mut h);
        *freq.entry(h.finish()).or_insert(1) += 1;
    }
    for w in chars.windows(2) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (w[0], w[1]).hash(&mut h);
        *freq.entry(h.finish()).or_insert(0) += 1;
    }
    let mut v = [0i32; 64];
    for (f, weight) in &freq {
        for bit in 0..64u32 {
            if (f >> bit) & 1 == 1 {
                v[bit as usize] += *weight as i32;
            } else {
                v[bit as usize] -= *weight as i32;
            }
        }
    }
    let mut out = 0u64;
    for (bit, x) in v.iter().enumerate() {
        if *x > 0 {
            out |= 1u64 << bit;
        }
    }
    out
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// 按相似度去重:保留先出现的,丢弃与已保留项相似度太高的(中文标题 hamming<=3 视为近似)
#[allow(dead_code)] // 测试使用;news.rs 走 index 级去重
pub fn dedup_titles(items: Vec<String>, max_dist: u32) -> Vec<String> {
    let mut kept: Vec<(String, u64)> = Vec::new();
    for it in items {
        let h = hash(&it);
        if kept.iter().any(|(_, kh)| hamming(*kh, h) <= max_dist) {
            continue;
        }
        kept.push((it, h));
    }
    kept.into_iter().map(|(s, _)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_dedup() {
        let a = "男子32元买彩票中1113万,过期两天兑奖被拒".to_string();
        let b = a.clone();
        let out = dedup_titles(vec![a, b], 3);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn different_kept() {
        let a = "今明两天北方有大范围降温雨雪天气过程".to_string();
        let b = "科研团队发布新一代量子计算原型机进展报告".to_string();
        let out = dedup_titles(vec![a, b], 3);
        assert_eq!(out.len(), 2);
    }
}
