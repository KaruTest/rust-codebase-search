use crate::database::{init_db, vector_search};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file: String,
    pub lines: String,
    pub content: String,
    pub score: f64,
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FormattedResult {
    pub file: String,
    pub lines: String,
    pub content: String,
    pub score: String,
    pub language: Option<String>,
}

pub fn search(
    query: &str,
    codebase_path: &str,
    limit: i64,
    _vector_only: bool,
) -> Result<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let conn = init_db()?;

    let codebase_id = if codebase_path.is_empty() {
        None
    } else {
        Some(codebase_path.to_string())
    };

    let embedding = vec![0.0_f32; 384]; // Placeholder

    let db_results = vector_search(&conn, codebase_id.as_deref(), &embedding, limit)?;

    let results: Vec<SearchResult> = db_results
        .into_iter()
        .map(|r| SearchResult {
            file: r.file_path,
            lines: format!("{}-{}", r.start_line, r.end_line),
            content: r.content,
            score: r.score,
            language: r.language,
        })
        .collect();

    Ok(results)
}

pub fn format_results(results: &[SearchResult]) -> Vec<FormattedResult> {
    results
        .iter()
        .map(|r| FormattedResult {
            file: r.file.clone(),
            lines: r.lines.clone(),
            content: r.content.clone(),
            score: format!("{:.4}", r.score),
            language: r.language.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_results_empty() {
        let results: Vec<SearchResult> = vec![];
        let formatted = format_results(&results);
        assert!(formatted.is_empty());
    }

    #[test]
    fn test_search_empty_query() {
        let results = search("", "", 10, false).unwrap();
        assert!(results.is_empty());
    }
}
