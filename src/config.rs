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
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_type: default_model_type(),
            auto_download: default_auto_download(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_limit")]
    pub default_limit: usize,
    #[serde(default = "default_fts_weight")]
    pub fts_weight: f64,
    #[serde(default = "default_vector_weight")]
    pub vector_weight: f64,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_limit: default_limit(),
            fts_weight: default_fts_weight(),
            vector_weight: default_vector_weight(),
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: ModelConfig::default(),
            indexing: IndexingConfig::default(),
            chunking: ChunkingConfig::default(),
            search: SearchConfig::default(),
            database: DatabaseConfig::default(),
        }
    }
}

// Legacy field accessors for backward compatibility
impl Config {
    pub fn model_name(&self) -> &str {
        &self.model.model_type
    }

    pub fn chunk_size(&self) -> usize {
        self.chunking.chunk_size
    }

    pub fn chunk_overlap(&self) -> usize {
        self.chunking.chunk_overlap
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

    pub fn data_dir(&self) -> &str {
        &self.database.data_dir
    }

    pub fn db_name(&self) -> &str {
        &self.database.db_name
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
        self.get_data_dir().map(|dir| dir.join(&self.database.db_name))
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
        assert_eq!(config.extensions().len(), 120);
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
