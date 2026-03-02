//! # Advanced Features Module
//!
//! This module provides advanced code search capabilities including:
//! - Code Relationship Graph (import dependencies, function calls, type hierarchy)
//! - Semantic Code Actions (API change impact analysis, test generation)
//! - Code Change Prediction (edit prediction, conflict detection)
//! - Multi-Codebase Search
//! - Local LLM Integration

use crate::database::{self, SearchFilters, SearchResult};
use crate::embedding::get_query_embedding_with_model;
use crate::error::Result;
use crate::manifest::get_codebase_hash;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

// ============================================================================
// Code Relationship Graph
// ============================================================================

/// Represents a node in the code relationship graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub name: String,
    pub file_path: String,
    pub line_number: Option<i64>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeType {
    Function,
    Class,
    Struct,
    Interface,
    Trait,
    Module,
    Import,
    Type,
    Variable,
}

/// Represents an edge in the code relationship graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EdgeType {
    Imports,
    Calls,
    Implements,
    Extends,
    Contains,
    References,
}

/// The complete code relationship graph for a codebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub codebase_id: String,
}

impl CodeGraph {
    /// Build a code graph from a codebase
    pub fn build(codebase_path: &str, model: &str) -> Result<Self> {
        let path = Path::new(codebase_path);
        let codebase_id = get_codebase_hash(&path.canonicalize()?);

        let conn = database::init_db()?;

        // Get all chunks for this codebase
        let mut stmt = conn.prepare(
            "SELECT file_path, content, language, start_line FROM chunks WHERE codebase_id = ?1",
        )?;

        let chunks: Vec<(String, String, Option<String>, i64)> = stmt
            .query_map(params![&codebase_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Process each chunk to extract graph elements
        for (file_path, content, language, start_line) in chunks {
            if let Some(lang) = language {
                let extracted = extract_graph_elements(&file_path, &content, &lang, start_line);
                nodes.extend(extracted.nodes);
                edges.extend(extracted.edges);
            }
        }

        // Build import edges by analyzing import statements
        let import_edges = build_import_edges(&nodes);
        edges.extend(import_edges);

        Ok(Self {
            nodes,
            edges,
            codebase_id,
        })
    }

    /// Get all nodes of a specific type
    pub fn get_nodes_by_type(&self, node_type: &NodeType) -> Vec<&GraphNode> {
        self.nodes.iter().filter(|n| &n.node_type == node_type).collect()
    }

    /// Get all edges for a specific node
    pub fn get_edges_for_node(&self, node_id: &str) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.from == node_id || e.to == node_id).collect()
    }

    /// Find the dependencies (imports) for a given file
    pub fn get_file_dependencies(&self, file_path: &str) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|n| n.file_path == file_path && n.node_type == NodeType::Import)
            .map(|n| n.name.clone())
            .collect()
    }

    /// Find files that depend on a given file
    pub fn get_dependents(&self, file_path: &str) -> Vec<String> {
        let imports = self.get_file_dependencies(file_path);
        self.nodes
            .iter()
            .filter(|n| {
                if let NodeType::Import = &n.node_type {
                    imports.contains(&n.name)
                } else {
                    false
                }
            })
            .map(|n| n.file_path.clone())
            .collect()
    }

    /// Traverse the graph to find all related nodes (BFS)
    pub fn traverse(&self, start_id: &str, max_depth: usize) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut queue: Vec<(String, usize)> = vec![(start_id.to_string(), 0)];
        let mut result = Vec::new();

        while let Some((node_id, depth)) = queue.pop() {
            if visited.contains(&node_id) || depth > max_depth {
                continue;
            }
            visited.insert(node_id.clone());
            result.push(node_id.clone());

            for edge in self.get_edges_for_node(&node_id) {
                if !visited.contains(&edge.to) {
                    queue.push((edge.to.clone(), depth + 1));
                }
            }
        }

        result
    }
}

struct ExtractedElements {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

fn extract_graph_elements(file_path: &str, content: &str, language: &str, start_line: i64) -> ExtractedElements {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    match language {
        "rust" => {
            extract_rust_elements(file_path, content, start_line, &mut nodes, &mut edges);
        }
        "javascript" | "typescript" | "js" | "ts" | "jsx" | "tsx" => {
            extract_js_ts_elements(file_path, content, start_line, &mut nodes, &mut edges);
        }
        "python" => {
            extract_python_elements(file_path, content, start_line, &mut nodes, &mut edges);
        }
        "go" => {
            extract_go_elements(file_path, content, start_line, &mut nodes, &mut edges);
        }
        "java" => {
            extract_java_elements(file_path, content, start_line, &mut nodes, &mut edges);
        }
        _ => {
            // Generic extraction for other languages
            extract_generic_elements(file_path, content, start_line, &mut nodes, &mut edges);
        }
    }

    ExtractedElements { nodes, edges }
}

fn extract_rust_elements(file_path: &str, content: &str, start_line: i64, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let file_id = format!("file:{}", file_path);

    // Find function definitions (fn)
    for (idx, line) in content.lines().enumerate() {
        let line_num = start_line + idx as i64;

        // Functions: fn name(
        if line.trim().starts_with("fn ") {
            if let Some(name) = extract_rust_name(line, "fn ") {
                let id = format!("fn:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Function,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Structs: struct Name
        else if line.trim().starts_with("struct ") {
            if let Some(name) = extract_rust_name(line, "struct ") {
                let id = format!("struct:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Struct,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Traits: trait Name
        else if line.trim().starts_with("trait ") {
            if let Some(name) = extract_rust_name(line, "trait ") {
                let id = format!("trait:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Trait,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Impl blocks: impl Name
        else if line.trim().starts_with("impl") {
            if let Some(name) = extract_impl_name(line) {
                let id = format!("impl:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Class,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
            }
        }
        // Use statements (imports)
        else if line.trim().starts_with("use ") {
            if let Some(module) = extract_use_statement(line) {
                let id = format!("import:{}:{}", file_path, module);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Import,
                    name: module,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Imports,
                });
            }
        }
    }
}

fn extract_rust_name(line: &str, prefix: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix(prefix) {
        let name = rest.split_whitespace().next()?.to_string();
        Some(name.trim_end_matches('{').trim_end_matches('(').to_string())
    } else {
        None
    }
}

fn extract_impl_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("impl") {
        let name = rest.split_whitespace().next()?;
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_use_statement(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("use ") {
        let module = rest.split(';').next()?.split_whitespace().next()?;
        Some(module.to_string())
    } else {
        None
    }
}

fn extract_js_ts_elements(file_path: &str, content: &str, start_line: i64, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let file_id = format!("file:{}", file_path);

    for (idx, line) in content.lines().enumerate() {
        let line_num = start_line + idx as i64;
        let trimmed = line.trim();

        // Functions: function name( or const name = ( or name( for arrow functions
        if trimmed.starts_with("function ") {
            if let Some(name) = extract_js_name(trimmed, "function ") {
                let id = format!("fn:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Function,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Classes: class Name
        else if trimmed.starts_with("class ") {
            if let Some(name) = extract_js_name(trimmed, "class ") {
                let id = format!("class:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Class,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Imports: import ... from
        else if trimmed.starts_with("import ") {
            if let Some(module) = extract_js_import(trimmed) {
                let id = format!("import:{}:{}", file_path, module);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Import,
                    name: module,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Imports,
                });
            }
        }
        // Interfaces (TypeScript): interface Name
        else if trimmed.starts_with("interface ") {
            if let Some(name) = extract_js_name(trimmed, "interface ") {
                let id = format!("interface:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Interface,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
    }
}

fn extract_js_name(line: &str, prefix: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix(prefix) {
        let name = rest.split(|c: char| c.is_whitespace() || c == '{' || c == '(').next()?;
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_js_import(line: &str) -> Option<String> {
    // import X from 'Y' or import { X } from 'Y'
    if let Some(from_idx) = line.find("from") {
        let rest = &line[from_idx + 4..];
        if let Some(quote_start) = rest.find('\'') {
            let start = quote_start + 1;
            if let Some(quote_end) = rest[start..].find('\'') {
                return Some(rest[start..start + quote_end].to_string());
            }
        }
    }
    None
}

fn extract_python_elements(file_path: &str, content: &str, start_line: i64, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let file_id = format!("file:{}", file_path);

    for (idx, line) in content.lines().enumerate() {
        let line_num = start_line + idx as i64;
        let trimmed = line.trim();

        // Functions: def name(
        if trimmed.starts_with("def ") {
            if let Some(name) = extract_python_name(trimmed, "def ") {
                let id = format!("fn:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Function,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Classes: class Name
        else if trimmed.starts_with("class ") {
            if let Some(name) = extract_python_name(trimmed, "class ") {
                let id = format!("class:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Class,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Imports: import X or from X import Y
        else if trimmed.starts_with("import ") {
            if let Some(module) = extract_python_import(trimmed, "import ") {
                let id = format!("import:{}:{}", file_path, module);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Import,
                    name: module,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Imports,
                });
            }
        }
        else if trimmed.starts_with("from ") {
            if let Some(module) = extract_python_from_import(trimmed) {
                let id = format!("import:{}:{}", file_path, module);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Import,
                    name: module,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Imports,
                });
            }
        }
    }
}

fn extract_python_name(line: &str, prefix: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix(prefix) {
        let name = rest.split('(').next()?.split_whitespace().next()?;
        Some(name.trim_end_matches(':').to_string())
    } else {
        None
    }
}

fn extract_python_import(line: &str, prefix: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix(prefix) {
        let module = rest.split_whitespace().next()?;
        Some(module.to_string())
    } else {
        None
    }
}

fn extract_python_from_import(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("from ") {
        let module = rest.split_whitespace().next()?;
        Some(module.to_string())
    } else {
        None
    }
}

fn extract_go_elements(file_path: &str, content: &str, start_line: i64, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let file_id = format!("file:{}", file_path);

    for (idx, line) in content.lines().enumerate() {
        let line_num = start_line + idx as i64;
        let trimmed = line.trim();

        // Functions: func Name(
        if trimmed.starts_with("func ") {
            if !trimmed.contains("(") {
                continue; // Package-level func
            }
            if let Some(name) = extract_go_name(trimmed) {
                let id = format!("fn:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Function,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Structs: type Name struct
        else if trimmed.starts_with("type ") && trimmed.contains("struct") {
            if let Some(name) = extract_go_type_name(trimmed) {
                let id = format!("struct:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Struct,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Interfaces: type Name interface
        else if trimmed.starts_with("type ") && trimmed.contains("interface") {
            if let Some(name) = extract_go_type_name(trimmed) {
                let id = format!("interface:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Interface,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Imports: import (
        else if trimmed.starts_with("import") {
            // Skip for now - need multi-line handling
        }
    }
}

fn extract_go_name(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("func ") {
        // Handle method receivers: func (r Type) Name(
        if rest.starts_with('(') {
            if let Some(close_paren) = rest.find(')') {
                let after_paren = &rest[close_paren + 1..];
                let name = after_paren.split('(').next()?.trim();
                return Some(name.to_string());
            }
        }
        let name = rest.split('(').next()?.trim();
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_go_type_name(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("type ") {
        let name = rest.split_whitespace().next()?;
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_java_elements(file_path: &str, content: &str, start_line: i64, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let file_id = format!("file:{}", file_path);

    for (idx, line) in content.lines().enumerate() {
        let line_num = start_line + idx as i64;
        let trimmed = line.trim();

        // Methods: public/private/protected Type name(
        if (trimmed.starts_with("public ") || trimmed.starts_with("private ") || trimmed.starts_with("protected "))
            && trimmed.contains('(')
            && !trimmed.contains("class ")
        {
            if let Some(name) = extract_java_method(trimmed) {
                let id = format!("fn:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Function,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Classes: public class Name
        else if trimmed.starts_with("class ") {
            if let Some(name) = extract_java_class(trimmed) {
                let id = format!("class:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Class,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Interfaces: public interface Name
        else if trimmed.starts_with("interface ") {
            if let Some(name) = extract_java_class(trimmed) {
                let id = format!("interface:{}:{}", file_path, name);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Interface,
                    name,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Contains,
                });
            }
        }
        // Imports
        else if trimmed.starts_with("import ") {
            if let Some(module) = extract_java_import(trimmed) {
                let id = format!("import:{}:{}", file_path, module);
                nodes.push(GraphNode {
                    id: id.clone(),
                    node_type: NodeType::Import,
                    name: module,
                    file_path: file_path.to_string(),
                    line_number: Some(line_num),
                    metadata: HashMap::new(),
                });
                edges.push(GraphEdge {
                    from: file_id.clone(),
                    to: id,
                    edge_type: EdgeType::Imports,
                });
            }
        }
    }
}

fn extract_java_method(line: &str) -> Option<String> {
    // Find the method name - it's between the last space before ( and (
    if let Some(paren_idx) = line.find('(') {
        let before_paren = &line[..paren_idx];
        let name = before_paren.split_whitespace().last()?;
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_java_class(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("class ") {
        let name = rest.split_whitespace().next()?;
        Some(name.to_string())
    } else if let Some(rest) = line.strip_prefix("interface ") {
        let name = rest.split_whitespace().next()?;
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_java_import(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("import ") {
        let module = rest.trim_end_matches(';').split_whitespace().last()?;
        Some(module.to_string())
    } else {
        None
    }
}

fn extract_generic_elements(file_path: &str, content: &str, start_line: i64, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let file_id = format!("file:{}", file_path);

    // Just create a file node for unknown languages
    nodes.push(GraphNode {
        id: file_id.clone(),
        node_type: NodeType::Module,
        name: file_path.to_string(),
        file_path: file_path.to_string(),
        line_number: None,
        metadata: HashMap::new(),
    });

    // Try to find any recognizable patterns
    for (idx, line) in content.lines().enumerate() {
        let line_num = start_line + idx as i64;
        let trimmed = line.trim();

        // Look for common function patterns
        if trimmed.contains("function ") || trimmed.contains("def ") || trimmed.contains("fn ") {
            let name = trimmed.split_whitespace().nth(1).unwrap_or("unknown").to_string();
            let id = format!("fn:{}:{}", file_path, name);
            nodes.push(GraphNode {
                id: id.clone(),
                node_type: NodeType::Function,
                name,
                file_path: file_path.to_string(),
                line_number: Some(line_num),
                metadata: HashMap::new(),
            });
            edges.push(GraphEdge {
                from: file_id.clone(),
                to: id,
                edge_type: EdgeType::Contains,
            });
        }
    }
}

fn build_import_edges(nodes: &[GraphNode]) -> Vec<GraphEdge> {
    let mut edges = Vec::new();

    // Group import nodes by file
    let imports_by_file: HashMap<String, Vec<&GraphNode>> = nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Import)
        .fold(HashMap::new(), |mut acc, node| {
            acc.entry(node.file_path.clone()).or_default().push(node);
            acc
        });

    // For each file, try to connect it to definitions of imported items
    for (file_path, imports) in imports_by_file {
        for import in imports {
            // Look for matching definitions in other files
            for node in nodes {
                if node.file_path != file_path {
                    let import_name = &import.name;
                    let node_name = &node.name;

                    // Check if the import matches the node name (simplified)
                    if node_name.contains(import_name) || import_name.contains(node_name) {
                        edges.push(GraphEdge {
                            from: format!("file:{}", file_path),
                            to: node.id.clone(),
                            edge_type: EdgeType::Imports,
                        });
                    }
                }
            }
        }
    }

    edges
}

// ============================================================================
// Semantic Code Actions
// ============================================================================

/// Represents a semantic action that can be performed on code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticAction {
    pub action_type: ActionType,
    pub description: String,
    pub target_files: Vec<ActionTarget>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    ApiChangeImpact,
    GenerateTests,
    FindUsages,
    FindDependents,
    Refactor,
    UpdateDocumentation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTarget {
    pub file_path: String,
    pub line_number: Option<i64>,
    pub reason: String,
}

/// Analyze the impact of an API change
pub fn analyze_api_change(codebase_path: &str, api_signature: &str, model: &str) -> Result<SemanticAction> {
    let path = Path::new(codebase_path);
    let codebase_id = get_codebase_hash(&path.canonicalize()?);

    let conn = database::init_db()?;

    // Search for usages of the API signature
    let query_embedding = get_query_embedding_with_model(api_signature, model);

    let filters = SearchFilters::default();
    let results = database::hybrid_search(&conn, api_signature, Some(&codebase_id), &query_embedding, 50, &filters, false)?;

    // Group results by file and analyze impact
    let mut file_impacts: HashMap<String, Vec<(i64, String)>> = HashMap::new();

    for result in results {
        let entry = file_impacts.entry(result.file_path.clone()).or_default();
        entry.push((result.start_line, result.content.clone()));
    }

    let mut targets = Vec::new();
    for (file_path, locations) in file_impacts {
        let reason = format!("Contains usage or definition of: {}", api_signature);
        targets.push(ActionTarget {
            file_path,
            line_number: locations.first().map(|(l, _)| *l),
            reason,
        });
    }

    Ok(SemanticAction {
        action_type: ActionType::ApiChangeImpact,
        description: format!("Files that may need updating for API change: {}", api_signature),
        target_files: targets,
        confidence: 0.85,
    })
}

/// Find test files related to the given source file
pub fn find_related_tests(codebase_path: &str, source_file: &str) -> Result<Vec<ActionTarget>> {
    let path = Path::new(codebase_path);
    let codebase_id = get_codebase_hash(&path.canonicalize()?);

    let conn = database::init_db()?;

    // Find test files by convention
    let test_patterns = vec![
        format!("{}.test", source_file.strip_suffix(".rs").unwrap_or(source_file)),
        format!("{}_test", source_file),
        format!("{}_tests", source_file),
        format!("test_{}", source_file),
        format!("tests/{}", source_file),
    ];

    let mut stmt = conn.prepare(
        "SELECT DISTINCT file_path FROM chunks WHERE codebase_id = ?1 AND (file_path LIKE '%test%' OR file_path LIKE '%spec%')",
    )?;

    let test_files: Vec<String> = stmt
        .query_map(params![&codebase_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut targets = Vec::new();
    for test_file in test_files {
        targets.push(ActionTarget {
            file_path: test_file,
            line_number: None,
            reason: "Potential test file".to_string(),
        });
    }

    Ok(targets)
}

// ============================================================================
// Code Change Prediction
// ============================================================================

/// Prediction about code changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePrediction {
    pub predicted_files: Vec<PredictedChange>,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedChange {
    pub file_path: String,
    pub change_type: ChangeType,
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    LikelyToEdit,
    MayAffect,
    LikelyTestChange,
    MayCauseConflict,
}

/// Predict which files will be edited together based on embeddings
pub fn predict_changes(codebase_path: &str, modified_files: &[String], model: &str) -> Result<ChangePrediction> {
    let path = Path::new(codebase_path);
    let codebase_id = get_codebase_hash(&path.canonicalize()?);

    let conn = database::init_db()?;

    // Get embeddings for modified files
    let mut file_embeddings: HashMap<String, Vec<f32>> = HashMap::new();

    for file in modified_files {
        let mut stmt = conn.prepare(
            "SELECT content, embedding FROM chunks WHERE codebase_id = ?1 AND file_path = ?2 LIMIT 1",
        )?;

        if let Ok((content, embedding_blob)) = stmt.query_row(params![&codebase_id, file], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        }) {
            let embedding = deserialize_embedding(&embedding_blob);
            file_embeddings.insert(file.clone(), embedding);
        }
    }

    // Find files with similar embeddings (likely to be edited together)
    let mut predictions: Vec<(String, f64)> = Vec::new();

    let mut all_files_stmt = conn.prepare(
        "SELECT DISTINCT file_path FROM chunks WHERE codebase_id = ?1",
    )?;

    let all_files: Vec<String> = all_files_stmt
        .query_map(params![&codebase_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for file in all_files {
        if modified_files.contains(&file) {
            continue;
        }

        // Get embedding for this file
        let mut stmt = conn.prepare(
            "SELECT embedding FROM chunks WHERE codebase_id = ?1 AND file_path = ?2 LIMIT 1",
        )?;

        if let Ok(embedding_blob) = stmt.query_row(params![&codebase_id, &file], |row| {
            row.get::<_, Vec<u8>>(0)
        }) {
            let embedding = deserialize_embedding(&embedding_blob);

            // Calculate similarity to any modified file
            let mut max_similarity = 0.0_f64;
            for (_, modified_emb) in &file_embeddings {
                let similarity = cosine_similarity(&embedding, modified_emb);
                if similarity > max_similarity {
                    max_similarity = similarity;
                }
            }

            if max_similarity > 0.5 {
                predictions.push((file, max_similarity));
            }
        }
    }

    // Sort by similarity and take top results
    predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let predicted_files: Vec<PredictedChange> = predictions
        .iter()
        .take(20)
        .map(|(file, prob)| {
            let change_type = if file.contains("test") || file.contains("spec") {
                ChangeType::LikelyTestChange
            } else {
                ChangeType::LikelyToEdit
            };
            PredictedChange {
                file_path: file.clone(),
                change_type,
                probability: *prob,
            }
        })
        .collect();

    let confidence = if predictions.is_empty() {
        0.0
    } else {
        predictions.iter().take(5).map(|(_, p)| p).sum::<f64>() / 5.0.min(predictions.len() as f64)
    };

    Ok(ChangePrediction {
        predicted_files,
        confidence,
        reasoning: format!(
            "Based on embedding similarity to {} modified file(s)",
            modified_files.len()
        ),
    })
}

fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

// ============================================================================
// Multi-Codebase Search
// ============================================================================

/// Search result with source attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCodebaseResult {
    pub file: String,
    pub lines: String,
    pub content: String,
    pub score: f64,
    pub language: Option<String>,
    pub codebase_id: String,
    pub codebase_path: String,
}

/// Search across multiple codebases
pub fn search_multi_codebase(
    query: &str,
    codebase_paths: &[String],
    limit: i64,
    model: &str,
) -> Result<Vec<MultiCodebaseResult>> {
    let query_embedding = get_query_embedding_with_model(query, model);

    let mut all_results: Vec<MultiCodebaseResult> = Vec::new();

    for codebase_path in codebase_paths {
        let path = Path::new(codebase_path);
        if !path.exists() {
            continue;
        }

        let canonical_path = path.canonicalize().map_err(|e| crate::error::CodeSearchError::Io(e))?;
        let codebase_id = get_codebase_hash(&canonical_path);

        let conn = database::init_db()?;

        let filters = SearchFilters::default();
        let db_results = database::hybrid_search(&conn, query, Some(&codebase_id), &query_embedding, limit, &filters, false)?;

        for result in db_results {
            all_results.push(MultiCodebaseResult {
                file: result.file_path,
                lines: format!("{}-{}", result.start_line, result.end_line),
                content: result.content,
                score: result.score,
                language: result.language,
                codebase_id: result.codebase_id,
                codebase_path: codebase_path.clone(),
            });
        }
    }

    // Sort by score
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(limit as usize);

    Ok(all_results)
}

/// Get list of all indexed codebases with their paths
pub fn list_codebase_paths() -> Result<Vec<(String, String)>> {
    let conn = database::init_db()?;

    let mut stmt = conn.prepare(
        "SELECT DISTINCT codebase_id FROM chunks",
    )?;

    let codebase_ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // For now, we can't get the original path from just the ID
    // In a full implementation, we'd store the path in the database
    let paths: Vec<(String, String)> = codebase_ids
        .into_iter()
        .map(|id| (id.clone(), id))
        .collect();

    Ok(paths)
}

// ============================================================================
// Local LLM Integration (Optional)
// ============================================================================

/// Configuration for local LLM integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            model: None,
            max_tokens: Some(512),
        }
    }
}

/// Query expansion using local LLM
pub fn expand_query_llm(query: &str, config: &LlmConfig) -> Result<String> {
    if !config.enabled || config.endpoint.is_none() {
        // Return original query if LLM is not enabled
        return Ok(query.to_string());
    }

    // In a full implementation, this would call the local LLM
    // For now, return the original query with some basic expansion
    let expanded = format!(
        "{} related code patterns, implementations, and usage examples",
        query
    );

    Ok(expanded)
}

/// Rerank search results using local LLM
pub fn rerank_results_llm(
    query: &str,
    results: &[SearchResult],
    config: &LlmConfig,
) -> Result<Vec<SearchResult>> {
    if !config.enabled || config.endpoint.is_none() || results.is_empty() {
        return Ok(results.to_vec());
    }

    // In a full implementation, this would use the LLM to rerank
    // For now, return results as-is
    Ok(results.to_vec())
}

/// Summarize a code chunk using local LLM
pub fn summarize_chunk_llm(content: &str, config: &LlmConfig) -> Result<String> {
    if !config.enabled || config.endpoint.is_none() {
        // Return truncated content if LLM is not enabled
        let summary: String = content.lines().take(5).collect::<Vec<_>>().join("\n");
        return Ok(summary);
    }

    // In a full implementation, this would call the LLM to summarize
    // For now, return first few lines
    let summary: String = content.lines().take(5).collect::<Vec<_>>().join("\n");
    Ok(summary)
}

// ============================================================================
// Graph Caching
// ============================================================================

use std::sync::Mutex;

static CACHED_GRAPHS: OnceLock<Mutex<HashMap<String, CodeGraph>>> = OnceLock::new();

/// Get or build a cached code graph for a codebase
pub fn get_cached_graph(codebase_path: &str, model: &str) -> Result<CodeGraph> {
    let path = Path::new(codebase_path);
    let canonical = path.canonicalize()?;
    let codebase_id = get_codebase_hash(&canonical);

    let cache = CACHED_GRAPHS.get_or_init(|| Mutex::new(HashMap::new));

    // Check if we have a cached graph
    {
        let graphs = cache.lock().map_err(|e| crate::error::CodeSearchError::InvalidConfiguration(e.to_string()))?;
        if let Some(graph) = graphs.get(&codebase_id) {
            return Ok(graph.clone());
        }
    }

    // Build new graph
    let graph = CodeGraph::build(codebase_path, model)?;

    // Cache it
    {
        let mut graphs = cache.lock().map_err(|e| crate::error::CodeSearchError::InvalidConfiguration(e.to_string()))?;
        graphs.insert(codebase_id, graph.clone());
    }

    Ok(graph)
}

/// Clear the cached graphs
pub fn clear_graph_cache() {
    if let Some(cache) = CACHED_GRAPHS.get() {
        if let Ok(mut graphs) = cache.lock() {
            graphs.clear();
        }
    }
}

// ============================================================================
// MCP Server Integration
// ============================================================================

/// Get graph as traversable MCP resources
pub fn get_graph_resources(codebase_path: &str, model: &str) -> Result<Vec<GraphResource>> {
    let graph = CodeGraph::build(codebase_path, model)?;

    let mut resources = Vec::new();

    // Add file nodes
    for node in graph.get_nodes_by_type(&NodeType::Module) {
        resources.push(GraphResource {
            uri: format!("codegraph://{}/file/{}", codebase_path, node.file_path),
            name: format!("File: {}", node.file_path),
            description: format!("File in codebase: {}", node.file_path),
            resource_type: "file".to_string(),
        });
    }

    // Add function nodes
    for node in graph.get_nodes_by_type(&NodeType::Function) {
        resources.push(GraphResource {
            uri: format!("codegraph://{}/function/{}", codebase_path, node.id),
            name: format!("Function: {}", node.name),
            description: format!("Function {} at line {:?}", node.name, node.line_number),
            resource_type: "function".to_string(),
        });
    }

    // Add class/struct nodes
    for node in graph.get_nodes_by_type(&NodeType::Class) {
        resources.push(GraphResource {
            uri: format!("codegraph://{}/class/{}", codebase_path, node.id),
            name: format!("Class: {}", node.name),
            description: format!("Class {} at line {:?}", node.name, node.line_number),
            resource_type: "class".to_string(),
        });
    }

    Ok(resources)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub resource_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert!(!config.enabled);
        assert!(config.endpoint.is_none());
    }

    #[test]
    fn test_expand_query_without_llm() {
        let config = LlmConfig::default();
        let result = expand_query_llm("test query", &config).unwrap();
        assert!(result.contains("test query"));
    }
}
