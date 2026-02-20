use crate::config::get_config;
use crate::error::{CodeSearchError, Result};
use directories::ProjectDirs;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

// Legacy constants for backward compatibility
#[deprecated(since = "0.3.0", note = "Use config.database.data_dir instead")]
pub const DATA_DIR: &str = "code-search";
#[deprecated(since = "0.3.0", note = "Use config.database.db_name instead")]
pub const DB_NAME: &str = "index.db";

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

pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path).map_err(CodeSearchError::Database)?;

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
            UNIQUE(codebase_id, file_path, start_line, end_line)
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_codebase ON chunks(codebase_id);
        CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
        CREATE INDEX IF NOT EXISTS idx_chunks_hash ON chunks(hash);

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

pub fn fts_search(
    conn: &Connection,
    query: &str,
    codebase_id: Option<&str>,
    limit: i64,
) -> Result<Vec<SearchResult>> {
    let fts_query = query
        .split_whitespace()
        .filter(|word| word.len() > 1)
        .collect::<Vec<_>>()
        .join(" OR ");

    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let sql = if codebase_id.is_some() {
        r#"
        SELECT c.id, c.codebase_id, c.file_path, c.start_line, c.end_line, c.content, c.language
        FROM chunks_fts fts
        JOIN chunks c ON c.id = fts.rowid
        WHERE chunks_fts MATCH ?1 AND c.codebase_id = ?2
        ORDER BY bm25(chunks_fts)
        LIMIT ?3
        "#
    } else {
        r#"
        SELECT c.id, c.codebase_id, c.file_path, c.start_line, c.end_line, c.content, c.language
        FROM chunks_fts fts
        JOIN chunks c ON c.id = fts.rowid
        WHERE chunks_fts MATCH ?1
        ORDER BY bm25(chunks_fts)
        LIMIT ?2
        "#
    };

    let mut stmt = conn.prepare(sql).map_err(CodeSearchError::Database)?;

    let mut results = Vec::new();

    if let Some(cid) = codebase_id {
        let rows = stmt
            .query_map(params![fts_query, cid, limit], |row| {
                Ok(SearchResult {
                    chunk_id: row.get(0)?,
                    codebase_id: row.get(1)?,
                    file_path: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    content: row.get(5)?,
                    language: row.get(6)?,
                    score: 1.0,
                    rank: 0,
                })
            })
            .map_err(CodeSearchError::Database)?;

        for row in rows {
            results.push(row.map_err(CodeSearchError::Database)?);
        }
    } else {
        let rows = stmt
            .query_map(params![fts_query, limit], |row| {
                Ok(SearchResult {
                    chunk_id: row.get(0)?,
                    codebase_id: row.get(1)?,
                    file_path: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    content: row.get(5)?,
                    language: row.get(6)?,
                    score: 1.0,
                    rank: 0,
                })
            })
            .map_err(CodeSearchError::Database)?;

        for row in rows {
            results.push(row.map_err(CodeSearchError::Database)?);
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
) -> Result<Vec<SearchResult>> {
    let config = get_config();
    let fts_weight = config.fts_weight();
    let vector_weight = config.vector_weight();

    let fts_limit = limit * 2;

    let mut fts_results = fts_search(conn, query_text, codebase_id, fts_limit)?;

    let mut vector_results = vector_search(conn, codebase_id, query_embedding, fts_limit)?;

    let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut combined: Vec<SearchResult> = Vec::new();

    for mut result in fts_results {
        if seen_ids.insert(result.chunk_id) {
            result.score = fts_weight;
            combined.push(result);
        }
    }

    for mut result in vector_results {
        if seen_ids.insert(result.chunk_id) {
            result.score = result.score * vector_weight;
            combined.push(result);
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

    Ok(combined)
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
