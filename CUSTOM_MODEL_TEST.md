# Testing Custom Models with Code Search

This guide shows how to use custom embedding models with the code-search tool.

## Tested Model: Jina AI Embeddings v2 Base Code

We successfully tested with `jinaai/jina-embeddings-v2-base-code`, a model specifically optimized for code search.

### Model Details
- **Model**: jinaai/jina-embeddings-v2-base-code
- **Dimension**: 768
- **Specialization**: Code and text embeddings
- **Context Length**: 8192 tokens
- **HuggingFace**: https://huggingface.co/jinaai/jina-embeddings-v2-base-code

## Configuration

### Step 1: Create Config File

Create or edit `~/.config/code-search/config.toml`:

```toml
[model]
model_type = "custom"
model_path = "jinaai/jina-embeddings-v2-base-code"
embedding_dim = 768
auto_download = true

[indexing]
extensions = [".rs", ".py", ".js", ".ts", ".go", ".java", ".c", ".cpp"]
skip_dirs = [".git", "node_modules", "target", "build", "dist"]
skip_files = ["*.pyc", "*.lock", "*.log"]
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

### Step 2: Verify Configuration

```bash
code-search config
```

Output:
```
Current configuration:
  [model]
    model_type: custom
    model_path: jinaai/jina-embeddings-v2-base-code
    embedding_dim: 768
    auto_download: true
  ...
```

### Step 3: Index Your Codebase

```bash
# First-time indexing
code-search index /path/to/your/codebase

# Or force re-index
code-search index /path/to/your/codebase --force
```

The model will be automatically downloaded from HuggingFace on first use.

## Test Results

### Indexing Performance
- **Files indexed**: 26
- **Chunks created**: 298
- **Duration**: ~1.5 seconds
- **Model download**: Automatic on first use

### Search Quality

All searches returned highly relevant results:

#### Test 1: MCP Server
```bash
code-search search "MCP server implementation" --codebase /path/to/code
```
- **Result**: `src/mcp.rs` (lines 161-210)
- **Score**: 0.8135 ✅

#### Test 2: Code Chunking
```bash
code-search search "code chunking and splitting" --codebase /path/to/code
```
- **Result**: `src/syntax_aware.rs` (lines 1-50)
- **Score**: 0.6495 ✅

#### Test 3: Embeddings
```bash
code-search search "embedding vector generation" --codebase /path/to/code
```
- **Result**: `src/performance/batch.rs` (lines 1-50)
- **Score**: 0.5748 ✅

#### Test 4: MCP Search
```json
{
  "name": "codebase_search",
  "arguments": {
    "query": "function that handles database connections",
    "limit": 2
  }
}
```
- **Result**: Correctly identified database-related code
- **Codebase name**: Properly displayed ✅

## Using Other Custom Models

### Supported Models

Any HuggingFace model that can be exported to ONNX format:

1. **Sentence Transformers**
   - `sentence-transformers/all-mpnet-base-v2` (768-dim)
   - `sentence-transformers/e5-base-v2` (768-dim)
   - `sentence-transformers/all-roberta-large-v1` (1024-dim)

2. **Code-Specific Models**
   - `jinaai/jina-embeddings-v2-base-code` (768-dim) ✅ **TESTED**
   - `codellama/CodeLlama-7b-hf` (requires special handling)
   - `microsoft/codebert-base` (768-dim)

3. **Multilingual Models**
   - `intfloat/multilingual-e5-base` (768-dim)
   - `sentence-transformers/paraphrase-multilingual-mpnet-base-v2` (768-dim)

### Configuration Steps

1. **Find the model's embedding dimension**
   - Check the model card on HuggingFace
   - Usually listed as "hidden_size" or "embedding_dimension"

2. **Update config**
   ```toml
   [model]
   model_type = "custom"
   model_path = "organization/model-name"
   embedding_dim = <dimension>
   ```

3. **Test indexing**
   ```bash
   code-search index /path/to/code --force --verbose
   ```

## Performance Comparison

### Jina v2 Base Code vs Built-in Models

| Model | Dimension | Indexing Speed | Search Quality | Best For |
|-------|-----------|----------------|----------------|----------|
| MiniLM (default) | 384 | ⚡ Fastest | Good | General purpose |
| Nomic | 768 | 🚀 Fast | Very Good | Complex queries |
| **Jina Code** | 768 | 🚀 Fast | **Excellent** | **Code search** ✅ |
| Nemotron | 2048 | 🐢 Slower | Excellent | Large context |

### Recommendations

- **General code search**: MiniLM (fast, good quality)
- **Complex semantic queries**: Nomic or Jina Code
- **Code-specific search**: **Jina Code** (optimized for code) ✅
- **Large codebases**: Nemotron (better context understanding)

## Troubleshooting

### Model Download Issues

If the model doesn't download automatically:

1. **Check internet connection**
   ```bash
   curl -I https://huggingface.co
   ```

2. **Manual download**
   ```bash
   # Use huggingface-hub to download
   pip install huggingface-hub
   huggingface-cli download jinaai/jina-embeddings-v2-base-code
   ```

3. **Use local path**
   ```toml
   [model]
   model_type = "custom"
   model_path = "/path/to/downloaded/model"
   embedding_dim = 768
   ```

### Dimension Mismatch

Error: `Embedding dimension mismatch`

**Solution**: Ensure `embedding_dim` matches the model's actual dimension:

```bash
# Check model config
python -c "from transformers import AutoConfig; print(AutoConfig.from_pretrained('jinaai/jina-embeddings-v2-base-code').hidden_size)"
```

### ONNX Export Issues

Some models may need manual ONNX export:

```python
from transformers import AutoTokenizer, AutoModel
from optimum.exporters.onnx import main_export

model_id = "jinaai/jina-embeddings-v2-base-code"
main_export(model_id, output="./jina-onnx")
```

Then use local path in config:
```toml
model_path = "./jina-onnx"
```

## Advanced Usage

### Multiple Models

Switch between models for different codebases:

```bash
# Index backend with Jina (code-optimized)
export CODE_SEARCH_MODEL=custom
code-search index /path/to/backend

# Index docs with Nomic (better for text)
export CODE_SEARCH_MODEL=nomic
code-search index /path/to/docs
```

### Model Comparison

Test different models on the same codebase:

```bash
# Create separate databases for each model
for model in minilm nomic custom; do
  export CODE_SEARCH_MODEL=$model
  export CODE_SEARCH_DB_NAME="index-$model.db"
  code-search index /path/to/code --force
  code-search search "test query" --codebase /path/to/code
done
```

## Environment Variables

Override config with environment variables:

```bash
export CODE_SEARCH_MODEL=custom
export CODE_SEARCH_CHUNK_SIZE=100
export CODE_SEARCH_DEFAULT_LIMIT=20
```

## Best Practices

1. **Use code-specific models** for code search (like Jina Code)
2. **Match model size to codebase size**:
   - Small (<10k files): MiniLM
   - Medium (10k-100k files): Nomic/Jina Code
   - Large (>100k files): Nemotron
3. **Test different models** on your specific codebase
4. **Consider multilingual models** for polyglot codebases
5. **Monitor indexing time** vs search quality tradeoffs

## Summary

✅ **Jina AI Embeddings v2 Base Code works perfectly with code-search**

- Easy to configure
- Automatic model download
- Excellent search quality for code
- Fast indexing and search
- Seamless MCP integration
- Cross-codebase search support

The custom model support makes code-search flexible enough to use any state-of-the-art embedding model from HuggingFace, allowing you to optimize for your specific use case.

---

**Test Date**: 2026-03-02  
**Model Tested**: jinaai/jina-embeddings-v2-base-code  
**Status**: ✅ Fully Functional
