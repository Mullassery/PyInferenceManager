use sha2::{Sha256, Digest};

pub struct EmbeddingKey;

impl EmbeddingKey {
    /// Derive a cache key hash from task description
    /// For MVP, use SHA256 of description (Phase 2: add embedding-based hashing)
    pub fn hash_task(description: &str, task_kind: &str) -> String {
        let combined = format!("{}:{}", task_kind, description);
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Derive a cache key hash from content (file attachments)
    pub fn hash_content(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }

    /// Combine task hash + content hash for full cache key
    pub fn derive_key(description: &str, task_kind: &str, content: &[u8]) -> String {
        let task_hash = Self::hash_task(description, task_kind);
        let content_hash = Self::hash_content(content);
        format!("{}|{}", task_hash, content_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_task_consistent() {
        let hash1 = EmbeddingKey::hash_task("analyze pdf", "document_analysis");
        let hash2 = EmbeddingKey::hash_task("analyze pdf", "document_analysis");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_task_different_for_different_input() {
        let hash1 = EmbeddingKey::hash_task("analyze pdf", "document_analysis");
        let hash2 = EmbeddingKey::hash_task("analyze word", "document_analysis");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_content() {
        let content = b"hello world";
        let hash1 = EmbeddingKey::hash_content(content);
        let hash2 = EmbeddingKey::hash_content(content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_content_differs_for_different_data() {
        let hash1 = EmbeddingKey::hash_content(b"hello");
        let hash2 = EmbeddingKey::hash_content(b"world");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_derive_key_combines_hashes() {
        let key = EmbeddingKey::derive_key("analyze", "doc", b"content");
        assert!(key.contains('|'));
        let parts: Vec<&str> = key.split('|').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_derive_key_consistent() {
        let key1 = EmbeddingKey::derive_key("analyze", "doc", b"content");
        let key2 = EmbeddingKey::derive_key("analyze", "doc", b"content");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_hash_is_hex_string() {
        let hash = EmbeddingKey::hash_task("test", "kind");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash.len(), 64); // SHA256 = 256 bits = 64 hex chars
    }
}
