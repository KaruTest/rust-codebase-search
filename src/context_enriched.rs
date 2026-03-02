//! Context-enriched code chunks
//!
//! This module provides functionality to add rich metadata to code chunks,
//! making them more useful for LLM consumption.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Additional metadata for code chunks to enhance LLM understanding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// File path relative to codebase root
    pub file_path: String,
    /// Programming language
    pub language: String,
    /// Function/method signatures in this chunk (if any)
    pub function_signatures: Vec<String>,
    /// Imported dependencies/modules
    pub imports: Vec<String>,
    /// Export definitions (functions, classes, etc.) in this chunk
    pub exports: Vec<String>,
    /// Structural context (class name, module name, etc.)
    pub context: Vec<String>,
    /// Type information if available
    pub types: Vec<String>,
    /// Comments and documentation in the chunk
    pub doc_comments: Vec<String>,
    /// Start and end line numbers
    pub line_range: LineRange,
    /// Chunk identifier
    pub chunk_id: String,
}

/// Line range for a chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

impl LineRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Enriched chunk combining content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedChunk {
    /// The actual code content
    pub content: String,
    /// Metadata about the chunk
    pub metadata: ChunkMetadata,
    /// Estimated token count
    pub token_count: usize,
}

impl EnrichedChunk {
    /// Create a new enriched chunk
    pub fn new(content: String, metadata: ChunkMetadata) -> Self {
        let token_count = estimate_tokens(&content);
        Self {
            content,
            metadata,
            token_count,
        }
    }

    /// Get the full enriched content as a string suitable for LLM context
    pub fn as_context_string(&self) -> String {
        let mut context = String::new();

        // Add file and language info
        context.push_str(&format!("// File: {}\n", self.metadata.file_path));
        context.push_str(&format!("// Language: {}\n", self.metadata.language));
        context.push_str(&format!("// Lines: {}-{}\n\n", self.metadata.line_range.start, self.metadata.line_range.end));

        // Add imports
        if !self.metadata.imports.is_empty() {
            context.push_str("// Imports:\n");
            for import in &self.metadata.imports {
                context.push_str(&format!("//   {}\n", import));
            }
            context.push('\n');
        }

        // Add function signatures
        if !self.metadata.function_signatures.is_empty() {
            context.push_str("// Functions:\n");
            for sig in &self.metadata.function_signatures {
                context.push_str(&format!("//   {}\n", sig));
            }
            context.push('\n');
        }

        // Add doc comments
        if !self.metadata.doc_comments.is_empty() {
            context.push_str("// Documentation:\n");
            for doc in &self.metadata.doc_comments {
                context.push_str(&format!("//   {}\n", doc));
            }
            context.push('\n');
        }

        // Add the actual code
        context.push_str(&self.content);

        context
    }
}

/// Extract imports from source code based on language
pub fn extract_imports(source: &str, language: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    match language {
        "rust" => {
            // Match: use crate::module;
            // Match: use crate::module::submodule;
            // Match: extern crate name;
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("use ") && !trimmed.contains("{") {
                    if let Some(semi) = trimmed.find(';') {
                        let import = &trimmed[4..semi];
                        if !import.contains("::") || import.starts_with("crate::")
                            || import.starts_with("super::") || import.starts_with("self::") {
                        } else {
                            imports.push(import.to_string());
                        }
                    }
                } else if trimmed.starts_with("extern crate ") {
                    if let Some(semi) = trimmed.find(';') {
                        let import = &trimmed[13..semi];
                        if import != "crate" {
                            imports.push(format!("extern crate {}", import));
                        }
                    }
                }
            }
        }
        "python" => {
            // Match: import module
            // Match: from module import name
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("import ") {
                    if let Some(as_pos) = trimmed.find(" as ") {
                        imports.push(trimmed[7..as_pos].to_string());
                    } else {
                        imports.push(trimmed[7..].to_string());
                    }
                } else if trimmed.starts_with("from ") {
                    if let Some(import_pos) = trimmed.get(5..).and_then(|s| s.find(" import")) {
                        let module = &trimmed[5..5 + import_pos];
                        imports.push(format!("from {} import ...", module));
                    }
                }
            }
        }
        "javascript" | "typescript" => {
            // Match: import { x } from 'module';
            // Match: import x from 'module';
            // Match: const x = require('module');
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("import ") {
                    if let Some(from_pos) = trimmed.get(7..).and_then(|s| s.find(" from")) {
                        let after_from = &trimmed[7 + " from".len()..];
                        if let Some(end) = after_from.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.' && c != '/' && c != '@') {
                            let module = &after_from[..end];
                            if !module.is_empty() {
                                imports.push(module.to_string());
                            }
                        }
                    }
                } else if trimmed.starts_with("const ") && trimmed.contains("require(") {
                    if let Some(start) = trimmed.find("require(") {
                        let after_require = &trimmed[start + 9..];
                        if let Some(end) = after_require.find(')') {
                            let module = &after_require[..end];
                            let clean = module.trim().trim_matches('"').trim_matches('\'');
                            imports.push(clean.to_string());
                        }
                    }
                }
            }
        }
        "go" => {
            // Match: import (
            //         "module"
            //        )
            // Match: import "module"
            let mut in_import_block = false;
            for line in &lines {
                let trimmed = line.trim();
                if trimmed == "import (" {
                    in_import_block = true;
                } else if trimmed == ")" && in_import_block {
                    in_import_block = false;
                } else if in_import_block {
                    let import = trimmed.trim_matches('"');
                    if !import.is_empty() {
                        imports.push(import.to_string());
                    }
                } else if trimmed.starts_with("import \"") {
                    if let Some(end) = trimmed[8..].find('"') {
                        imports.push(trimmed[8..8 + end].to_string());
                    }
                }
            }
        }
        "java" => {
            // Match: import package.Class;
            // Match: import package.*;
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("import ") {
                    if let Some(semi) = trimmed.find(';') {
                        let import = &trimmed[7..semi];
                        imports.push(import.to_string());
                    }
                }
            }
        }
        _ => {
            // Generic import detection - look for common patterns
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("#include")
                    || trimmed.starts_with("require")
                    || trimmed.starts_with("include")
                {
                    imports.push(trimmed.to_string());
                }
            }
        }
    }

    // Deduplicate
    let unique: HashSet<_> = imports.drain(..).collect();
    imports = unique.into_iter().collect();
    imports.sort();

    imports
}

/// Extract function/method signatures from source code
pub fn extract_function_signatures(source: &str, language: &str) -> Vec<String> {
    let mut signatures = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    match language {
        "rust" => {
            // Match: pub fn name(...) -> Type { ...
            // Match: fn name(...) -> Type { ...
            // Match: async fn name(...) -> Type { ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub async fn ")
                    || trimmed.starts_with("async fn ")
                {
                    // Get the function signature (up to the opening brace or ->)
                    let sig = if let Some(brace_pos) = trimmed.find('{') {
                        trimmed[..brace_pos].trim_end().to_string()
                    } else if let Some(arrow_pos) = trimmed.find("->") {
                        // Include return type
                        if let Some(brace_pos) = trimmed[arrow_pos..].find('{') {
                            trimmed[..arrow_pos + brace_pos + 1].trim_end().to_string()
                        } else {
                            trimmed.to_string()
                        }
                    } else {
                        trimmed.to_string()
                    };
                    if !sig.is_empty() {
                        signatures.push(sig);
                    }
                }
            }
        }
        "python" => {
            // Match: def name(...): ...
            // Match: async def name(...): ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                    let sig = if let Some(colon_pos) = trimmed.find(':') {
                        trimmed[..colon_pos].to_string()
                    } else {
                        trimmed.to_string()
                    };
                    if !sig.is_empty() {
                        signatures.push(sig);
                    }
                }
            }
        }
        "javascript" | "typescript" => {
            // Match: function name(...) { ...
            // Match: const name = (...) => { ...
            // Match: async function name(...) { ...
            // Match: name(...) { ... (method)
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("function ")
                    || trimmed.starts_with("async function ")
                    || trimmed.starts_with("const ")
                    || trimmed.starts_with("let ")
                    || trimmed.starts_with("var ")
                {
                    // Extract function name
                    if let Some(paren_pos) = trimmed.find('(') {
                        let before_paren = &trimmed[..paren_pos];
                        let name = if before_paren.contains("=>") {
                            // Arrow function
                            if let Some(eq_pos) = before_paren.find("=>") {
                                before_paren[..eq_pos].trim().to_string()
                            } else {
                                continue;
                            }
                        } else {
                            // Regular function
                            if let Some(space_pos) = before_paren.rfind(' ') {
                                before_paren[space_pos + 1..].to_string()
                            } else {
                                before_paren.to_string()
                            }
                        };

                        if !name.is_empty() && name != "function" && !name.starts_with('{') {
                            signatures.push(format!("{}(...)", name));
                        }
                    }
                }
            }
        }
        "go" => {
            // Match: func name(...) ...
            // Match: func (receiver) name(...) ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("func ") {
                    let sig = if let Some(brace_pos) = trimmed.find('{') {
                        trimmed[..brace_pos].trim_end().to_string()
                    } else {
                        trimmed.to_string()
                    };
                    if !sig.is_empty() {
                        signatures.push(sig);
                    }
                }
            }
        }
        "java" => {
            // Match: public/private/protected void name(...) { ...
            // Match: void name(...) { ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.contains('(') && trimmed.contains(')') {
                    // Look for method-like lines
                    let words: Vec<&str> = trimmed.split_whitespace().collect();
                    if words.len() >= 2 {
                        let last_word = words.last().unwrap();
                        if last_word.contains('(') && !last_word.contains('{') {
                            // Likely a method declaration
                            if let Some(paren_pos) = trimmed.find('(') {
                                let before_paren = &trimmed[..paren_pos];
                                if let Some(space_pos) = before_paren.rfind(' ') {
                                    let return_type = &trimmed[..space_pos];
                                    let name = &trimmed[space_pos + 1..];
                                    signatures.push(format!("{} {}(...)", return_type, name));
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    signatures
}

/// Extract documentation comments from source code
pub fn extract_doc_comments(source: &str, language: &str) -> Vec<String> {
    let mut doc_comments = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    match language {
        "rust" => {
            // Match: /// doc comment
            // Match: /** ... */
            let mut current_doc = String::new();
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("///") {
                    let doc = trimmed[3..].trim();
                    if !doc.is_empty() {
                        if current_doc.is_empty() {
                            current_doc = doc.to_string();
                        } else {
                            current_doc.push_str(" ");
                            current_doc.push_str(doc);
                        }
                    }
                } else if !current_doc.is_empty() {
                    doc_comments.push(current_doc.clone());
                    current_doc.clear();
                }
            }
            if !current_doc.is_empty() {
                doc_comments.push(current_doc);
            }
        }
        "python" => {
            // Match: """ docstring """
            // Match: ''' docstring '''
            let mut in_docstring = false;
            let mut current_doc = String::new();
            let mut docstring_char = '"';

            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                    let is_start = !in_docstring;
                    docstring_char = if trimmed.starts_with("\"\"\"") {
                        '"'
                    } else {
                        '\''
                    };

                    if is_start {
                        in_docstring = true;
                        let content = &trimmed[3..];
                        if content.ends_with("\"\"\"") || content.ends_with("'''") {
                            // Single-line docstring
                            let doc = content[..content.len() - 3].trim();
                            if !doc.is_empty() {
                                doc_comments.push(doc.to_string());
                            }
                            in_docstring = false;
                        } else if !content.is_empty() && content != "\"\"\"" && content != "'''" {
                            current_doc.push_str(content);
                            current_doc.push('\n');
                        }
                    } else {
                        // End of docstring
                        let triple_quotes = docstring_char.to_string().repeat(3);
                        if let Some(end_pos) = trimmed.find(&triple_quotes) {
                            current_doc.push_str(&trimmed[..end_pos]);
                        }
                        if !current_doc.trim().is_empty() {
                            doc_comments.push(current_doc.trim().to_string());
                        }
                        current_doc.clear();
                        in_docstring = false;
                    }
                } else if in_docstring {
                    current_doc.push_str(trimmed);
                    current_doc.push('\n');
                }
            }
        }
        "javascript" | "typescript" => {
            // Match: /** ... */
            // Match: /* ... */
            let mut in_comment = false;
            let mut current_doc = String::new();

            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("/**") {
                    in_comment = true;
                    let content = &trimmed[3..];
                    if content.ends_with("*/") {
                        // Single-line comment
                        let doc = content[..content.len() - 2].trim();
                        if !doc.is_empty() {
                            doc_comments.push(doc.to_string());
                        }
                        in_comment = false;
                    } else if !content.is_empty() && content != "*/" {
                        current_doc.push_str(content);
                        current_doc.push('\n');
                    }
                } else if in_comment && trimmed.starts_with("*/") {
                    if !current_doc.trim().is_empty() {
                        doc_comments.push(current_doc.trim().to_string());
                    }
                    current_doc.clear();
                    in_comment = false;
                } else if in_comment {
                    // Remove leading * from each line
                    let cleaned = if trimmed.starts_with('*') {
                        if trimmed.len() > 1 {
                            &trimmed[1..]
                        } else {
                            ""
                        }
                    } else {
                        trimmed
                    };
                    if !cleaned.is_empty() {
                        current_doc.push_str(cleaned);
                        current_doc.push('\n');
                    }
                }
            }
        }
        _ => {}
    }

    doc_comments
}

/// Extract structural context (class names, module names, etc.)
pub fn extract_context(source: &str, language: &str) -> Vec<String> {
    let mut context = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    match language {
        "rust" => {
            // Match: mod name;
            // Match: pub mod name;
            // Match: struct name { ...
            // Match: enum name { ...
            // Match: trait name { ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("mod ")
                    || trimmed.starts_with("pub mod ")
                {
                    if let Some(semi) = trimmed.find(';') {
                        let name = if trimmed.starts_with("pub mod ") {
                            &trimmed[8..semi]
                        } else {
                            &trimmed[4..semi]
                        };
                        context.push(format!("mod {}", name));
                    }
                } else if trimmed.starts_with("struct ")
                    || trimmed.starts_with("pub struct ")
                {
                    let name = if trimmed.starts_with("pub struct ") {
                        if let Some(brace) = trimmed[12..].find('{') {
                            &trimmed[12..12 + brace]
                        } else {
                            &trimmed[12..]
                        }
                    } else {
                        if let Some(brace) = trimmed[7..].find('{') {
                            &trimmed[7..7 + brace]
                        } else {
                            &trimmed[7..]
                        }
                    };
                    context.push(format!("struct {}", name.trim()));
                } else if trimmed.starts_with("enum ")
                    || trimmed.starts_with("pub enum ")
                {
                    let name = if trimmed.starts_with("pub enum ") {
                        if let Some(brace) = trimmed[10..].find('{') {
                            &trimmed[10..10 + brace]
                        } else {
                            &trimmed[10..]
                        }
                    } else {
                        if let Some(brace) = trimmed[5..].find('{') {
                            &trimmed[5..5 + brace]
                        } else {
                            &trimmed[5..]
                        }
                    };
                    context.push(format!("enum {}", name.trim()));
                }
            }
        }
        "python" => {
            // Match: class Name: ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("class ") {
                    if let Some(colon) = trimmed.find(':') {
                        let name = &trimmed[6..colon];
                        context.push(format!("class {}", name));
                    }
                }
            }
        }
        "javascript" | "typescript" => {
            // Match: class Name { ...
            // Match: export class Name { ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                    let name = if trimmed.starts_with("export class ") {
                        if let Some(brace) = trimmed[13..].find('{') {
                            &trimmed[13..13 + brace]
                        } else {
                            &trimmed[13..]
                        }
                    } else {
                        if let Some(brace) = trimmed[6..].find('{') {
                            &trimmed[6..6 + brace]
                        } else {
                            &trimmed[6..]
                        }
                    };
                    context.push(format!("class {}", name.trim()));
                }
            }
        }
        _ => {}
    }

    context
}

/// Extract type information from source code
pub fn extract_types(source: &str, language: &str) -> Vec<String> {
    let mut types = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    match language {
        "rust" => {
            // Match: type Name = ...
            // Match: struct Name { ... }
            // Match: enum Name { ... }
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("type ")
                    && trimmed.contains('=')
                {
                    if let Some(eq) = trimmed.find('=') {
                        let name = &trimmed[5..eq].trim();
                        if !name.is_empty() {
                            types.push(name.to_string());
                        }
                    }
                }
            }
        }
        "typescript" => {
            // Match: interface Name { ...
            // Match: type Name = ...
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("interface ") {
                    if let Some(brace) = trimmed.find('{') {
                        let name = &trimmed[10..brace];
                        types.push(format!("interface {}", name.trim()));
                    }
                } else if trimmed.starts_with("type ") && trimmed.contains('=') {
                    if let Some(eq) = trimmed.find('=') {
                        let name = &trimmed[5..eq].trim();
                        if let Some(end) = name.find('&') {
                            types.push(name[..end].trim().to_string());
                        } else if let Some(end) = name.find('|') {
                            types.push(name[..end].trim().to_string());
                        } else {
                            types.push(name.to_string());
                        }
                    }
                }
            }
        }
        _ => {}
    }

    types
}

/// Estimate token count (rough approximation)
pub fn estimate_tokens(text: &str) -> usize {
    // Rough estimate: 1 token ≈ 4 characters for code
    // This is a rough approximation
    text.len() / 4
}

/// Enrich a code chunk with metadata
pub fn enrich_chunk(
    content: &str,
    file_path: &str,
    language: &str,
    start_line: usize,
    end_line: usize,
    chunk_id: &str,
) -> EnrichedChunk {
    let imports = extract_imports(content, language);
    let function_signatures = extract_function_signatures(content, language);
    let doc_comments = extract_doc_comments(content, language);
    let context = extract_context(content, language);
    let types = extract_types(content, language);

    // Extract exports (functions and classes that might be exported)
    let mut exports = Vec::new();
    exports.extend(function_signatures.iter().cloned());
    exports.extend(context.iter().filter(|c| c.starts_with("class ")).cloned());

    let metadata = ChunkMetadata {
        file_path: file_path.to_string(),
        language: language.to_string(),
        function_signatures,
        imports,
        exports,
        context,
        types,
        doc_comments,
        line_range: LineRange::new(start_line, end_line),
        chunk_id: chunk_id.to_string(),
    };

    EnrichedChunk::new(content.to_string(), metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_imports() {
        let source = r#"
use std::collections::HashMap;
use crate::module::submodule;
use super::parent;

extern crate log;
"#;
        let imports = extract_imports(source, "rust");
        assert!(!imports.is_empty());
    }

    #[test]
    fn test_extract_python_imports() {
        let source = r#"
import os
import sys
from typing import List, Dict
from collections import Counter
"#;
        let imports = extract_imports(source, "python");
        assert!(!imports.is_empty());
    }

    #[test]
    fn test_extract_function_signatures_rust() {
        let source = r#"
pub fn main() {
    println!("Hello");
}

async fn fetch_data(url: String) -> Result<String, Error> {
    Ok(String::new())
}

fn simple() -> i32 {
    42
}
"#;
        let sigs = extract_function_signatures(source, "rust");
        assert!(sigs.iter().any(|s| s.contains("main")));
        assert!(sigs.iter().any(|s| s.contains("fetch_data")));
    }

    #[test]
    fn test_extract_doc_comments_python() {
        let source = r#"
def foo():
    """This is a docstring."""
    pass
"#;
        let docs = extract_doc_comments(source, "python");
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_estimate_tokens() {
        let code = "fn main() { println!(\"Hello\"); }";
        let tokens = estimate_tokens(code);
        assert!(tokens > 0);
    }

    #[test]
    fn test_enrich_chunk() {
        let source = r#"
use std::io;

/// Main function
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
"#;
        let chunk = enrich_chunk(source, "test.rs", "rust", 1, 6, "abc123");
        assert!(!chunk.metadata.imports.is_empty());
        assert!(!chunk.metadata.function_signatures.is_empty());
        assert!(!chunk.metadata.doc_comments.is_empty());
    }
}
