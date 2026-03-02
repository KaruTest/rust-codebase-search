# code-search

A high-performance semantic code search tool written in Rust. Index codebases and search using hybrid vector similarity + full-text search. Designed for AI-assisted development with full MCP (Model Context Protocol) support.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [Indexing](#indexing-a-codebase)
  - [Searching](#searching-indexed-code)
  - [Cross-Codebase Search](#cross-codebase-search)
  - [Status](#checking-status)
  - [Delete](#deleting-an-indexed-codebase)
- [Configuration](#configuration)
  - [Config File](#config-file)
  - [Custom Embedding Models](#custom-embedding-models)
  - [Environment Variables](#environment-variable-overrides)
- [MCP Server Setup](#mcp-server-setup)
  - [Available MCP Tools](#available-mcp-tools)
  - [Claude Desktop Setup](#claude-desktop-setup)
  - [Zed Editor Setup](#zed-editor-setup)
  - [VSCode Setup](#vscode-setup)
  - [MCP Tool Examples](#mcp-tool-examples)
- [AI Assistant Prompt Examples](#ai-assistant-prompt-examples)
  - [Indexing a Codebase](#example-1-indexing-a-codebase)
  - [Searching Code](#example-2-searching-for-code)
  - [Implementing a Feature](#example-3-implementing-a-new-feature)
  - [Cross-Codebase Search](#example-4-cross-codebase-search)
- [Library Usage](#library-usage)
- [Embedding Models](#embedding-models)
- [Data Storage](#data-storage)
- [Performance](#performance)
- [Development](#development)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

```bash
# Build
cargo build --release

# Index a codebase
./target/release/code-search index /path/to/codebase

# Search code
./target/release/code-search search "database connection" --codebase /path/to/codebase

# Check status
./target/release/code-search status --list

# Start MCP server for AI integration
./target/release/code-search mcp
```

---

## Features

- **Semantic Search**: Find code by meaning using vector embeddings
- **Hybrid Search**: Combines vector similarity with full-text search using RRF (Reciprocal Rank Fusion)
- **Cross-Codebase Search**: Search across multiple indexed codebases simultaneously
- **Human-Readable Names**: Easy identification of indexed codebases with names and paths
- **Language Detection**: Automatic detection of 50+ programming languages
- **Syntax-Aware Chunking**: Intelligent splitting using tree-sitter AST parsing
- **Context-Enriched Results**: Metadata includes function signatures, imports, and documentation
- **Gitignore Support**: Respect `.gitignore` patterns when indexing
- **Incremental Updates**: Track changes using SHA256 manifests
- **Multiple Embedding Models**: Support for MiniLM, Nomic, Nemotron, and custom models
- **MCP Server**: Full Model Context Protocol implementation for AI integration
- **Parallel Processing**: Utilize rayon for fast indexing and search
- **Fast Search**: Sub-100ms query latency

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

# Search by codebase name (easier!)
code-search search "error handling" --codebase my-project-name

# Limit results
code-search search "async function" --codebase /path/to/codebase --limit 5

# Vector-only search (no full-text)
code-search search "parse JSON" --codebase /path/to/codebase --vector-only

# Pretty print with colors
code-search search "http client" --codebase /path/to/codebase --pretty

# Filter by language
code-search search "function definition" --codebase /path/to/codebase --language rust

# Use specific model
code-search search "authentication" --codebase /path/to/codebase --model nomic
```

### Cross-Codebase Search

Search across all indexed codebases at once:

```bash
# Search ALL indexed codebases (omit --codebase parameter)
code-search search "API endpoint for user authentication"

# Results will include which codebase each result came from
```

### Checking Status

```bash
# Show global status
code-search status

# List all indexed codebases with human-readable names
code-search status --list

# List in JSON format
code-search status --list --json
```

Example output:
```
Indexed codebases:

  my-backend-api (/home/user/projects/backend-api)
    ID: abc123def456
    Files: 245, Chunks: 1823
    Model: nomic

  my-frontend-app (/home/user/projects/frontend-app)
    ID: def789ghi012
    Files: 132, Chunks: 956
    Model: minilm
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
model_type = "minilm"      # "minilm", "nomic", "nemotron", or "custom"
auto_download = true

# Custom model configuration (when model_type = "custom")
# model_path = "jinaai/jina-embeddings-v2-base-code"  # HuggingFace model ID
# embedding_dim = 768  # Required for custom models

[indexing]
extensions = [".rs", ".py", ".js", ".ts", ".go", ".java"]
skip_dirs = [".git", "node_modules", "target"]
skip_files = ["*.pyc", "*.lock"]
use_gitignore = true
batch_size = 32

[chunking]
chunk_size = 50
chunk_overlap = 10
token_budget = "medium"  # "small" (256), "medium" (512), or "large" (1024)
use_syntax_aware = true  # Use tree-sitter for intelligent chunking

[search]
default_limit = 10
fts_weight = 0.6        # Weight for full-text search (0.0-1.0)
vector_weight = 0.4     # Weight for vector search (0.0-1.0)
enable_fuzzy = true     # Enable fuzzy matching for typos
enable_ltr = true       # Enable learning-to-rank personalization
fuzzy_max_distance = 2  # Max edit distance for fuzzy matching

[database]
data_dir = "code-search"
db_name = "index.db"
```

### Custom Embedding Models

Use any HuggingFace model that supports ONNX export:

```toml
[model]
model_type = "custom"
model_path = "jinaai/jina-embeddings-v2-base-code"  # HuggingFace model ID
embedding_dim = 768
auto_download = true
```

**Popular custom models:**
- `jinaai/jina-embeddings-v2-base-code` (768-dim) - Optimized for code ✅
- `sentence-transformers/all-mpnet-base-v2` (768-dim) - General purpose
- `BAAI/bge-large-en-v1.5` (1024-dim) - High quality
- `intfloat/multilingual-e5-base` (768-dim) - Multilingual

See [CUSTOM_MODEL_TEST.md](CUSTOM_MODEL_TEST.md) for detailed testing results.

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

This project includes a full MCP (Model Context Protocol) server for IDE integration. The MCP server provides tools for semantic code search that can be used by Claude, Zed, VSCode, and other MCP-compatible clients.

### Starting the MCP Server

```bash
# Start the MCP server (uses stdio transport)
code-search mcp

# Or with the binary path
/path/to/code-search mcp
```

The MCP server runs in stdio mode, making it compatible with Claude Desktop, Zed, and VSCode.

### Testing the MCP Server

You can test the MCP server manually using JSON-RPC:

```bash
# Start the server
code-search mcp

# Send initialize request (from another terminal or script)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | code-search mcp

# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | code-search mcp
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `codebase_index` | Index a codebase for semantic search |
| `codebase_search` | Search indexed code using semantic similarity |
| `codebase_status` | List all indexed codebases and stats |
| `codebase_delete` | Remove a codebase from the index |

### Tool Details

**codebase_index:**
```json
{
  "path": "/path/to/codebase",
  "force": false,
  "verbose": false,
  "model": "minilm",
  "tags": "backend,api,auth"
}
```

**codebase_search:**
```json
{
  "query": "function that handles authentication",
  "codebase": "my-project-name",  // Optional - omit to search ALL codebases
  "limit": 10
}
```

The `codebase` parameter can be:
- **Omitted**: Searches ALL indexed codebases
- **Codebase name**: e.g., "my-backend-api"
- **Full path**: e.g., "/home/user/projects/backend"
- **Codebase ID**: e.g., "abc123def456"

**codebase_status:**
```json
{}
```

Returns human-readable names and full paths for all indexed codebases.

**codebase_delete:**
```json
{
  "path": "/path/to/codebase"
}
```

### Claude Desktop Setup

Add to `claude_desktop_config.json`:

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

**Config locations:**
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%/Claude/claude_desktop_config.json`

### Zed Editor Setup

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

### VSCode Setup

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

### MCP Tool Examples

#### Example 1: Check what's indexed
```json
{
  "name": "codebase_status",
  "arguments": {}
}
```

Returns:
```json
{
  "codebases": [
    {
      "id": "abc123...",
      "name": "my-backend-api",
      "path": "/home/user/projects/backend",
      "chunk_count": 1823,
      "file_count": 245,
      "model": "nomic"
    }
  ]
}
```

#### Example 2: Index a new codebase
```json
{
  "name": "codebase_index",
  "arguments": {
    "path": "/home/user/projects/frontend",
    "tags": "frontend,react,ui"
  }
}
```

#### Example 3: Search specific codebase
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "API endpoint for user authentication",
    "codebase": "my-backend-api",
    "limit": 5
  }
}
```

#### Example 4: Search ALL codebases
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "database connection pooling implementation",
    "limit": 10
  }
}
```

Results include `codebase_name` field to identify which codebase each result came from.

---

## AI Assistant Prompt Examples

Here are example prompts you can use with AI assistants (Claude, GPT-4, etc.) that have access to the MCP server.

### Example 1: Indexing a Codebase

**Prompt:**
```
I have a backend API project at /home/user/projects/my-api that I'd like to index for semantic search. 

Please:
1. Check if there are any codebases already indexed
2. Index the backend API project
3. Verify it was indexed successfully
```

**Expected AI Actions:**
1. Calls `codebase_status` to see current state
2. Calls `codebase_index` with path `/home/user/projects/my-api`
3. Calls `codebase_status` again to confirm indexing

---

### Example 2: Searching for Code

**Prompt:**
```
I need to find the code that handles user authentication in my backend API. 

Please search for:
1. Authentication logic
2. JWT token generation
3. Password validation

Show me the most relevant code chunks.
```

**Expected AI Actions:**
1. Calls `codebase_search` with query "user authentication logic"
2. Calls `codebase_search` with query "JWT token generation"
3. Calls `codebase_search` with query "password validation"
4. Presents results with file paths, line numbers, and code snippets

---

### Example 3: Implementing a New Feature

**Prompt:**
```
I need to implement a rate limiting feature for my API endpoints. 

Please:
1. Search for existing middleware or request handling code
2. Find where authentication is currently applied
3. Look for any existing rate limiting or throttling code
4. Search for configuration management patterns

Then provide recommendations for where to add the rate limiting logic.
```

**Expected AI Actions:**
1. Calls `codebase_search` with "middleware request handling"
2. Calls `codebase_search` with "authentication middleware"
3. Calls `codebase_search` with "rate limiting throttling"
4. Calls `codebase_search` with "configuration management"
5. Analyzes results and provides implementation recommendations

**Example Output:**
```
Based on the search results, I found:

1. **Middleware Pattern**: Your API uses middleware in src/middleware/auth.rs (lines 45-78)
   - This is where authentication is applied
   - Good place to add rate limiting middleware

2. **Existing Request Handling**: Found in src/handlers/request_handler.rs (lines 120-150)
   - Request context is built here
   - Rate limiting check could be added here

3. **Configuration**: Config is loaded from src/config.rs (lines 30-65)
   - Add rate limit settings to the config struct

Recommendation:
Create a new file src/middleware/rate_limit.rs and integrate it into your middleware chain similar to the auth middleware pattern.
```

---

### Example 4: Cross-Codebase Search

**Prompt:**
```
I'm building a frontend that needs to integrate with my backend API. 

Please:
1. Check what codebases are indexed
2. Search the backend for all API endpoints
3. Search for authentication flow in the backend
4. Look for API documentation or OpenAPI specs

I need to understand the backend API structure to build the frontend integration.
```

**Expected AI Actions:**
1. Calls `codebase_status` to list all codebases
2. Calls `codebase_search` (without codebase param) for "API endpoint definitions"
3. Calls `codebase_search` for "authentication flow login"
4. Calls `codebase_search` for "OpenAPI swagger documentation"
5. Presents comprehensive overview of backend API structure

**Example Output:**
```
I searched across your indexed codebases and found:

## Backend API: my-backend-api

**API Endpoints Found:**
- POST /api/auth/login (src/handlers/auth.rs:45-78)
- POST /api/auth/register (src/handlers/auth.rs:80-120)
- GET /api/users/profile (src/handlers/users.rs:30-55)
- PUT /api/users/profile (src/handlers/users.rs:57-90)

**Authentication Flow:**
- Uses JWT tokens (src/auth/jwt.rs:20-65)
- Token expiry: 24 hours
- Refresh token endpoint: POST /api/auth/refresh

**API Documentation:**
- OpenAPI spec found at docs/api/openapi.yaml
- Base URL: https://api.example.com/v1

For your frontend integration, you'll need to:
1. Implement JWT token storage and refresh logic
2. Add Authorization header to all API requests
3. Handle 401 responses by redirecting to login
```

---

### Example 5: Refactoring Code

**Prompt:**
```
I want to refactor the database connection logic in my backend API. 

Please:
1. Find all places where database connections are created
2. Search for connection pooling configuration
3. Look for transaction handling code
4. Find any existing database utility functions

Then suggest a refactoring approach to centralize database connection management.
```

**Expected AI Actions:**
1. Multiple `codebase_search` calls to find database-related code
2. Analyze patterns and duplication
3. Provide refactoring recommendations with specific file locations

---

## Library Usage

```rust
use code_search::{
    init_db, get_query_embedding, hybrid_search,
    splitter::split_file, indexing::Indexer, IndexingOptions
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create indexer
    let config = IndexingOptions {
        verbose: true,
        ..Default::default()
    };
    let mut indexer = Indexer::new(config);
    let stats = indexer.index_codebase("/path/to/codebase")?;

    // Search code
    let query = "database connection";
    let embedding = get_query_embedding(query);
    let conn = init_db()?;
    let results = hybrid_search(&conn, query, None, &embedding, 10, &Default::default(), false)?;

    for result in results {
        println!("{} - Score: {:.4}", result.file_path, result.score);
    }
    Ok(())
}
```

---

## Embedding Models

| Model | Dimension | Use Case | Performance |
|-------|-----------|----------|-------------|
| MiniLM (default) | 384 | Fast, lightweight | ⚡⚡⚡ Fast |
| Nomic | 768 | Higher quality | ⚡⚡ Fast |
| Nemotron | 2048 | Large context | ⚡ Slower |
| **Custom** | User-defined | Use any model | Varies |

**MiniLM:**
- Repo: `sentence-transformers/all-MiniLM-L6-v2`
- Prefix: None
- Best for: General purpose, fast indexing

**Nomic:**
- Repo: `nomic-ai/nomic-embed-text-v1.5`
- Prefix: `search_document: ` / `search_query: `
- Best for: Complex semantic queries

**Custom Models:**
- Any HuggingFace model with ONNX support
- Tested: `jinaai/jina-embeddings-v2-base-code` ✅
- See [CUSTOM_MODEL_TEST.md](CUSTOM_MODEL_TEST.md) for details

---

## Data Storage

**Location:**
- Linux: `~/.local/share/code-search/`
- macOS: `~/Library/Application Support/code-search/`
- Windows: `%APPDATA%/code-search/`

**Files:**
- `index.db` - SQLite database with chunks, vectors, and metadata
- `manifests/` - SHA256 manifests for incremental updates

**Database Schema:**
- `chunks` - Code chunks with embeddings
- `codebases` - Codebase metadata (name, path, model, tags)
- `chunks_fts` - Full-text search index
- `search_clicks` - Learning-to-rank feedback

---

## Performance

- **Indexing**: 10x faster than Python
- **Search latency**: <100ms for typical queries
- **Memory**: <500MB for 100k files
- **Scalability**: Tested with 500k+ chunks

**Optimizations:**
1. Parallel processing with rayon
2. Batch database inserts
3. Statement caching
4. HNSW vector indexing
5. Query embedding caching

---

## Development

**Project structure:**
```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── cli.rs               # CLI commands
├── config.rs            # Configuration management
├── database.rs          # SQLite operations
├── embedding.rs         # ML model inference
├── splitter.rs          # Code chunking
├── indexing.rs          # Indexing logic
├── search.rs            # Search API
├── mcp.rs               # MCP server implementation
├── syntax_aware.rs      # Tree-sitter parsing
├── context_enriched.rs  # Metadata extraction
├── query_expansion.rs   # Query processing
├── session.rs           # Multi-step search
└── performance/         # Performance modules
    ├── hnsw.rs          # HNSW indexing
    ├── cache.rs         # Query caching
    ├── batch.rs         # Batch processing
    └── distributed.rs   # Distributed support
```

**Commands:**
```bash
cargo build              # Build
cargo test              # Run tests
cargo test --lib        # Unit tests only
cargo bench             # Run benchmarks
cargo fmt --check       # Check formatting
cargo clippy            # Linting
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

For custom models, verify the model ID and dimension:
```bash
# Test model exists
curl -I https://huggingface.co/jinaai/jina-embeddings-v2-base-code
```

### Database Migration Issues

If you see errors after updating:
```bash
# Backup and recreate database
cp ~/.local/share/code-search/index.db ~/.local/share/code-search/index.db.backup
rm ~/.local/share/code-search/index.db
code-search index /path/to/codebase --force
```

### Cross-Codebase Search Not Working

Ensure all codebases are indexed with the same embedding model for best results:
```bash
# Check models used
code-search status --list

# Re-index if needed with same model
code-search index /path/to/codebase --model nomic
```

### MCP Server Not Responding

1. Check the binary path is correct
2. Ensure the binary is executable: `chmod +x /path/to/code-search`
3. Test manually: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | code-search mcp`

---

## Additional Resources

- [REVIEW.md](REVIEW.md) - Comprehensive implementation review
- [CUSTOM_MODEL_TEST.md](CUSTOM_MODEL_TEST.md) - Custom model testing guide
- [CROSS_CODEBASE_SEARCH.md](CROSS_CODEBASE_SEARCH.md) - Cross-codebase search documentation
- [plan.md](plan.md) - Development roadmap

---

## License

[Specify your license]

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Ensure tests pass: `cargo test`
5. Run linting: `cargo clippy`
6. Format code: `cargo fmt`
7. Submit a pull request
