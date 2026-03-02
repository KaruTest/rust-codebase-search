use crate::config::get_config;
use crate::error::{CodeSearchError, Result};
use directories::ProjectDirs;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// Legacy constants for backward compatibility
#[deprecated(since = "0.3.0", note = "Use config.database.data_dir instead")]
pub const DATA_DIR: &str = "code-search";
#[deprecated(since = "0.3.0", note = "Use config.database.db_name instead")]
pub const DB_NAME: &str = "index.db";

// BM25 default parameters
const BM25_K1: f64 = 1.5;
const BM25_B: f64 = 0.75;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: Option<i64>,
    pub codebase_id: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub content: String,
    pub language: Option<String>,
    pub embedding: Vec<f32>,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: i64,
    pub codebase_id: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub content: String,
    pub language: Option<String>,
    pub score: f64,
    pub rank: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub total_chunks: i64,
    pub total_files: i64,
    pub total_codebases: i64,
}

fn get_data_dir() -> Result<PathBuf> {
    let proj_dirs =
        ProjectDirs::from("com.code-search", "code-search", "code-search").ok_or_else(|| {
            CodeSearchError::Io(std::io::Error::other("Failed to get project directories"))
        })?;
    let config = get_config();
    let data_dir = proj_dirs.data_dir().join(config.data_dir());
    fs::create_dir_all(&data_dir).map_err(CodeSearchError::Io)?;
    Ok(data_dir)
}

pub fn get_db_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join(get_config().db_name()))
}

pub fn reset_db() -> Result<()> {
    let db_path = get_db_path()?;
    if db_path.exists() {
        fs::remove_file(&db_path).map_err(CodeSearchError::Io)?;
    }
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<()> {
    let migrations = [
        ("ALTER TABLE chunks ADD COLUMN author TEXT", "author"),
        (
            "ALTER TABLE chunks ADD COLUMN created_at INTEGER",
            "created_at",
        ),
        (
            "ALTER TABLE chunks ADD COLUMN modified_at INTEGER",
            "modified_at",
        ),
    ];

    for (sql, column) in &migrations {
        let column_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('chunks') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !column_exists {
            conn.execute(sql, []).map_err(CodeSearchError::Database)?;
        }
    }

    Ok(())
}

pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path).map_err(CodeSearchError::Database)?;

    run_migrations(&conn)?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            codebase_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            content TEXT NOT NULL,
            language TEXT,
            hash TEXT NOT NULL,
            embedding BLOB,
            author TEXT,
            created_at INTEGER,
            modified_at INTEGER,
            UNIQUE(codebase_id, file_path, start_line, end_line)
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_codebase ON chunks(codebase_id);
        CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
        CREATE INDEX IF NOT EXISTS idx_chunks_hash ON chunks(hash);
        CREATE INDEX IF NOT EXISTS idx_chunks_language ON chunks(language);
        CREATE INDEX IF NOT EXISTS idx_chunks_author ON chunks(author);
        CREATE INDEX IF NOT EXISTS idx_chunks_modified ON chunks(modified_at);

        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            content,
            file_path,
            content='chunks',
            content_rowid='id'
        );

        CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
            INSERT INTO chunks_fts(rowid, content, file_path)
            VALUES (NEW.id, NEW.content, NEW.file_path);
        END;

        CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, content, file_path)
            VALUES ('delete', OLD.id, OLD.content, OLD.file_path);
        END;

        CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, content, file_path)
            VALUES ('delete', OLD.id, OLD.content, OLD.file_path);
            INSERT INTO chunks_fts(rowid, content, file_path)
            VALUES (NEW.id, NEW.content, NEW.file_path);
        END;

        -- Click-through feedback table for Learning-to-Rank
        CREATE TABLE IF NOT EXISTS search_clicks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL,
            chunk_id INTEGER NOT NULL,
            click_rank INTEGER NOT NULL,
            clicked_at INTEGER NOT NULL,
            codebase_id TEXT,
            FOREIGN KEY (chunk_id) REFERENCES chunks(id)
        );

        CREATE INDEX IF NOT EXISTS idx_clicks_query ON search_clicks(query_text);
        CREATE INDEX IF NOT EXISTS idx_clicks_chunk ON search_clicks(chunk_id);
        CREATE INDEX IF NOT EXISTS idx_clicks_codebase ON search_clicks(codebase_id);

        -- Query popularity for query-dependent weights
        CREATE TABLE IF NOT EXISTS query_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL UNIQUE,
            search_count INTEGER DEFAULT 0,
            last_searched INTEGER NOT NULL,
            avg_result_count REAL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_query_stats_query ON query_stats(query_text);
        "#,
    )
    .map_err(CodeSearchError::Database)?;

    Ok(conn)
}

pub fn insert_chunks(conn: &Connection, chunks: &[Chunk]) -> Result<i64> {
    let tx = conn
        .unchecked_transaction()
        .map_err(CodeSearchError::Database)?;

    let mut stmt = tx
        .prepare(
            "INSERT OR REPLACE INTO chunks (codebase_id, file_path, start_line, end_line, content, language, hash, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .map_err(CodeSearchError::Database)?;

    let mut inserted_count = 0;

    for chunk in chunks {
        let embedding_blob: Vec<u8> = chunk
            .embedding
            .iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();

        stmt.execute(params![
            &chunk.codebase_id,
            &chunk.file_path,
            &chunk.start_line,
            &chunk.end_line,
            &chunk.content,
            &chunk.language,
            &chunk.hash,
            &embedding_blob,
        ])
        .map_err(CodeSearchError::Database)?;

        inserted_count += 1;
    }

    drop(stmt);

    tx.commit().map_err(CodeSearchError::Database)?;

    Ok(inserted_count)
}

pub fn delete_chunks_for_file(
    conn: &Connection,
    codebase_id: &str,
    file_path: &str,
) -> Result<i64> {
    let tx = conn
        .unchecked_transaction()
        .map_err(CodeSearchError::Database)?;

    tx.execute(
        "DELETE FROM chunks WHERE codebase_id = ?1 AND file_path = ?2",
        params![codebase_id, file_path],
    )
    .map_err(CodeSearchError::Database)?;

    let deleted_count = tx.changes() as i64;

    tx.commit().map_err(CodeSearchError::Database)?;

    Ok(deleted_count)
}

pub fn delete_chunks_for_codebase(conn: &Connection, codebase_id: &str) -> Result<i64> {
    let tx = conn
        .unchecked_transaction()
        .map_err(CodeSearchError::Database)?;

    tx.execute(
        "DELETE FROM chunks WHERE codebase_id = ?1",
        params![codebase_id],
    )
    .map_err(CodeSearchError::Database)?;

    let deleted_count = tx.changes() as i64;

    tx.commit().map_err(CodeSearchError::Database)?;

    Ok(deleted_count)
}

/// Search filters for advanced filtering
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub language: Option<String>,
    pub after_timestamp: Option<i64>,
    pub author: Option<String>,
    pub file_type: Option<String>,
    pub imports: Option<String>,
}

pub fn fts_search(
    conn: &Connection,
    query: &str,
    codebase_id: Option<&str>,
    limit: i64,
    filters: &SearchFilters,
) -> Result<Vec<SearchResult>> {
    let fts_query = query
        .split_whitespace()
        .filter(|word| word.len() > 1)
        .collect::<Vec<_>>()
        .join(" OR ");

    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    // Build dynamic query with filters
    let mut conditions = vec!["chunks_fts MATCH ?1".to_string()];
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(fts_query.clone())];
    let mut param_idx = 2;

    if let Some(cid) = codebase_id {
        conditions.push(format!("c.codebase_id = ?{}", param_idx));
        params_vec.push(Box::new(cid.to_string()));
        param_idx += 1;
    }

    if let Some(ref lang) = filters.language {
        conditions.push(format!("c.language = ?{}", param_idx));
        params_vec.push(Box::new(lang.clone()));
        param_idx += 1;
    }

    if let Some(ref author) = filters.author {
        conditions.push(format!("c.author = ?{}", param_idx));
        params_vec.push(Box::new(author.clone()));
        param_idx += 1;
    }

    if let Some(ref file_type) = filters.file_type {
        conditions.push(format!("c.file_path LIKE ?{}", param_idx));
        params_vec.push(Box::new(format!("%.{}", file_type)));
        param_idx += 1;
    }

    if let Some(after) = filters.after_timestamp {
        conditions.push(format!("c.modified_at > ?{}", param_idx));
        params_vec.push(Box::new(after));
        param_idx += 1;
    }

    let where_clause = conditions.join(" AND ");

    // Use improved BM25 with explicit k1 and b parameters
    let sql = format!(
        r#"
        SELECT c.id, c.codebase_id, c.file_path, c.start_line, c.end_line, c.content, c.language,
               bm25(chunks_fts, {}, {}) as bm25_score
        FROM chunks_fts fts
        JOIN chunks c ON c.id = fts.rowid
        WHERE {}
        ORDER BY bm25_score
        LIMIT ?{}
        "#,
        BM25_K1, BM25_B, where_clause, param_idx
    );

    let mut stmt = conn.prepare(&sql).map_err(CodeSearchError::Database)?;

    params_vec.push(Box::new(limit));

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let mut results = Vec::new();

    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(SearchResult {
                chunk_id: row.get(0)?,
                codebase_id: row.get(1)?,
                file_path: row.get(2)?,
                start_line: row.get(3)?,
                end_line: row.get(4)?,
                content: row.get(5)?,
                language: row.get(6)?,
                score: row.get::<_, f64>(7)?.abs(),
                rank: 0,
            })
        })
        .map_err(CodeSearchError::Database)?;

    for row in rows {
        results.push(row.map_err(CodeSearchError::Database)?);
    }

    // Normalize BM25 scores to 0-1 range
    let max_score: f64 = results
        .iter()
        .map(|r| r.score)
        .fold(0.0, |max, s| max.max(s));
    if max_score > 0.0 {
        for result in &mut results {
            result.score = result.score / max_score;
        }
    }

    for (i, result) in results.iter_mut().enumerate() {
        result.rank = (i + 1) as i64;
    }

    Ok(results)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

pub fn vector_search(
    conn: &Connection,
    codebase_id: Option<&str>,
    query_embedding: &[f32],
    limit: i64,
) -> Result<Vec<SearchResult>> {
    let sql = if codebase_id.is_some() {
        "SELECT id, codebase_id, file_path, start_line, end_line, content, language, embedding FROM chunks WHERE codebase_id = ?1"
    } else {
        "SELECT id, codebase_id, file_path, start_line, end_line, content, language, embedding FROM chunks"
    };

    let mut stmt = conn.prepare(sql).map_err(CodeSearchError::Database)?;

    let mut candidates: Vec<(SearchResult, Vec<f32>)> = Vec::new();

    if let Some(cid) = codebase_id {
        let rows = stmt
            .query_map(params![cid], |row| {
                let embedding_blob: Vec<u8> = row.get(7)?;
                Ok((
                    SearchResult {
                        chunk_id: row.get(0)?,
                        codebase_id: row.get(1)?,
                        file_path: row.get(2)?,
                        start_line: row.get(3)?,
                        end_line: row.get(4)?,
                        content: row.get(5)?,
                        language: row.get(6)?,
                        score: 0.0,
                        rank: 0,
                    },
                    embedding_blob,
                ))
            })
            .map_err(CodeSearchError::Database)?;

        for row in rows {
            let (result, blob) = row.map_err(CodeSearchError::Database)?;
            let embedding = deserialize_embedding(&blob);
            candidates.push((result, embedding));
        }
    } else {
        let rows = stmt
            .query_map([], |row| {
                let embedding_blob: Vec<u8> = row.get(7)?;
                Ok((
                    SearchResult {
                        chunk_id: row.get(0)?,
                        codebase_id: row.get(1)?,
                        file_path: row.get(2)?,
                        start_line: row.get(3)?,
                        end_line: row.get(4)?,
                        content: row.get(5)?,
                        language: row.get(6)?,
                        score: 0.0,
                        rank: 0,
                    },
                    embedding_blob,
                ))
            })
            .map_err(CodeSearchError::Database)?;

        for row in rows {
            let (result, blob) = row.map_err(CodeSearchError::Database)?;
            let embedding = deserialize_embedding(&blob);
            candidates.push((result, embedding));
        }
    }

    let mut scored: Vec<SearchResult> = candidates
        .into_iter()
        .map(|(mut result, embedding)| {
            result.score = cosine_similarity(query_embedding, &embedding);
            result
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit as usize);

    for (i, result) in scored.iter_mut().enumerate() {
        result.rank = (i + 1) as i64;
    }

    Ok(scored)
}

pub fn hybrid_search(
    conn: &Connection,
    query_text: &str,
    codebase_id: Option<&str>,
    query_embedding: &[f32],
    limit: i64,
    filters: &SearchFilters,
    enable_fuzzy: bool,
) -> Result<Vec<SearchResult>> {
    let config = get_config();
    let mut fts_weight = config.fts_weight();
    let mut vector_weight = config.vector_weight();

    // Query-dependent weight adjustment based on popularity
    if let Ok(Some(popularity)) = get_query_popularity(conn, query_text) {
        // More popular queries get balanced weights, rare queries favor FTS
        let popularity_factor = (popularity as f64).min(100.0) / 100.0;
        fts_weight = fts_weight * 0.5 + 0.5 * (1.0 - popularity_factor);
        vector_weight = vector_weight * 0.5 + 0.5 * popularity_factor;
    }

    // Apply language weights if available
    let mut language_weights: HashMap<String, f64> = HashMap::new();
    if let Some(cid) = codebase_id {
        if let Ok(weights) = get_language_weights(conn, cid) {
            language_weights = weights;
        }
    }

    let fts_limit = limit * 3; // Get more results for fusion

    let mut fts_results = fts_search(conn, query_text, codebase_id, fts_limit, filters)?;

    let mut vector_results = vector_search(conn, codebase_id, query_embedding, fts_limit)?;

    // Get LTR click boosts
    let click_boosts = get_click_boosts(conn, query_text).unwrap_or_default();

    // Apply LTR boosting to FTS results
    for result in &mut fts_results {
        if let Some(boost) = click_boosts.get(&result.chunk_id) {
            result.score = result.score + boost * 0.3;
        }

        // Apply language weight
        if let Some(ref lang) = result.language {
            if let Some(weight) = language_weights.get(lang) {
                result.score = result.score * (1.0 + weight * 0.2);
            }
        }
    }

    // Apply LTR boosting and language weights to vector results
    for result in &mut vector_results {
        if let Some(boost) = click_boosts.get(&result.chunk_id) {
            result.score = result.score + boost * 0.3;
        }

        if let Some(ref lang) = result.language {
            if let Some(weight) = language_weights.get(lang) {
                result.score = result.score * (1.0 + weight * 0.2);
            }
        }
    }

    // Coverage scoring: measure how many query terms are covered
    let query_terms: Vec<&str> = query_text.split_whitespace().collect();

    // Store rankings for RRF before consuming results
    let fts_ranks: Vec<_> = fts_results.iter().map(|r| r.chunk_id).collect();
    let vector_ranks: Vec<_> = vector_results.iter().map(|r| r.chunk_id).collect();

    let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut combined: Vec<SearchResult> = Vec::new();

    for mut result in fts_results {
        if seen_ids.insert(result.chunk_id) {
            // Calculate coverage score
            let coverage = calculate_coverage(query_text, &result.content);
            result.score = fts_weight * (result.score + coverage * 0.2);
            combined.push(result);
        }
    }

    for mut result in vector_results {
        if seen_ids.insert(result.chunk_id) {
            // Calculate coverage score
            let coverage = calculate_coverage(query_text, &result.content);
            result.score = vector_weight * (result.score + coverage * 0.2);
            combined.push(result);
        }
    }

    // Apply fuzzy matching if enabled
    if enable_fuzzy {
        apply_fuzzy_boost(query_text, &mut combined, 2);
    }

    // RRF (Reciprocal Rank Fusion) for combining rankings
    let rrf_results = reciprocal_rank_fusion_ids(&fts_ranks, &vector_ranks, 60.0);

    // Merge RRF scores with original scores
    for result in &mut combined {
        if let Some(rrf_score) = rrf_results.get(&result.chunk_id) {
            result.score = result.score * 0.7 + rrf_score * 0.3;
        }
    }

    combined.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    combined.truncate(limit as usize);

    for (i, result) in combined.iter_mut().enumerate() {
        result.rank = (i + 1) as i64;
    }

    // Record search for query stats
    let _ = record_search(conn, query_text, combined.len() as i64);

    Ok(combined)
}

/// Calculate query term coverage in content
fn calculate_coverage(query: &str, content: &str) -> f64 {
    let query_terms: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 1).collect();
    if query_terms.is_empty() {
        return 0.0;
    }

    let content_lower = content.to_lowercase();
    let covered = query_terms
        .iter()
        .filter(|term| content_lower.contains(&term.to_lowercase()))
        .count();

    covered as f64 / query_terms.len() as f64
}

/// Reciprocal Rank Fusion to combine multiple rankings
fn reciprocal_rank_fusion(
    fts_results: Vec<SearchResult>,
    vector_results: Vec<SearchResult>,
    k: f64,
) -> HashMap<i64, f64> {
    let mut rrf_scores: HashMap<i64, f64> = HashMap::new();

    for (rank, result) in fts_results.iter().enumerate() {
        let score = 1.0 / (k + (rank + 1) as f64);
        *rrf_scores.entry(result.chunk_id).or_insert(0.0) += score;
    }

    for (rank, result) in vector_results.iter().enumerate() {
        let score = 1.0 / (k + (rank + 1) as f64);
        *rrf_scores.entry(result.chunk_id).or_insert(0.0) += score;
    }

    rrf_scores
}

/// Reciprocal Rank Fusion using chunk IDs only (avoids moving results)
fn reciprocal_rank_fusion_ids(fts_ids: &[i64], vector_ids: &[i64], k: f64) -> HashMap<i64, f64> {
    let mut rrf_scores: HashMap<i64, f64> = HashMap::new();

    for (rank, chunk_id) in fts_ids.iter().enumerate() {
        let score = 1.0 / (k + (rank + 1) as f64);
        *rrf_scores.entry(*chunk_id).or_insert(0.0) += score;
    }

    for (rank, chunk_id) in vector_ids.iter().enumerate() {
        let score = 1.0 / (k + (rank + 1) as f64);
        *rrf_scores.entry(*chunk_id).or_insert(0.0) += score;
    }

    rrf_scores
}

pub fn get_codebase_stats(conn: &Connection, codebase_id: &str) -> Result<Option<Stats>> {
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*) as chunk_count, COUNT(DISTINCT file_path) as file_count 
             FROM chunks WHERE codebase_id = ?1",
        )
        .map_err(CodeSearchError::Database)?;

    let row = stmt
        .query_row(params![codebase_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(CodeSearchError::Database)?;

    let (chunk_count, file_count) = row;

    if chunk_count == 0 {
        Ok(None)
    } else {
        Ok(Some(Stats {
            total_chunks: chunk_count,
            total_files: file_count,
            total_codebases: 1,
        }))
    }
}

pub fn get_global_stats(conn: &Connection) -> Result<Option<Stats>> {
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*) as total_chunks, COUNT(DISTINCT file_path) as total_files, COUNT(DISTINCT codebase_id) as total_codebases FROM chunks",
        )
        .map_err(CodeSearchError::Database)?;

    let row = stmt
        .query_row([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(CodeSearchError::Database)?;

    let (total_chunks, total_files, total_codebases) = row;

    if total_chunks == 0 {
        Ok(None)
    } else {
        Ok(Some(Stats {
            total_chunks,
            total_files,
            total_codebases,
        }))
    }
}

pub fn list_indexed_codebases(conn: &Connection) -> Result<Vec<(String, i64, i64)>> {
    let mut stmt = conn
        .prepare(
            "SELECT codebase_id, COUNT(*) as chunks, COUNT(DISTINCT file_path) as files
             FROM chunks
             GROUP BY codebase_id
             ORDER BY codebase_id",
        )
        .map_err(CodeSearchError::Database)?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(CodeSearchError::Database)?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(CodeSearchError::Database)?);
    }

    Ok(results)
}

// ============== Learning-to-Rank Functions ==============

/// Record a click event for Learning-to-Rank
pub fn record_click(
    conn: &Connection,
    query_text: &str,
    chunk_id: i64,
    click_rank: i64,
    codebase_id: Option<&str>,
) -> Result<()> {
    let tx = conn
        .unchecked_transaction()
        .map_err(CodeSearchError::Database)?;

    tx.execute(
        "INSERT INTO search_clicks (query_text, chunk_id, click_rank, clicked_at, codebase_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            query_text,
            chunk_id,
            click_rank,
            chrono::Utc::now().timestamp(),
            codebase_id
        ],
    )
    .map_err(CodeSearchError::Database)?;

    tx.commit().map_err(CodeSearchError::Database)?;

    // Also update query stats
    update_query_stats(conn, query_text)?;

    Ok(())
}

/// Update query statistics for query-dependent weighting
fn update_query_stats(conn: &Connection, query_text: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    conn.execute(
        "INSERT INTO query_stats (query_text, search_count, last_searched, avg_result_count)
         VALUES (?1, 1, ?2, 0)
         ON CONFLICT(query_text) DO UPDATE SET
            search_count = search_count + 1,
            last_searched = ?2",
        params![query_text, now],
    )
    .map_err(CodeSearchError::Database)?;

    Ok(())
}

/// Record a search event (for query stats)
pub fn record_search(conn: &Connection, query_text: &str, result_count: i64) -> Result<()> {
    let now = chrono::Utc::now().timestamp();

    conn.execute(
        "INSERT INTO query_stats (query_text, search_count, last_searched, avg_result_count)
         VALUES (?1, 1, ?2, ?3)
         ON CONFLICT(query_text) DO UPDATE SET
            search_count = search_count + 1,
            avg_result_count = (avg_result_count * search_count + ?3) / (search_count + 1)",
        params![query_text, now, result_count as f64],
    )
    .map_err(CodeSearchError::Database)?;

    Ok(())
}

/// Get click boost scores for a given query
pub fn get_click_boosts(conn: &Connection, query_text: &str) -> Result<HashMap<i64, f64>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT chunk_id, COUNT(*) as click_count,
                   AVG(1.0 / click_rank) as avg_reciprocal_rank
            FROM search_clicks
            WHERE query_text = ?1
            GROUP BY chunk_id
            "#,
        )
        .map_err(CodeSearchError::Database)?;

    let rows = stmt
        .query_map(params![query_text], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(2)?))
        })
        .map_err(CodeSearchError::Database)?;

    let mut boosts = HashMap::new();
    for row in rows {
        let (chunk_id, avg_rr) = row.map_err(CodeSearchError::Database)?;
        // Boost is proportional to reciprocal rank (higher rank = more boost)
        boosts.insert(chunk_id, avg_rr);
    }

    Ok(boosts)
}

/// Get query frequency for adaptive weighting
pub fn get_query_popularity(conn: &Connection, query_text: &str) -> Result<Option<i64>> {
    let mut stmt = conn
        .prepare("SELECT search_count FROM query_stats WHERE query_text = ?1")
        .map_err(CodeSearchError::Database)?;

    let result = stmt
        .query_row(params![query_text], |row| row.get::<_, i64>(0))
        .ok();

    Ok(result)
}

/// Get language weights for the codebase
pub fn get_language_weights(conn: &Connection, codebase_id: &str) -> Result<HashMap<String, f64>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT language, COUNT(*) as count
            FROM chunks
            WHERE codebase_id = ?1 AND language IS NOT NULL
            GROUP BY language
            "#,
        )
        .map_err(CodeSearchError::Database)?;

    let rows = stmt
        .query_map(params![codebase_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(CodeSearchError::Database)?;

    let mut lang_counts: HashMap<String, i64> = HashMap::new();
    for row in rows {
        let (lang, count) = row.map_err(CodeSearchError::Database)?;
        lang_counts.insert(lang, count);
    }

    // Convert counts to weights (inverse frequency - less common = higher weight)
    let total: i64 = lang_counts.values().sum();
    let mut weights = HashMap::new();

    if total > 0 {
        for (lang, count) in lang_counts {
            let weight = 1.0 - (count as f64 / total as f64);
            weights.insert(lang, weight * 2.0); // Scale to 0-2 range
        }
    }

    Ok(weights)
}

// ============== Fuzzy Matching ==============

/// Calculate Levenshtein distance between two strings
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[len1][len2]
}

/// Generate fuzzy query variants
pub fn generate_fuzzy_variants(query: &str) -> Vec<String> {
    let mut variants = vec![query.to_string()];

    // Common typos and corrections
    let typo_corrections = [
        ("teh", "the"),
        ("adn", "and"),
        ("thsi", "this"),
        ("taht", "that"),
        ("wihch", "which"),
        ("functon", "function"),
        ("fucntion", "function"),
        ("functoin", "function"),
        ("mothed", "method"),
        ("paramter", "parameter"),
        ("paramter", "parameter"),
        ("varaible", "variable"),
        ("varible", "variable"),
    ];

    let query_lower = query.to_lowercase();
    for (typo, correction) in typo_corrections {
        if query_lower.contains(typo) {
            variants.push(query_lower.replace(typo, correction));
        }
    }

    // Generate acronym expansions for common programming terms
    let acronym_expansions = [
        ("api", "application programming interface"),
        ("sdk", "software development kit"),
        ("cli", "command line interface"),
        ("gui", "graphical user interface"),
        ("url", "uniform resource locator"),
        ("http", "hypertext transfer protocol"),
        ("json", "javascript object notation"),
        ("xml", "extensible markup language"),
        ("sql", "structured query language"),
        ("orm", "object relational mapping"),
        ("mvc", "model view controller"),
        ("rest", "representational state transfer"),
        ("crud", "create read update delete"),
        ("jwt", "json web token"),
        ("tls", "transport layer security"),
        ("ssl", "secure sockets layer"),
    ];

    for (acronym, expansion) in acronym_expansions {
        if query_lower == acronym {
            variants.push(expansion.to_string());
            variants.push(format!("{} {}", expansion, acronym));
        }
    }

    // Add common variations
    if query_lower.ends_with("ing") {
        variants.push(query_lower[..query_lower.len() - 3].to_string());
    }
    if query_lower.ends_with("tion") {
        variants.push(query_lower[..query_lower.len() - 4].to_string());
    }
    if query_lower.ends_with("er") {
        variants.push(query_lower[..query_lower.len() - 2].to_string());
    }

    // Deduplicate
    variants.sort();
    variants.dedup();

    variants
}

/// Apply fuzzy matching to search results
pub fn apply_fuzzy_boost(query: &str, results: &mut [SearchResult], max_edit_distance: usize) {
    let query_terms: Vec<&str> = query.split_whitespace().collect();

    for result in results.iter_mut() {
        let mut fuzzy_score = 0.0;

        for term in &query_terms {
            // Check if term appears in content
            if result.content.to_lowercase().contains(&term.to_lowercase()) {
                fuzzy_score += 1.0;
                continue;
            }

            // Check for close matches using edit distance
            let content_lower = result.content.to_lowercase();
            let mut best_distance = max_edit_distance + 1;

            for word in content_lower.split_whitespace() {
                let dist = levenshtein_distance(term, word);
                if dist < best_distance {
                    best_distance = dist;
                }
            }

            if best_distance <= max_edit_distance {
                fuzzy_score += 1.0 - (best_distance as f64 / (max_edit_distance + 1) as f64);
            }
        }

        // Normalize fuzzy score
        if !query_terms.is_empty() {
            fuzzy_score /= query_terms.len() as f64;
        }

        // Apply fuzzy boost to score
        result.score = result.score + fuzzy_score * 0.2;
    }
}
