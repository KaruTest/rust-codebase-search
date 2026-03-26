# Codebase Search MCP Server

A high-performance semantic code search MCP (Model Context Protocol) server for AI-assisted development. Index codebases and search using hybrid vector similarity + full-text search directly from Claude Code, Claude Desktop, Zed, VSCode, and other MCP-compatible clients.

---

## Quick Start

```bash
# Download the latest release for your platform
# Linux: https://github.com/yourusername/rust-codebase-search/releases/latest/download/code-search-linux
# Windows: https://github.com/yourusername/rust-codebase-search/releases/latest/download/code-search.exe

# Make it executable (Linux only)
chmod +x code-search-linux

# Configure it with your MCP client (see below)
```

---

## Installation

### Download Pre-built Binaries

Pre-built binaries are available for Linux and Windows from the [GitHub Releases](https://github.com/yourusername/rust-codebase-search/releases) page.

**Linux:**
```bash
# Download the latest Linux binary
wget https://github.com/yourusername/rust-codebase-search/releases/latest/download/code-search-linux

# Make it executable
chmod +x code-search-linux

# Move to your preferred location
sudo mv code-search-linux /usr/local/bin/code-search
```

**Windows:**
```powershell
# Download the latest Windows binary
# https://github.com/yourusername/rust-codebase-search/releases/latest/download/code-search.exe

# Move to your preferred location (e.g., C:\Tools\code-search.exe)
```

**No build required!** The binaries include all dependencies and are ready to use.

### Build from Source (Optional)

If you prefer to build from source or need a custom build:

```bash
# Prerequisites: Rust 1.70 or later
git clone <repository-url>
cd rust-codebase-search
cargo build --release
```

---

## MCP Server Setup

### Claude Code Setup

1. **Download and install the binary** (see Installation section above)

2. **Create `.mcp.json` in your project root:**

**Linux:**
```json
{
  "codebase-search": {
    "command": "/usr/local/bin/code-search",
    "args": ["mcp"],
    "env": {
      "RUST_LOG": "info"
    }
  }
}
```

**Windows:**
```json
{
  "codebase-search": {
    "command": "C:\\Tools\\code-search.exe",
    "args": ["mcp"],
    "env": {
      "RUST_LOG": "info"
    }
  }
}
```

3. **Update Claude Code Settings:**

Add to `~/.claude/settings.json`:
```json
{
  "enableAllProjectMcpServers": true
}
```

3. **Restart Claude Code** to load the MCP server.

### Claude Desktop Setup

Add to `claude_desktop_config.json`:

**Linux:**
```json
{
  "mcpServers": {
    "code-search": {
      "command": "/usr/local/bin/code-search",
      "args": ["mcp"]
    }
  }
}
```

**Windows:**
```json
{
  "mcpServers": {
    "code-search": {
      "command": "C:\\Tools\\code-search.exe",
      "args": ["mcp"]
    }
  }
}
```

**Config locations:**
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%/Claude/claude_desktop_config.json`

### Zed Editor Setup

Add to `~/.zed/settings.json` (Linux) or `%USERPROFILE%\.zed\settings.json` (Windows):

**Linux:**
```json
{
  "mcp": {
    "code-search": {
      "command": ["/usr/local/bin/code-search", "mcp"]
    }
  }
}
```

**Windows:**
```json
{
  "mcp": {
    "code-search": {
      "command": ["C:\\Tools\\code-search.exe", "mcp"]
    }
  }
}
```

### VSCode Setup

Add to `settings.json`:

**Linux:**
```json
{
  "mcpServers": {
    "code-search": {
      "command": "/usr/local/bin/code-search",
      "args": ["mcp"]
    }
  }
}
```

**Windows:**
```json
{
  "mcpServers": {
    "code-search": {
      "command": "C:\\Tools\\code-search.exe",
      "args": ["mcp"]
    }
  }
}
```

---

## MCP Priority Rules Setup

**IMPORTANT**: To ensure the MCP server is used as the primary source for code searches (instead of random grep/glob), set up MCP priority rules:

### 1. Copy MCP Priority Rules to Your Project

```bash
# Create the rules directory
mkdir -p .claude/rules

# Copy the priority rules
cp docs/MCP_PRIORITY.md .claude/rules/mcp-priority.md
```

### 2. Restart Your MCP Client

Restart Claude Code/Claude Desktop to load the priority rules.

### 3. Verify It's Working

Ask your AI assistant: "Find database functions in this codebase"

If configured correctly, it should use `codebase_search` instead of grep/glob.

**Why MCP Priority Matters:**
- ✅ **Complete Coverage**: Semantic search finds ALL related functions
- ✅ **Consistent Results**: Same query always returns same results
- ✅ **Better Understanding**: MCP knows file relationships and imports
- ✅ **Reliable Changes**: Modifications based on complete codebase knowledge

---

## Available MCP Tools

| Tool | Description |
|------|-------------|
| `codebase_index` | Index a codebase for semantic search |
| `codebase_search` | Search indexed code using semantic similarity |
| `codebase_status` | List all indexed codebases and stats |
| `codebase_delete` | Remove a codebase from the index |

### Tool Usage Examples

#### Index a Codebase
```json
{
  "name": "codebase_index",
  "arguments": {
    "path": "/home/user/projects/my-api",
    "tags": "backend,api,rust",
    "model": "minilm"
  }
}
```

#### Search Specific Codebase
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "database connection handling",
    "codebase": "my-api",
    "limit": 10
  }
}
```

#### Search ALL Codebases
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "authentication implementation",
    "limit": 10
  }
}
```

#### Check Status
```json
{
  "name": "codebase_status",
  "arguments": {}
}
```

---

## Using with AI Assistants

### Example 1: Understanding Code

**Prompt:** "How does the authentication system work in this codebase?"

**AI Actions:**
1. Uses `codebase_search` to find authentication-related code
2. Analyzes results from `src/auth/*.rs`, `src/middleware/auth.rs`
3. Provides comprehensive explanation of JWT tokens, login flow, etc.

### Example 2: Finding Functions

**Prompt:** "Find all database error handling functions"

**AI Actions:**
1. Uses `codebase_search` with semantic query "database error handling"
2. Identifies all relevant functions across multiple files
3. Shows file paths, line numbers, and code snippets

### Example 3: Implementing Features

**Prompt:** "I need to add rate limiting to my API endpoints"

**AI Actions:**
1. Searches for existing middleware patterns
2. Finds where authentication is applied
3. Recommends where to add rate limiting based on codebase structure

### Example 4: Cross-Codebase Search

**Prompt:** "Show me all API endpoints across my backend and frontend projects"

**AI Actions:**
1. Uses `codebase_status` to see all indexed codebases
2. Searches across all codebases for "API endpoint definitions"
3. Provides comprehensive overview of backend APIs and frontend API calls

---

## Key Features

- **Semantic Search**: Find code by meaning using vector embeddings
- **Hybrid Search**: Combines vector similarity with full-text search using RRF
- **Cross-Codebase Search**: Search across multiple indexed codebases simultaneously
- **Language Detection**: Automatic detection of 50+ programming languages
- **Syntax-Aware Chunking**: Intelligent code splitting using tree-sitter AST parsing
- **Gitignore Support**: Respect `.gitignore` patterns when indexing
- **Incremental Updates**: Track changes using SHA256 manifests
- **Multiple Models**: Support for MiniLM, Nomic, Nemotron, and custom models
- **Fast Performance**: Sub-100ms search latency, 10x faster than Python

---

## Configuration Options

The MCP server supports several configuration options:

### Embedding Models

- **minilm** (default): Fast, lightweight (384-dim)
- **nomic**: Higher quality (768-dim)
- **nemotron**: Large context (2048-dim)
- **custom**: Use any HuggingFace model with ONNX support

### Indexing Options

- **force**: Re-index all files (skip incremental updates)
- **verbose**: Enable detailed output during indexing
- **model**: Specify embedding model
- **tags**: Add comma-separated tags to codebase

---

## Data Storage

**Location:**
- Linux: `~/.local/share/code-search/`
- macOS: `~/Library/Application Support/code-search/`
- Windows: `%APPDATA%/code-search/`

**Files:**
- `index.db` - SQLite database with chunks, vectors, and metadata
- `manifests/` - SHA256 manifests for incremental updates

---

## Testing MCP Server

Test your MCP server installation:

**Linux:**
```bash
# Test server initialization
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | /usr/local/bin/code-search mcp

# List available tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | /usr/local/bin/code-search mcp

# Test codebase search
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"codebase_status","arguments":{}}}' | /usr/local/bin/code-search mcp
```

**Windows:**
```powershell
# Test server initialization (PowerShell)
'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | C:\Tools\code-search.exe mcp
```

---

## Troubleshooting

### MCP Server Not Connecting

1. **Verify binary path** is correct in configuration (Linux: `/usr/local/bin/code-search`, Windows: `C:\Tools\code-search.exe`)
2. **Check binary is executable** (Linux only): `chmod +x /usr/local/bin/code-search`
3. **Test manually**: Run the test commands above
4. **Check logs**: Set `RUST_LOG=debug` in environment variables

### Indexing Issues

**Linux:**
```bash
# Re-index with force flag
echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"codebase_index","arguments":{"path":"/path/to/codebase","force":true}}}' | /usr/local/bin/code-search mcp
```

**Windows:**
```powershell
# Re-index with force flag
'{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"codebase_index","arguments":{"path":"C:\\path\\to\\codebase","force":true}}}' | C:\Tools\code-search.exe mcp
```

### Clear All Data

```bash
# Remove database and start fresh
rm ~/.local/share/code-search/index.db
```

---

## Performance

- **Indexing**: ~1.5s for 26 files
- **Search latency**: <100ms for typical queries
- **Memory**: <500MB for 100k files
- **Scalability**: Tested with 500k+ chunks

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