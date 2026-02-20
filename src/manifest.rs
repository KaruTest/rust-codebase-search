use crate::error::{CodeSearchError, Result};
use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Changes {
    pub added: Vec<(String, String)>,
    pub modified: Vec<(String, String)>,
    pub removed: Vec<String>,
}

pub fn get_codebase_hash(codebase_path: &Path) -> String {
    let path_str = codebase_path.to_string_lossy();
    let hash = Sha256::digest(path_str.as_bytes());
    let hex = hex::encode(hash);
    hex[..16].to_string()
}

pub fn get_manifest_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from("com", "code-search", "code-search").ok_or_else(|| {
        CodeSearchError::Manifest("Failed to get project directories".to_string())
    })?;
    let manifests_dir = project_dirs.data_dir().join("manifests");
    fs::create_dir_all(&manifests_dir).map_err(CodeSearchError::Io)?;
    Ok(manifests_dir)
}

pub fn load_manifest(manifest_path: &Path) -> Result<HashMap<String, String>> {
    let content = fs::read_to_string(manifest_path).map_err(CodeSearchError::Io)?;
    let manifest: HashMap<String, String> =
        serde_json::from_str(&content).map_err(CodeSearchError::Serialization)?;
    Ok(manifest)
}

pub fn save_manifest(manifest_path: &Path, manifest: &HashMap<String, String>) -> Result<()> {
    let content = serde_json::to_string_pretty(manifest).map_err(CodeSearchError::Serialization)?;
    fs::write(manifest_path, content).map_err(CodeSearchError::Io)?;
    Ok(())
}

pub fn hash_file_content(content: &[u8]) -> String {
    let hash = Sha256::digest(content);
    let hex = hex::encode(hash);
    hex[..16].to_string()
}

pub fn get_changes(codebase_path: &Path, manifest: &HashMap<String, String>) -> Result<Changes> {
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

        if let Ok(content) = fs::read(file_path) {
            let hash = hash_file_content(&content);
            current_files.insert(rel_path.clone(), hash.clone());

            if let Some(old_hash) = manifest.get(&rel_path) {
                if old_hash != &hash {
                    changes.modified.push((rel_path.clone(), hash));
                }
            } else {
                changes.added.push((rel_path, hash));
            }
        }
    }

    for path in manifest.keys() {
        if !current_files.contains_key(path) {
            changes.removed.push(path.clone());
        }
    }

    Ok(changes)
}

pub fn save_manifest_internal(
    manifest_path: &Path,
    manifest: &HashMap<String, String>,
) -> Result<()> {
    save_manifest(manifest_path, manifest)
}

pub fn load_manifest_internal(manifest_path: &Path) -> Result<HashMap<String, String>> {
    load_manifest(manifest_path)
}

pub fn delete_manifest(codebase_id: &str) -> Result<()> {
    let manifest_dir = get_manifest_path()?;
    let manifest_path = manifest_dir.join(format!("{}.json", codebase_id));
    if manifest_path.exists() {
        fs::remove_file(&manifest_path).map_err(CodeSearchError::Io)?;
    }
    Ok(())
}
