//! MCP Server Implementation
//!
//! This module implements the Model Context Protocol server for code search,
//! providing semantic search capabilities to Claude Desktop, Zed, and other MCP clients.

use crate::config::get_config;
use crate::database::{
    delete_chunks_for_codebase, get_codebase_stats, get_global_stats, hybrid_search, init_db,
    SearchFilters,
};
use crate::embedding::{ensure_model_available_with_model, get_query_embedding_with_model};
use crate::error::{CodeSearchError, Result};
use crate::indexing::{list_indexed_codebases, CodebaseInfo, Indexer, IndexingOptions};
use crate::manifest::get_codebase_hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

// ============================================================================
// MCP Protocol Types
// ============================================================================

/// JSON-RPC request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// JSON-RPC response message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcResponse {
    Success(JsonRpcSuccessResponse),
    Error(JsonRpcErrorResponse),
}

/// Successful JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcSuccessResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(default)]
    pub result: serde_json::Value,
}

/// Error JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub error: JsonRpcError,
}

/// JSON-RPC error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

// ============================================================================
// MCP Protocol Structures
// ============================================================================

/// MCP Server capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerCapabilities {
    #[serde(default)]
    pub tools: Option<ToolsCapability>,
    #[serde(default)]
    pub resources: Option<ResourcesCapability>,
    #[serde(default)]
    pub prompts: Option<PromptsCapability>,
    #[serde(rename = "streaming", default)]
    pub streaming: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(default)]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    #[serde(default)]
    pub subscribe: Option<bool>,
    #[serde(default)]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    #[serde(default)]
    pub list_changed: Option<bool>,
}

/// Initialize result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// Tool list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListResult {
    pub tools: Vec<Tool>,
}

/// Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// Resource list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceListResult {
    pub resources: Vec<Resource>,
}

/// Resource content result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContentResult {
    pub contents: Vec<ResourceContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
}

// ============================================================================
// MCP Server Implementation
// ============================================================================

/// MCP Server state
pub struct McpServer {
    capabilities: ServerCapabilities,
    codebases: HashMap<String, CodebaseInfo>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                resources: Some(ResourcesCapability {
                    subscribe: Some(false),
                    list_changed: Some(false),
                }),
                prompts: Some(PromptsCapability {
                    list_changed: Some(false),
                }),
                streaming: Some(true),
            },
            codebases: HashMap::new(),
        }
    }

    /// Handle an incoming JSON-RPC request
    pub fn handle_request(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(&request.id),
            "tools/list" => self.handle_tools_list(&request.id),
            "tools/call" => self.handle_tools_call(&request.id, &request.params),
            "resources/list" => self.handle_resources_list(&request.id),
            "resources/read" => self.handle_resources_read(&request.id, &request.params),
            "ping" => self.handle_ping(&request.id),
            _ => JsonRpcResponse::Error(JsonRpcErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                error: JsonRpcError {
                    code: -32601, // Method not found
                    message: format!("Unknown method: {}", request.method),
                    data: serde_json::Value::Null,
                },
            }),
        }
    }

    fn handle_initialize(&mut self, id: &serde_json::Value) -> JsonRpcResponse {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: self.capabilities.clone(),
            server_info: ServerInfo {
                name: "code-search".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        JsonRpcResponse::Success(JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            result: serde_json::to_value(result).unwrap_or(serde_json::Value::Null),
        })
    }

    fn handle_tools_list(&self, id: &serde_json::Value) -> JsonRpcResponse {
        let tools = vec![
            Tool {
                name: "codebase_index".to_string(),
                description: "Index a codebase for semantic search. Scans all supported files, generates embeddings, and stores them for searching.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the codebase to index"
                        },
                        "force": {
                            "type": "boolean",
                            "description": "Force re-indexing of all files",
                            "default": false
                        },
                        "verbose": {
                            "type": "boolean",
                            "description": "Enable verbose output",
                            "default": false
                        },
                        "model": {
                            "type": "string",
                            "description": "Embedding model to use (minilm, nomic, nemotron)",
                            "default": "minilm"
                        }
                    },
                    "required": ["path"]
                }),
            },
            Tool {
                name: "codebase_search".to_string(),
                description: "Search indexed code using semantic similarity and full-text search. Returns code chunks that match the query.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language search query"
                        },
                        "codebase": {
                            "type": "string",
                            "description": "Path to the indexed codebase"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum number of results",
                            "default": 10
                        }
                    },
                    "required": ["query", "codebase"]
                }),
            },
            Tool {
                name: "codebase_status".to_string(),
                description: "List all indexed codebases and their statistics.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "list": {
                            "type": "boolean",
                            "description": "List all indexed codebases",
                            "default": true
                        }
                    }
                }),
            },
            Tool {
                name: "codebase_delete".to_string(),
                description: "Remove a codebase from the index. Deletes all chunks and metadata associated with the codebase.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the codebase to delete"
                        }
                    },
                    "required": ["path"]
                }),
            },
        ];

        JsonRpcResponse::Success(JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            result: serde_json::to_value(ToolListResult { tools })
                .unwrap_or(serde_json::Value::Null),
        })
    }

    fn handle_tools_call(
        &mut self,
        id: &serde_json::Value,
        params: &serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse tool name and arguments
        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => {
                return JsonRpcResponse::Error(JsonRpcErrorResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.clone(),
                    error: JsonRpcError {
                        code: -32602, // Invalid params
                        message: "Missing tool name".to_string(),
                        data: serde_json::Value::Null,
                    },
                });
            }
        };

        let args = params.get("arguments").and_then(|v| v.as_object());

        let result = match tool_name {
            "codebase_index" => self.tool_codebase_index(args),
            "codebase_search" => self.tool_codebase_search(args),
            "codebase_status" => self.tool_codebase_status(args),
            "codebase_delete" => self.tool_codebase_delete(args),
            _ => {
                return JsonRpcResponse::Error(JsonRpcErrorResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.clone(),
                    error: JsonRpcError {
                        code: -32601,
                        message: format!("Unknown tool: {}", tool_name),
                        data: serde_json::Value::Null,
                    },
                });
            }
        };

        match result {
            Ok(value) => JsonRpcResponse::Success(JsonRpcSuccessResponse {
                jsonrpc: "2.0".to_string(),
                id: id.clone(),
                result: value,
            }),
            Err(e) => JsonRpcResponse::Error(JsonRpcErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: id.clone(),
                error: JsonRpcError {
                    code: -32000,
                    message: e.to_string(),
                    data: serde_json::Value::Null,
                },
            }),
        }
    }

    fn tool_codebase_index(
        &mut self,
        args: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        let args = args.ok_or_else(|| CodeSearchError::Other("Missing arguments".to_string()))?;

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeSearchError::Other("Missing path argument".to_string()))?;

        let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
        let verbose = args
            .get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let model = args
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("minilm");

        let path = Path::new(path);
        if !path.exists() {
            return Err(CodeSearchError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Codebase path does not exist: {}", path.display()),
            )));
        }

        let config = get_config();
        let model = if model == "minilm" {
            config.model.model_type.as_str()
        } else {
            model
        };

        // Ensure model is available
        if let Err(e) = ensure_model_available_with_model(model) {
            eprintln!("Warning: Could not load embedding model: {}", e);
        }

        let indexing_options = IndexingOptions {
            force,
            verbose,
            use_gitignore: true,
            model_name: Some(model.to_string()),
            ..Default::default()
        };

        let mut indexer = Indexer::new(indexing_options);
        let stats = indexer.index_codebase(path)?;

        // Update cached codebases
        self.refresh_codebase_list();

        Ok(serde_json::json!({
            "success": true,
            "message": stats.to_string(),
            "stats": {
                "files_indexed": stats.files_indexed,
                "chunks_created": stats.chunks_created,
                "duration_ms": stats.duration_ms
            }
        }))
    }

    fn tool_codebase_search(
        &self,
        args: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        let args = args.ok_or_else(|| CodeSearchError::Other("Missing arguments".to_string()))?;

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeSearchError::Other("Missing query argument".to_string()))?;

        let codebase_path = args
            .get("codebase")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeSearchError::Other("Missing codebase argument".to_string()))?;

        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as i64;

        if query.trim().is_empty() {
            return Ok(serde_json::json!({ "results": [] }));
        }

        let path = Path::new(codebase_path);
        if !path.exists() {
            return Err(CodeSearchError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Codebase path does not exist: {}", codebase_path),
            )));
        }

        let config = get_config();
        let model = config.model.model_type.as_str();

        let canonical_path = path.canonicalize().map_err(CodeSearchError::Io)?;
        let codebase_id = get_codebase_hash(&canonical_path);

        let conn = init_db()?;

        // Check if codebase is indexed
        let stats = get_codebase_stats(&conn, &codebase_id)?;
        if stats.is_none() {
            return Err(CodeSearchError::CodebaseNotIndexed(
                codebase_path.to_string(),
            ));
        }

        // Ensure model is available
        ensure_model_available_with_model(model).map_err(|e| {
            CodeSearchError::EmbeddingModelLoad(format!(
                "Failed to load embedding model '{}': {}",
                model, e
            ))
        })?;

        let query_embedding = get_query_embedding_with_model(query, model);

        let filters = SearchFilters::default();
        let db_results = hybrid_search(
            &conn,
            query,
            Some(&codebase_id),
            &query_embedding,
            limit,
            &filters,
            false,
        )?;

        let results: Vec<serde_json::Value> = db_results
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "file": r.file_path,
                    "lines": format!("{}-{}", r.start_line, r.end_line),
                    "content": r.content,
                    "score": r.score,
                    "language": r.language,
                    "rank": r.rank
                })
            })
            .collect();

        Ok(serde_json::json!({ "results": results }))
    }

    fn tool_codebase_status(
        &self,
        args: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        let _args = args;

        let codebases = list_indexed_codebases()?;

        let conn = init_db()?;
        let global_stats = get_global_stats(&conn)?;

        let codebase_list: Vec<serde_json::Value> = codebases
            .into_iter()
            .map(|cb| {
                serde_json::json!({
                    "codebase_id": cb.codebase_id,
                    "chunk_count": cb.chunk_count,
                    "file_count": cb.file_count
                })
            })
            .collect();

        Ok(serde_json::json!({
            "codebases": codebase_list,
            "global_stats": global_stats
        }))
    }

    fn tool_codebase_delete(
        &mut self,
        args: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        let args = args.ok_or_else(|| CodeSearchError::Other("Missing arguments".to_string()))?;

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CodeSearchError::Other("Missing path argument".to_string()))?;

        let path = Path::new(path);
        if !path.exists() {
            return Err(CodeSearchError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Codebase path does not exist: {}", path.display()),
            )));
        }

        let canonical_path = path.canonicalize().map_err(CodeSearchError::Io)?;
        let codebase_id = get_codebase_hash(&canonical_path);

        let conn = init_db()?;

        let stats = get_codebase_stats(&conn, &codebase_id)?;
        if stats.is_none() {
            return Ok(serde_json::json!({
                "success": false,
                "message": format!("Codebase '{}' is not indexed.", path.display())
            }));
        }

        let deleted_count = delete_chunks_for_codebase(&conn, &codebase_id)?;
        crate::manifest::delete_manifest(&codebase_id)?;

        // Update cached codebases
        self.refresh_codebase_list();

        Ok(serde_json::json!({
            "success": true,
            "message": format!("Deleted codebase '{}' ({} chunks removed)", path.display(), deleted_count),
            "deleted_chunks": deleted_count
        }))
    }

    fn handle_resources_list(&self, id: &serde_json::Value) -> JsonRpcResponse {
        let resources = self.build_resource_list();

        JsonRpcResponse::Success(JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            result: serde_json::to_value(ResourceListResult { resources })
                .unwrap_or(serde_json::Value::Null),
        })
    }

    fn handle_resources_read(
        &self,
        id: &serde_json::Value,
        params: &serde_json::Value,
    ) -> JsonRpcResponse {
        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(uri) => uri,
            None => {
                return JsonRpcResponse::Error(JsonRpcErrorResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.clone(),
                    error: JsonRpcError {
                        code: -32602,
                        message: "Missing uri parameter".to_string(),
                        data: serde_json::Value::Null,
                    },
                });
            }
        };

        let content = match self.read_resource(uri) {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::Error(JsonRpcErrorResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.clone(),
                    error: JsonRpcError {
                        code: -32001,
                        message: e.to_string(),
                        data: serde_json::Value::Null,
                    },
                });
            }
        };

        JsonRpcResponse::Success(JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            result: content,
        })
    }

    fn handle_ping(&self, id: &serde_json::Value) -> JsonRpcResponse {
        JsonRpcResponse::Success(JsonRpcSuccessResponse {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            result: serde_json::json!({ "status": "ok" }),
        })
    }

    fn build_resource_list(&self) -> Vec<Resource> {
        let mut resources = Vec::new();

        // Add summary resource for each indexed codebase
        for (name, info) in &self.codebases {
            let uri = format!("codebase://{}/summary", name);
            resources.push(Resource {
                uri,
                name: format!("{} Summary", name),
                description: Some(format!(
                    "Statistics for indexed codebase: {} files, {} chunks",
                    info.file_count, info.chunk_count
                )),
                mime_type: Some("application/json".to_string()),
            });
        }

        resources
    }

    fn read_resource(&self, uri: &str) -> Result<serde_json::Value> {
        // Parse URI: codebase://{name}/summary or codebase://{name}/file/{path}
        if let Some(rest) = uri.strip_prefix("codebase://") {
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            if parts.is_empty() {
                return Err(CodeSearchError::Other("Invalid resource URI".to_string()));
            }

            let name = parts[0];

            if parts.len() == 1 || parts[1] == "summary" {
                // Return summary
                if let Some(info) = self.codebases.get(name) {
                    return Ok(serde_json::json!({
                        "codebase_id": info.codebase_id,
                        "file_count": info.file_count,
                        "chunk_count": info.chunk_count
                    }));
                } else {
                    return Err(CodeSearchError::Other(format!(
                        "Codebase not found: {}",
                        name
                    )));
                }
            }
        }

        Err(CodeSearchError::Other(format!("Unknown resource: {}", uri)))
    }

    fn refresh_codebase_list(&mut self) {
        if let Ok(codebases) = list_indexed_codebases() {
            self.codebases = codebases
                .into_iter()
                .map(|cb| (cb.codebase_id.clone(), cb))
                .collect();
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Stdio Transport
// ============================================================================

// Global flag for server loop
static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Run the MCP server using stdio transport
pub fn run_mcp_server() {
    // Set up signal handler for graceful shutdown
    #[cfg(unix)]
    {
        use std::sync::Once;
        static SET_SIGNAL: Once = Once::new();
        SET_SIGNAL.call_once(|| unsafe {
            signal_hook::low_level::register(signal_hook::consts::SIGINT, || {
                RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
            })
            .ok();
        });
    }

    let mut server = McpServer::new();
    server.refresh_codebase_list();

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut reader = BufReader::new(stdin);
    let mut writer = stdout.lock();

    while RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
        // Read a line from stdin
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                eprintln!("Error reading from stdin: {}", e);
                break;
            }
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(e) => {
                let error_response = JsonRpcResponse::Error(JsonRpcErrorResponse {
                    jsonrpc: "2.0".to_string(),
                    id: serde_json::Value::Null,
                    error: JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: serde_json::Value::Null,
                    },
                });
                let _ = writeln!(
                    writer,
                    "{}",
                    serde_json::to_string(&error_response).unwrap_or_default()
                );
                let _ = writer.flush();
                continue;
            }
        };

        // Handle request and send response
        let response = server.handle_request(&request);
        let response_str = serde_json::to_string(&response).unwrap_or_default();
        let _ = writeln!(writer, "{}", response_str);
        let _ = writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_initialization() {
        let server = McpServer::new();
        assert!(server.capabilities.tools.is_some());
        assert!(server.capabilities.resources.is_some());
        assert!(server.capabilities.streaming.is_some());
    }

    #[test]
    fn test_tools_list() {
        let server = McpServer::new();
        let id = serde_json::Value::Number(serde_json::Number::from(1));
        let response = server.handle_tools_list(&id);

        match response {
            JsonRpcResponse::Success(resp) => {
                assert!(resp.result.get("tools").is_some());
            }
            _ => panic!("Expected success response"),
        }
    }
}
