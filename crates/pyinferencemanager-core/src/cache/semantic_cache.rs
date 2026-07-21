use crate::types::{CacheEntry, CacheHit};
use crate::Result;
use super::sqlite_store::SqliteCacheStore;
use super::embedding_key::EmbeddingKey;
use uuid::Uuid;

pub struct SemanticCache {
    store: SqliteCacheStore,
    similarity_threshold: f32,
    ttl_seconds: u64,
}

impl SemanticCache {
    pub fn new(db_path: &str, ttl_seconds: u64) -> Result<Self> {
        let store = SqliteCacheStore::new(db_path)?;
        Ok(SemanticCache {
            store,
            similarity_threshold: 0.88,
            ttl_seconds,
        })
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold;
        self
    }

    pub async fn lookup(&self, description: &str, task_kind: &str, _content: &[u8]) -> Result<Option<CacheHit>> {
        let key_hash = EmbeddingKey::hash_task(description, task_kind);

        if let Some(entry) = self.store.lookup(&key_hash, task_kind)? {
            if entry.freshness_score >= self.similarity_threshold {
                return Ok(Some(CacheHit::new(entry, 1.0)));
            }
        }

        Ok(None)
    }

    pub async fn store(&self, description: &str, task_kind: &str, result: String, _content: &[u8]) -> Result<()> {
        let key_hash = EmbeddingKey::hash_task(description, task_kind);

        let entry = CacheEntry::new(
            Uuid::new_v4().to_string(),
            key_hash,
            vec![],
            task_kind.to_string(),
            result,
            self.ttl_seconds,
        );

        self.store.store(&entry)?;
        Ok(())
    }

    pub async fn evict_expired(&self) -> Result<u64> {
        self.store.evict_expired()
    }

    pub async fn stats(&self) -> Result<CacheStatistics> {
        let stats = self.store.stats()?;
        Ok(CacheStatistics {
            entries: stats.entries,
            total_hits: stats.total_hits,
            total_tokens_saved: stats.total_tokens_saved,
        })
    }

    pub async fn clear(&self) -> Result<()> {
        self.store.clear()
    }
}

#[derive(Debug, Clone)]
pub struct CacheStatistics {
    pub entries: u64,
    pub total_hits: u64,
    pub total_tokens_saved: u64,
}

impl Default for SemanticCache {
    fn default() -> Self {
        SemanticCache::new("~/.pyinferencemanager/cache.db", 3600).unwrap_or_else(|_| {
            SemanticCache {
                store: SqliteCacheStore::new(":memory:").unwrap(),
                similarity_threshold: 0.88,
                ttl_seconds: 3600,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_cache() -> Result<SemanticCache> {
        SemanticCache::new(":memory:", 3600)
    }

    #[tokio::test]
    async fn test_semantic_cache_new() {
        let cache = create_temp_cache();
        assert!(cache.is_ok());
    }

    #[tokio::test]
    async fn test_store_and_lookup() -> Result<()> {
        let cache = create_temp_cache()?;

        cache
            .store("analyze pdf", "document_analysis", "result text".to_string(), b"content")
            .await?;

        let hit = cache.lookup("analyze pdf", "document_analysis", b"content").await?;
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().entry.result, "result text");

        Ok(())
    }

    #[tokio::test]
    async fn test_lookup_miss() -> Result<()> {
        let cache = create_temp_cache()?;

        let hit = cache.lookup("nonexistent", "document_analysis", b"").await?;
        assert!(hit.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_stats() -> Result<()> {
        let cache = create_temp_cache()?;

        cache
            .store("analyze", "doc", "result1".to_string(), b"")
            .await?;
        cache
            .store("extract", "doc", "result2".to_string(), b"")
            .await?;

        let stats = cache.stats().await?;
        assert_eq!(stats.entries, 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_clear_cache() -> Result<()> {
        let cache = create_temp_cache()?;

        cache
            .store("analyze", "doc", "result".to_string(), b"")
            .await?;
        cache.clear().await?;

        let stats = cache.stats().await?;
        assert_eq!(stats.entries, 0);

        Ok(())
    }

    #[test]
    fn test_default_cache() {
        let _cache = SemanticCache::default();
    }
}
