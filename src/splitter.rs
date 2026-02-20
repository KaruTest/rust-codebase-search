use crate::config::get_config;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// Legacy constants for backward compatibility
#[deprecated(since = "0.3.0", note = "Use config.chunking.chunk_size instead")]
pub const DEFAULT_CHUNK_SIZE: usize = 50;
#[deprecated(since = "0.3.0", note = "Use config.chunking.chunk_overlap instead")]
pub const DEFAULT_OVERLAP: usize = 10;

// Helper functions to get values from config
fn get_default_chunk_size() -> usize {
    get_config().chunk_size()
}

fn get_default_overlap() -> usize {
    get_config().chunk_overlap()
}

pub fn language_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();

    map.insert(".rs", "rust");
    map.insert(".py", "python");
    map.insert(".js", "javascript");
    map.insert(".jsx", "javascript");
    map.insert(".ts", "typescript");
    map.insert(".tsx", "typescript");
    map.insert(".java", "java");
    map.insert(".go", "go");
    map.insert(".c", "c");
    map.insert(".cpp", "cpp");
    map.insert(".cc", "cpp");
    map.insert(".cxx", "cpp");
    map.insert(".h", "c");
    map.insert(".hpp", "cpp");
    map.insert(".cs", "csharp");
    map.insert(".php", "php");
    map.insert(".rb", "ruby");
    map.insert(".swift", "swift");
    map.insert(".kt", "kotlin");
    map.insert(".kts", "kotlin");
    map.insert(".scala", "scala");
    map.insert(".sc", "scala");
    map.insert(".m", "objective-c");
    map.insert(".mm", "objective-c");
    map.insert(".sh", "shell");
    map.insert(".bash", "shell");
    map.insert(".zsh", "shell");
    map.insert(".fish", "shell");
    map.insert(".ps1", "powershell");
    map.insert(".sql", "sql");
    map.insert(".pl", "perl");
    map.insert(".pm", "perl");
    map.insert(".lua", "lua");
    map.insert(".r", "r");
    map.insert(".R", "r");
    map.insert(".jl", "julia");
    map.insert(".dart", "dart");
    map.insert(".nim", "nim");
    map.insert(".cr", "crystal");
    map.insert(".elm", "elm");
    map.insert(".erl", "erlang");
    map.insert(".hrl", "erlang");
    map.insert(".ex", "elixir");
    map.insert(".exs", "elixir");
    map.insert(".clj", "clojure");
    map.insert(".cljs", "clojure");
    map.insert(".cljc", "clojure");
    map.insert(".hs", "haskell");
    map.insert(".lhs", "haskell");
    map.insert(".fs", "fsharp");
    map.insert(".fsi", "fsharp");
    map.insert(".fsx", "fsharp");
    map.insert(".ml", "ocaml");
    map.insert(".mli", "ocaml");
    map.insert(".v", "verilog");
    map.insert(".vh", "verilog");
    map.insert(".vhd", "vhdl");
    map.insert(".sv", "systemverilog");
    map.insert(".svh", "systemverilog");
    map.insert(".cob", "cobol");
    map.insert(".cbl", "cobol");
    map.insert(".cpy", "cobol");
    map.insert(".f", "fortran");
    map.insert(".f90", "fortran");
    map.insert(".f95", "fortran");
    map.insert(".f03", "fortran");
    map.insert(".f08", "fortran");
    map.insert(".adb", "ada");
    map.insert(".ads", "ada");
    map.insert(".pas", "pascal");
    map.insert(".pp", "pascal");
    map.insert(".inc", "pascal");
    map.insert(".asm", "assembly");
    map.insert(".s", "assembly");
    map.insert(".S", "assembly");
    map.insert(".nasm", "assembly");
    map.insert(".makefile", "makefile");
    map.insert("Makefile", "makefile");
    map.insert(".mk", "makefile");
    map.insert(".cmake", "cmake");
    map.insert("CMakeLists.txt", "cmake");
    map.insert(".gradle", "gradle");
    map.insert("build.gradle", "gradle");
    map.insert("build.gradle.kts", "gradle");
    map.insert(".xml", "xml");
    map.insert(".html", "html");
    map.insert(".htm", "html");
    map.insert(".css", "css");
    map.insert(".scss", "scss");
    map.insert(".sass", "sass");
    map.insert(".less", "less");
    map.insert(".json", "json");
    map.insert(".yaml", "yaml");
    map.insert(".yml", "yaml");
    map.insert(".toml", "toml");
    map.insert(".ini", "ini");
    map.insert(".cfg", "config");
    map.insert(".conf", "config");
    map.insert(".md", "markdown");
    map.insert(".markdown", "markdown");
    map.insert(".rst", "rst");
    map.insert(".tex", "tex");
    map.insert(".bib", "bibtex");
    map.insert(".dockerfile", "dockerfile");
    map.insert("Dockerfile", "dockerfile");
    map.insert(".dockerignore", "dockerignore");
    map.insert(".gitignore", "gitignore");
    map.insert(".gitattributes", "gitattributes");
    map.insert(".gitmodules", "gitmodules");
    map.insert(".gitconfig", "gitconfig");
    map.insert(".editorconfig", "editorconfig");
    map.insert(".eslintrc", "eslint");
    map.insert(".eslintrc.js", "eslint");
    map.insert(".eslintrc.json", "eslint");
    map.insert(".eslintrc.yaml", "eslint");
    map.insert(".eslintrc.yml", "eslint");
    map.insert(".prettierrc", "prettier");
    map.insert(".prettierrc.js", "prettier");
    map.insert(".prettierrc.json", "prettier");
    map.insert(".prettierrc.yaml", "prettier");
    map.insert(".prettierrc.yml", "prettier");
    map.insert(".babelrc", "babel");
    map.insert(".babelrc.js", "babel");
    map.insert(".babelrc.json", "babel");
    map.insert(".tsconfig.json", "tsconfig");
    map.insert(".pylintrc", "pylint");
    map.insert(".flake8", "flake8");
    map.insert(".mypy.ini", "mypy");
    map.insert(".isort.cfg", "isort");
    map.insert("setup.py", "python");
    map.insert("setup.cfg", "python");
    map.insert("pyproject.toml", "python");
    map.insert("requirements.txt", "python");
    map.insert("Pipfile", "python");
    map.insert("poetry.lock", "python");
    map.insert(".npmignore", "npm");
    map.insert(".yarnignore", "yarn");
    map.insert("package.json", "npm");
    map.insert("package-lock.json", "npm");
    map.insert("yarn.lock", "yarn");
    map.insert("pnpm-lock.yaml", "pnpm");
    map.insert("Cargo.toml", "rust");
    map.insert("Cargo.lock", "rust");
    map.insert("go.mod", "go");
    map.insert("go.sum", "go");
    map.insert("Gopkg.toml", "go");
    map.insert("Gopkg.lock", "go");
    map.insert("composer.json", "php");
    map.insert("composer.lock", "php");
    map.insert("Gemfile", "ruby");
    map.insert("Gemfile.lock", "ruby");
    map.insert("Rakefile", "ruby");
    map.insert("Podfile", "ruby");
    map.insert("Podfile.lock", "ruby");
    map.insert(".gemspec", "ruby");
    map.insert("pubspec.yaml", "dart");
    map.insert("pubspec.lock", "dart");
    map.insert("mix.exs", "elixir");
    map.insert("mix.lock", "elixir");
    map.insert("rebar.config", "erlang");
    map.insert("project.clj", "clojure");
    map.insert("build.sbt", "scala");
    map.insert("pom.xml", "maven");
    map.insert("build.gradle", "gradle");
    map.insert("build.gradle.kts", "gradle");
    map.insert("settings.gradle", "gradle");
    map.insert("settings.gradle.kts", "gradle");
    map.insert("gradle.properties", "gradle");
    map.insert(".gitlab-ci.yml", "gitlab-ci");
    map.insert(".travis.yml", "travis");
    map.insert("appveyor.yml", "appveyor");
    map.insert("Jenkinsfile", "jenkins");
    map.insert("azure-pipelines.yml", "azure-pipelines");
    map.insert("circleci", "circleci");
    map.insert(".circleci", "circleci");
    map.insert("workflow.yml", "github-actions");
    map.insert(".github/workflows/*.yml", "github-actions");
    map.insert(".github/workflows/*.yaml", "github-actions");
    map.insert("terraform", "terraform");
    map.insert(".tf", "terraform");
    map.insert(".tfvars", "terraform");
    map.insert("hcl", "hcl");
    map.insert("nomad", "nomad");
    map.insert(".nomad", "nomad");
    map.insert("consul", "consul");
    map.insert(".hcl", "consul");
    map.insert("vault", "vault");
    map.insert(".vault", "vault");
    map.insert(".proto", "protobuf");
    map.insert(".graphql", "graphql");
    map.insert(".gql", "graphql");
    map.insert(".prisma", "prisma");
    map.insert(".pug", "pug");
    map.insert(".jade", "pug");
    map.insert(".hbs", "handlebars");
    map.insert(".handlebars", "handlebars");
    map.insert(".mustache", "mustache");
    map.insert(".ejs", "ejs");
    map.insert(".liquid", "liquid");
    map.insert(".twig", "twig");
    map.insert(".blade.php", "blade");
    map.insert(".volt", "volt");
    map.insert(".slim", "slim");
    map.insert(".erb", "erb");
    map.insert(".rhtml", "erb");
    map.insert(".haml", "haml");
    map.insert(".razor", "razor");
    map.insert(".aspx", "aspx");
    map.insert(".ascx", "aspx");
    map.insert(".master", "aspx");
    map.insert(".vue", "vue");
    map.insert(".svelte", "svelte");
    map.insert(".wasm", "wasm");
    map.insert(".wat", "wat");
    map.insert(".sol", "solidity");
    map.insert(".vy", "vyper");
    map.insert(".cairo", "cairo");
    map.insert(".move", "move");
    map.insert(".cap", "capsule");
    map.insert(".wit", "wit");
    map.insert(".wast", "wast");
    map.insert(".tla", "tla+");
    map.insert(".dfy", "dafny");
    map.insert(".spl", "sparkle");
    map.insert(".coq", "coq");
    map.insert(".v", "coq");
    map.insert(".lean", "lean");
    map.insert(".agda", "agda");
    map.insert(".idr", "idris");
    map.insert(".lidr", "idris");
    map.insert(".purescript", "purescript");
    map.insert(".purs", "purescript");
    map.insert(".elm", "elm");
    map.insert(".glsl", "glsl");
    map.insert(".vert", "glsl");
    map.insert(".frag", "glsl");
    map.insert(".hlsl", "hlsl");
    map.insert(".wgsl", "wgsl");
    map.insert(".metal", "metal");
    map.insert(".slang", "slang");
    map.insert(".spv", "spirv");
    map.insert(".d", "d");
    map.insert(".di", "d");
    map.insert(".nim", "nim");
    map.insert(".nims", "nim");
    map.insert(".cr", "crystal");
    map.insert(".ecr", "crystal");
    map.insert(".slang", "slang");
    map.insert(".zig", "zig");
    map.insert(".zon", "zig");
    map.insert(".odin", "odin");
    map.insert(".jai", "jai");
    map.insert(".pony", "pony");
    map.insert(".lobster", "lobster");
    map.insert(".wren", "wren");
    map.insert(".nelua", "nelua");
    map.insert(".fyr", "fyr");
    map.insert(".ferret", "ferret");
    map.insert(".gleam", "gleam");
    map.insert(".gleam", "gleam");
    map.insert(".inko", "inko");
    map.insert(".mojo", "mojo");
    map.insert(".wat", "wat");
    map.insert(".wit", "wit");
    map.insert(".wast", "wast");
    map.insert(".witx", "witx");
    map.insert(".wai", "wai");
    map.insert(".wai-bindgen", "wai-bindgen");
    map.insert(".webidl", "webidl");
    map.insert(".idl", "idl");
    map.insert(".odl", "idl");
    map.insert(".thrift", "thrift");
    map.insert(".avsc", "avro");
    map.insert(".avdl", "avro");
    map.insert(".avpr", "avro");
    map.insert(".proto3", "protobuf");
    map.insert(".protodevel", "protobuf");
    map.insert(".fbs", "flatbuffers");
    map.insert(".capnp", "capnproto");
    map.insert(".asn", "asn1");
    map.insert(".asn1", "asn1");
    map.insert(".der", "asn1");
    map.insert(".cer", "asn1");
    map.insert(".pem", "pem");
    map.insert(".crt", "pem");
    map.insert(".key", "key");
    map.insert(".p12", "p12");
    map.insert(".pfx", "p12");
    map.insert(".csr", "csr");
    map.insert(".tsr", "csr");
    map.insert(".der", "der");

    map
}

pub fn detect_language(file_path: &str) -> String {
    let lang_map = language_map();

    if let Some(file_name) = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
    {
        if let Some(lang) = lang_map.get(file_name) {
            return lang.to_string();
        }
    }

    if let Some(ext) = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
    {
        let ext_with_dot = format!(".{}", ext);
        if let Some(lang) = lang_map.get(ext_with_dot.as_str()) {
            return lang.to_string();
        }
    }

    "unknown".to_string()
}

#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub chunk_id: String,
    pub file_path: String,
    pub language: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

pub fn generate_chunk_id(file_path: &str, start_line: usize, end_line: usize) -> String {
    let input = format!("{}:{}-{}", file_path, start_line, end_line);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)[..16].to_string()
}

pub fn split_file(
    file_path: &str,
    content: &str,
    chunk_size: Option<usize>,
    overlap: Option<usize>,
) -> Vec<CodeChunk> {
    let chunk_size = chunk_size.unwrap_or_else(get_default_chunk_size);
    let overlap = overlap.unwrap_or_else(get_default_overlap);

    let lines: Vec<&str> = content.lines().collect();
    let language = detect_language(file_path);
    let mut chunks = Vec::new();

    if lines.is_empty() {
        return chunks;
    }

    let mut start = 0;

    while start < lines.len() {
        let end = std::cmp::min(start + chunk_size, lines.len());
        let chunk_content: String = lines[start..end].join("\n");

        let chunk_id = generate_chunk_id(file_path, start + 1, end);

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

        start = end - overlap;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("test.rs"), "rust");
        assert_eq!(detect_language("test.py"), "python");
        assert_eq!(detect_language("test.js"), "javascript");
        assert_eq!(detect_language("test.ts"), "typescript");
        assert_eq!(detect_language("test.go"), "go");
        assert_eq!(detect_language("Cargo.toml"), "rust");
        assert_eq!(detect_language("Makefile"), "makefile");
        assert_eq!(detect_language("unknown.xyz"), "unknown");
    }

    #[test]
    fn test_generate_chunk_id() {
        let id1 = generate_chunk_id("test.rs", 1, 50);
        let id2 = generate_chunk_id("test.rs", 1, 50);
        let id3 = generate_chunk_id("test.rs", 2, 51);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_split_file_basic() {
        let content = (1..=100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split_file("test.rs", &content, Some(50), Some(10));

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 50);
        assert_eq!(chunks[1].start_line, 41);
        assert_eq!(chunks[1].end_line, 90);
        assert_eq!(chunks[2].start_line, 81);
        assert_eq!(chunks[2].end_line, 100);
    }

    #[test]
    fn test_split_file_small() {
        let content = "line1\nline2\nline3";
        let chunks = split_file("test.rs", content, Some(50), Some(10));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
    }

    #[test]
    fn test_split_file_empty() {
        let chunks = split_file("test.rs", "", Some(50), Some(10));
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_language_map() {
        let map = language_map();
        assert!(map.contains_key(".rs"));
        assert!(map.contains_key(".py"));
        assert!(map.contains_key(".js"));
        assert!(map.contains_key("Cargo.toml"));
        assert!(map.contains_key("Makefile"));
    }
}
