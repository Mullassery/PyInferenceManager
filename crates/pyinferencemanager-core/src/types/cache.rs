use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheKey {
    pub task_kind: String,
    pub embedding: Vec<f32>,
    pub content_hash: String,
}

impl CacheKey {
    pub fn new(task_kind: String, embedding: Vec<f32>, content_hash: String) -> Self {
        CacheKey {
            task_kind,
            embedding,
            content_hash,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub id: String,
    pub key_hash: String,
    pub embedding: Vec<f32>,
    pub task_kind: String,
    pub result: String,
    pub tokens_saved: u32,
    pub hit_count: u32,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub freshness_score: f32,
}

impl CacheEntry {
    pub fn new(
        id: String,
        key_hash: String,
        embedding: Vec<f32>,
        task_kind: String,
        result: String,
        ttl_seconds: u64,
    ) -> Self {
        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_seconds as i64);

        CacheEntry {
            id,
            key_hash,
            embedding,
            task_kind,
            result,
            tokens_saved: 0,
            hit_count: 0,
            created_at: now,
            expires_at,
            freshness_score: 1.0,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn record_hit(&mut self, tokens_saved: u32) {
        self.hit_count += 1;
        self.tokens_saved += tokens_saved;
        self.freshness_score = (self.freshness_score * 0.95).max(0.5);
    }

    pub fn is_fresh(&self) -> bool {
        !self.is_expired() && self.freshness_score > 0.5
    }
}

#[derive(Debug, Clone)]
pub struct CacheHit {
    pub entry: CacheEntry,
    pub similarity: f32,
}

impl CacheHit {
    pub fn new(entry: CacheEntry, similarity: f32) -> Self {
        CacheHit { entry, similarity }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_new() {
        let key = CacheKey::new(
            "document_analysis".to_string(),
            vec![0.1, 0.2, 0.3],
            "hash123".to_string(),
        );
        assert_eq!(key.task_kind, "document_analysis");
        assert_eq!(key.embedding.len(), 3);
        assert_eq!(key.content_hash, "hash123");
    }

    #[test]
    fn test_cache_entry_new() {
        let entry = CacheEntry::new(
            "entry1".to_string(),
            "key_hash1".to_string(),
            vec![0.1, 0.2],
            "document_analysis".to_string(),
            "result text".to_string(),
            3600,
        );

        assert_eq!(entry.id, "entry1");
        assert_eq!(entry.result, "result text");
        assert_eq!(entry.hit_count, 0);
        assert_eq!(entry.tokens_saved, 0);
        assert_eq!(entry.freshness_score, 1.0);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry::new(
            "entry1".to_string(),
            "key_hash1".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result".to_string(),
            0, // Expires immediately
        );

        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_entry_record_hit() {
        let mut entry = CacheEntry::new(
            "entry1".to_string(),
            "key_hash1".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result".to_string(),
            3600,
        );

        entry.record_hit(100);
        assert_eq!(entry.hit_count, 1);
        assert_eq!(entry.tokens_saved, 100);
        assert!(entry.freshness_score < 1.0);

        entry.record_hit(50);
        assert_eq!(entry.hit_count, 2);
        assert_eq!(entry.tokens_saved, 150);
    }

    #[test]
    fn test_cache_entry_freshness() {
        let entry = CacheEntry::new(
            "entry1".to_string(),
            "key_hash1".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result".to_string(),
            3600,
        );

        assert!(entry.is_fresh());

        let mut expired_entry = entry.clone();
        expired_entry.expires_at = Utc::now() - Duration::seconds(1);
        assert!(!expired_entry.is_fresh());

        let mut stale_entry = entry.clone();
        stale_entry.freshness_score = 0.4;
        assert!(!stale_entry.is_fresh());
    }

    #[test]
    fn test_cache_hit_new() {
        let entry = CacheEntry::new(
            "entry1".to_string(),
            "key_hash1".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result".to_string(),
            3600,
        );

        let hit = CacheHit::new(entry, 0.95);
        assert_eq!(hit.similarity, 0.95);
        assert_eq!(hit.entry.id, "entry1");
    }
}
