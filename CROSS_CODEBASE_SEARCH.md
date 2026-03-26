# Cross-Codebase Search Implementation

## Problem

The original implementation had two issues:

1. **Confusing codebase identifiers**: The `status` command showed random hash strings (like `194bce0702f40e63`) which made it difficult for AIs to know which codebase to search.

2. **No cross-codebase search**: AIs couldn't search across multiple indexed codebases, which is essential for scenarios like:
   - Frontend searching backend API code
   - Multiple microservices
   - Shared libraries

## Solution

### 1. Human-Readable Codebase Metadata

Added a new `codebases` table that stores:
- **name**: Human-readable codebase name (directory name)
- **path**: Full canonical path to the codebase
- **indexed_at**: Timestamp when first indexed
- **last_updated**: Timestamp of last update
- **model**: Embedding model used
- **tags**: Optional tags for categorization

### 2. Enhanced Status Command

The `codebase_status` command now returns:

```json
{
  "id": "194bce0702f40e63",
  "name": "rust-codebase-search",
  "path": "/home/karutoil/rust-codebase-search",
  "chunk_count": 354,
  "file_count": 36,
  "model": "nomic",
  "tags": null
}
```

**CLI Output:**
```
Indexed codebases:

  rust-codebase-search (/home/karutoil/rust-codebase-search)
    ID: 194bce0702f40e63
    Files: 36, Chunks: 354
    Model: nomic
```

### 3. Cross-Codebase Search

The `codebase_search` tool now supports:

**Search all codebases:**
```json
{
  "query": "authentication function"
}
```

**Search specific codebase by name:**
```json
{
  "query": "authentication function",
  "codebase": "backend-api"
}
```

**Search specific codebase by path:**
```json
{
  "query": "authentication function",
  "codebase": "/path/to/backend"
}
```

**Search specific codebase by ID:**
```json
{
  "query": "authentication function",
  "codebase": "194bce0702f40e63"
}
```

### 4. Enhanced Search Results

Search results now include codebase identification:

```json
{
  "results": [
    {
      "file": "src/auth/login.rs",
      "lines": "45-78",
      "content": "pub fn authenticate_user(...) { ... }",
      "score": 0.89,
      "language": "rust",
      "rank": 1,
      "codebase_id": "194bce0702f40e63",
      "codebase_name": "backend-api"
    }
  ],
  "query": "authentication function",
  "searched_all_codebases": false
}
```

## Usage Examples

### For AI Assistants

**1. Check what's indexed:**
```json
{
  "name": "codebase_status",
  "arguments": {}
}
```

Returns list with human-readable names and full paths.

**2. Search across all codebases:**
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "API endpoint for user authentication"
  }
}
```

Searches ALL indexed codebases simultaneously.

**3. Search specific codebase:**
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "database connection pooling",
    "codebase": "backend-api"
  }
}
```

Searches only the "backend-api" codebase.

**4. Index with tags:**
```json
{
  "name": "codebase_index",
  "arguments": {
    "path": "/path/to/backend",
    "tags": "backend,api,auth,microservice"
  }
}
```

Tags help categorize codebases (future feature).

### Frontend + Backend Scenario

**Index both:**
```bash
# Index backend
code-search index /path/to/backend --model nomic

# Index frontend
code-search index /path/to/frontend --model nomic
```

**Check status:**
```bash
code-search status --list
```

Output:
```
Indexed codebases:

  backend-api (/path/to/backend)
    ID: abc123...
    Files: 120, Chunks: 1500
    
  frontend-app (/path/to/frontend)
    ID: def456...
    Files: 85, Chunks: 900
```

**Frontend searches backend:**
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "API endpoint for user profile",
    "codebase": "backend-api"
  }
}
```

**Or search both:**
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "user authentication flow"
  }
}
```

## Benefits

1. **Clear Identification**: AIs can easily identify codebases by name
2. **Cross-Project Understanding**: Understand relationships between projects
3. **Flexible Searching**: Search one or all codebases
4. **Better Context**: Results include which codebase they came from
5. **Tagging Support**: Future categorization and filtering capabilities

## Technical Details

### Database Schema

```sql
CREATE TABLE codebases (
    codebase_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    indexed_at INTEGER NOT NULL,
    last_updated INTEGER,
    model TEXT,
    tags TEXT
);
```

### Backward Compatibility

- Old codebases will be automatically registered on next indexing
- Existing codebase_id hashes remain unchanged
- All existing search functionality preserved

### Future Enhancements

1. **Tag-based filtering**: `{"query": "...", "tags": ["backend", "api"]}`
2. **Codebase relationships**: Define dependencies between codebases
3. **Multi-codebase ranking**: Adjust results based on codebase relevance
4. **Codebase groups**: Organize codebases into logical groups

## Migration

For existing indexed codebases:

```bash
# Re-index to populate metadata (quick, only scans changes)
code-search index /path/to/codebase

# Or force re-index if needed
code-search index /path/to/codebase --force
```

The metadata will be automatically populated during indexing.

---

**Implementation Date:** 2026-03-02  
**Status:** ✅ Complete and Tested
