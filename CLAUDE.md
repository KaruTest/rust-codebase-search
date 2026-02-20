# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a high-performance semantic code search tool written in Rust. It indexes codebases and enables searching using hybrid vector similarity + full-text search.

## Common Commands

```bash
# Build the project
cargo build --release   # Optimized build (binary at target/release/code-search)
cargo build             # Development build

# Run tests
cargo test              # All tests
cargo test --lib        # Library unit tests only
cargo test --test integration_test  # Integration tests

# Run benchmarks
cargo bench

# Check formatting and linting
cargo fmt --check
cargo clippy -- -D warnings
```

## Architecture

The codebase is organized into these core modules:

- **`embedding.rs`** - ONNX Runtime integration for ML model inference. Loads sentence-transformer models and generates vector embeddings for code chunks. Supports MiniLM (384-dim) and Nomic (768-dim) models.
- **`database.rs`** - SQLite operations using the sqlite-vec extension for vector storage. Handles chunk storage, vector similarity search, and FTS full-text search.
- **`indexing.rs`** - Codebase scanning and chunking logic. Walks directories, respects .gitignore, and manages incremental updates via SHA256 manifests.
- **`search.rs`** - Search API with hybrid search (RRF - Reciprocal Rank Fusion combining vector + full-text) and vector-only modes.
- **`splitter.rs`** - Intelligent code chunking with overlap and automatic language detection for 60+ file types.
- **`cli.rs`** - Command-line interface using clap. Provides `index`, `search`, `status`, and `mcp` subcommands.

### Data Flow

1. **Indexing**: File → Split into chunks → Generate embeddings (ONNX) → Store in SQLite
2. **Search**: Query → Generate embedding → Vector search + FTS search → RRF fusion → Results

## MCP Server

This project exposes an MCP (Model Context Protocol) server for IDE integration. The server tools are:
- `codebase_status` - List indexed codebases
- `codebase_index` - Index a codebase for searching
- `codebase_search` - Semantic search across indexed code
- `codebase_delete` - Remove a codebase from index

## Code Search Preferences

**IMPORTANT: When searching for code in this codebase, ALWAYS use the MCP code-search server instead of manual grep/glob searches.**

The MCP server provides superior results through:
- Embedding-based semantic similarity search
- Understanding code functionality, not just text matches
- Natural language queries like "find function that handles authentication"

**Preferred (MCP):**
- Use `mcp__code-search__codebase_search` for semantic queries
- Use `mcp__code-search__codebase_index` to index if needed

**Fallback only:**
- Use grep/glob only when MCP is unavailable or you need exact text matching

## Claude Settings

The `.claude/settings.local.json` file configures permissions:
- WebSearch enabled
- Context7 library documentation lookup enabled
- WebFetch allowed for crates.io, github.com, docs.rs, raw.githubusercontent.com, ort.pyke.io

## Key Dependencies

- **ort** - ONNX Runtime Rust bindings (for embedding inference)
- **rusqlite** with bundled SQLite - Database with vector support
- **rayon** - Parallel processing for file scanning and embedding generation
- **clap** - CLI argument parsing
- **ignore** - .gitignore pattern matching

## Data Storage

Index data is stored in `~/.claude/code-search-data/`:
- `index.db` - SQLite database with chunks, vectors, and FTS index
- `manifests/` - SHA256 manifests for incremental indexing
