# code-search

A high-performance semantic code search tool written in Rust. Index codebases and search using hybrid vector similarity + full-text search.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [Indexing](#indexing-a-codebase)
  - [Searching](#searching-indexed-code)
  - [Status](#checking-status)
  - [Delete](#deleting-an-indexed-codebase)
- [Configuration](#configuration)
  - [Config File](#config-file)
  - [Environment Variables](#environment-variable-overrides)
- [MCP Server Setup](#mcp-server-setup)
- [Claude Skill](#claude-skill)
- [Library Usage](#library-usage)
- [Embedding Models](#embedding-models)
- [Data Storage](#data-storage)
- [Performance](#performance)
- [Development](#development)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

```bash
# Install/build
cargo build --release

# Index a codebase
./target/release/code-search index /path/to/codebase

# Search code
./target/release/code-search search "database connection" --codebase /path/to/codebase
```

---

## Features

- **Semantic Search**: Find code by meaning using vector embeddings
- **Hybrid Search**: Combines vector similarity with full-text search using RRF (Reciprocal Rank Fusion)
- **Language Detection**: Automatic detection of 50+ programming languages
- **Code Chunking**: Intelligent splitting of files into searchable chunks with overlap
- **Gitignore Support**: Respect `.gitignore` patterns when indexing
- **Incremental Updates**: Track changes using SHA256 manifests and update efficiently
- **Multiple Embedding Models**: Support for MiniLM (384-dim) and Nomic (768-dim)
- **Parallel Processing**: Utilize rayon for parallel file scanning and embedding generation
- **Fast Search**: Sub-100ms query latency for typical searches

---

## Installation

### Prerequisites

- Rust 1.70 or later
- ONNX Runtime binaries (automatically downloaded by ort crate)

### Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd rust-codebase-search

# Build the project
cargo build --release

# The binary will be at target/release/code-search
```

### Development Build

```bash
cargo build
cargo test
```

---

## Usage

### Indexing a Codebase

```bash
# Basic indexing
code-search index /path/to/codebase

# Force re-index all files
code-search index /path/to/codebase --force

# Verbose output
code-search index /path/to/codebase --verbose

# Use a different embedding model
code-search index /path/to/codebase --model nomic

# Disable .gitignore filtering
code-search index /path/to/codebase --no-gitignore
```

### Searching Indexed Code

```bash
# Basic search
code-search search "database connection" --codebase /path/to/codebase

# Limit results
code-search search "async function" --codebase /path/to/codebase --limit 5

# Vector-only search (no full-text)
code-search search "error handling" --codebase /path/to/codebase --vector-only

# Pretty print with colors
code-search search "parse JSON" --codebase /path/to/codebase --pretty

# Use specific model
code-search search "http client" --codebase /path/to/codebase --model nomic
```

### Checking Status

```bash
# Show global status
code-search status

# List all indexed codebases
code-search status --list

# List in JSON format
code-search status --list --json
```

### Deleting an Indexed Codebase

```bash
code-search delete /path/to/codebase
```

---

## Configuration

### Config File

code-search uses a TOML config file. Create one with:

```bash
code-search config --create        # Create default config file
code-search config --path         # Show config file path
code-search config                # Show current configuration
```

**Config file locations:**
- Linux: `~/.config/code-search/config.toml`
- macOS: `~/Library/Application Support/code-search/config.toml`
- Windows: `%APPDATA%/code-search/config.toml`

**Example config:**

```toml
[model]
model_type = "minilm"      # "minilm" or "nomic"
auto_download = true

[indexing]
extensions = [".rs", ".py", ".js", ".ts", ".go", ".java"]
skip_dirs = [".git", "node_modules", "target"]
skip_files = ["*.pyc", "*.lock"]
use_gitignore = true
batch_size = 32

[chunking]
chunk_size = 50
chunk_overlap = 10

[search]
default_limit = 10
fts_weight = 0.6
vector_weight = 0.4

[database]
data_dir = "code-search"
db_name = "index.db"
```

### Environment Variable Overrides

| Variable | Description |
|----------|-------------|
| `CODE_SEARCH_MODEL` | Model type |
| `CODE_SEARCH_CHUNK_SIZE` | Chunk size |
| `CODE_SEARCH_DEFAULT_LIMIT` | Default result limit |
| `CODE_SEARCH_FTS_WEIGHT` | FTS weight |
| `CODE_SEARCH_VECTOR_WEIGHT` | Vector weight |
| `CODE_SEARCH_DATA_DIR` | Data directory |
| `CODE_SEARCH_DB_NAME` | Database filename |

---

## MCP Server Setup

This project includes an MCP (Model Context Protocol) server for IDE integration.

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "code-search": {
      "command": "path/to/code-search",
      "args": ["mcp"]
    }
  }
}
```

**Config locations:**
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%/Claude/claude_desktop_config.json`

### Zed Editor

Add to `~/.zed/settings.json`:

```json
{
  "mcp": {
    "code-search": {
      "command": ["/path/to/code-search", "mcp"]
    }
  }
}
```

### VSCode

Add to `settings.json`:

```json
{
  "mcpServers": {
    "code-search": {
      "command": "/path/to/code-search",
      "args": ["mcp"]
    }
  }
}
```

---

## Claude Skill

This project includes a Claude skill for semantic code search.

**Skill location:** `.claude/skills/codebase-search/SKILL.md`

**Available tools:**
| Tool | Purpose |
|------|---------|
| `codebase_status` | Check which codebases are indexed |
| `codebase_index` | Index a codebase |
| `codebase_search` | Search code semantically |
| `codebase_delete` | Remove a codebase |

**Usage:**
```bash
# Check status first
codebase_status

# Index if needed
codebase_index path=/my/project

# Search with natural language
codebase_search query="find authentication function" codebase=/my/project
```

---

## Library Usage

```rust
use code_search::{
    init_db, get_query_embedding, hybrid_search,
    splitter::split_file, indexing::Indexer, IndexingConfig
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create indexer
    let config = IndexingConfig {
        verbose: true,
        ..Default::default()
    };
    let mut indexer = Indexer::new(config);
    let stats = indexer.index_codebase("/path/to/codebase")?;

    // Search code
    let query = "database connection";
    let embedding = get_query_embedding(query);
    let results = hybrid_search(&conn, query, None, &embedding, 10)?;

    for result in results {
        println!("{} - Score: {:.4}", result.file_path, result.score);
    }
    Ok(())
}
```

---

## Embedding Models

| Model | Dimension | Use Case |
|-------|-----------|----------|
| MiniLM (default) | 384 | Fast, lightweight |
| Nomic | 768 | Higher quality |

**MiniLM:**
- Repo: `sentence-transformers/all-MiniLM-L6-v2`
- Prefix: None

**Nomic:**
- Repo: `nomic-ai/nomic-embed-text-v1.5`
- Prefix: `search_document: ` / `search_query: `

---

## Data Storage

**Location:**
- Linux: `~/.local/share/code-search/`
- macOS: `~/Library/Application Support/code-search/`
- Windows: `%APPDATA%/code-search/`

**Files:**
- `index.db` - SQLite database with chunks and vectors
- `manifests/` - SHA256 manifests for incremental updates

---

## Performance

- **Indexing**: 10x faster than Python
- **Search latency**: <100ms
- **Memory**: <500MB for 100k files

**Optimizations:**
1. Parallel processing with rayon
2. Batch database inserts
3. Statement caching

---

## Development

**Project structure:**
```
src/
├── main.rs       # CLI entry
├── lib.rs        # Library exports
├── cli.rs        # CLI commands
├── config.rs     # Configuration
├── database.rs   # SQLite operations
├── embedding.rs  # ML model inference
├── splitter.rs   # Code chunking
├── indexing.rs   # Indexing logic
└── search.rs     # Search API
```

**Commands:**
```bash
cargo build          # Build
cargo test          # Run tests
cargo test --lib    # Unit tests only
cargo bench         # Run benchmarks
cargo fmt --check  # Check formatting
cargo clippy        # Linting
```

---

## Troubleshooting

### Reset Index

```bash
# Delete database
rm ~/.local/share/code-search/index.db

# Or use CLI
code-search delete /path/to/codebase
```

### Clear Cached Models

```bash
rm -rf ~/.cache/huggingface/hub/
cargo clean && cargo build --release
```

### Model Download Issues

Models are auto-downloaded from Hugging Face. Ensure you can access `huggingface.co`.

---

## License

[Specify your license]

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Ensure tests pass
5. Submit a pull request
