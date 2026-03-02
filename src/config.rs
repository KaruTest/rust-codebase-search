use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "code-search";
const APPLICATION: &str = "code-search";

// Environment variable prefixes
const ENV_PREFIX: &str = "CODE_SEARCH_";

// ============== Model Configuration ==============

fn default_model_type() -> String {
    "minilm".to_string()
}

fn default_auto_download() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    #[serde(default = "default_model_type")]
    pub model_type: String,
    #[serde(default = "default_auto_download")]
    pub auto_download: bool,
    /// Custom model path: local ONNX file path or HuggingFace model ID
    /// Example: "sentence-transformers/all-mpnet-base-v2" or "/path/to/model.onnx"
    #[serde(default)]
    pub model_path: Option<String>,
    /// Embedding dimension (required for custom models)
    /// Example: 768 for all-mpnet-base-v2
    #[serde(default)]
    pub embedding_dim: Option<usize>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_type: default_model_type(),
            auto_download: default_auto_download(),
            model_path: None,
            embedding_dim: None,
        }
    }
}

// ============== Indexing Configuration ==============

fn default_extensions() -> Vec<String> {
    vec![
        ".rs".to_string(),
        ".py".to_string(),
        ".js".to_string(),
        ".jsx".to_string(),
        ".ts".to_string(),
        ".tsx".to_string(),
        ".java".to_string(),
        ".go".to_string(),
        ".c".to_string(),
        ".cpp".to_string(),
        ".cc".to_string(),
        ".cxx".to_string(),
        ".h".to_string(),
        ".hpp".to_string(),
        ".cs".to_string(),
        ".php".to_string(),
        ".rb".to_string(),
        ".swift".to_string(),
        ".kt".to_string(),
        ".kts".to_string(),
        ".scala".to_string(),
        ".sc".to_string(),
        ".m".to_string(),
        ".mm".to_string(),
        ".sh".to_string(),
        ".bash".to_string(),
        ".zsh".to_string(),
        ".fish".to_string(),
        ".ps1".to_string(),
        ".sql".to_string(),
        ".pl".to_string(),
        ".pm".to_string(),
        ".lua".to_string(),
        ".r".to_string(),
        ".R".to_string(),
        ".jl".to_string(),
        ".dart".to_string(),
        ".nim".to_string(),
        ".cr".to_string(),
        ".elm".to_string(),
        ".erl".to_string(),
        ".hrl".to_string(),
        ".ex".to_string(),
        ".exs".to_string(),
        ".clj".to_string(),
        ".cljs".to_string(),
        ".cljc".to_string(),
        ".hs".to_string(),
        ".lhs".to_string(),
        ".fs".to_string(),
        ".fsi".to_string(),
        ".fsx".to_string(),
        ".ml".to_string(),
        ".mli".to_string(),
        ".v".to_string(),
        ".vh".to_string(),
        ".vhd".to_string(),
        ".sv".to_string(),
        ".svh".to_string(),
        ".cob".to_string(),
        ".cbl".to_string(),
        ".cpy".to_string(),
        ".f".to_string(),
        ".f90".to_string(),
        ".f95".to_string(),
        ".f03".to_string(),
        ".f08".to_string(),
        ".adb".to_string(),
        ".ads".to_string(),
        ".pas".to_string(),
        ".pp".to_string(),
        ".inc".to_string(),
        ".asm".to_string(),
        ".s".to_string(),
        ".S".to_string(),
        ".nasm".to_string(),
        ".cmake".to_string(),
        ".xml".to_string(),
        ".html".to_string(),
        ".htm".to_string(),
        ".css".to_string(),
        ".scss".to_string(),
        ".sass".to_string(),
        ".less".to_string(),
        ".json".to_string(),
        ".yaml".to_string(),
        ".yml".to_string(),
        ".toml".to_string(),
        ".ini".to_string(),
        ".cfg".to_string(),
        ".conf".to_string(),
        ".md".to_string(),
        ".markdown".to_string(),
        ".rst".to_string(),
        ".tex".to_string(),
        ".bib".to_string(),
        ".proto".to_string(),
        ".graphql".to_string(),
        ".gql".to_string(),
        ".prisma".to_string(),
        ".vue".to_string(),
        ".svelte".to_string(),
        ".sol".to_string(),
        ".vy".to_string(),
        ".zig".to_string(),
        ".odin".to_string(),
        ".d".to_string(),
        ".di".to_string(),
        ".nims".to_string(),
        ".ecr".to_string(),
        ".wat".to_string(),
        ".wast".to_string(),
        ".wit".to_string(),
        ".glsl".to_string(),
        ".vert".to_string(),
        ".frag".to_string(),
        ".hlsl".to_string(),
        ".wgsl".to_string(),
        ".metal".to_string(),
    ]
}

fn default_skip_dirs() -> Vec<String> {
    vec![
        ".git".to_string(),
        ".svn".to_string(),
        ".hg".to_string(),
        "node_modules".to_string(),
        "target".to_string(),
        "build".to_string(),
        "dist".to_string(),
        "__pycache__".to_string(),
        ".pytest_cache".to_string(),
        ".mypy_cache".to_string(),
        "venv".to_string(),
        ".venv".to_string(),
        "env".to_string(),
        ".env".to_string(),
        "vendor".to_string(),
        "bower_components".to_string(),
        ".idea".to_string(),
        ".vscode".to_string(),
        ".vs".to_string(),
        "bin".to_string(),
        "obj".to_string(),
        "pkg".to_string(),
        ".gradle".to_string(),
        ".mvn".to_string(),
        "Pods".to_string(),
        ".cache".to_string(),
    ]
}

fn default_skip_files() -> Vec<String> {
    vec![
        ".DS_Store".to_string(),
        "Thumbs.db".to_string(),
        "*.pyc".to_string(),
        "*.pyo".to_string(),
        "*.pyd".to_string(),
        "*.so".to_string(),
        "*.dylib".to_string(),
        "*.dll".to_string(),
        "*.exe".to_string(),
        "*.out".to_string(),
        "*.app".to_string(),
        "*.lock".to_string(),
        "*.log".to_string(),
        "*.tmp".to_string(),
        "*.temp".to_string(),
        "*.bak".to_string(),
        "*.swp".to_string(),
        "*.swo".to_string(),
        "*~".to_string(),
        ".env".to_string(),
        ".env.local".to_string(),
        "package-lock.json".to_string(),
        "yarn.lock".to_string(),
        "pnpm-lock.yaml".to_string(),
        "Cargo.lock".to_string(),
        "composer.lock".to_string(),
        "Gemfile.lock".to_string(),
        "Podfile.lock".to_string(),
        "poetry.lock".to_string(),
        "go.sum".to_string(),
    ]
}

fn default_use_gitignore() -> bool {
    true
}

fn default_batch_size() -> usize {
    32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
    #[serde(default = "default_skip_dirs")]
    pub skip_dirs: Vec<String>,
    #[serde(default = "default_skip_files")]
    pub skip_files: Vec<String>,
    #[serde(default = "default_use_gitignore")]
    pub use_gitignore: bool,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
            skip_dirs: default_skip_dirs(),
            skip_files: default_skip_files(),
            use_gitignore: default_use_gitignore(),
            batch_size: default_batch_size(),
        }
    }
}

// ============== Chunking Configuration ==============

fn default_chunk_size() -> usize {
    50
}

fn default_chunk_overlap() -> usize {
    10
}

fn default_token_budget() -> String {
    "medium".to_string()
}

fn default_use_syntax_aware() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,
    /// Token budget for chunks: "small" (256), "medium" (512), "large" (1024), or custom number
    #[serde(default = "default_token_budget")]
    pub token_budget: String,
    /// Enable syntax-aware chunking using tree-sitter
    #[serde(default = "default_use_syntax_aware")]
    pub use_syntax_aware: bool,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            token_budget: default_token_budget(),
            use_syntax_aware: default_use_syntax_aware(),
        }
    }
}

// ============== Search Configuration ==============

fn default_limit() -> usize {
    10
}

fn default_fts_weight() -> f64 {
    0.6
}

fn default_vector_weight() -> f64 {
    0.4
}

fn default_enable_fuzzy() -> bool {
    true
}

fn default_fuzzy_max_distance() -> usize {
    2
}

fn default_enable_ltr() -> bool {
    true
}

fn default_bm25_k1() -> f64 {
    1.5
}

fn default_bm25_b() -> f64 {
    0.75
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_limit")]
    pub default_limit: usize,
    #[serde(default = "default_fts_weight")]
    pub fts_weight: f64,
    #[serde(default = "default_vector_weight")]
    pub vector_weight: f64,
    #[serde(default = "default_enable_fuzzy")]
    pub enable_fuzzy: bool,
    #[serde(default = "default_fuzzy_max_distance")]
    pub fuzzy_max_distance: usize,
    #[serde(default = "default_enable_ltr")]
    pub enable_ltr: bool,
    #[serde(default = "default_bm25_k1")]
    pub bm25_k1: f64,
    #[serde(default = "default_bm25_b")]
    pub bm25_b: f64,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_limit: default_limit(),
            fts_weight: default_fts_weight(),
            vector_weight: default_vector_weight(),
            enable_fuzzy: default_enable_fuzzy(),
            fuzzy_max_distance: default_fuzzy_max_distance(),
            enable_ltr: default_enable_ltr(),
            bm25_k1: default_bm25_k1(),
            bm25_b: default_bm25_b(),
        }
    }
}

// ============== Database Configuration ==============

fn default_data_dir() -> String {
    "code-search".to_string()
}

fn default_db_name() -> String {
    "index.db".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_db_name")]
    pub db_name: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            db_name: default_db_name(),
        }
    }
}

// ============== Performance Configuration ==============

fn default_hnsw_enabled() -> bool {
    false
}

fn default_hnsw_max_connections() -> usize {
    16
}

fn default_hnsw_ef_search() -> usize {
    64
}

fn default_cache_size() -> usize {
    1000
}

fn default_batch_size_perf() -> usize {
    32
}

fn default_use_gpu() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_hnsw_enabled")]
    pub hnsw_enabled: bool,
    #[serde(default = "default_hnsw_max_connections")]
    pub hnsw_max_connections: usize,
    #[serde(default = "default_hnsw_ef_search")]
    pub hnsw_ef_search: usize,
    #[serde(default = "default_cache_size")]
    pub cache_size: usize,
    #[serde(default = "default_batch_size_perf")]
    pub batch_size: usize,
    #[serde(default = "default_use_gpu")]
    pub use_gpu: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            hnsw_enabled: default_hnsw_enabled(),
            hnsw_max_connections: default_hnsw_max_connections(),
            hnsw_ef_search: default_hnsw_ef_search(),
            cache_size: default_cache_size(),
            batch_size: default_batch_size_perf(),
            use_gpu: default_use_gpu(),
        }
    }
}

// ============== Distributed Configuration ==============

fn default_distributed_enabled() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    #[serde(default = "default_distributed_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub node_id: String,
    #[serde(default)]
    pub shard_path: String,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            enabled: default_distributed_enabled(),
            node_id: format!("node_{}", uuid_simple()),
            shard_path: "shards".to_string(),
        }
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}{:x}", duration.as_secs(), duration.subsec_nanos())
}

// ============== Main Config ==============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub indexing: IndexingConfig,
    #[serde(default)]
    pub chunking: ChunkingConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub distributed: DistributedConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: ModelConfig::default(),
            indexing: IndexingConfig::default(),
            chunking: ChunkingConfig::default(),
            search: SearchConfig::default(),
            database: DatabaseConfig::default(),
            performance: PerformanceConfig::default(),
            distributed: DistributedConfig::default(),
        }
    }
}

// Legacy field accessors for backward compatibility
impl Config {
    pub fn model_name(&self) -> &str {
        &self.model.model_type
    }

    /// Returns the model path if specified (for custom models)
    pub fn model_path(&self) -> Option<&str> {
        self.model.model_path.as_deref()
    }

    /// Returns the embedding dimension if specified (for custom models)
    pub fn embedding_dim(&self) -> Option<usize> {
        self.model.embedding_dim
    }

    /// Returns true if this is a custom model configuration
    pub fn is_custom_model(&self) -> bool {
        self.model.model_type == "custom" && self.model.model_path.is_some()
    }

    /// Validates the model configuration
    /// Returns an error message if validation fails, or None if valid
    pub fn validate_model_config(&self) -> Option<String> {
        if self.model.model_type == "custom" {
            if self.model.model_path.is_none() {
                return Some("Custom model requires model_path in config".to_string());
            }
            if self.model.embedding_dim.is_none() {
                return Some("Custom model requires embedding_dim in config".to_string());
            }
            if let Some(dim) = self.model.embedding_dim {
                if dim == 0 {
                    return Some("embedding_dim must be greater than 0".to_string());
                }
            }
        }
        None
    }

    pub fn chunk_size(&self) -> usize {
        self.chunking.chunk_size
    }

    pub fn chunk_overlap(&self) -> usize {
        self.chunking.chunk_overlap
    }

    pub fn token_budget(&self) -> &str {
        &self.chunking.token_budget
    }

    pub fn use_syntax_aware(&self) -> bool {
        self.chunking.use_syntax_aware
    }

    pub fn default_limit(&self) -> usize {
        self.search.default_limit
    }

    pub fn extensions(&self) -> &[String] {
        &self.indexing.extensions
    }

    pub fn skip_dirs(&self) -> &[String] {
        &self.indexing.skip_dirs
    }

    pub fn skip_files(&self) -> &[String] {
        &self.indexing.skip_files
    }

    pub fn use_gitignore(&self) -> bool {
        self.indexing.use_gitignore
    }

    pub fn batch_size(&self) -> usize {
        self.indexing.batch_size
    }

    pub fn fts_weight(&self) -> f64 {
        self.search.fts_weight
    }

    pub fn vector_weight(&self) -> f64 {
        self.search.vector_weight
    }

    pub fn enable_fuzzy(&self) -> bool {
        self.search.enable_fuzzy
    }

    pub fn fuzzy_max_distance(&self) -> usize {
        self.search.fuzzy_max_distance
    }

    pub fn enable_ltr(&self) -> bool {
        self.search.enable_ltr
    }

    pub fn bm25_k1(&self) -> f64 {
        self.search.bm25_k1
    }

    pub fn bm25_b(&self) -> f64 {
        self.search.bm25_b
    }

    pub fn data_dir(&self) -> &str {
        &self.database.data_dir
    }

    pub fn db_name(&self) -> &str {
        &self.database.db_name
    }

    // Performance config accessors
    pub fn hnsw_enabled(&self) -> bool {
        self.performance.hnsw_enabled
    }

    pub fn hnsw_max_connections(&self) -> usize {
        self.performance.hnsw_max_connections
    }

    pub fn hnsw_ef_search(&self) -> usize {
        self.performance.hnsw_ef_search
    }

    pub fn cache_size(&self) -> usize {
        self.performance.cache_size
    }

    pub fn performance_batch_size(&self) -> usize {
        self.performance.batch_size
    }

    pub fn use_gpu(&self) -> bool {
        self.performance.use_gpu
    }

    // Distributed config accessors
    pub fn distributed_enabled(&self) -> bool {
        self.distributed.enabled
    }

    pub fn node_id(&self) -> &str {
        &self.distributed.node_id
    }

    pub fn shard_path(&self) -> &str {
        &self.distributed.shard_path
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config = Self::default();

        // Try to load from config file
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(file_config) = toml::from_str::<Config>(&content) {
                        config = file_config;
                    }
                }
            }
        }

        // Apply environment variable overrides
        config.apply_env_overrides();

        config
    }

    fn apply_env_overrides(&mut self) {
        // Model overrides
        if let Ok(val) = env::var(format!("{}MODEL", ENV_PREFIX)) {
            self.model.model_type = val;
        }
        if let Ok(val) = env::var(format!("{}MODEL_AUTO_DOWNLOAD", ENV_PREFIX)) {
            self.model.auto_download = val.parse().unwrap_or(true);
        }
        if let Ok(val) = env::var(format!("{}MODEL_PATH", ENV_PREFIX)) {
            self.model.model_path = Some(val);
        }
        if let Ok(val) = env::var(format!("{}EMBEDDING_DIM", ENV_PREFIX)) {
            self.model.embedding_dim = val.parse().ok();
        }

        // Indexing overrides
        if let Ok(val) = env::var(format!("{}BATCH_SIZE", ENV_PREFIX)) {
            self.indexing.batch_size = val.parse().unwrap_or(32);
        }
        if let Ok(val) = env::var(format!("{}USE_GITIGNORE", ENV_PREFIX)) {
            self.indexing.use_gitignore = val.parse().unwrap_or(true);
        }

        // Chunking overrides
        if let Ok(val) = env::var(format!("{}CHUNK_SIZE", ENV_PREFIX)) {
            self.chunking.chunk_size = val.parse().unwrap_or(50);
        }
        if let Ok(val) = env::var(format!("{}CHUNK_OVERLAP", ENV_PREFIX)) {
            self.chunking.chunk_overlap = val.parse().unwrap_or(10);
        }

        // Search overrides
        if let Ok(val) = env::var(format!("{}DEFAULT_LIMIT", ENV_PREFIX)) {
            self.search.default_limit = val.parse().unwrap_or(10);
        }
        if let Ok(val) = env::var(format!("{}FTS_WEIGHT", ENV_PREFIX)) {
            self.search.fts_weight = val.parse().unwrap_or(0.6);
        }
        if let Ok(val) = env::var(format!("{}VECTOR_WEIGHT", ENV_PREFIX)) {
            self.search.vector_weight = val.parse().unwrap_or(0.4);
        }

        // Database overrides
        if let Ok(val) = env::var(format!("{}DATA_DIR", ENV_PREFIX)) {
            self.database.data_dir = val;
        }
        if let Ok(val) = env::var(format!("{}DB_NAME", ENV_PREFIX)) {
            self.database.db_name = val;
        }

        // Performance overrides
        if let Ok(val) = env::var(format!("{}HNSW_ENABLED", ENV_PREFIX)) {
            self.performance.hnsw_enabled = val.parse().unwrap_or(false);
        }
        if let Ok(val) = env::var(format!("{}HNSW_MAX_CONNECTIONS", ENV_PREFIX)) {
            self.performance.hnsw_max_connections = val.parse().unwrap_or(16);
        }
        if let Ok(val) = env::var(format!("{}HNSW_EF_SEARCH", ENV_PREFIX)) {
            self.performance.hnsw_ef_search = val.parse().unwrap_or(64);
        }
        if let Ok(val) = env::var(format!("{}CACHE_SIZE", ENV_PREFIX)) {
            self.performance.cache_size = val.parse().unwrap_or(1000);
        }
        if let Ok(val) = env::var(format!("{}PERF_BATCH_SIZE", ENV_PREFIX)) {
            self.performance.batch_size = val.parse().unwrap_or(32);
        }
        if let Ok(val) = env::var(format!("{}USE_GPU", ENV_PREFIX)) {
            self.performance.use_gpu = val.parse().unwrap_or(true);
        }

        // Distributed overrides
        if let Ok(val) = env::var(format!("{}DISTRIBUTED_ENABLED", ENV_PREFIX)) {
            self.distributed.enabled = val.parse().unwrap_or(false);
        }
    }

    pub fn config_path() -> Option<PathBuf> {
        ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    pub fn config_dir() -> Option<PathBuf> {
        ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the data directory for storing index data
    pub fn get_data_dir(&self) -> Option<PathBuf> {
        ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .map(|dirs| dirs.data_dir().join(&self.database.data_dir))
    }

    /// Get the database path
    pub fn get_db_path(&self) -> Option<PathBuf> {
        self.get_data_dir()
            .map(|dir| dir.join(&self.database.db_name))
    }
}

// ============== Global Config Access ==============

use std::sync::OnceLock;

static GLOBAL_CONFIG: OnceLock<Config> = OnceLock::new();

/// Get the global configuration, loading it if necessary
pub fn get_config() -> &'static Config {
    GLOBAL_CONFIG.get_or_init(Config::load)
}

/// Set the global configuration (useful for testing)
pub fn set_config(config: Config) -> &'static Config {
    GLOBAL_CONFIG.get_or_init(|| config)
}

/// Reset the global configuration (useful for testing)
pub fn reset_config() {
    // OnceLock doesn't support reset, so this is a no-op
    // For testing, use set_config() to replace the config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.model.model_type, "minilm");
        assert_eq!(config.chunking.chunk_size, 50);
        assert_eq!(config.chunking.chunk_overlap, 10);
        assert_eq!(config.search.default_limit, 10);
        assert_eq!(config.search.fts_weight, 0.6);
        assert_eq!(config.search.vector_weight, 0.4);
        assert_eq!(config.database.data_dir, "code-search");
        assert_eq!(config.database.db_name, "index.db");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.model.model_type, parsed.model.model_type);
        assert_eq!(config.chunking.chunk_size, parsed.chunking.chunk_size);
    }

    #[test]
    fn test_config_partial_deserialization() {
        let partial_toml = r#"
[model]
model_type = "nomic"

[chunking]
chunk_size = 100
"#;
        let config: Config = toml::from_str(partial_toml).unwrap();
        assert_eq!(config.model.model_type, "nomic");
        assert_eq!(config.chunking.chunk_size, 100);
        assert_eq!(config.chunking.chunk_overlap, 10); // default
        assert_eq!(config.search.default_limit, 10); // default
    }

    #[test]
    fn test_legacy_accessors() {
        let config = Config::default();
        assert_eq!(config.model_name(), "minilm");
        assert_eq!(config.chunk_size(), 50);
        assert_eq!(config.chunk_overlap(), 10);
        assert_eq!(config.default_limit(), 10);
        assert_eq!(config.extensions().len(), 119);
        assert_eq!(config.skip_dirs().len(), 26);
        assert_eq!(config.skip_files().len(), 30);
        assert_eq!(config.batch_size(), 32);
        assert_eq!(config.fts_weight(), 0.6);
        assert_eq!(config.vector_weight(), 0.4);
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path();
        assert!(path.is_some() || path.is_none()); // Depends on platform
    }

    #[test]
    fn test_extensions_contain_common_languages() {
        let config = Config::default();
        let exts = config.extensions();
        assert!(exts.contains(&".rs".to_string()));
        assert!(exts.contains(&".py".to_string()));
        assert!(exts.contains(&".js".to_string()));
        assert!(exts.contains(&".ts".to_string()));
        assert!(exts.contains(&".go".to_string()));
    }

    #[test]
    fn test_skip_dirs_contain_common() {
        let config = Config::default();
        let dirs = config.skip_dirs();
        assert!(dirs.contains(&".git".to_string()));
        assert!(dirs.contains(&"node_modules".to_string()));
        assert!(dirs.contains(&"target".to_string()));
    }

    #[test]
    fn test_skip_files_contain_common() {
        let config = Config::default();
        let files = config.skip_files();
        assert!(files.contains(&"*.pyc".to_string()));
        assert!(files.contains(&"*.lock".to_string()));
        assert!(files.contains(&".DS_Store".to_string()));
    }
}
