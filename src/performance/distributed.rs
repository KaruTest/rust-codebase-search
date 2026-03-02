// Distributed Support Module
// Provides multi-shard indexing, query routing, and replication support

use crate::database::SearchResult;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Shard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    /// Unique identifier for this shard
    pub shard_id: String,
    /// Path to shard database
    pub db_path: PathBuf,
    /// Number of chunks in this shard
    pub chunk_count: i64,
    /// Whether this shard is read-only
    pub read_only: bool,
}

/// Distributed configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// Enable distributed mode
    pub enabled: bool,
    /// Shard configuration
    pub shards: Vec<ShardConfig>,
    /// Replication factor (number of replicas per chunk)
    pub replication_factor: usize,
    /// Current node ID
    pub node_id: String,
    /// List of known peer nodes
    pub peer_nodes: Vec<String>,
    /// Consistency level (strong, eventual)
    pub consistency: ConsistencyLevel,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shards: Vec::new(),
            replication_factor: 1,
            node_id: format!("node_{}", uuid_simple()),
            peer_nodes: Vec::new(),
            consistency: ConsistencyLevel::Strong,
        }
    }
}

/// Consistency level for distributed operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    Strong,
    Eventual,
}

/// Generate a simple UUID
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{:x}{:x}",
        duration.as_secs(),
        duration.subsec_nanos()
    )
}

/// Shard router for distributing queries across shards
pub struct ShardRouter {
    config: RwLock<DistributedConfig>,
    // Cache of shard to use based on hash
    routing_cache: RwLock<HashMap<String, usize>>,
}

impl ShardRouter {
    pub fn new(config: DistributedConfig) -> Self {
        Self {
            config: RwLock::new(config),
            routing_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get the shard index for a given codebase
    pub fn get_shard_for_codebase(&self, codebase_id: &str) -> Option<usize> {
        // Use consistent hashing to route to a shard
        let config = self.config.read().unwrap();
        if config.shards.is_empty() {
            return None;
        }

        // Hash-based routing
        let hash = simple_hash(codebase_id);
        let shard_idx = hash % config.shards.len();

        Some(shard_idx)
    }

    /// Get all shards that should be queried (for replication)
    pub fn get_shards_for_codebase(&self, codebase_id: &str) -> Vec<usize> {
        let config = self.config.read().unwrap();
        if config.shards.is_empty() {
            return Vec::new();
        }

        let hash = simple_hash(codebase_id);
        let mut shards = Vec::new();

        for i in 0..config.replication_factor.min(config.shards.len()) {
            let idx = (hash + i) % config.shards.len();
            shards.push(idx);
        }

        shards
    }

    /// Add a new shard
    pub fn add_shard(&self, shard: ShardConfig) {
        let mut config = self.config.write().unwrap();
        config.shards.push(shard);
    }

    /// Remove a shard
    pub fn remove_shard(&self, shard_id: &str) -> bool {
        let mut config = self.config.write().unwrap();
        if let Some(pos) = config.shards.iter().position(|s| s.shard_id == shard_id) {
            config.shards.remove(pos);
            return true;
        }
        false
    }

    /// Get current configuration
    pub fn get_config(&self) -> DistributedConfig {
        self.config.read().unwrap().clone()
    }

    /// Update peer nodes
    pub fn update_peers(&self, peers: Vec<String>) {
        let mut config = self.config.write().unwrap();
        config.peer_nodes = peers;
    }
}

/// Simple hash function for routing
fn simple_hash(s: &str) -> usize {
    let mut hash: usize = 5381;
    for c in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(c as usize);
    }
    hash
}

/// Query router that combines results from multiple shards
pub struct DistributedQueryRouter {
    router: Arc<ShardRouter>,
    model: String,
}

impl DistributedQueryRouter {
    pub fn new(router: Arc<ShardRouter>, model: &str) -> Self {
        Self {
            router,
            model: model.to_string(),
        }
    }

    /// Route a search query to appropriate shards
    pub fn route_search(
        &self,
        query: &str,
        codebase_id: Option<&str>,
        limit: usize,
    ) -> DistributedSearchPlan {
        let config = self.router.get_config();

        if !config.enabled || config.shards.is_empty() {
            return DistributedSearchPlan {
                shards_to_query: vec![0], // Single node mode
                requires_merge: false,
            };
        }

        let shards_to_query = if let Some(cid) = codebase_id {
            // Route to specific shards for this codebase
            self.router.get_shards_for_codebase(cid)
        } else {
            // Query all shards for global search
            (0..config.shards.len()).collect()
        };

        let requires_merge = shards_to_query.len() > 1;
        DistributedSearchPlan {
            shards_to_query,
            requires_merge,
        }
    }

    /// Merge results from multiple shards using RRF
    pub fn merge_results(
        &self,
        results: Vec<Vec<SearchResult>>,
        _limit: usize,
    ) -> Vec<SearchResult> {
        if results.is_empty() {
            return Vec::new();
        }

        if results.len() == 1 {
            return results.into_iter().next().unwrap();
        }

        // Reciprocal Rank Fusion
        let mut ranked: HashMap<i64, (SearchResult, f64)> = HashMap::new();

        for shard_results in results {
            for (rank, result) in shard_results.iter().enumerate() {
                let rrf_score = 1.0 / (rank as f64 + 60.0); // Standard RRF with k=60

                if let Some((existing, score)) = ranked.get_mut(&result.chunk_id) {
                    *score += rrf_score;
                    // Keep the best-scoring result
                    if result.score > existing.score {
                        *existing = result.clone();
                    }
                } else {
                    ranked.insert(
                        result.chunk_id,
                        (result.clone(), rrf_score),
                    );
                }
            }
        }

        // Sort by combined RRF score
        let mut final_results: Vec<SearchResult> = ranked
            .into_values()
            .map(|(mut result, score)| {
                result.score = score;
                result
            })
            .collect();

        final_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        final_results
    }
}

/// Plan for distributed search
#[derive(Debug, Clone)]
pub struct DistributedSearchPlan {
    pub shards_to_query: Vec<usize>,
    pub requires_merge: bool,
}

/// Shard manager for handling multiple database shards
pub struct ShardManager {
    base_path: PathBuf,
    shards: RwLock<HashMap<String, PathBuf>>,
}

impl ShardManager {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            shards: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new shard
    pub fn create_shard(&self, codebase_id: &str) -> Result<PathBuf> {
        let shard_id = format!("shard_{}", simple_hash(codebase_id));
        let shard_path = self.base_path.join(format!("{}.db", shard_id));

        // Initialize the shard database
        self.init_shard_db(&shard_path)?;

        self.shards
            .write()
            .unwrap()
            .insert(codebase_id.to_string(), shard_path.clone());

        Ok(shard_path)
    }

    /// Get shard path for a codebase
    pub fn get_shard_path(&self, codebase_id: &str) -> Option<PathBuf> {
        self.shards
            .read()
            .unwrap()
            .get(codebase_id)
            .cloned()
    }

    /// Initialize shard database schema
    fn init_shard_db(&self, path: &PathBuf) -> Result<()> {
        use rusqlite::Connection;

        let conn = Connection::open(path)?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                codebase_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                content TEXT NOT NULL,
                language TEXT,
                hash TEXT NOT NULL,
                embedding BLOB,
                UNIQUE(codebase_id, file_path, start_line, end_line)
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_codebase ON chunks(codebase_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                file_path,
                content='chunks',
                content_rowid='id'
            );
            "#,
        )?;

        Ok(())
    }

    /// List all shards
    pub fn list_shards(&self) -> Vec<(String, PathBuf)> {
        self.shards
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// Global distributed state
static DISTRIBUTED_STATE: std::sync::OnceLock<Arc<ShardRouter>> = std::sync::OnceLock::new();

/// Get the global distributed router
pub fn get_distributed_router() -> Option<&'static Arc<ShardRouter>> {
    DISTRIBUTED_STATE.get()
}

/// Initialize distributed mode
pub fn init_distributed(config: DistributedConfig) -> Arc<ShardRouter> {
    let router = Arc::new(ShardRouter::new(config));
    DISTRIBUTED_STATE.set(router.clone()).ok();
    router
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_router() {
        let config = DistributedConfig {
            enabled: true,
            shards: vec![
                ShardConfig {
                    shard_id: "shard_0".to_string(),
                    db_path: PathBuf::from("/tmp/shard0.db"),
                    chunk_count: 1000,
                    read_only: false,
                },
                ShardConfig {
                    shard_id: "shard_1".to_string(),
                    db_path: PathBuf::from("/tmp/shard1.db"),
                    chunk_count: 1500,
                    read_only: false,
                },
            ],
            replication_factor: 1,
            node_id: "node_1".to_string(),
            peer_nodes: vec![],
            consistency: ConsistencyLevel::Strong,
        };

        let router = ShardRouter::new(config);

        // Test consistent routing
        let shard1 = router.get_shard_for_codebase("test_codebase");
        let shard2 = router.get_shard_for_codebase("test_codebase");

        assert_eq!(shard1, shard2); // Same codebase should go to same shard
    }

    #[test]
    fn test_result_merging() {
        let router = Arc::new(ShardRouter::new(DistributedConfig::default()));
        let query_router = DistributedQueryRouter::new(router, "minilm");

        // Create sample results from 2 shards
        let shard1_results = vec![
            SearchResult {
                chunk_id: 1,
                codebase_id: "test".to_string(),
                file_path: "a.rs".to_string(),
                start_line: 1,
                end_line: 10,
                content: "content".to_string(),
                language: Some("rust".to_string()),
                score: 0.9,
                rank: 1,
            },
            SearchResult {
                chunk_id: 2,
                codebase_id: "test".to_string(),
                file_path: "b.rs".to_string(),
                start_line: 5,
                end_line: 15,
                content: "content".to_string(),
                language: Some("rust".to_string()),
                score: 0.8,
                rank: 2,
            },
        ];

        let shard2_results = vec![
            SearchResult {
                chunk_id: 1,
                codebase_id: "test".to_string(),
                file_path: "a.rs".to_string(),
                start_line: 1,
                end_line: 10,
                content: "content".to_string(),
                language: Some("rust".to_string()),
                score: 0.85,
                rank: 1,
            },
            SearchResult {
                chunk_id: 3,
                codebase_id: "test".to_string(),
                file_path: "c.rs".to_string(),
                start_line: 10,
                end_line: 20,
                content: "content".to_string(),
                language: Some("rust".to_string()),
                score: 0.7,
                rank: 2,
            },
        ];

        let merged = query_router.merge_results(
            vec![shard1_results, shard2_results],
            10,
        );

        // Chunk 1 should have higher score due to appearing in both shards
        assert!(merged.len() > 0);
    }
}
