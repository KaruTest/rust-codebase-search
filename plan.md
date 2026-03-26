# Code Search Enhancement Plan

## Vision
Transform this code search tool from a solid semantic search into a **next-generation LLM-native code intelligence platform** that excels at both MCP-driven AI interactions and human CLI usage.

---

## Phase 1: MCP Server Implementation (HIGH PRIORITY)

### 1.1 Full MCP Protocol Implementation
The MCP server is documented but not implemented. Implement the complete MCP protocol:

- **JSON-RPC message handling** - Full request/response cycle
- **Tool definitions** - `codebase_index`, `codebase_search`, `codebase_status`, `codebase_delete`
- **Server capabilities** - Announce streaming, resources, prompts
- **Stdio transport** - For Claude Desktop/Zed integration

### 1.2 Streaming Responses
Implement MCP streaming for real-time results:
- Stream search results as they're found
- Show progress during indexing
- Enable LLMs to process partial results

### 1.3 Server-Side Resources
Expose indexed codebases as MCP resources:
- `/codebase://{name}/file/{path}` - Access specific files
- `/codebase://{name}/summary` - Codebase statistics
- Auto-complete for resource URIs

---

## Phase 2: LLM-Centric Features

### 2.1 Syntax-Aware Chunking
Instead of line-count chunking, split at semantic boundaries:
- **Function boundaries** - Split at function/class definitions
- **Import boundaries** - Keep imports with their usage
- **AST-aware metadata** - Add function signatures, types, dependencies
- **Language-specific parsers** - Use tree-sitter for accurate parsing

### 2.2 Context-Enriched Chunks
Add rich metadata to each chunk:
- Function/class name and signature
- Imported dependencies
- File path and language
- surrounding context (imports, parent functions)
- Git blame for authorship

### 2.3 Query Expansion
Use lightweight LLM or heuristics to expand queries:
- Expand "auth" → "authentication login oauth jwt token"
- Add synonyms based on language (e.g., "fn" → "function")
- Handle common typos automatically

### 2.4 Token Budget Management
Optimize for LLM context windows:
- Configurable chunk sizes (small/medium/large)
- Prioritize most relevant chunks
- Allow "expand to context" tool for more info
- Summarize large files instead of returning full content

### 2.5 Multi-Step Query Support
Enable complex reasoning workflows:
- Chain searches: "find auth logic → find where it's called → find tests"
- Store intermediate results in session
- Support "search within results" refinement

---

## Phase 3: Search Quality Enhancements

### 3.1 BM25 Ranking for FTS
Replace simple FTS scoring with BM25:
- Term frequency saturation
- Document length normalization
- Better relevance than current approach

### 3.2 Learning-to-Rank
Add click-through relevance feedback:
- Track which results users click
- Use implicit feedback to improve ranking
- Simple logistic regression or gradient boosting

### 3.3 Improved Hybrid Scoring
Enhanced RRF with:
- Per-language weight tuning
- Query-dependent weight adjustment
- Coverage scoring (does result cover the query intent)

### 3.4 Filtering System
Add rich filters:
- `--language rust,python` - Programming language
- `--after 2024-01-01` - Date filtering
- `--author username` - Git blame author
- `--imports tokio` - Files using specific imports
- `--file-type test` - Test files vs implementation

### 3.5 Fuzzy Matching
Add tolerance for typos:
- Edit distance for exact searches
- Phonetic matching for function names
- Acronym expansion (UTF → "Unicode Transformation Format")

---

## Phase 4: User Experience

### 4.1 Interactive CLI Search
Enhanced TUI for manual searching:
- fzf-style fuzzy finder with preview
- Arrow keys to navigate results
- Real-time search as you type
- Syntax-highlighted code preview

### 4.2 Search History & Favorites
- `code-search history` - View past searches
- `code-search favorite <query>` - Save useful queries
- `code-search recent` - Recent codebases
- Shell completions for common queries

### 4.3 Rich Output Formats
Multiple output modes:
- `--format json` - Machine-readable
- `--format pretty` - Human-friendly with colors
- `--format markdown` - For documentation
- `--format stream` - JSON Lines for piping

### 4.4 Result Visualization
- Syntax highlighting with colors
- Diff-style context (+/- lines)
- File tree view for directory results
- Graphviz output for code relationships

### 4.5 Autocomplete
- Shell completions (bash, zsh, fish)
- Interactive query suggestions
- Language-specific query templates

---

## Phase 5: Performance & Scale

### 5.1 HNSW Vector Indexing
Replace brute-force vector search:
- 10-100x faster queries
- Configurable recall/speed tradeoff
- Use `sqlite-vec` extension or manual implementation

### 5.2 Query Caching
- Cache embeddings for common queries
- LRU cache with configurable size
- Invalidate on index updates

### 5.3 Batch Embedding Optimization
- Batch multiple chunks for embedding
- GPU acceleration if available
- Progress reporting for large indexing jobs

### 5.4 Distributed Support
- Multiple index shards
- Query routing across machines
- Eventually consistent replication

---

## Phase 6: Advanced Features

### 6.1 Code Relationship Graph
Build and query code relationships:
- Import dependency graph
- Function call graph
- Type hierarchy
- Expose via MCP as traversable resources

### 6.2 Semantic Code Actions
Go beyond search to actions:
- "Find all places that need updating for this API change"
- "Generate tests for this function"
- "Extract this function to a separate file"
- Combine search with code transformation

### 6.3 Code Change Prediction
Use embeddings to predict:
- Which files will be edited together
- Potential merge conflicts
- Affected tests for a code change

### 6.4 Multi-Codebase Search
Search across multiple codebases:
- Cross-repository search
- Library vs application code separation
- Unified results with source attribution

### 6.5 Local LLM Integration
Optional local inference:
- Query expansion with small local model
- Chunk summarization
- Result reranking

---

## Priority Recommendations

### Must Have (MVP+)
1. ✅ Implement full MCP server
2. ✅ Add streaming responses
3. ✅ Syntax-aware chunking
4. ✅ BM25 ranking
5. ✅ Basic filtering (language, file type)

### Should Have
6. Interactive CLI (fzf-style)
7. Query expansion
8. Search history
9. Rich output formats
10. HNSW indexing

### Crazy / Next Level
11. Code relationship graph
12. Semantic code actions
13. Multi-codebase search
14. Local LLM integration
15. Code change prediction

---

## Implementation Order

1. **Week 1-2**: MCP server implementation + streaming
2. **Week 3**: Syntax-aware chunking + context enrichment
3. **Week 4**: BM25 + improved hybrid scoring
4. **Week 5**: Filtering + basic CLI enhancements
5. **Week 6**: Interactive CLI + search history
6. **Week 7-8**: HNSW + performance optimization
7. **Week 9+**: Advanced features (graph, actions, etc.)

---

*This plan positions the tool as a first-class LLM code intelligence platform while maintaining excellent manual search capabilities.*
