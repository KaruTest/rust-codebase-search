use code_search::{detect_language, hash_file_content, split_file};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_language_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("language_detection");

    let test_files = vec![
        "test.rs",
        "test.py",
        "test.js",
        "test.tsx",
        "test.go",
        "test.java",
        "test.cpp",
    ];

    for file in test_files {
        group.bench_with_input(BenchmarkId::from_parameter(file), file, |b, path| {
            b.iter(|| detect_language(black_box(path)));
        });
    }

    group.finish();
}

fn bench_file_splitting(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_splitting");

    // Create test content of different sizes
    let small_content = (1..=50)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let medium_content = (1..=500)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let large_content = (1..=5000)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    group.bench_function("small_50_lines", |b| {
        b.iter(|| split_file(black_box("test.rs"), black_box(&small_content), None, None));
    });

    group.bench_function("medium_500_lines", |b| {
        b.iter(|| split_file(black_box("test.rs"), black_box(&medium_content), None, None));
    });

    group.bench_function("large_5000_lines", |b| {
        b.iter(|| split_file(black_box("test.rs"), black_box(&large_content), None, None));
    });

    group.finish();
}

fn bench_hash_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_generation");

    let small_content = b"Small content";
    let medium_content = (0..1024).map(|_| b'a').collect::<Vec<_>>();
    let large_content = (0..10240).map(|_| b'a').collect::<Vec<_>>();

    group.bench_function("small", |b| {
        b.iter(|| hash_file_content(black_box(small_content)));
    });

    group.bench_function("medium_1kb", |b| {
        b.iter(|| hash_file_content(black_box(&medium_content)));
    });

    group.bench_function("large_10kb", |b| {
        b.iter(|| hash_file_content(black_box(&large_content)));
    });

    group.finish();
}

fn bench_chunk_id_generation(c: &mut Criterion) {
    use code_search::generate_chunk_id;

    let mut group = c.benchmark_group("chunk_id_generation");

    group.bench_function("generate_id", |b| {
        b.iter(|| generate_chunk_id(black_box("src/main.rs"), black_box(1), black_box(50)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_language_detection,
    bench_file_splitting,
    bench_hash_generation,
    bench_chunk_id_generation
);
criterion_main!(benches);
