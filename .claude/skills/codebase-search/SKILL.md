# Codebase Search Skill

This skill provides semantic code search capabilities using vector embeddings and full-text search.

## Overview

The code-search tool enables natural language queries over codebases by:
- Indexing code files and generating vector embeddings
- Supporting hybrid search (semantic + keyword matching)
- Providing rich metadata about code chunks
- Integrating with MCP-compatible AI tools

## Available Tools

### 1. codebase_status

Check which codebases are currently indexed.

**Parameters:** None (or `{"list": true}`)

**Example:**
```json
{
  "name": "codebase_status",
  "arguments": {}
}
```

**Returns:**
```json
{
  "codebases": [
    {
      "codebase_id": "abc123...",
      "chunk_count": 1500,
      "file_count": 120
    }
  ],
  "global_stats": {
    "total_codebases": 1,
    "total_files": 120,
    "total_chunks": 1500
  }
}
```

### 2. codebase_index

Index a codebase for semantic search.

**Parameters:**
- `path` (required): Path to the codebase directory
- `force` (optional): Force re-indexing of all files (default: false)
- `verbose` (optional): Enable verbose output (default: false)
- `model` (optional): Embedding model to use - "minilm", "nomic", or "nemotron" (default: "minilm")

**Example:**
```json
{
  "name": "codebase_index",
  "arguments": {
    "path": "/path/to/your/project",
    "verbose": true,
    "model": "minilm"
  }
}
```

**Returns:**
```json
{
  "success": true,
  "message": "Indexed 120 files, created 1500 chunks in 5.2s",
  "stats": {
    "files_indexed": 120,
    "chunks_created": 1500,
    "duration_ms": 5200
  }
}
```

### 3. codebase_search

Search indexed code using natural language queries.

**Parameters:**
- `query` (required): Natural language search query
- `codebase` (required): Path to the indexed codebase
- `limit` (optional): Maximum number of results (default: 10)

**Example:**
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "function that handles user authentication",
    "codebase": "/path/to/your/project",
    "limit": 5
  }
}
```

**Returns:**
```json
{
  "results": [
    {
      "file": "src/auth/login.rs",
      "lines": "45-78",
      "content": "pub fn authenticate_user(...) { ... }",
      "score": 0.89,
      "language": "rust",
      "rank": 1
    }
  ]
}
```

### 4. codebase_delete

Remove a codebase from the index.

**Parameters:**
- `path` (required): Path to the codebase to delete

**Example:**
```json
{
  "name": "codebase_delete",
  "arguments": {
    "path": "/path/to/your/project"
  }
}
```

## Best Practices

### 1. Check Status Before Searching

Always check if a codebase is indexed before searching:

```json
// First: Check status
{
  "name": "codebase_status",
  "arguments": {}
}

// If not indexed, index it
{
  "name": "codebase_index",
  "arguments": {
    "path": "/path/to/project"
  }
}

// Then search
{
  "name": "codebase_search",
  "arguments": {
    "query": "database connection pooling",
    "codebase": "/path/to/project"
  }
}
```

### 2. Use Natural Language Queries

The search is semantic, so use natural language:

✅ **Good:**
- "function that handles authentication"
- "where is the database connection established"
- "error handling for API requests"
- "configuration file parsing"

❌ **Avoid:**
- Exact function names only
- Single keywords
- Code syntax fragments

### 3. Interpret Results

Each result includes:
- **file**: Path to the source file
- **lines**: Line numbers (e.g., "45-78")
- **content**: The actual code chunk
- **score**: Relevance score (0.0-1.0, higher is better)
- **language**: Programming language
- **rank**: Result ranking (1 is most relevant)

### 4. Use Appropriate Limit

- For quick lookups: `limit: 5`
- For comprehensive search: `limit: 20`
- For exploration: `limit: 50`

### 5. Re-index When Code Changes

If the codebase has been modified:
```json
{
  "name": "codebase_index",
  "arguments": {
    "path": "/path/to/project",
    "force": true
  }
}
```

## Supported Languages

The tool supports 119+ file extensions including:
- **Languages**: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Ruby, PHP, Swift, Kotlin, Scala, and many more
- **Config**: JSON, YAML, TOML, XML
- **Docs**: Markdown, reStructuredText
- **Web**: HTML, CSS, Vue, Svelte
- **Other**: SQL, GraphQL, Protocol Buffers, Solidity

## Search Capabilities

### Semantic Understanding
The search understands:
- Function purpose and behavior
- Code patterns and idioms
- Conceptual relationships
- Documentation and comments

### Hybrid Search
Combines:
- **Vector search**: Semantic similarity using embeddings
- **Full-text search**: Keyword matching with BM25 ranking
- **RRF fusion**: Reciprocal Rank Fusion for optimal results

### Advanced Features
- Syntax-aware chunking at semantic boundaries
- Context-enriched results with metadata
- Query expansion and typo tolerance
- Multi-step search workflows

## Example Workflows

### Finding Authentication Code
```json
// Step 1: Search for authentication
{
  "name": "codebase_search",
  "arguments": {
    "query": "user authentication and login",
    "codebase": "/my/project",
    "limit": 10
  }
}

// Step 2: Refine search based on results
{
  "name": "codebase_search",
  "arguments": {
    "query": "JWT token validation",
    "codebase": "/my/project",
    "limit": 5
  }
}
```

### Understanding API Structure
```json
// Find API endpoints
{
  "name": "codebase_search",
  "arguments": {
    "query": "REST API endpoint definitions",
    "codebase": "/my/project",
    "limit": 20
  }
}

// Find request handlers
{
  "name": "codebase_search",
  "arguments": {
    "query": "HTTP request handler",
    "codebase": "/my/project",
    "limit": 10
  }
}
```

### Debugging Error Handling
```json
// Find error handling code
{
  "name": "codebase_search",
  "arguments": {
    "query": "error handling for database failures",
    "codebase": "/my/project",
    "limit": 15
  }
}
```

## Troubleshooting

### Codebase Not Found
If search returns "Codebase not indexed":
```json
{
  "name": "codebase_index",
  "arguments": {
    "path": "/path/to/project"
  }
}
```

### No Results Found
If search returns no results:
1. Try broader queries
2. Use different terminology
3. Check if the code exists in indexed files
4. Verify the codebase path is correct

### Slow Performance
For large codebases:
- Results are typically returned in <100ms
- Indexing may take several minutes for 10k+ files
- Use `verbose: true` to monitor progress

## Technical Details

### Embedding Models
- **MiniLM** (default): Fast, 384 dimensions, good quality
- **Nomic**: Higher quality, 768 dimensions, better for complex queries
- **Nemotron**: Advanced model for specialized use cases

### Chunking Strategy
- Syntax-aware splitting at function/class boundaries
- Token budget management (small/medium/large)
- Overlap for context preservation
- Metadata extraction (imports, signatures, docs)

### Data Storage
- Location: `~/.local/share/code-search/` (Linux)
- Database: SQLite with vector extension
- Manifests: SHA256 hashes for incremental updates

## Tips for AI Assistants

1. **Always check status first** - Don't assume codebases are indexed
2. **Use descriptive queries** - Natural language works best
3. **Explain results to users** - Help them understand what was found
4. **Suggest refinements** - If results aren't perfect, try different queries
5. **Note file locations** - Provide file paths and line numbers to users
6. **Consider context** - Look at surrounding code when analyzing results

## Integration Example

```javascript
// MCP client usage
const result = await mcp.callTool("codebase_search", {
  query: "database connection pool implementation",
  codebase: "/path/to/project",
  limit: 10
});

// Process results
result.results.forEach(r => {
  console.log(`${r.file}:${r.lines} (score: ${r.score})`);
  console.log(r.content);
});
```

## Resources

- **GitHub**: [Repository URL]
- **Documentation**: See README.md
- **Issues**: [Issue Tracker URL]

---

**Note**: This skill requires the code-search MCP server to be running. Ensure it's configured in your MCP client settings.
