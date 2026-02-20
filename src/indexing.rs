use crate::config::get_config;
use crate::database::{delete_chunks_for_file, get_codebase_stats, init_db, insert_chunks, Chunk};
use crate::embedding::{
    get_embedding_with_model, get_embeddings_batch_with_model, zero_embedding_with_model,
};
use crate::error::{CodeSearchError, Result};
use crate::gitignore::GitignoreMatcher;
use crate::manifest::{
    get_codebase_hash, get_manifest_path, hash_file_content, load_manifest_internal,
    save_manifest_internal, Changes,
};
use crate::splitter::split_file;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

// Helper function to get extensions from config
fn get_extensions() -> Vec<&'static str> {
    get_config()
        .extensions()
        .iter()
        .map(|s| s.as_str())
        .collect()
}

// Helper function to get skip dirs from config
fn get_skip_dirs() -> Vec<&'static str> {
    get_config()
        .skip_dirs()
        .iter()
        .map(|s| s.as_str())
        .collect()
}

// Helper function to get skip files from config
fn get_skip_files() -> Vec<&'static str> {
    get_config()
        .skip_files()
        .iter()
        .map(|s| s.as_str())
        .collect()
}

// Helper function to get batch size from config
fn get_batch_size() -> usize {
    get_config().batch_size()
}


#[derive(Debug, Clone)]
pub struct IndexingOptions {
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub force: bool,
    pub verbose: bool,
    pub use_gitignore: bool,
    pub model_name: Option<String>,
}

impl Default for IndexingOptions {
    fn default() -> Self {
        Self {
            chunk_size: None,
            chunk_overlap: None,
            force: false,
            verbose: false,
            use_gitignore: true,
            model_name: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct IndexingStats {
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_removed: usize,
    pub chunks_created: usize,
    pub chunks_removed: usize,
    pub duration_ms: u64,
}

impl std::fmt::Display for IndexingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Indexing completed:")?;
        writeln!(f, "  Files indexed: {}", self.files_indexed)?;
        writeln!(f, "  Files skipped: {}", self.files_skipped)?;
        writeln!(f, "  Files removed: {}", self.files_removed)?;
        writeln!(f, "  Chunks created: {}", self.chunks_created)?;
        writeln!(f, "  Chunks removed: {}", self.chunks_removed)?;
        writeln!(f, "  Duration: {}ms", self.duration_ms)
    }
}

pub struct Indexer {
    config: IndexingOptions,
}

impl Indexer {
    pub fn new(config: IndexingOptions) -> Self {
        Self { config }
    }

    pub fn index_codebase<P: AsRef<Path>>(&mut self, codebase_path: P) -> Result<IndexingStats> {
        let start = Instant::now();
        let codebase_path = codebase_path.as_ref().canonicalize()?;
        let codebase_id = get_codebase_hash(&codebase_path);
        let model = self
            .config
            .model_name
            .as_deref()
            .unwrap_or(get_config().model_name());

        if self.config.verbose {
            println!("Codebase ID: {}", codebase_id);
            println!("Codebase path: {}", codebase_path.display());
            println!("Model: {}", model);
        }

        let conn = init_db()?;

        if self.config.force {
            if self.config.verbose {
                println!("Force flag set, removing existing index...");
            }
            let removed = crate::database::delete_chunks_for_codebase(&conn, &codebase_id)?;
            if self.config.verbose {
                println!("Removed {} existing chunks", removed);
            }
        }

        let gitignore_matcher = if self.config.use_gitignore {
            Some(GitignoreMatcher::new(&codebase_path)?)
        } else {
            None
        };

        let manifest_path = get_manifest_path()?.join(format!("{}.json", codebase_id));
        let existing_manifest = if manifest_path.exists() {
            load_manifest_internal(&manifest_path)?
        } else {
            HashMap::new()
        };

        let changes = if self.config.force {
            get_all_files(
                &codebase_path,
                gitignore_matcher.as_ref(),
                self.config.verbose,
            )?
        } else {
            get_changes_with_gitignore(
                &codebase_path,
                &existing_manifest,
                gitignore_matcher.as_ref(),
                self.config.verbose,
            )?
        };

        let mut stats = IndexingStats::default();

        for file_path in &changes.removed {
            if self.config.verbose {
                println!("Removing: {}", file_path);
            }
            let deleted = delete_chunks_for_file(&conn, &codebase_id, file_path)?;
            stats.chunks_removed += deleted as usize;
            stats.files_removed += 1;
        }

        let files_to_index: Vec<(String, String)> =
            changes.added.into_iter().chain(changes.modified).collect();

        stats.files_indexed = files_to_index.len();

        if files_to_index.is_empty() {
            if self.config.verbose {
                println!("No files to index");
            }
            stats.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(stats);
        }

        let pb = if !self.config.verbose {
            let pb = ProgressBar::new(files_to_index.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };

        let mut new_manifest = existing_manifest;

        for (rel_path, _hash) in &files_to_index {
            new_manifest.remove(rel_path);
        }

        let chunk_size = self.config.chunk_size;
        let chunk_overlap = self.config.chunk_overlap;
        let verbose = self.config.verbose;
        let model_owned = model.to_string();

        let all_chunks: Vec<Vec<Chunk>> = files_to_index
            .par_iter()
            .filter_map(|(rel_path, hash)| {
                let full_path = codebase_path.join(rel_path);
                process_file(
                    &full_path,
                    rel_path,
                    &codebase_id,
                    hash,
                    &model_owned,
                    chunk_size,
                    chunk_overlap,
                    verbose,
                )
                .ok()
            })
            .collect();

        for chunks in all_chunks {
            if !chunks.is_empty() {
                let inserted = insert_chunks(&conn, &chunks)?;
                stats.chunks_created += inserted as usize;
            }
            if let Some(ref pb) = pb {
                pb.inc(1);
            }
        }

        if let Some(pb) = pb {
            pb.finish_and_clear();
        }

        for (rel_path, hash) in &files_to_index {
            new_manifest.insert(rel_path.clone(), hash.clone());
        }

        save_manifest_internal(&manifest_path, &new_manifest)?;

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    pub fn get_stats<P: AsRef<Path>>(
        &self,
        codebase_path: P,
    ) -> Result<Option<crate::database::Stats>> {
        let codebase_path = codebase_path.as_ref().canonicalize()?;
        let codebase_id = get_codebase_hash(&codebase_path);
        let conn = init_db()?;
        get_codebase_stats(&conn, &codebase_id)
    }
}

#[allow(clippy::too_many_arguments)]
fn process_file(
    file_path: &Path,
    rel_path: &str,
    codebase_id: &str,
    hash: &str,
    model: &str,
    chunk_size: Option<usize>,
    chunk_overlap: Option<usize>,
    verbose: bool,
) -> Result<Vec<Chunk>> {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            if verbose {
                eprintln!("Skipping file {} (read error: {})", file_path.display(), e);
            }
            return Ok(Vec::new());
        }
    };

    if content.is_empty() {
        return Ok(Vec::new());
    }

    let code_chunks = split_file(rel_path, &content, chunk_size, chunk_overlap);

    if code_chunks.is_empty() {
        return Ok(Vec::new());
    }

    let chunks: Vec<Chunk> = code_chunks
        .into_iter()
        .map(|chunk| {
            let embedding = get_embedding_with_model(&chunk.content, model);
            Chunk {
                id: None,
                codebase_id: codebase_id.to_string(),
                file_path: chunk.file_path,
                start_line: chunk.start_line as i64,
                end_line: chunk.end_line as i64,
                content: chunk.content,
                language: Some(chunk.language),
                embedding,
                hash: hash.to_string(),
            }
        })
        .collect();

    Ok(chunks)
}

fn get_all_files(
    codebase_path: &Path,
    gitignore_matcher: Option<&GitignoreMatcher>,
    verbose: bool,
) -> Result<Changes> {
    let mut changes = Changes::default();
    let mut current_files: HashMap<String, String> = HashMap::new();

    for entry in walkdir::WalkDir::new(codebase_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        let rel_path = match file_path.strip_prefix(codebase_path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        if let Some(matcher) = gitignore_matcher {
            if matcher.is_ignored(file_path) {
                continue;
            }
        }

        if should_skip_file(&rel_path) {
            continue;
        }

        if let Ok(content) = fs::read(file_path) {
            let hash = hash_file_content(&content);
            current_files.insert(rel_path.clone(), hash.clone());
            changes.added.push((rel_path.clone(), hash));

            if verbose {
                println!("Found: {}", rel_path);
            }
        }
    }

    Ok(changes)
}

fn get_changes_with_gitignore(
    codebase_path: &Path,
    manifest: &HashMap<String, String>,
    gitignore_matcher: Option<&GitignoreMatcher>,
    verbose: bool,
) -> Result<Changes> {
    let mut changes = Changes::default();
    let mut current_files: HashMap<String, String> = HashMap::new();

    for entry in walkdir::WalkDir::new(codebase_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let file_path = entry.path();
        let rel_path = match file_path.strip_prefix(codebase_path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        if let Some(matcher) = gitignore_matcher {
            if matcher.is_ignored(file_path) {
                continue;
            }
        }

        if should_skip_file(&rel_path) {
            continue;
        }

        if let Ok(content) = fs::read(file_path) {
            let hash = hash_file_content(&content);
            current_files.insert(rel_path.clone(), hash.clone());

            if let Some(old_hash) = manifest.get(&rel_path) {
                if old_hash != &hash {
                    changes.modified.push((rel_path.clone(), hash));
                    if verbose {
                        println!("Modified: {}", rel_path);
                    }
                }
            } else {
                changes.added.push((rel_path.clone(), hash));
                if verbose {
                    println!("Added: {}", rel_path);
                }
            }
        }
    }

    for path in manifest.keys() {
        if !current_files.contains_key(path) {
            changes.removed.push(path.clone());
            if verbose {
                println!("Removed: {}", path);
            }
        }
    }

    Ok(changes)
}

fn should_skip_file(rel_path: &str) -> bool {
    let skip_dirs = get_skip_dirs();
    let skip_files = get_skip_files();
    let extensions = get_extensions();

    for dir in skip_dirs {
        if rel_path.starts_with(&format!("{}/", dir)) || rel_path.contains(&format!("/{}/", dir)) {
            return true;
        }
    }

    let path = PathBuf::from(rel_path);
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    for skip_file in skip_files {
        if skip_file.starts_with('*') {
            let ext = skip_file.trim_start_matches('*');
            if file_name.ends_with(ext) {
                return true;
            }
        } else if file_name == skip_file {
            return true;
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_with_dot = format!(".{}", ext.to_lowercase());
        if !extensions.contains(&ext_with_dot.as_str()) {
            return true;
        }
    }

    false
}

#[derive(Debug, Clone)]
pub struct FileHash {
    pub path: PathBuf,
    pub relative_path: String,
    pub hash: String,
    pub size: u64,
}

pub fn scan_codebase(
    codebase_path: &Path,
    gitignore_matcher: Option<&GitignoreMatcher>,
) -> Result<Vec<FileHash>> {
    let codebase_path = codebase_path.canonicalize().map_err(CodeSearchError::Io)?;
    let skip_dirs = get_skip_dirs();

    let entries: Vec<walkdir::DirEntry> = walkdir::WalkDir::new(&codebase_path)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    if skip_dirs.contains(&name) {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    let file_hashes: Vec<FileHash> = entries
        .par_iter()
        .filter_map(|entry| {
            let file_path = entry.path();

            if let Some(matcher) = gitignore_matcher {
                if matcher.is_ignored(file_path) {
                    return None;
                }
            }

            let relative_path = match file_path.strip_prefix(&codebase_path) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => return None,
            };

            if should_skip_file(&relative_path) {
                return None;
            }

            let content = match fs::read(file_path) {
                Ok(c) => c,
                Err(_) => return None,
            };

            let size = content.len() as u64;
            let hash = compute_file_hash(&content);

            Some(FileHash {
                path: file_path.to_path_buf(),
                relative_path,
                hash,
                size,
            })
        })
        .collect();

    Ok(file_hashes)
}

pub fn compute_file_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(result)[..16].to_string()
}

pub fn index_codebase<P: AsRef<Path>>(
    codebase_path: P,
    model: &str,
    force_reindex: bool,
) -> Result<IndexingStats> {
    let start = Instant::now();
    let codebase_path = codebase_path
        .as_ref()
        .canonicalize()
        .map_err(CodeSearchError::Io)?;
    let codebase_id = get_codebase_hash(&codebase_path);

    println!("Scanning codebase: {}", codebase_path.display());

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Scanning files...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let gitignore_matcher = GitignoreMatcher::new(&codebase_path)?;
    let file_hashes = scan_codebase(&codebase_path, Some(&gitignore_matcher))?;
    pb.finish_with_message(format!("Found {} files", file_hashes.len()));

    let manifest_path = get_manifest_path()?.join(format!("{}.json", codebase_id));
    let mut manifest = if force_reindex || !manifest_path.exists() {
        HashMap::new()
    } else {
        load_manifest_internal(&manifest_path).unwrap_or_default()
    };

    let changes = detect_changes(&file_hashes, &manifest);

    let mut stats = IndexingStats {
        files_indexed: 0,
        files_skipped: file_hashes.len() - changes.added.len() - changes.modified.len(),
        ..Default::default()
    };

    if changes.added.is_empty() && changes.modified.is_empty() && changes.removed.is_empty() {
        println!("No changes detected. Index is up to date.");
        stats.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(stats);
    }

    println!(
        "Changes: {} added, {} modified, {} removed",
        changes.added.len(),
        changes.modified.len(),
        changes.removed.len()
    );

    let conn = init_db()?;

    let delete_pb = ProgressBar::new(changes.removed.len() as u64);
    delete_pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    delete_pb.set_message("Removing deleted files...");

    for file_path in &changes.removed {
        delete_chunks_for_file(&conn, &codebase_id, file_path)?;
        manifest.remove(file_path);
        stats.files_removed += 1;
        delete_pb.inc(1);
    }
    delete_pb.finish();

    let files_to_index: Vec<_> = changes
        .added
        .iter()
        .chain(changes.modified.iter())
        .collect();

    if files_to_index.is_empty() {
        save_manifest_internal(&manifest_path, &manifest)?;
        stats.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(stats);
    }

    let process_pb = ProgressBar::new(files_to_index.len() as u64);
    process_pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.green/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    process_pb.set_message("Processing files...");

    let mut all_chunks: Vec<Chunk> = Vec::new();
    stats.files_indexed = files_to_index.len();

    for (relative_path, file_hash) in &files_to_index {
        let file_path = codebase_path.join(relative_path);

        delete_chunks_for_file(&conn, &codebase_id, relative_path)?;

        match process_file_for_indexing(&file_path, relative_path, &codebase_id, file_hash) {
            Ok(chunks) => {
                all_chunks.extend(chunks);
            }
            Err(e) => {
                eprintln!("Warning: Failed to process {}: {}", relative_path, e);
            }
        }

        process_pb.inc(1);
    }
    process_pb.finish_with_message("Files processed");

    if all_chunks.is_empty() {
        save_manifest_internal(&manifest_path, &manifest)?;
        stats.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(stats);
    }

    println!("Generating embeddings for {} chunks...", all_chunks.len());

    let embed_pb = ProgressBar::new(all_chunks.len() as u64);
    embed_pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.magenta/blue} {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    embed_pb.set_message("Generating embeddings...");

    let contents: Vec<String> = all_chunks.iter().map(|c| c.content.clone()).collect();
    let embeddings = get_embeddings_batch_with_model(&contents, get_batch_size(), false, model);

    for (i, chunk) in all_chunks.iter_mut().enumerate() {
        if i < embeddings.len() {
            chunk.embedding = embeddings[i].clone();
        } else {
            chunk.embedding = zero_embedding_with_model(model);
        }
        embed_pb.inc(1);
    }
    embed_pb.finish_with_message("Embeddings generated");

    stats.chunks_created = all_chunks.len();

    let insert_pb = ProgressBar::new(1);
    insert_pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.yellow/blue} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    insert_pb.set_message("Inserting chunks into database...");

    insert_chunks(&conn, &all_chunks)?;
    insert_pb.finish_with_message("Chunks inserted");

    for (relative_path, file_hash) in &files_to_index {
        manifest.insert(relative_path.clone(), file_hash.clone());
    }

    save_manifest_internal(&manifest_path, &manifest)?;

    stats.duration_ms = start.elapsed().as_millis() as u64;
    println!(
        "Indexing complete: {} files indexed, {} chunks created ({}ms)",
        stats.files_indexed, stats.chunks_created, stats.duration_ms
    );

    Ok(stats)
}

fn process_file_for_indexing(
    file_path: &Path,
    relative_path: &str,
    codebase_id: &str,
    file_hash: &str,
) -> Result<Vec<Chunk>> {
    let content = fs::read_to_string(file_path).map_err(|_| CodeSearchError::FileRead {
        path: file_path.to_string_lossy().to_string(),
    })?;

    if content.is_empty() {
        return Ok(Vec::new());
    }

    let code_chunks = split_file(relative_path, &content, None, None);

    let chunks: Vec<Chunk> = code_chunks
        .into_iter()
        .map(|chunk| Chunk {
            id: None,
            codebase_id: codebase_id.to_string(),
            file_path: chunk.file_path,
            start_line: chunk.start_line as i64,
            end_line: chunk.end_line as i64,
            content: chunk.content,
            language: Some(chunk.language),
            embedding: vec![],
            hash: file_hash.to_string(),
        })
        .collect();

    Ok(chunks)
}

fn detect_changes(file_hashes: &[FileHash], manifest: &HashMap<String, String>) -> Changes {
    let mut changes = Changes::default();
    let mut current_files: HashMap<String, String> = HashMap::new();

    for file_hash in file_hashes {
        current_files.insert(file_hash.relative_path.clone(), file_hash.hash.clone());

        if let Some(old_hash) = manifest.get(&file_hash.relative_path) {
            if old_hash != &file_hash.hash {
                changes
                    .modified
                    .push((file_hash.relative_path.clone(), file_hash.hash.clone()));
            }
        } else {
            changes
                .added
                .push((file_hash.relative_path.clone(), file_hash.hash.clone()));
        }
    }

    for path in manifest.keys() {
        if !current_files.contains_key(path) {
            changes.removed.push(path.clone());
        }
    }

    changes
}

pub fn list_indexed_codebases() -> Result<Vec<CodebaseInfo>> {
    let conn = init_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT codebase_id, COUNT(*) as chunk_count, COUNT(DISTINCT file_path) as file_count 
             FROM chunks 
             GROUP BY codebase_id",
        )
        .map_err(CodeSearchError::Database)?;

    let codebases = stmt
        .query_map([], |row| {
            Ok(CodebaseInfo {
                codebase_id: row.get(0)?,
                chunk_count: row.get(1)?,
                file_count: row.get(2)?,
            })
        })
        .map_err(CodeSearchError::Database)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CodeSearchError::Database)?;

    Ok(codebases)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CodebaseInfo {
    pub codebase_id: String,
    pub chunk_count: i64,
    pub file_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_skip_file() {
        assert!(should_skip_file(".git/config"));
        assert!(should_skip_file("node_modules/package/index.js"));
        assert!(should_skip_file("target/debug/main"));
        assert!(should_skip_file("image.png"));
        assert!(should_skip_file("archive.zip"));

        assert!(!should_skip_file("src/main.rs"));
        assert!(!should_skip_file("lib.py"));
        assert!(!should_skip_file("index.js"));
    }

    #[test]
    fn test_default_config() {
        let config = IndexingOptions::default();
        assert!(!config.force);
        assert!(!config.verbose);
        assert!(config.use_gitignore);
    }
}
