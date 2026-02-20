use ignore::gitignore::GitignoreBuilder;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct GitignoreMatcher {
    codebase_path: PathBuf,
    gitignores: RwLock<HashMap<PathBuf, ignore::gitignore::Gitignore>>,
}

impl GitignoreMatcher {
    pub fn new<P: AsRef<Path>>(codebase_path: P) -> Result<Self, std::io::Error> {
        let codebase_path = codebase_path.as_ref().canonicalize()?;

        let mut gitignores = HashMap::new();

        if let Err(e) = Self::load_gitignores_recursive(&codebase_path, &mut gitignores) {
            eprintln!("Warning: Error loading .gitignore files: {}", e);
        }

        Ok(Self {
            codebase_path,
            gitignores: RwLock::new(gitignores),
        })
    }

    fn load_gitignores_recursive(
        base_path: &Path,
        gitignores: &mut HashMap<PathBuf, ignore::gitignore::Gitignore>,
    ) -> Result<(), std::io::Error> {
        for entry in WalkBuilder::new(base_path)
            .hidden(false)
            .git_ignore(false)
            .require_git(false)
            .max_depth(Some(10))
            .build()
        {
            let entry = entry.map_err(|e| std::io::Error::other(format!("Walk error: {}", e)))?;
            let path = entry.path();

            if path.file_name() == Some(std::ffi::OsStr::new(".gitignore")) {
                if let Some(parent) = path.parent() {
                    let mut builder = GitignoreBuilder::new(parent);

                    if let Ok(content) = std::fs::read_to_string(path) {
                        for line in content.lines() {
                            let _ = builder.add_line(Some(path.to_path_buf()), line);
                        }
                    }

                    if let Ok(gitignore) = builder.build() {
                        gitignores.insert(parent.to_path_buf(), gitignore);
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn get_gitignore_for_path(&self, file_path: &Path) -> Option<ignore::gitignore::Gitignore> {
        let absolute_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.codebase_path.join(file_path)
        };

        let mut current = if absolute_path.is_dir() {
            absolute_path
        } else if let Some(parent) = absolute_path.parent() {
            parent.to_path_buf()
        } else {
            return None;
        };

        loop {
            {
                let gitignores = self.gitignores.read().ok()?;
                if let Some(gitignore) = gitignores.get(&current) {
                    return Some(gitignore.clone());
                }
            }

            if current == self.codebase_path {
                break;
            }

            if let Some(parent) = current.parent() {
                if !parent.starts_with(&self.codebase_path) {
                    break;
                }
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        None
    }

    pub fn is_ignored<P: AsRef<Path>>(&self, file_path: P) -> bool {
        let file_path = file_path.as_ref();

        let absolute_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.codebase_path.join(file_path)
        };

        if !absolute_path.starts_with(&self.codebase_path) {
            return false;
        }

        let relative_path = match absolute_path.strip_prefix(&self.codebase_path) {
            Ok(p) => p,
            Err(_) => return false,
        };

        let is_dir = absolute_path.is_dir();

        let gitignores = self.gitignores.read().ok();
        let gitignores = match gitignores {
            Some(g) => g,
            None => return false,
        };

        let mut parent = if is_dir {
            &absolute_path
        } else if let Some(p) = absolute_path.parent() {
            p
        } else {
            return false;
        };

        loop {
            if let Some(gitignore) = gitignores.get(parent) {
                match gitignore.matched(relative_path, is_dir) {
                    ignore::Match::Ignore(_) => return true,
                    ignore::Match::Whitelist(_) => return false,
                    ignore::Match::None => {}
                }
            }

            if parent == self.codebase_path {
                break;
            }

            match parent.parent() {
                Some(p) if p.starts_with(&self.codebase_path) => parent = p,
                _ => break,
            }
        }

        if !is_dir {
            if let Some(dir_path) = relative_path.parent() {
                if dir_path != Path::new("") {
                    let mut current = self.codebase_path.clone();
                    for component in dir_path.components() {
                        current.push(component);
                        if self.check_path_ignored(&gitignores, &current, dir_path, true) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    fn check_path_ignored(
        &self,
        gitignores: &HashMap<PathBuf, ignore::gitignore::Gitignore>,
        absolute_path: &Path,
        relative_path: &Path,
        is_dir: bool,
    ) -> bool {
        let mut parent = if is_dir {
            absolute_path
        } else if let Some(p) = absolute_path.parent() {
            p
        } else {
            return false;
        };

        loop {
            if let Some(gitignore) = gitignores.get(parent) {
                match gitignore.matched(relative_path, is_dir) {
                    ignore::Match::Ignore(_) => return true,
                    ignore::Match::Whitelist(_) => return false,
                    ignore::Match::None => {}
                }
            }

            if parent == self.codebase_path {
                break;
            }

            match parent.parent() {
                Some(p) if p.starts_with(&self.codebase_path) => parent = p,
                _ => break,
            }
        }

        false
    }

    pub fn filter_paths(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        use rayon::prelude::*;

        paths
            .par_iter()
            .filter(|path| !self.is_ignored(path))
            .cloned()
            .collect()
    }

    pub fn codebase_path(&self) -> &Path {
        &self.codebase_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    fn create_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        fs::create_dir_all(path.join("src")).unwrap();
        fs::create_dir_all(path.join("target")).unwrap();
        fs::create_dir_all(path.join("tests")).unwrap();

        let mut gitignore = File::create(path.join(".gitignore")).unwrap();
        writeln!(gitignore, "target/").unwrap();
        writeln!(gitignore, "*.o").unwrap();
        writeln!(gitignore, "*.tmp").unwrap();

        File::create(path.join("main.rs"))
            .unwrap()
            .write_all(b"fn main() {}")
            .unwrap();
        File::create(path.join("src/lib.rs"))
            .unwrap()
            .write_all(b"pub fn hello() {}")
            .unwrap();
        File::create(path.join("src/test.o"))
            .unwrap()
            .write_all(b"binary")
            .unwrap();
        File::create(path.join("test.tmp"))
            .unwrap()
            .write_all(b"temp")
            .unwrap();
        File::create(path.join("tests/test.rs"))
            .unwrap()
            .write_all(b"#[test]")
            .unwrap();

        dir
    }

    #[test]
    fn test_create_matcher() {
        let dir = create_test_repo();
        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        assert_eq!(matcher.codebase_path(), dir.path());
    }

    #[test]
    fn test_is_ignored_target() {
        let dir = create_test_repo();
        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        assert!(matcher.is_ignored("target"));
        assert!(matcher.is_ignored("target/"));
        assert!(matcher.is_ignored(Path::new("target").join("debug")));
    }

    #[test]
    fn test_is_not_ignored_source() {
        let dir = create_test_repo();
        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        assert!(!matcher.is_ignored("main.rs"));
        assert!(!matcher.is_ignored("src/lib.rs"));
        assert!(!matcher.is_ignored("tests/test.rs"));
    }

    #[test]
    fn test_is_ignored_patterns() {
        let dir = create_test_repo();
        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        assert!(matcher.is_ignored("src/test.o"));
        assert!(matcher.is_ignored("test.tmp"));

        assert!(!matcher.is_ignored("src/lib.rs"));
    }

    #[test]
    fn test_filter_paths() {
        let dir = create_test_repo();
        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        let paths = vec![
            PathBuf::from("main.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("target/debug"),
            PathBuf::from("src/test.o"),
            PathBuf::from("test.tmp"),
            PathBuf::from("tests/test.rs"),
        ];

        let filtered = matcher.filter_paths(&paths);

        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&PathBuf::from("main.rs")));
        assert!(filtered.contains(&PathBuf::from("src/lib.rs")));
        assert!(filtered.contains(&PathBuf::from("tests/test.rs")));
    }

    #[test]
    fn test_absolute_paths() {
        let dir = create_test_repo();
        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        let abs_main = dir.path().join("main.rs");
        let abs_target = dir.path().join("target");

        assert!(!matcher.is_ignored(&abs_main));
        assert!(matcher.is_ignored(&abs_target));
    }

    #[test]
    fn test_nested_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        fs::create_dir_all(path.join("src")).unwrap();
        fs::create_dir_all(path.join("src/generated")).unwrap();

        let mut root_gitignore = File::create(path.join(".gitignore")).unwrap();
        writeln!(root_gitignore, "*.log").unwrap();

        let mut src_gitignore = File::create(path.join("src/.gitignore")).unwrap();
        writeln!(src_gitignore, "generated/").unwrap();

        File::create(path.join("app.log")).unwrap();
        File::create(path.join("src/main.rs")).unwrap();
        File::create(path.join("src/generated/code.rs")).unwrap();

        let matcher = GitignoreMatcher::new(dir.path()).unwrap();

        assert!(matcher.is_ignored("app.log"));
        assert!(!matcher.is_ignored("src/main.rs"));
        assert!(matcher.is_ignored("src/generated"));
        assert!(matcher.is_ignored("src/generated/code.rs"));
    }
}
