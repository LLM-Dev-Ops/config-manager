//! Performance benchmarks for cache operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use llm_config_cache::CacheManager;
use llm_config_core::{ConfigEntry, ConfigMetadata, ConfigValue, Environment};
use tempfile::TempDir;
use uuid::Uuid;

fn create_test_entry(namespace: &str, key: &str, env: Environment) -> ConfigEntry {
    ConfigEntry {
        id: Uuid::new_v4(),
        namespace: namespace.to_string(),
        key: key.to_string(),
        value: ConfigValue::String("benchmark-value".to_string()),
        environment: env,
        version: 1,
        metadata: ConfigMetadata {
            created_at: chrono::Utc::now(),
            created_by: "bench".to_string(),
            updated_at: chrono::Utc::now(),
            updated_by: "bench".to_string(),
            tags: vec![],
            description: None,
        },
    }
}

fn bench_cache_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_put");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let cache = CacheManager::new(size, temp_dir.path()).unwrap();
                    let entries: Vec<_> = (0..size)
                        .map(|i| {
                            create_test_entry(
                                "bench/ns",
                                &format!("key{}", i),
                                Environment::Development,
                            )
                        })
                        .collect();
                    (cache, entries)
                },
                |(cache, entries)| {
                    for entry in entries {
                        cache.put(entry).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_cache_get_l1_hit(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheManager::new(1000, temp_dir.path()).unwrap();

    // Pre-populate cache
    for i in 0..100 {
        let entry = create_test_entry("bench/ns", &format!("key{}", i), Environment::Development);
        cache.put(entry).unwrap();
    }

    c.bench_function("cache_get_l1_hit", |b| {
        b.iter(|| {
            cache
                .get(
                    black_box("bench/ns"),
                    black_box("key50"),
                    black_box("development"),
                )
                .unwrap()
        });
    });
}

fn bench_cache_get_l2_hit(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheManager::new(10, temp_dir.path()).unwrap(); // Small L1 to force L2 usage

    // Pre-populate cache with more entries than L1 can hold
    for i in 0..100 {
        let entry = create_test_entry("bench/ns", &format!("key{}", i), Environment::Development);
        cache.put(entry).unwrap();
    }

    // Clear L1 to force L2 access
    cache.clear_l1();

    c.bench_function("cache_get_l2_hit", |b| {
        b.iter(|| {
            cache
                .get(
                    black_box("bench/ns"),
                    black_box("key50"),
                    black_box("development"),
                )
                .unwrap()
        });
    });
}

fn bench_cache_invalidate(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_invalidate");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let cache = CacheManager::new(size, temp_dir.path()).unwrap();

                    // Pre-populate
                    for i in 0..size {
                        let entry = create_test_entry(
                            "bench/ns",
                            &format!("key{}", i),
                            Environment::Development,
                        );
                        cache.put(entry).unwrap();
                    }
                    cache
                },
                |cache| {
                    for i in 0..size {
                        cache
                            .invalidate("bench/ns", &format!("key{}", i), "development")
                            .unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_cache_mixed_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheManager::new(100, temp_dir.path()).unwrap();

    // Pre-populate with some data
    for i in 0..50 {
        let entry = create_test_entry("bench/ns", &format!("key{}", i), Environment::Development);
        cache.put(entry).unwrap();
    }

    c.bench_function("cache_mixed_ops", |b| {
        let mut counter = 50;
        b.iter(|| {
            // Mix of operations: 70% reads, 20% writes, 10% invalidations
            for i in 0..100 {
                match i % 10 {
                    0 => {
                        // Invalidate
                        let _ =
                            cache.invalidate("bench/ns", &format!("key{}", i % 50), "development");
                    }
                    1 | 2 => {
                        // Write
                        let entry = create_test_entry(
                            "bench/ns",
                            &format!("key{}", counter),
                            Environment::Development,
                        );
                        cache.put(entry).unwrap();
                        counter += 1;
                    }
                    _ => {
                        // Read
                        let _ = cache.get("bench/ns", &format!("key{}", i % 50), "development");
                    }
                }
            }
        });
    });
}

fn bench_cache_promotion(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let cache = CacheManager::new(10, temp_dir.path()).unwrap(); // Small L1

    // Fill cache with more entries than L1 can hold
    for i in 0..100 {
        let entry = create_test_entry("bench/ns", &format!("key{}", i), Environment::Development);
        cache.put(entry).unwrap();
    }

    cache.clear_l1();

    c.bench_function("cache_promotion_l2_to_l1", |b| {
        b.iter(|| {
            // Access entries that will be promoted from L2 to L1
            for i in 0..5 {
                cache
                    .get(
                        black_box("bench/ns"),
                        black_box(&format!("key{}", i * 20)),
                        black_box("development"),
                    )
                    .unwrap();
            }
            // Clear L1 for next iteration
            cache.clear_l1();
        });
    });
}

criterion_group!(
    benches,
    bench_cache_put,
    bench_cache_get_l1_hit,
    bench_cache_get_l2_hit,
    bench_cache_invalidate,
    bench_cache_mixed_operations,
    bench_cache_promotion
);
criterion_main!(benches);
