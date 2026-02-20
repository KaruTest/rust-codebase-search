#[cfg(test)]
mod integration_tests {
    use code_search::{
        manifest,
        splitter::{detect_language, generate_chunk_id, split_file},
    };
    use std::path::Path;

    #[test]
    fn test_file_hashing() {
        let content = b"test content";
        let hash1 = manifest::hash_file_content(content);
        let hash2 = manifest::hash_file_content(content);

        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_eq!(hash1.len(), 16, "Hash should be 16 characters");

        let different_content = b"different content";
        let hash3 = manifest::hash_file_content(different_content);

        assert_ne!(
            hash1, hash3,
            "Different content should produce different hash"
        );
    }

    #[test]
    fn test_chunk_id_generation() {
        let id1 = generate_chunk_id("src/main.rs", 1, 50);
        let id2 = generate_chunk_id("src/main.rs", 1, 50);
        let id3 = generate_chunk_id("src/main.rs", 2, 51);

        assert_eq!(id1, id2, "Same parameters should produce same ID");
        assert_ne!(id1, id3, "Different parameters should produce different ID");
    }

    #[test]
    fn test_codebase_hash() {
        let path1 = Path::new("/path/to/codebase1");
        let path2 = Path::new("/path/to/codebase1");

        let hash1 = manifest::get_codebase_hash(path1);
        let hash2 = manifest::get_codebase_hash(path2);

        assert_eq!(hash1, hash2, "Same path should produce same hash");
        assert_eq!(hash1.len(), 16, "Hash should be 16 characters");
    }

    #[test]
    fn test_language_detection_comprehensive() {
        let test_cases = vec![
            ("file.rs", "rust"),
            ("file.py", "python"),
            ("file.js", "javascript"),
            ("file.tsx", "typescript"),
            ("file.go", "go"),
            ("file.java", "java"),
            ("file.cpp", "cpp"),
            ("file.h", "c"),
            ("file.rb", "ruby"),
            ("file.php", "php"),
            ("file.swift", "swift"),
            ("file.kt", "kotlin"),
            ("file.scala", "scala"),
            ("file.lua", "lua"),
        ];

        for (filename, expected_lang) in test_cases {
            let detected = detect_language(filename);
            assert_eq!(
                detected, expected_lang,
                "Expected {} for {}, got {:?}",
                expected_lang, filename, detected
            );
        }
    }

    #[test]
    fn test_chunk_overlap() {
        let content = (1..=100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split_file("test.rs", &content, Some(50), Some(10));

        assert!(chunks.len() > 1, "Should have multiple chunks with overlap");

        for i in 0..chunks.len() - 1 {
            let current_end = chunks[i].end_line;
            let next_start = chunks[i + 1].start_line;
            let overlap = current_end - next_start + 1;

            assert_eq!(overlap, 10, "Should have 10 lines of overlap");
        }
    }

    #[test]
    fn test_empty_file_handling() {
        let chunks = split_file("test.rs", "", None, None);
        assert!(chunks.is_empty(), "Empty file should produce no chunks");
    }

    #[test]
    fn test_chunk_size_boundaries() {
        let exact_size = (1..=50)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split_file("test.rs", &exact_size, Some(50), Some(10));
        assert_eq!(chunks.len(), 1, "50 lines should produce exactly 1 chunk");

        let one_over = (1..=51)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split_file("test.rs", &one_over, Some(50), Some(10));
        assert_eq!(
            chunks.len(),
            2,
            "51 lines should produce 2 chunks with overlap"
        );
    }
}
