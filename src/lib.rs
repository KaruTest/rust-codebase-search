//! # code-search
//!
//! A fast, semantic code search library for codebases using vector embeddings and full-text search.
//!
//! ## Features
//!
//! - **Semantic Search**: Use vector embeddings to find code by meaning, not just keywords
//! - **Full-Text Search**: Hybrid search combining semantic and keyword matching
//! - **Language Detection**: Automatic detection of programming languages
//! - **Code Chunking**: Intelligent splitting of files into searchable chunks
//! - **Gitignore Support**: Respect .gitignore patterns when indexing
//! - **Incremental Updates**: Track changes and update index efficiently
//! - **Multiple Embedding Models**: Support for various sentence-transformer models
//!
//! ## Quick Start
//!
//! ### Indexing a Codebase
//!
//! ```no_run
//! use code_search::indexing::{Indexer, IndexingOptions};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = IndexingOptions {
//!     verbose: true,
//!     ..Default::default()
//! };
//!
//! let mut indexer = Indexer::new(config);
//! let stats = indexer.index_codebase("/path/to/codebase")?;
//! println!("{}", stats);
//! # Ok(())
//! # }
//! ```
//!
//! ### Searching Code
//!
//! ```no_run
//! use code_search::search::search;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let query = "database connection handling";
//! let codebase_path = "/path/to/codebase";
//! let results = search(query, codebase_path, 10, false)?;
//!
//! for result in results {
//!     println!("{} ({}): score={:.4}", result.file, result.lines, result.score);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Using Embeddings Directly
//!
//! ```no_run
//! use code_search::embedding::{get_embedding, get_query_embedding, ensure_model_available};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! ensure_model_available()?;
//! let embedding = get_embedding("function to connect to database");
//! println!("Embedding dimension: {}", embedding.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Modules Overview
//!
//! ### Core Modules
//!
//! - [`embedding`]: Embedding model loading and inference
//! - [`indexing`]: Codebase indexing with incremental updates
//! - [`database`]: SQLite database operations and search functions
//! - [`search`]: High-level search API with result formatting
//!
//! ### Utility Modules
//!
//! - [`splitter`]: Code splitting and language detection
//! - [`gitignore`]: Gitignore pattern matching
//! - [`manifest`]: Manifest tracking for incremental updates
//! - [`error`]: Error types and Result alias
//! - [`cli`]: Command-line interface

pub mod cli;
pub mod config;
pub mod context_enriched;
pub mod database;
pub mod embedding;
pub mod error;
pub mod gitignore;
pub mod indexing;
pub mod manifest;
pub mod mcp;
pub mod performance;
pub mod query_expansion;
pub mod search;
pub mod session;
pub mod splitter;
pub mod syntax_aware;

#[cfg(feature = "advanced")]
pub mod advanced;

pub use cli::{run, Cli};
pub use config::{
    get_config, set_config, reset_config, Config, ChunkingConfig, DatabaseConfig,
    DistributedConfig, ModelConfig, PerformanceConfig, SearchConfig,
};
pub use database::{
    delete_chunks_for_codebase, delete_chunks_for_file, get_codebase_stats, get_db_path,
    get_global_stats, hybrid_search, init_db, insert_chunks, reset_db, vector_search, Chunk,
    SearchResult, SearchFilters, Stats, CodebaseMetadata, register_codebase, 
    get_codebase_metadata, list_codebases_with_metadata, DATA_DIR, DB_NAME,
};
pub use embedding::{
    check_available, check_available_with_model, ensure_model_available,
    ensure_model_available_with_model, get_embedding, get_embedding_with_model,
    get_embeddings_batch, get_embeddings_batch_with_model, get_model_dimension,
    get_query_embedding, get_query_embedding_with_model, is_model_loaded, zero_embedding,
    zero_embedding_with_model, EmbeddingModel, ModelType, DEFAULT_MODEL,
};
pub use error::{CodeSearchError, Result};
pub use gitignore::GitignoreMatcher;
pub use indexing::{list_indexed_codebases, CodebaseInfo, Indexer, IndexingOptions, IndexingStats};
pub use manifest::{
    get_changes, get_codebase_hash, get_manifest_path, hash_file_content, load_manifest,
    save_manifest, Changes,
};
pub use search::{format_results, search, FormattedResult, SearchResult as SearchAPIResult};
pub use splitter::{
    detect_language, generate_chunk_id, language_map, split_file, CodeChunk, DEFAULT_CHUNK_SIZE,
    DEFAULT_OVERLAP,
};
pub use syntax_aware::{
    get_language_config, is_language_supported, parse_source, split_file_syntax_aware, LanguageConfig,
    Point, SyntaxNode,
};
pub use context_enriched::{
    enrich_chunk, extract_context, extract_doc_comments, extract_function_signatures,
    extract_imports, extract_types, estimate_tokens, ChunkMetadata, EnrichedChunk, LineRange,
};
pub use query_expansion::{
    correct_typos, expand_query, expand_query_fts, process_query, ProcessedQuery,
    TokenBudget,
};
pub use session::{
    MultiStepSearchRequest, MultiStepSearchResult, QueryStep, SearchSession, SessionManager,
    SessionSearchResult, SessionSummary,
};

// Performance module exports
pub use performance::{
    batch::{BatchConfig, BatchProgress, BatchResult, detect_gpu_acceleration, get_optimal_batch_size, process_embeddings_batch, process_embeddings_with_callback, GpuInfo, GpuProvider},
    cache::{get_query_cache, init_query_cache, invalidate_query_cache, CacheStats, QueryCache},
    distributed::{DistributedConfig as DistConfig, DistributedQueryRouter, DistributedSearchPlan, ShardConfig, ShardManager, ShardRouter, ConsistencyLevel, get_distributed_router, init_distributed},
    hnsw::{distance_to_similarity, HnswConfig, HnswIndex},
};

#[cfg(feature = "advanced")]
pub use advanced::{
    analyze_api_change, predict_changes, find_related_tests, get_cached_graph,
    get_graph_resources, search_multi_codebase, rerank_results_llm, summarize_chunk_llm,
    CodeGraph, ChangePrediction, ChangeType, EdgeType, GraphEdge, GraphNode, GraphResource,
    LlmConfig, MultiCodebaseResult, NodeType, PredictedChange, SemanticAction, ActionType,
    ActionTarget,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_language("test.rs"), "rust");
        assert_eq!(detect_language("test.py"), "python");
        assert_eq!(detect_language("test.js"), "javascript");
    }

    #[test]
    fn test_code_chunking() {
        let content = (1..=100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split_file("test.rs", &content, Some(50), Some(10));

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].language, "rust");
    }

    #[test]
    fn test_zero_embedding() {
        let embedding = zero_embedding();
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&v| v == 0.0));
    }
}
