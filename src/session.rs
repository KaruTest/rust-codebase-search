//! Multi-step query support with session storage
//!
//! This module provides functionality for chaining searches and storing
//! intermediate results in a session for iterative query refinement.

use crate::database::SearchResult as DbSearchResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A single query step in a multi-step search session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStep {
    /// Unique step ID
    pub step_id: String,
    /// The query text for this step
    pub query: String,
    /// Results from this step
    pub results: Vec<SessionSearchResult>,
    /// Refined query based on previous results (if any)
    pub refined_query: Option<String>,
    /// Timestamp when this step was executed
    pub timestamp: i64,
    /// Step number in the sequence
    pub step_number: usize,
}

/// Search result enriched with session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSearchResult {
    /// Chunk/file path
    pub file: String,
    /// Line range
    pub lines: String,
    /// Content snippet
    pub content: String,
    /// Relevance score
    pub score: f64,
    /// Language
    pub language: Option<String>,
    /// Whether this result was selected by the user
    pub selected: bool,
    /// User notes about this result
    pub notes: Option<String>,
}

/// A search session that can chain multiple queries
#[derive(Debug, Clone)]
pub struct SearchSession {
    /// Unique session ID
    pub session_id: String,
    /// All query steps in this session
    pub steps: Vec<QueryStep>,
    /// Current step number
    current_step: usize,
    /// Files that have been marked as relevant
    relevant_files: HashMap<String, f64>,
    /// Query history for expansion
    query_history: Vec<String>,
    /// Created at timestamp
    created_at: i64,
}

impl SearchSession {
    /// Create a new search session
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            steps: Vec::new(),
            current_step: 0,
            relevant_files: HashMap::new(),
            query_history: Vec::new(),
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Add a query step to the session
    pub fn add_step(&mut self, query: String, results: Vec<DbSearchResult>) -> QueryStep {
        let step_id = format!("{}_{}", self.session_id, self.steps.len());

        let session_results: Vec<SessionSearchResult> = results
            .into_iter()
            .map(|r| SessionSearchResult {
                file: r.file_path,
                lines: format!("{}-{}", r.start_line, r.end_line),
                content: r.content,
                score: r.score,
                language: r.language,
                selected: false,
                notes: None,
            })
            .collect();

        let step = QueryStep {
            step_id,
            query: query.clone(),
            results: session_results,
            refined_query: None,
            timestamp: chrono::Utc::now().timestamp(),
            step_number: self.steps.len() + 1,
        };

        self.query_history.push(query);
        self.steps.push(step.clone());

        self.current_step = self.steps.len() - 1;

        step
    }

    /// Get the current step
    pub fn current_step_data(&self) -> Option<&QueryStep> {
        self.steps.get(self.current_step)
    }

    /// Get all steps
    pub fn get_steps(&self) -> &[QueryStep] {
        &self.steps
    }

    /// Mark a result as selected
    pub fn select_result(&mut self, step_index: usize, result_index: usize) -> bool {
        if let Some(step) = self.steps.get_mut(step_index) {
            if let Some(result) = step.results.get_mut(result_index) {
                result.selected = true;

                // Update relevance score for this file
                let entry = self.relevant_files.entry(result.file.clone()).or_insert(0.0);
                *entry += result.score;

                return true;
            }
        }
        false
    }

    /// Add notes to a result
    pub fn add_result_notes(&mut self, step_index: usize, result_index: usize, notes: String) -> bool {
        if let Some(step) = self.steps.get_mut(step_index) {
            if let Some(result) = step.results.get_mut(result_index) {
                result.notes = Some(notes);
                return true;
            }
        }
        false
    }

    /// Get relevant files sorted by score
    pub fn get_relevant_files(&self) -> Vec<(String, f64)> {
        let mut files: Vec<_> = self.relevant_files.iter().map(|(k, v)| (k.clone(), *v)).collect();
        files.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        files
    }

    /// Generate a refined query based on selected results
    pub fn generate_refined_query(&self) -> String {
        let relevant = self.get_relevant_files();

        if relevant.is_empty() {
            // Fall back to last query
            return self.query_history.last().cloned().unwrap_or_default();
        }

        // Use the top relevant files to guide the next query
        let top_files: Vec<_> = relevant.iter().take(3).map(|(f, _)| f.clone()).collect();

        // Build a refined query that includes context from relevant files
        if let Some(last_query) = self.query_history.last() {
            format!("{} in {}", last_query, top_files.join(" OR "))
        } else {
            String::new()
        }
    }

    /// Get query history
    pub fn get_query_history(&self) -> &[String] {
        &self.query_history
    }

    /// Get step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

/// Manager for multiple search sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, SearchSession>>>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session
    pub async fn create_session(&self, session_id: String) -> SearchSession {
        let session = SearchSession::new(session_id.clone());
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session.clone());
        session
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<SearchSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Add a step to a session
    pub async fn add_step(
        &self,
        session_id: &str,
        query: String,
        results: Vec<DbSearchResult>,
    ) -> Option<QueryStep> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            Some(session.add_step(query, results))
        } else {
            None
        }
    }

    /// Select a result in a session
    pub async fn select_result(
        &self,
        session_id: &str,
        step_index: usize,
        result_index: usize,
    ) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.select_result(step_index, result_index)
        } else {
            false
        }
    }

    /// Get all session IDs
    pub async fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id).is_some()
    }

    /// Get session summary
    pub async fn get_session_summary(&self, session_id: &str) -> Option<SessionSummary> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            Some(SessionSummary {
                session_id: session.session_id.clone(),
                step_count: session.steps.len(),
                relevant_file_count: session.relevant_files.len(),
                created_at: session.created_at,
                query_history: session.query_history.clone(),
            })
        } else {
            None
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a search session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub step_count: usize,
    pub relevant_file_count: usize,
    pub created_at: i64,
    pub query_history: Vec<String>,
}

/// Request to perform a multi-step search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiStepSearchRequest {
    /// Initial query
    pub initial_query: String,
    /// Whether to use query expansion
    pub enable_expansion: bool,
    /// Whether to use typo correction
    pub enable_typo_correction: bool,
    /// Maximum number of steps
    pub max_steps: usize,
    /// Maximum results per step
    pub results_limit: usize,
}

impl Default for MultiStepSearchRequest {
    fn default() -> Self {
        Self {
            initial_query: String::new(),
            enable_expansion: true,
            enable_typo_correction: true,
            max_steps: 5,
            results_limit: 10,
        }
    }
}

/// Result of a multi-step search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiStepSearchResult {
    pub session_id: String,
    pub steps: Vec<QueryStep>,
    pub final_results: Vec<SessionSearchResult>,
    pub relevant_files: Vec<(String, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_session_creation() {
        let session = SearchSession::new("test_session".to_string());
        assert_eq!(session.session_id, "test_session");
        assert!(session.steps.is_empty());
    }

    #[test]
    fn test_add_step() {
        let mut session = SearchSession::new("test".to_string());
        let db_results = vec![DbSearchResult {
            chunk_id: 1,
            codebase_id: "test".to_string(),
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 10,
            content: "fn main() {}".to_string(),
            language: Some("rust".to_string()),
            score: 0.9,
            rank: 1,
        }];

        let step = session.add_step("test query".to_string(), db_results);
        assert_eq!(step.query, "test query");
        assert_eq!(step.results.len(), 1);
    }

    #[test]
    fn test_select_result() {
        let mut session = SearchSession::new("test".to_string());
        let db_results = vec![DbSearchResult {
            chunk_id: 1,
            codebase_id: "test".to_string(),
            file_path: "test.rs".to_string(),
            start_line: 1,
            end_line: 10,
            content: "fn main() {}".to_string(),
            language: Some("rust".to_string()),
            score: 0.9,
            rank: 1,
        }];

        session.add_step("test".to_string(), db_results);
        let selected = session.select_result(0, 0);
        assert!(selected);
    }

    #[test]
    fn test_relevant_files() {
        let mut session = SearchSession::new("test".to_string());
        let db_results = vec![
            DbSearchResult {
                chunk_id: 1,
                codebase_id: "test".to_string(),
                file_path: "test1.rs".to_string(),
                start_line: 1,
                end_line: 10,
                content: "fn main() {}".to_string(),
                language: Some("rust".to_string()),
                score: 0.9,
                rank: 1,
            },
            DbSearchResult {
                chunk_id: 2,
                codebase_id: "test".to_string(),
                file_path: "test2.rs".to_string(),
                start_line: 1,
                end_line: 10,
                content: "fn foo() {}".to_string(),
                language: Some("rust".to_string()),
                score: 0.8,
                rank: 2,
            },
        ];

        session.add_step("test".to_string(), db_results);
        session.select_result(0, 0);
        session.select_result(0, 1);

        let relevant = session.get_relevant_files();
        assert_eq!(relevant.len(), 2);
    }

    #[tokio::test]
    async fn test_session_manager() {
        let manager = SessionManager::new();
        let sessions = manager.list_sessions().await;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_session_manager_create() {
        let manager = SessionManager::new();
        manager.create_session("test".to_string()).await;
        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_session_manager_delete() {
        let manager = SessionManager::new();
        manager.create_session("test".to_string()).await;
        manager.delete_session("test").await;
        let sessions = manager.list_sessions().await;
        assert!(sessions.is_empty());
    }
}
