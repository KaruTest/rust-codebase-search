// Query Caching Module
// Provides LRU cache for query embeddings with invalidation support

use crate::embedding::get_query_embedding_with_model;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// Cache entry for a query embedding
struct CacheEntry {
    embedding: Vec<f32>,
    hash: u64,
    access_count: u64,
    timestamp: std::time::Instant,
}

/// LRU Cache for query embeddings
pub struct QueryCache {
    max_size: usize,
    cache: RwLock<std::collections::HashMap<String, CacheEntry>>,
    hits: AtomicU64,
    misses: AtomicU64,
    model_name: String,
    // Version counter for invalidation
    version: AtomicU64,
    current_version: RwLock<u64>,
}

impl QueryCache {
    /// Create a new query cache
    pub fn new(max_size: usize, model_name: &str) -> Self {
        Self {
            max_size,
            cache: RwLock::new(std::collections::HashMap::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            model_name: model_name.to_string(),
            version: AtomicU64::new(0),
            current_version: RwLock::new(0),
        }
    }

    /// Get the cache key for a query
    fn get_cache_key(query: &str, codebase_id: Option<&str>) -> String {
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        if let Some(cid) = codebase_id {
            cid.hash(&mut hasher);
        }
        format!("{:x}_{}", hasher.finish(), query)
    }

    /// Get embedding from cache or generate it
    pub fn get(&self, query: &str, codebase_id: Option<&str>) -> Vec<f32> {
        let key = Self::get_cache_key(query, codebase_id);

        // Check cache
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(&key) {
                // Check if cache is still valid (version matches)
                if let Ok(version) = self.current_version.read() {
                    if entry.hash == *version {
                        self.hits.fetch_add(1, Ordering::Relaxed);
                        return entry.embedding.clone();
                    }
                }
            }
        }

        // Cache miss - generate embedding
        self.misses.fetch_add(1, Ordering::Relaxed);
        let embedding = get_query_embedding_with_model(query, &self.model_name);

        // Store in cache
        if let Ok(mut cache) = self.cache.write() {
            // Evict if full (simple LRU: remove oldest entry)
            if cache.len() >= self.max_size {
                if let Some((oldest_key, _)) = cache.iter().next() {
                    let key_to_remove = oldest_key.clone();
                    cache.remove(&key_to_remove);
                }
            }

            let current_version = *self.current_version.read().unwrap();
            cache.insert(
                key,
                CacheEntry {
                    embedding: embedding.clone(),
                    hash: current_version,
                    access_count: 1,
                    timestamp: std::time::Instant::now(),
                },
            );
        }

        embedding
    }

    /// Invalidate the cache (call when index is updated)
    pub fn invalidate(&self) {
        self.version.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut version) = self.current_version.write() {
            *version += 1;
        }

        // Clear cache entries
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        CacheStats {
            hits,
            misses,
            total_requests: total,
            hit_rate: if total > 0 {
                hits as f64 / total as f64
            } else {
                0.0
            },
            size: self.cache.read().map(|c| c.len()).unwrap_or(0),
            max_size: self.max_size,
        }
    }

    /// Clear the cache manually
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Pre-warm cache with common queries
    pub fn warm(&self, queries: &[(&str, Option<&str>)]) {
        for (query, codebase_id) in queries {
            self.get(query, *codebase_id);
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub total_requests: u64,
    pub hit_rate: f64,
    pub size: usize,
    pub max_size: usize,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Cache Statistics:")?;
        writeln!(f, "  Hits: {}", self.hits)?;
        writeln!(f, "  Misses: {}", self.misses)?;
        writeln!(f, "  Hit Rate: {:.2}%", self.hit_rate * 100.0)?;
        writeln!(f, "  Size: {}/{}", self.size, self.max_size)?;
        Ok(())
    }
}

/// Global query cache instance
static QUERY_CACHE: OnceLock<Arc<QueryCache>> = OnceLock::new();

/// Get the global query cache
pub fn get_query_cache() -> &'static Arc<QueryCache> {
    QUERY_CACHE.get_or_init(|| Arc::new(QueryCache::new(1000, "minilm")))
}

/// Initialize the query cache with custom settings
pub fn init_query_cache(max_size: usize, model_name: &str) -> &'static Arc<QueryCache> {
    QUERY_CACHE.get_or_init(|| Arc::new(QueryCache::new(max_size, model_name)))
}

/// Invalidate the global query cache
pub fn invalidate_query_cache() {
    if let Some(cache) = QUERY_CACHE.get() {
        cache.invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = QueryCache::new(3, "minilm");

        // First call should be a miss
        let emb1 = cache.get("test query", None);
        assert_eq!(emb1.len(), 384);

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        // Second call should be a hit
        let emb2 = cache.get("test query", None);
        assert_eq!(emb1, emb2);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = QueryCache::new(3, "minilm");

        // Fill cache
        cache.get("query1", None);
        cache.get("query2", None);

        assert_eq!(cache.stats().size, 2);

        // Invalidate
        cache.invalidate();

        // Cache should be cleared
        assert_eq!(cache.stats().size, 0);
    }

    #[test]
    fn test_cache_with_codebase_filter() {
        let cache = QueryCache::new(10, "minilm");

        // Same query with different codebase IDs should be cached separately
        let emb1 = cache.get("test", Some("codebase1"));
        let emb2 = cache.get("test", Some("codebase2"));
        let emb3 = cache.get("test", None);

        // They should all be cached (3 misses)
        assert_eq!(cache.stats().misses, 3);
    }
}
