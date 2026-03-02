//! Syntax-aware code chunking using tree-sitter
//!
//! This module provides intelligent code chunking that respects language syntax
//! by splitting at function, class, method, and other structural boundaries
//! rather than arbitrary line counts.

use crate::splitter::{detect_language, CodeChunk};

/// Represents a syntax node extracted from the AST
#[derive(Debug, Clone)]
pub struct SyntaxNode {
    pub node_type: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Point,
    pub end_point: Point,
    pub children: Vec<SyntaxNode>,
}

/// Point in the source code (line, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}

impl Point {
    pub fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }

    pub fn from_tree_sitter(point: tree_sitter::Point) -> Self {
        Self {
            row: point.row,
            column: point.column,
        }
    }
}

/// Language-specific node types to extract for chunking
#[derive(Debug, Clone)]
pub struct LanguageConfig {
    /// Node types that represent top-level definitions (functions, classes, etc.)
    pub definitions: Vec<&'static str>,
    /// Node types that represent nested definitions (methods, inner classes)
    pub nested_definitions: Vec<&'static str>,
    /// Node types to skip entirely
    pub skip_nodes: Vec<&'static str>,
    /// Comment node type
    pub comment: &'static str,
    /// String node type
    pub string: &'static str,
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self::rust()
    }
}

impl LanguageConfig {
    pub fn rust() -> Self {
        Self {
            definitions: vec![
                "function_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "impl_item",
                "macro_definition",
            ],
            nested_definitions: vec![
                "function_item",
                "closure_expression",
                "struct_item",
                "enum_item",
            ],
            skip_nodes: vec!["attribute_item", "line_comment", "block_comment"],
            comment: "line_comment",
            string: "string_literal",
        }
    }

    pub fn python() -> Self {
        Self {
            definitions: vec![
                "function_definition",
                "class_definition",
                "async_function_definition",
            ],
            nested_definitions: vec!["function_definition", "class_definition", "lambda"],
            skip_nodes: vec!["decorator", "comment"],
            comment: "comment",
            string: "string",
        }
    }

    pub fn javascript() -> Self {
        Self {
            definitions: vec![
                "function_declaration",
                "class_declaration",
                "arrow_function",
                "generator_function_declaration",
            ],
            nested_definitions: vec![
                "function_declaration",
                "class_declaration",
                "arrow_function",
                "method_definition",
            ],
            skip_nodes: vec!["comment", "export_statement"],
            comment: "comment",
            string: "string",
        }
    }

    pub fn typescript() -> Self {
        Self {
            definitions: vec![
                "function_declaration",
                "class_declaration",
                "arrow_function",
                "interface_declaration",
                "type_alias_declaration",
                "enum_declaration",
            ],
            nested_definitions: vec![
                "function_declaration",
                "class_declaration",
                "arrow_function",
                "method_definition",
            ],
            skip_nodes: vec!["comment", "export_statement"],
            comment: "comment",
            string: "string",
        }
    }

    pub fn go() -> Self {
        Self {
            definitions: vec![
                "function_declaration",
                "method_declaration",
                "type_declaration",
            ],
            nested_definitions: vec!["function_declaration", "method_declaration"],
            skip_nodes: vec!["line_comment", "block_comment"],
            comment: "line_comment",
            string: "interpreted_string_literal",
        }
    }

    pub fn java() -> Self {
        Self {
            definitions: vec![
                "method_declaration",
                "class_declaration",
                "interface_declaration",
                "enum_declaration",
                "record_declaration",
            ],
            nested_definitions: vec!["method_declaration", "class_declaration"],
            skip_nodes: vec!["line_comment", "block_comment"],
            comment: "line_comment",
            string: "string_literal",
        }
    }

    pub fn c() -> Self {
        Self {
            definitions: vec![
                "function_definition",
                "struct_specifier",
                "union_specifier",
                "enum_specifier",
                "typedef",
            ],
            nested_definitions: vec!["function_definition"],
            skip_nodes: vec!["comment"],
            comment: "comment",
            string: "string_literal",
        }
    }

    pub fn cpp() -> Self {
        Self {
            definitions: vec![
                "function_definition",
                "class_specifier",
                "struct_specifier",
                "namespace_definition",
                "template_declaration",
            ],
            nested_definitions: vec!["function_definition", "class_specifier"],
            skip_nodes: vec!["comment"],
            comment: "comment",
            string: "string_literal",
        }
    }

    pub fn ruby() -> Self {
        Self {
            definitions: vec!["method", "class", "module", "def"],
            nested_definitions: vec!["method", "class", "def"],
            skip_nodes: vec!["comment"],
            comment: "comment",
            string: "string",
        }
    }

    pub fn bash() -> Self {
        Self {
            definitions: vec!["function_definition"],
            nested_definitions: vec!["function_definition"],
            skip_nodes: vec!["comment"],
            comment: "comment",
            string: "string",
        }
    }

    pub fn json() -> Self {
        Self {
            definitions: vec!["object", "array"],
            nested_definitions: vec!["object", "array"],
            skip_nodes: vec![],
            comment: "",
            string: "string",
        }
    }

    pub fn yaml() -> Self {
        Self {
            definitions: vec!["block_mapping", "block_sequence"],
            nested_definitions: vec!["block_mapping", "block_sequence"],
            skip_nodes: vec!["comment"],
            comment: "comment",
            string: "string",
        }
    }
}

/// Get language configuration based on detected language
pub fn get_language_config(language: &str) -> LanguageConfig {
    match language {
        "rust" => LanguageConfig::rust(),
        "python" => LanguageConfig::python(),
        "javascript" | "jsx" => LanguageConfig::javascript(),
        "typescript" | "tsx" => LanguageConfig::typescript(),
        "go" => LanguageConfig::go(),
        "java" => LanguageConfig::java(),
        "c" | "h" => LanguageConfig::c(),
        "cpp" | "hpp" | "cc" | "cxx" => LanguageConfig::cpp(),
        "ruby" => LanguageConfig::ruby(),
        "shell" | "bash" | "zsh" | "sh" => LanguageConfig::bash(),
        "json" => LanguageConfig::json(),
        "yaml" | "yml" => LanguageConfig::yaml(),
        _ => LanguageConfig::default(),
    }
}

/// Parse source code and extract syntax nodes
pub fn parse_source(
    source: &str,
    language: &str,
) -> Result<Vec<SyntaxNode>, String> {
    let lang = match language {
        "rust" => tree_sitter_rust::LANGUAGE,
        "python" => tree_sitter_python::LANGUAGE,
        "javascript" | "jsx" => tree_sitter_javascript::LANGUAGE,
        "typescript" | "tsx" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        "go" => tree_sitter_go::LANGUAGE,
        "java" => tree_sitter_java::LANGUAGE,
        "c" => tree_sitter_c::LANGUAGE,
        "cpp" | "hpp" | "cc" | "cxx" => tree_sitter_cpp::LANGUAGE,
        "ruby" => tree_sitter_ruby::LANGUAGE,
        "bash" | "shell" | "sh" | "zsh" => tree_sitter_bash::LANGUAGE,
        "json" => tree_sitter_json::LANGUAGE,
        "yaml" | "yml" => tree_sitter_yaml::LANGUAGE,
        _ => {
            return Err(format!("Unsupported language for syntax parsing: {}", language));
        }
    };

    let mut parser = tree_sitter::Parser::new();
    let lang_obj: tree_sitter::Language = lang.into();
    parser.set_language(&lang_obj).map_err(|e| e.to_string())?;

    let tree = parser.parse(source, None).ok_or("Failed to parse source")?;
    let root_node = tree.root_node();

    let config = get_language_config(language);
    let mut nodes = Vec::new();

    extract_nodes(root_node, &config, source, &mut nodes);

    Ok(nodes)
}

/// Recursively extract relevant nodes from the AST
fn extract_nodes(
    node: tree_sitter::Node,
    config: &LanguageConfig,
    source: &str,
    nodes: &mut Vec<SyntaxNode>,
) {
    let node_type = node.kind();

    // Skip unwanted nodes
    if config.skip_nodes.contains(&node_type) {
        return;
    }

    // Check if this is a definition we want to capture
    if config.definitions.contains(&node_type) || config.nested_definitions.contains(&node_type) {
        let start_point = Point::from_tree_sitter(node.start_position());
        let end_point = Point::from_tree_sitter(node.end_position());

        let mut children = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            extract_nodes(child, config, source, &mut children);
        }

        nodes.push(SyntaxNode {
            node_type: node_type.to_string(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_point,
            end_point,
            children,
        });
    } else {
        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            extract_nodes(child, config, source, nodes);
        }
    }
}

/// Convert byte offset to line number
fn byte_to_line(source: &str, byte_offset: usize) -> usize {
    source[..byte_offset.min(source.len())].lines().count() + 1
}

/// Convert byte range to line range
fn byte_range_to_line_range(source: &str, start_byte: usize, end_byte: usize) -> (usize, usize) {
    let start_line = byte_to_line(source, start_byte);
    let end_line = byte_to_line(source, end_byte);
    (start_line, end_line)
}

/// Split source code into syntax-aware chunks
pub fn split_file_syntax_aware(
    file_path: &str,
    content: &str,
    max_tokens: Option<usize>,
) -> Vec<CodeChunk> {
    let language = detect_language(file_path);

    // If language is not supported for syntax parsing, fall back to line-based
    let supported_languages = vec![
        "rust", "python", "javascript", "jsx", "typescript", "tsx",
        "go", "java", "c", "cpp", "hpp", "cc", "cxx", "ruby", "bash",
        "shell", "sh", "zsh", "json", "yaml", "yml",
    ];

    if !supported_languages.contains(&language.as_str()) {
        return split_file_line_based(file_path, content, max_tokens);
    }

    let nodes = match parse_source(content, &language) {
        Ok(n) => n,
        Err(_) => {
            return split_file_line_based(file_path, content, max_tokens);
        }
    };

    if nodes.is_empty() {
        return split_file_line_based(file_path, content, max_tokens);
    }

    // Calculate max tokens per chunk (rough estimate: 1 token ≈ 4 chars)
    let tokens_per_chunk = max_tokens.unwrap_or(500);
    let chars_per_chunk = tokens_per_chunk * 4;

    let mut chunks = Vec::new();
    let mut current_chunk_start = 0;
    let mut current_chunk_content = String::new();
    let mut current_chunk_nodes: Vec<SyntaxNode> = Vec::new();

    for node in &nodes {
        let node_content = &content[node.start_byte..node.end_byte];

        // If adding this node would exceed the limit, start a new chunk
        if current_chunk_content.len() + node_content.len() > chars_per_chunk && !current_chunk_content.is_empty() {
            // Create chunk from accumulated content
            let (start_line, end_line) = byte_range_to_line_range(
                content,
                current_chunk_start,
                current_chunk_start + current_chunk_content.len(),
            );

            chunks.push(CodeChunk {
                chunk_id: crate::splitter::generate_chunk_id(file_path, start_line, end_line),
                file_path: file_path.to_string(),
                language: language.clone(),
                start_line,
                end_line,
                content: current_chunk_content.clone(),
            });

            // Start new chunk with overlap (include last node for context)
            if let Some(last_node) = current_chunk_nodes.last() {
                current_chunk_start = last_node.start_byte;
                current_chunk_content = content[last_node.start_byte..].to_string();
            } else {
                current_chunk_start = node.start_byte;
                current_chunk_content = String::new();
            }
            current_chunk_nodes.clear();
        }

        if current_chunk_content.is_empty() {
            current_chunk_start = node.start_byte;
        }

        current_chunk_content.push_str(node_content);
        current_chunk_content.push('\n');
        current_chunk_nodes.push(node.clone());
    }

    // Don't forget the last chunk
    if !current_chunk_content.is_empty() {
        let (start_line, end_line) = byte_range_to_line_range(
            content,
            current_chunk_start,
            current_chunk_start + current_chunk_content.len(),
        );

        chunks.push(CodeChunk {
            chunk_id: crate::splitter::generate_chunk_id(file_path, start_line, end_line),
            file_path: file_path.to_string(),
            language: language.clone(),
            start_line,
            end_line,
            content: current_chunk_content,
        });
    }

    // If we only got one chunk but it's too large, split it
    if chunks.len() == 1 && chunks[0].content.len() > chars_per_chunk {
        return split_file_line_based(file_path, content, max_tokens);
    }

    chunks
}

/// Fallback to line-based chunking
fn split_file_line_based(
    file_path: &str,
    content: &str,
    max_tokens: Option<usize>,
) -> Vec<CodeChunk> {
    let tokens_per_chunk = max_tokens.unwrap_or(500);
    let chars_per_chunk = tokens_per_chunk * 4;
    let overlap_chars = chars_per_chunk / 4; // 25% overlap

    let language = detect_language(file_path);
    let lines: Vec<&str> = content.lines().collect();
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < lines.len() {
        let mut end = start;
        let mut char_count = 0;

        while end < lines.len() && char_count < chars_per_chunk {
            char_count += lines[end].len() + 1; // +1 for newline
            end += 1;
        }

        let chunk_content: String = lines[start..end].join("\n");
        let chunk_id = crate::splitter::generate_chunk_id(file_path, start + 1, end);

        chunks.push(CodeChunk {
            chunk_id,
            file_path: file_path.to_string(),
            language: language.clone(),
            start_line: start + 1,
            end_line: end,
            content: chunk_content,
        });

        if end >= lines.len() {
            break;
        }

        start = end - (overlap_chars / 50).max(1); // Rough line estimate
    }

    chunks
}

/// Check if syntax-aware chunking is available for a language
pub fn is_language_supported(language: &str) -> bool {
    let supported = vec![
        "rust", "python", "javascript", "jsx", "typescript", "tsx",
        "go", "java", "c", "cpp", "hpp", "cc", "cxx", "ruby", "bash",
        "shell", "sh", "zsh", "json", "yaml", "yml",
    ];
    supported.contains(&language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_language_config_rust() {
        let config = get_language_config("rust");
        assert!(config.definitions.contains(&"function_item"));
        assert!(config.definitions.contains(&"struct_item"));
    }

    #[test]
    fn test_get_language_config_python() {
        let config = get_language_config("python");
        assert!(config.definitions.contains(&"function_definition"));
        assert!(config.definitions.contains(&"class_definition"));
    }

    #[test]
    fn test_is_language_supported() {
        assert!(is_language_supported("rust"));
        assert!(is_language_supported("python"));
        assert!(is_language_supported("javascript"));
        assert!(!is_language_supported("unknown"));
    }

    #[test]
    fn test_split_file_syntax_aware_rust() {
        let source = r#"
fn main() {
    println!("Hello, world!");
}

fn another_function() {
    let x = 42;
}

struct MyStruct {
    field: i32,
}
"#;
        let chunks = split_file_syntax_aware("test.rs", source, Some(100));
        // Should get multiple chunks or fall back gracefully
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_split_file_syntax_aware_unsupported() {
        let source = "some unknown content";
        let chunks = split_file_syntax_aware("test.xyz", source, Some(100));
        // Should fall back to line-based
        assert!(!chunks.is_empty());
    }
}
