use crate::config::Config;
use crate::database::{delete_chunks_for_codebase, get_codebase_stats, get_global_stats, init_db};
use crate::embedding::{ensure_model_available_with_model, get_query_embedding_with_model};
use crate::error::{CodeSearchError, Result};
use crate::indexing::{list_indexed_codebases, Indexer, IndexingOptions};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::Path;

#[derive(Parser)]
#[command(name = "code-search")]
#[command(about = "Semantic code search using embeddings", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Index a codebase for searching")]
    Index {
        #[arg(value_name = "CODEBASE_PATH", help = "Path to the codebase to index")]
        codebase_path: String,
        #[arg(long, short, help = "Force re-indexing of all files")]
        force: bool,
        #[arg(long, short, help = "Enable verbose output")]
        verbose: bool,
        #[arg(long, help = "Disable gitignore filtering")]
        no_gitignore: bool,
        #[arg(
            long,
            value_name = "MODEL",
            help = "Embedding model to use (minilm, nomic)",
            default_value = "minilm"
        )]
        model: String,
    },
    #[command(about = "Search indexed code")]
    Search {
        #[arg(value_name = "QUERY", help = "Search query")]
        query: String,
        #[arg(
            long,
            value_name = "CODEBASE",
            help = "Path to the indexed codebase",
            required = true
        )]
        codebase: String,
        #[arg(
            long,
            short,
            value_name = "N",
            help = "Maximum number of results",
            default_value = "10"
        )]
        limit: i64,
        #[arg(long, help = "Use vector search only (no FTS)")]
        vector_only: bool,
        #[arg(long, short, help = "Pretty print results with colors")]
        pretty: bool,
        #[arg(
            long,
            value_name = "MODEL",
            help = "Embedding model to use (minilm, nomic)",
            default_value = "minilm"
        )]
        model: String,
    },
    #[command(about = "Show status of indexed codebases")]
    Status {
        #[arg(long, short, help = "List all indexed codebases")]
        list: bool,
        #[arg(long, help = "Output in JSON format")]
        json: bool,
    },
    #[command(about = "Delete an indexed codebase")]
    Delete {
        #[arg(value_name = "CODEBASE_PATH", help = "Path to the codebase to delete")]
        codebase_path: String,
    },
    #[command(about = "Show current configuration")]
    Config {
        #[arg(long, help = "Show config file path")]
        path: bool,
        #[arg(long, help = "Create default config file")]
        create: bool,
    },
}

pub fn run(cli: Cli) -> Result<()> {
    let config = Config::load();
    match cli.command {
        Commands::Index {
            codebase_path,
            force,
            verbose,
            no_gitignore,
            model,
        } => run_index(
            &codebase_path,
            force,
            verbose,
            !no_gitignore,
            &model,
            &config,
        ),
        Commands::Search {
            query,
            codebase,
            limit,
            vector_only,
            pretty,
            model,
        } => run_search(
            &query,
            &codebase,
            limit,
            vector_only,
            pretty,
            &model,
            &config,
        ),
        Commands::Status { list, json } => run_status(list, json),
        Commands::Delete { codebase_path } => run_delete(&codebase_path),
        Commands::Config { path, create } => run_config(path, create, &config),
    }
}

fn run_index(
    codebase_path: &str,
    force: bool,
    verbose: bool,
    use_gitignore: bool,
    model: &str,
    config: &Config,
) -> Result<()> {
    let model = if model == "minilm" {
        config.model.model_type.as_str()
    } else {
        model
    };
    let path = Path::new(codebase_path);
    if !path.exists() {
        return Err(CodeSearchError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Codebase path does not exist: {}", codebase_path),
        )));
    }

    if verbose {
        println!("Loading embedding model '{}'...", model);
    }

    if let Err(e) = ensure_model_available_with_model(model) {
        eprintln!("Warning: Could not load embedding model: {}", e);
        eprintln!("Indexing will continue without embeddings (search will not work until model is available)");
    }

    let config = IndexingOptions {
        force,
        verbose,
        use_gitignore,
        model_name: Some(model.to_string()),
        ..Default::default()
    };

    let mut indexer = Indexer::new(config);

    match indexer.index_codebase(codebase_path) {
        Ok(stats) => {
            println!("{}", stats);
            Ok(())
        }
        Err(e) => {
            eprintln!("Indexing failed: {}", e);
            Err(e)
        }
    }
}

fn run_search(
    query: &str,
    codebase_path: &str,
    limit: i64,
    _vector_only: bool,
    pretty: bool,
    model: &str,
    config: &Config,
) -> Result<()> {
    let model = if model == "minilm" {
        config.model.model_type.as_str()
    } else {
        model
    };
    let limit = if limit == 10 {
        config.search.default_limit as i64
    } else {
        limit
    };
    let path = Path::new(codebase_path);
    if !path.exists() {
        return Err(CodeSearchError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Codebase path does not exist: {}", codebase_path),
        )));
    }

    let canonical_path = path.canonicalize().map_err(CodeSearchError::Io)?;
    let codebase_id = crate::manifest::get_codebase_hash(&canonical_path);

    let conn = init_db()?;

    let stats = get_codebase_stats(&conn, &codebase_id)?;
    if stats.is_none() {
        return Err(CodeSearchError::CodebaseNotIndexed(
            codebase_path.to_string(),
        ));
    }

    ensure_model_available_with_model(model).map_err(|e| {
        CodeSearchError::EmbeddingModelLoad(format!(
            "Failed to load embedding model '{}': {}",
            model, e
        ))
    })?;

    let query_embedding = get_query_embedding_with_model(query, model);

    let db_results =
        crate::database::hybrid_search(&conn, query, Some(&codebase_id), &query_embedding, limit)?;

    let results: Vec<crate::search::SearchResult> = db_results
        .into_iter()
        .map(|r| crate::search::SearchResult {
            file: r.file_path,
            lines: format!("{}-{}", r.start_line, r.end_line),
            content: r.content,
            score: r.score,
            language: r.language,
        })
        .collect();

    if results.is_empty() {
        println!("No results found for query: {}", query);
        return Ok(());
    }

    if pretty {
        print_results_pretty(&results);
    } else {
        print_results_simple(&results);
    }

    Ok(())
}

fn run_status(list: bool, json: bool) -> Result<()> {
    let conn = init_db()?;

    if list {
        let codebases = list_indexed_codebases()?;

        if codebases.is_empty() {
            println!("No codebases indexed.");
            return Ok(());
        }

        if json {
            println!("{}", serde_json::to_string_pretty(&codebases).unwrap());
        } else {
            println!("Indexed codebases:");
            println!();
            for cb in codebases {
                println!(
                    "  {} ({} files, {} chunks)",
                    cb.codebase_id, cb.file_count, cb.chunk_count
                );
            }
        }
    } else {
        let stats = get_global_stats(&conn)?;

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "null".to_string())
            );
        } else if let Some(stats) = stats {
            println!("Global status:");
            println!("  Total codebases: {}", stats.total_codebases);
            println!("  Total files: {}", stats.total_files);
            println!("  Total chunks: {}", stats.total_chunks);
        } else {
            println!("No code indexed.");
        }
    }

    Ok(())
}

fn run_delete(codebase_path: &str) -> Result<()> {
    let path = Path::new(codebase_path);
    if !path.exists() {
        return Err(CodeSearchError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Codebase path does not exist: {}", codebase_path),
        )));
    }

    let canonical_path = path.canonicalize().map_err(CodeSearchError::Io)?;
    let codebase_id = crate::manifest::get_codebase_hash(&canonical_path);

    let conn = init_db()?;

    let stats = get_codebase_stats(&conn, &codebase_id)?;
    if stats.is_none() {
        println!("Codebase '{}' is not indexed.", codebase_path);
        return Ok(());
    }

    let deleted_count = delete_chunks_for_codebase(&conn, &codebase_id)?;
    crate::manifest::delete_manifest(&codebase_id)?;

    println!(
        "Deleted codebase '{}' ({} chunks removed)",
        codebase_path, deleted_count
    );

    Ok(())
}

fn run_config(show_path: bool, create: bool, config: &Config) -> Result<()> {
    // Handle --create flag
    if create {
        match Config::config_path() {
            Some(path) => {
                if path.exists() {
                    println!("Config file already exists: {}", path.display());
                } else {
                    // Create parent directories if needed
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    // Write default config
                    let toml_str = toml::to_string_pretty(config).unwrap();
                    std::fs::write(&path, toml_str)?;
                    println!("Created config file: {}", path.display());
                }
            }
            None => {
                eprintln!("Config path not available");
            }
        }
        return Ok(());
    }

    if show_path {
        match Config::config_path() {
            Some(path) => println!("{}", path.display()),
            None => println!("Config path not available"),
        }
    } else {
        println!("Current configuration:");

        // Model config
        println!("  [model]");
        println!("    model_type: {}", config.model.model_type);
        println!("    auto_download: {}", config.model.auto_download);

        // Indexing config
        println!("  [indexing]");
        println!("    extensions: {} entries", config.indexing.extensions.len());
        println!("    skip_dirs: {} entries", config.indexing.skip_dirs.len());
        println!("    skip_files: {} entries", config.indexing.skip_files.len());
        println!("    use_gitignore: {}", config.indexing.use_gitignore);
        println!("    batch_size: {}", config.indexing.batch_size);

        // Chunking config
        println!("  [chunking]");
        println!("    chunk_size: {}", config.chunking.chunk_size);
        println!("    chunk_overlap: {}", config.chunking.chunk_overlap);

        // Search config
        println!("  [search]");
        println!("    default_limit: {}", config.search.default_limit);
        println!("    fts_weight: {}", config.search.fts_weight);
        println!("    vector_weight: {}", config.search.vector_weight);

        // Database config
        println!("  [database]");
        println!("    data_dir: {}", config.database.data_dir);
        println!("    db_name: {}", config.database.db_name);

        println!();
        match Config::config_path() {
            Some(path) => println!("Config file: {}", path.display()),
            None => println!("Config file: not available"),
        }
    }
    Ok(())
}

fn print_results_simple(results: &[crate::search::SearchResult]) {
    for (i, result) in results.iter().enumerate() {
        println!("{}. {} ({})", i + 1, result.file, result.lines);
        if let Some(lang) = &result.language {
            println!("   Language: {}", lang);
        }
        println!("   Score: {:.4}", result.score);
        println!();
        for line in result.content.lines() {
            println!("   {}", line);
        }
        println!();
    }
}

fn print_results_pretty(results: &[crate::search::SearchResult]) {
    use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

    let mut stdout = StandardStream::stdout(ColorChoice::Auto);

    for (i, result) in results.iter().enumerate() {
        stdout
            .set_color(ColorSpec::new().set_fg(Some(Color::Green)))
            .ok();
        let _ = writeln!(stdout, "{}. {} ({})", i + 1, result.file, result.lines);

        if let Some(lang) = &result.language {
            stdout
                .set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))
                .ok();
            let _ = writeln!(stdout, "   Language: {}", lang);
        }

        stdout
            .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))
            .ok();
        let _ = writeln!(stdout, "   Score: {:.4}", result.score);

        stdout.reset().ok();
        let _ = writeln!(stdout);

        for line in result.content.lines() {
            let _ = writeln!(stdout, "   {}", line);
        }

        let _ = writeln!(stdout);
    }

    stdout.reset().ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cli_index() {
        let cli = Cli::try_parse_from([
            "code-search",
            "index",
            "/path/to/code",
            "--verbose",
            "--force",
        ]);
        assert!(cli.is_ok());
        if let Ok(cli) = cli {
            match cli.command {
                Commands::Index {
                    codebase_path,
                    force,
                    verbose,
                    ..
                } => {
                    assert_eq!(codebase_path, "/path/to/code");
                    assert!(force);
                    assert!(verbose);
                }
                _ => panic!("Expected Index command"),
            }
        }
    }

    #[test]
    fn test_parse_cli_search() {
        let cli = Cli::try_parse_from([
            "code-search",
            "search",
            "test query",
            "--codebase",
            "/path",
            "--limit",
            "5",
            "--pretty",
        ]);
        assert!(cli.is_ok());
        if let Ok(cli) = cli {
            match cli.command {
                Commands::Search {
                    query,
                    codebase,
                    limit,
                    pretty,
                    ..
                } => {
                    assert_eq!(query, "test query");
                    assert_eq!(codebase, "/path");
                    assert_eq!(limit, 5);
                    assert!(pretty);
                }
                _ => panic!("Expected Search command"),
            }
        }
    }

    #[test]
    fn test_parse_cli_status() {
        let cli = Cli::try_parse_from(["code-search", "status", "--list", "--json"]);
        assert!(cli.is_ok());
        if let Ok(cli) = cli {
            match cli.command {
                Commands::Status { list, json } => {
                    assert!(list);
                    assert!(json);
                }
                _ => panic!("Expected Status command"),
            }
        }
    }

    #[test]
    fn test_parse_cli_delete() {
        let cli = Cli::try_parse_from(["code-search", "delete", "/path/to/code"]);
        assert!(cli.is_ok());
        if let Ok(cli) = cli {
            match cli.command {
                Commands::Delete { codebase_path } => {
                    assert_eq!(codebase_path, "/path/to/code");
                }
                _ => panic!("Expected Delete command"),
            }
        }
    }

    #[test]
    fn test_parse_cli_config() {
        let cli = Cli::try_parse_from(["code-search", "config"]);
        assert!(cli.is_ok());
        if let Ok(cli) = cli {
            match cli.command {
                Commands::Config { path } => {
                    assert!(!path);
                }
                _ => panic!("Expected Config command"),
            }
        }
    }

    #[test]
    fn test_parse_cli_config_path() {
        let cli = Cli::try_parse_from(["code-search", "config", "--path"]);
        assert!(cli.is_ok());
        if let Ok(cli) = cli {
            match cli.command {
                Commands::Config { path } => {
                    assert!(path);
                }
                _ => panic!("Expected Config command"),
            }
        }
    }
}
