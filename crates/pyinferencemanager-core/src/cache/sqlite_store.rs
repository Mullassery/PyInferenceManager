use rusqlite::{Connection, params, OptionalExtension};
use crate::types::CacheEntry;
use crate::Result;
use std::sync::Mutex;

pub struct SqliteCacheStore {
    conn: Mutex<Connection>,
}

impl SqliteCacheStore {
    pub fn new(path: &str) -> Result<Self> {
        let expanded_path = if path.starts_with('~') {
            let home = std::env::var("HOME")
                .map_err(|_| crate::Error::CacheError("HOME not set".to_string()))?;
            path.replace("~", &home)
        } else {
            path.to_string()
        };

        let conn = Connection::open(&expanded_path)
            .map_err(|e| crate::Error::CacheError(format!("Failed to open DB: {}", e)))?;

        let store = SqliteCacheStore {
            conn: Mutex::new(conn),
        };

        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            crate::Error::CacheError("Failed to acquire lock".to_string())
        })?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache_entries (
                id TEXT PRIMARY KEY,
                key_hash TEXT NOT NULL,
                task_kind TEXT NOT NULL,
                result TEXT NOT NULL,
                tokens_saved INTEGER DEFAULT 0,
                hit_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                freshness_score REAL DEFAULT 1.0
            )",
            [],
        ).map_err(|e| crate::Error::CacheError(format!("Schema creation failed: {}", e)))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_key_hash ON cache_entries(key_hash)",
            [],
        ).map_err(|e| crate::Error::CacheError(format!("Index creation failed: {}", e)))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_expires_at ON cache_entries(expires_at)",
            [],
        ).map_err(|e| crate::Error::CacheError(format!("Index creation failed: {}", e)))?;

        Ok(())
    }

    pub fn store(&self, entry: &CacheEntry) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            crate::Error::CacheError("Failed to acquire lock".to_string())
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO cache_entries
            (id, key_hash, task_kind, result, tokens_saved, hit_count, created_at, expires_at, freshness_score)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                &entry.id,
                &entry.key_hash,
                &entry.task_kind,
                &entry.result,
                entry.tokens_saved,
                entry.hit_count,
                entry.created_at.to_rfc3339(),
                entry.expires_at.to_rfc3339(),
                entry.freshness_score,
            ],
        ).map_err(|e| crate::Error::CacheError(format!("Store failed: {}", e)))?;

        Ok(())
    }

    pub fn lookup(&self, key_hash: &str, task_kind: &str) -> Result<Option<CacheEntry>> {
        let conn = self.conn.lock().map_err(|_| {
            crate::Error::CacheError("Failed to acquire lock".to_string())
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, key_hash, task_kind, result, tokens_saved, hit_count,
                created_at, expires_at, freshness_score
                FROM cache_entries
                WHERE key_hash = ? AND task_kind = ? AND expires_at > datetime('now')
                ORDER BY freshness_score DESC
                LIMIT 1",
            )
            .map_err(|e| crate::Error::CacheError(format!("Prepare failed: {}", e)))?;

        let result = stmt
            .query_row(params![key_hash, task_kind], |row| {
                Ok(CacheEntry {
                    id: row.get(0)?,
                    key_hash: row.get(1)?,
                    embedding: vec![],
                    task_kind: row.get(2)?,
                    result: row.get(3)?,
                    tokens_saved: row.get(4)?,
                    hit_count: row.get(5)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    expires_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    freshness_score: row.get(8)?,
                })
            })
            .optional()
            .map_err(|e| crate::Error::CacheError(format!("Query failed: {}", e)))?;

        Ok(result)
    }

    pub fn evict_expired(&self) -> Result<u64> {
        let conn = self.conn.lock().map_err(|_| {
            crate::Error::CacheError("Failed to acquire lock".to_string())
        })?;

        let rows_deleted = conn
            .execute("DELETE FROM cache_entries WHERE expires_at < datetime('now')", [])
            .map_err(|e| crate::Error::CacheError(format!("Eviction failed: {}", e)))?;

        Ok(rows_deleted as u64)
    }

    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            crate::Error::CacheError("Failed to acquire lock".to_string())
        })?;

        conn.execute("DELETE FROM cache_entries", [])
            .map_err(|e| crate::Error::CacheError(format!("Clear failed: {}", e)))?;

        Ok(())
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let conn = self.conn.lock().map_err(|_| {
            crate::Error::CacheError("Failed to acquire lock".to_string())
        })?;

        let entry_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cache_entries", [], |row| row.get(0))
            .map_err(|e| crate::Error::CacheError(format!("Count query failed: {}", e)))?;

        let total_hit_count: i64 = conn
            .query_row("SELECT COALESCE(SUM(hit_count), 0) FROM cache_entries", [], |row| {
                row.get(0)
            })
            .map_err(|e| crate::Error::CacheError(format!("Hit count query failed: {}", e)))?;

        let total_tokens_saved: i64 = conn
            .query_row("SELECT COALESCE(SUM(tokens_saved), 0) FROM cache_entries", [], |row| {
                row.get(0)
            })
            .map_err(|e| crate::Error::CacheError(format!("Tokens saved query failed: {}", e)))?;

        Ok(CacheStats {
            entries: entry_count as u64,
            total_hits: total_hit_count as u64,
            total_tokens_saved: total_tokens_saved as u64,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: u64,
    pub total_hits: u64,
    pub total_tokens_saved: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_store() -> Result<SqliteCacheStore> {
        SqliteCacheStore::new(":memory:")
    }

    #[test]
    fn test_sqlite_store_new() {
        let store = create_temp_store();
        assert!(store.is_ok());
    }

    #[test]
    fn test_store_and_lookup() -> Result<()> {
        let store = create_temp_store()?;

        let entry = CacheEntry::new(
            "entry1".to_string(),
            "hash123".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result text".to_string(),
            3600,
        );

        store.store(&entry)?;

        let found = store.lookup("hash123", "document_analysis")?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "entry1");

        Ok(())
    }

    #[test]
    fn test_lookup_fresh_entry() -> Result<()> {
        let store = create_temp_store()?;

        let entry = CacheEntry::new(
            "entry1".to_string(),
            "hash123".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result text".to_string(),
            3600,
        );

        store.store(&entry)?;

        let found = store.lookup("hash123", "document_analysis")?;
        assert!(found.is_some());

        Ok(())
    }

    #[test]
    fn test_lookup_nonexistent() -> Result<()> {
        let store = create_temp_store()?;
        let found = store.lookup("nonexistent", "document_analysis")?;
        assert!(found.is_none());
        Ok(())
    }

    #[test]
    fn test_cache_stats() -> Result<()> {
        let store = create_temp_store()?;

        let entry1 = CacheEntry::new(
            "entry1".to_string(),
            "hash1".to_string(),
            vec![],
            "doc".to_string(),
            "result1".to_string(),
            3600,
        );

        let entry2 = CacheEntry::new(
            "entry2".to_string(),
            "hash2".to_string(),
            vec![],
            "doc".to_string(),
            "result2".to_string(),
            3600,
        );

        store.store(&entry1)?;
        store.store(&entry2)?;

        let stats = store.stats()?;
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.total_hits, 0);

        Ok(())
    }

    #[test]
    fn test_clear_cache() -> Result<()> {
        let store = create_temp_store()?;

        let entry = CacheEntry::new(
            "entry1".to_string(),
            "hash123".to_string(),
            vec![],
            "document_analysis".to_string(),
            "result text".to_string(),
            3600,
        );

        store.store(&entry)?;
        store.clear()?;

        let stats = store.stats()?;
        assert_eq!(stats.entries, 0);

        Ok(())
    }
}
