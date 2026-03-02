//! Query expansion for improved semantic search
//!
//! This module provides functionality to expand search queries with synonyms
//! and handle typos for more robust search results.

use levenshtein::levenshtein;
use std::collections::{HashMap, HashSet};

/// Synonym groups for common programming terms
/// Maps a term to a list of synonyms
pub fn get_synonyms() -> HashMap<&'static str, Vec<&'static str>> {
    let mut synonyms = HashMap::new();

    // Authentication related
    synonyms.insert("auth", vec!["authentication", "login", "logout", "oauth", "jwt", "token", "credential", "session", "authorize"]);
    synonyms.insert("authentication", vec!["auth", "login", "oauth", "jwt", "token", "credential", "session", "authorize"]);
    synonyms.insert("login", vec!["auth", "authentication", "signin", "credential", "session"]);
    synonyms.insert("oauth", vec!["authentication", "auth", "token", "authorization"]);
    synonyms.insert("jwt", vec!["token", "authentication", "auth", "bearer", "json web token"]);
    synonyms.insert("token", vec!["jwt", "oauth", "authentication", "auth", "bearer", "access token"]);

    // Database related
    synonyms.insert("db", vec!["database", "sql", "query", "postgresql", "mysql", "sqlite", "mongodb", "redis"]);
    synonyms.insert("database", vec!["db", "sql", "query", "postgresql", "mysql", "sqlite", "mongodb", "redis", "datastore"]);
    synonyms.insert("query", vec!["sql", "database", "db", "select", "filter", "search"]);
    synonyms.insert("sql", vec!["database", "query", "postgresql", "mysql", "mariadb"]);
    synonyms.insert("crud", vec!["create", "read", "update", "delete", "database", "persist"]);

    // API related
    synonyms.insert("api", vec!["endpoint", "rest", "http", "request", "service", "interface"]);
    synonyms.insert("rest", vec!["api", "http", "endpoint", "restful", "resource"]);
    synonyms.insert("endpoint", vec!["api", "route", "url", "path", "handler"]);
    synonyms.insert("http", vec!["request", "response", "rest", "api", "network"]);

    // Error handling
    synonyms.insert("error", vec!["exception", "fail", "failure", "invalid", "exception", "err"]);
    synonyms.insert("exception", vec!["error", "fail", "failure", "throw", "catch"]);
    synonyms.insert("validate", vec!["validation", "check", "verify", "sanitize", "ensure"]);

    // Configuration
    synonyms.insert("config", vec!["configuration", "setting", "option", "preference"]);
    synonyms.insert("env", vec!["environment", "config", "variable", "setting"]);

    // Async/concurrency
    synonyms.insert("async", vec!["asynchronous", "await", "promise", "future", "concurrent", "parallel"]);
    synonyms.insert("thread", vec!["async", "concurrent", "parallel", "task", "worker", "process"]);
    synonyms.insert("parallel", vec!["concurrent", "async", "parallel", "multithread", "threadpool"]);

    // File I/O
    synonyms.insert("file", vec!["filesystem", "read", "write", "io", "stream", "path"]);
    synonyms.insert("read", vec!["load", "fetch", "get", "retrieve", "parse"]);
    synonyms.insert("write", vec!["save", "store", "persist", "output", "export"]);

    // Testing
    synonyms.insert("test", vec!["testing", "unit", "integration", "spec", "mock", "fixture"]);
    synonyms.insert("mock", vec!["stub", "fake", "test double", "dummy", "spy"]);

    // Web related
    synonyms.insert("web", vec!["http", "url", "request", "response", "server", "client"]);
    synonyms.insert("server", vec!["backend", "service", "daemon", "host", "endpoint"]);
    synonyms.insert("client", vec!["frontend", "browser", "consumer", "user"]);

    // Cache
    synonyms.insert("cache", vec!["caching", "memoize", "store", "buffer", "redis", "memcached"]);
    synonyms.insert("memory", vec!["cache", "ram", "store", "buffer"]);

    // Logger
    synonyms.insert("log", vec!["logging", "debug", "info", "warn", "error", "trace"]);
    synonyms.insert("debug", vec!["log", "logging", "debugging", "troubleshoot"]);

    // Security
    synonyms.insert("security", vec!["secure", "encryption", "crypt", "hash", "salt", "cipher", "tls", "ssl"]);
    synonyms.insert("encrypt", vec!["crypt", "cipher", "encode", "secure", "hash"]);
    synonyms.insert("hash", vec!["digest", "checksum", "md5", "sha", "bcrypt", "encrypt"]);

    // Common programming concepts
    synonyms.insert("function", vec!["method", "procedure", "routine", "fn", "func"]);
    synonyms.insert("class", vec!["type", "struct", "object", "prototype", "model"]);
    synonyms.insert("object", vec!["instance", "class", "struct", "entity", "model"]);
    synonyms.insert("interface", vec!["protocol", "contract", "trait", "abstract"]);
    synonyms.insert("module", vec!["package", "library", "component", "namespace", "crate"]);
    synonyms.insert("return", vec!["result", "output", "yield", "throw"]);

    // Data structures
    synonyms.insert("list", vec!["array", "vector", "sequence", "collection", "enumerable"]);
    synonyms.insert("map", vec!["dictionary", "hashmap", "object", "associative", "key-value"]);
    synonyms.insert("set", vec!["collection", "unique", "distinct", "hashset"]);

    // User management
    synonyms.insert("user", vec!["account", "member", "person", "customer", "client", "principal"]);
    synonyms.insert("role", vec!["permission", "privilege", "access", "policy", "capability"]);
    synonyms.insert("permission", vec!["role", "privilege", "access", "authorization", "capability"]);

    // Message/queue
    synonyms.insert("message", vec!["event", "notification", "signal", "payload"]);
    synonyms.insert("queue", vec!["message queue", "broker", "stream", "kafka", "rabbitmq"]);

    // Deployment
    synonyms.insert("deploy", vec!["deployment", "release", "publish", "push", "install"]);
    synonyms.insert("build", vec!["compile", "bundle", "package", "artifact"]);

    // Response types
    synonyms.insert("json", vec!["javascript object notation", "api response", "data"]);
    synonyms.insert("xml", vec!["markup", "html", "document"]);
    synonyms.insert("html", vec!["markup", "web page", "dom", "template"]);

    synonyms
}

/// Expand a query with synonyms
/// Returns the original query plus expanded terms
pub fn expand_query(query: &str) -> Vec<String> {
    let synonyms = get_synonyms();
    let mut expanded = Vec::new();

    // Add original query
    expanded.push(query.to_string());

    // Split query into words
    let words: Vec<&str> = query.split_whitespace().collect();

    for word in words {
        let word_lower = word.to_lowercase();

        // Check if this word has synonyms
        if let Some(syn_list) = synonyms.get(word_lower.as_str()) {
            for syn in syn_list {
                let expanded_term = query.replace(word, *syn);
                if expanded_term != query {
                    expanded.push(expanded_term);
                }
            }
        }

        // Also check if any synonym contains this word (reverse lookup)
        for (key, syn_list) in &synonyms {
            for syn in syn_list {
                if syn.contains(&word_lower) || word_lower.contains(syn) {
                    // Add the key term
                    let expanded_term = query.replace(word, *key);
                    if expanded_term != query {
                        expanded.push(expanded_term);
                    }
                }
            }
        }
    }

    // Deduplicate
    let unique: HashSet<_> = expanded.drain(..).collect();
    expanded = unique.into_iter().collect();

    expanded
}

/// Expand query with boolean OR for FTS
/// Creates an expanded query that matches any of the terms
pub fn expand_query_fts(query: &str) -> String {
    let expanded = expand_query(query);

    // If only one term, return as-is
    if expanded.len() == 1 {
        return query.to_string();
    }

    // Join with OR for FTS5
    // But first, let's extract unique significant terms
    let mut terms: Vec<String> = Vec::new();

    // Add original words
    for word in query.split_whitespace() {
        let word_lower = word.to_lowercase();
        if word_lower.len() > 2 {
            terms.push(word_lower);
        }
    }

    // Add synonyms for each word
    let synonyms = get_synonyms();
    for word in query.split_whitespace() {
        let word_lower = word.to_lowercase();

        // Add direct synonyms
        if let Some(syn_list) = synonyms.get(word_lower.as_str()) {
            for syn in syn_list {
                if syn.len() > 2 {
                    terms.push(syn.to_string());
                }
            }
        }
    }

    // Deduplicate
    let unique: HashSet<_> = terms.drain(..).collect();
    terms = unique.into_iter().collect();

    if terms.is_empty() {
        return query.to_string();
    }

    terms.join(" OR ")
}

/// Correct typos in query terms using Levenshtein distance
/// Returns the corrected query
pub fn correct_typos(query: &str, max_distance: usize) -> String {
    let synonyms = get_synonyms();

    // Collect all known terms from synonyms
    let mut known_terms: HashSet<String> = HashSet::new();
    for (key, syn_list) in &synonyms {
        known_terms.insert(key.to_string());
        for syn in syn_list {
            known_terms.insert(syn.to_string());
        }
    }

    let mut corrected_words: Vec<String> = Vec::new();

    for word in query.split_whitespace() {
        let word_lower = word.to_lowercase();

        // Skip short words
        if word_lower.len() <= 2 {
            corrected_words.push(word.to_string());
            continue;
        }

        // Check if it's a known term
        if known_terms.contains(&word_lower) {
            corrected_words.push(word.to_string());
            continue;
        }

        // Find closest match
        let mut best_match = word.to_string();
        let mut best_distance = max_distance + 1;

        for known in &known_terms {
            let distance = levenshtein(&word_lower, known);
            if distance < best_distance && distance > 0 {
                best_distance = distance;
                best_match = known.clone();
            }
        }

        // Only correct if within threshold
        if best_distance <= max_distance {
            // Preserve original case if it was capitalized
            if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                let mut chars = best_match.chars();
                match chars.next() {
                    None => corrected_words.push(best_match),
                    Some(first) => {
                        corrected_words.push(first.to_uppercase().chain(chars).collect());
                    }
                }
            } else {
                corrected_words.push(best_match);
            }
        } else {
            corrected_words.push(word.to_string());
        }
    }

    corrected_words.join(" ")
}

/// Process a query with both expansion and typo correction
pub fn process_query(query: &str, enable_expansion: bool, enable_typo_correction: bool) -> ProcessedQuery {
    let original = query.to_string();

    // First, try typo correction
    let corrected = if enable_typo_correction {
        correct_typos(query, 2)
    } else {
        query.to_string()
    };

    // Then expand
    let expanded = if enable_expansion {
        expand_query_fts(&corrected)
    } else {
        corrected.clone()
    };

    let expansion_terms = if enable_expansion {
        expand_query(&corrected)
    } else {
        vec![corrected.clone()]
    };

    ProcessedQuery {
        original,
        corrected,
        expanded,
        expansion_terms,
    }
}

/// Result of query processing
#[derive(Debug, Clone)]
pub struct ProcessedQuery {
    /// Original query
    pub original: String,
    /// Query after typo correction
    pub corrected: String,
    /// Query expanded with synonyms (for FTS)
    pub expanded: String,
    /// List of all expansion terms
    pub expansion_terms: Vec<String>,
}

/// Token budget management for chunks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenBudget {
    /// Small chunks (~256 tokens) - for focused, specific searches
    Small,
    /// Medium chunks (~512 tokens) - balanced
    Medium,
    /// Large chunks (~1024 tokens) - for broader context
    Large,
    /// Custom token count
    Custom(usize),
}

impl TokenBudget {
    /// Get the token count for this budget
    pub fn tokens(&self) -> usize {
        match self {
            TokenBudget::Small => 256,
            TokenBudget::Medium => 512,
            TokenBudget::Large => 1024,
            TokenBudget::Custom(n) => *n,
        }
    }

    /// Get approximate character count (rough estimate: 1 token ≈ 4 chars)
    pub fn chars(&self) -> usize {
        self.tokens() * 4
    }

    /// Get overlap between chunks (25% of tokens)
    pub fn overlap_chars(&self) -> usize {
        self.chars() / 4
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        TokenBudget::Medium
    }
}

impl From<usize> for TokenBudget {
    fn from(n: usize) -> Self {
        TokenBudget::Custom(n)
    }
}

impl From<&str> for TokenBudget {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "small" => TokenBudget::Small,
            "medium" => TokenBudget::Medium,
            "large" => TokenBudget::Large,
            _ => TokenBudget::Medium,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_synonyms() {
        let synonyms = get_synonyms();
        assert!(synonyms.contains_key("auth"));
        assert!(synonyms.contains_key("database"));
        assert!(synonyms.contains_key("api"));
    }

    #[test]
    fn test_expand_query_auth() {
        let expanded = expand_query("auth");
        assert!(expanded.iter().any(|s| s.contains("authentication")));
        assert!(expanded.iter().any(|s| s.contains("login")));
        assert!(expanded.iter().any(|s| s.contains("oauth")));
    }

    #[test]
    fn test_expand_query_database() {
        let expanded = expand_query("database");
        assert!(expanded.iter().any(|s| s.contains("db")));
        assert!(expanded.iter().any(|s| s.contains("sql")));
    }

    #[test]
    fn test_expand_query_fts() {
        let expanded = expand_query_fts("auth login");
        assert!(expanded.contains("auth") || expanded.contains("login"));
    }

    #[test]
    fn test_correct_typos() {
        // Test with intentional typo
        let corrected = correct_typos("authentiction", 2);
        assert!(corrected.contains("authentication") || corrected.contains("auth"));
    }

    #[test]
    fn test_correct_typos_no_change() {
        // Test with correct word
        let corrected = correct_typos("authentication", 2);
        assert!(corrected.contains("authentication"));
    }

    #[test]
    fn test_process_query() {
        let result = process_query("authtication", true, true);
        assert!(!result.original.is_empty());
        assert!(!result.corrected.is_empty());
        assert!(!result.expanded.is_empty());
    }

    #[test]
    fn test_token_budget() {
        assert_eq!(TokenBudget::Small.tokens(), 256);
        assert_eq!(TokenBudget::Medium.tokens(), 512);
        assert_eq!(TokenBudget::Large.tokens(), 1024);
    }

    #[test]
    fn test_token_budget_from_str() {
        assert_eq!(TokenBudget::from("small"), TokenBudget::Small);
        assert_eq!(TokenBudget::from("MEDIUM"), TokenBudget::Medium);
        assert_eq!(TokenBudget::from("LARGE"), TokenBudget::Large);
    }

    #[test]
    fn test_token_budget_chars() {
        assert_eq!(TokenBudget::Small.chars(), 256 * 4);
        assert_eq!(TokenBudget::Medium.chars(), 512 * 4);
    }
}
