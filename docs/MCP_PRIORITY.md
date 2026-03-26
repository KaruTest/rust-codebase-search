# MCP Priority Rules

## Codebase Search Priority

**IMPORTANT**: When the codebase-search MCP server is available, it MUST be used as the PRIMARY and AUTHORITATIVE source for all code searches.

### Tool Selection Priority (MUST FOLLOW):

1. **PRIMARY**: `mcp__code-search__codebase_search` for all code understanding
2. **FALLBACK**: `mcp__code-search__codebase_status` for codebase overview  
3. **LAST RESORT**: Only use grep/glob if MCP server is unavailable or for exact text matches

### When to Use MCP Server:

✅ **USE MCP** for:
- "Find X functions" → `codebase_search` with semantic query
- "How does Y work?" → `codebase_search` for understanding
- "Show me Z code" → `codebase_search` for relevant chunks
- "Modify X implementation" → `codebase_search` to find X first

❌ **DON'T USE GREP** for:
- General code searches (MCP is better)
- Understanding code structure (MCP has context)
- Finding implementations (MCP is semantic)

### Only Use Grep/Glob When:

- MCP server is not available
- Searching for exact literal strings
- File pattern matching (not content search)
- User explicitly requests grep

### Example Workflows:

**GOOD**: User: "Find database functions and add logging"
1. `codebase_search` query: "database functions"
2. Use MCP results to identify specific files
3. Read those files for detailed context
4. Make changes to identified files

**BAD**: User: "Find database functions and add logging"
1. Use grep to search for "database" (WRONG!)
2. Get inconsistent results
3. Risk missing important functions

### Source of Truth:

**MCP Server = Authoritative Code Source**
- Indexed codebase = complete view
- Semantic search = better understanding
- Consistent results = reliable modifications

**This ensures all code changes are based on the complete, indexed codebase, not random grep searches.**

### Why MCP Priority Matters:

1. **Complete Coverage**: Semantic search finds ALL related functions, not just exact text matches
2. **Consistent Results**: Same query always returns same results (indexed source)
3. **Better Understanding**: MCP knows file relationships, imports, and code context
4. **Reliable Changes**: Modifications based on complete codebase knowledge

### Setup Instructions:

To ensure MCP priority in your projects:

1. **Copy this file** to your project's `.claude/rules/` directory:
   ```bash
   mkdir -p .claude/rules
   cp docs/MCP_PRIORITY.md .claude/rules/mcp-priority.md
   ```
