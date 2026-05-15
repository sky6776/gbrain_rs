//! 统一查询缓存 — 基于 query_hash + TTL 的结果缓存（§31）
//!
//! 缓存 unified_query 的结果，避免重复计算。
//! 缓存 key 由 query + strategy + limit + filters 组合计算。
//! 写入操作（upload_source, promotion_apply 等）自动失效相关缓存。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::types::UnifiedQueryResult;

/// 缓存条目
struct CacheEntry {
    value: UnifiedQueryResult,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// 统一查询缓存
pub struct QueryCache {
    entries: Mutex<HashMap<String, CacheEntry>>,
    max_entries: usize,
    default_ttl: Duration,
}

impl QueryCache {
    /// 创建查询缓存
    pub fn new(max_entries: usize, ttl_secs: u64) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            max_entries,
            default_ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// 生成缓存 key
    /// 修复：加入 filter_slug，避免 Provenance 策略按不同 slug 查不同页面时串缓存
    pub fn make_cache_key(
        query: &str,
        strategy: &str,
        limit: i64,
        include_evidence: bool,
        include_provenance: bool,
        filter_slug: Option<&str>,
    ) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        strategy.hash(&mut hasher);
        limit.hash(&mut hasher);
        include_evidence.hash(&mut hasher);
        include_provenance.hash(&mut hasher);
        filter_slug.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// 获取缓存值
    pub fn get(&self, key: &str) -> Option<UnifiedQueryResult> {
        let mut entries = self.entries.lock().ok()?;
        match entries.get(key) {
            Some(entry) if !entry.is_expired() => Some(entry.value.clone()),
            _ => {
                entries.remove(key);
                None
            }
        }
    }

    /// 写入缓存
    pub fn set(&self, key: String, value: UnifiedQueryResult) {
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= self.max_entries {
                // 淘汰最旧条目
                if let Some(oldest_key) = entries
                    .iter()
                    .min_by_key(|(_, e)| e.created_at)
                    .map(|(k, _)| k.clone())
                {
                    entries.remove(&oldest_key);
                }
            }
            entries.insert(
                key,
                CacheEntry {
                    value,
                    created_at: Instant::now(),
                    ttl: self.default_ttl,
                },
            );
        }
    }

    /// 失效所有缓存（写入操作后调用）
    pub fn invalidate_all(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    /// 失效按前缀匹配的缓存
    pub fn invalidate_by_prefix(&self, prefix: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|k, _| !k.starts_with(prefix));
        }
    }

    /// 获取当前条目数
    pub fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// 清理过期条目
    pub fn evict_expired(&self) -> usize {
        if let Ok(mut entries) = self.entries.lock() {
            let before = entries.len();
            entries.retain(|_, e| !e.is_expired());
            before - entries.len()
        } else {
            0
        }
    }
}
