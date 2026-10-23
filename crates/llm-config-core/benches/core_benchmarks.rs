//! Performance benchmarks for core configuration operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use llm_config_core::{ConfigManager, ConfigValue, Environment};
use llm_config_crypto::{Algorithm, SecretKey};
use tempfile::TempDir;

fn bench_config_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_set");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    ConfigManager::new(temp_dir.path()).unwrap()
                },
                |manager| {
                    for i in 0..size {
                        manager
                            .set(
                                "benchmark/ns",
                                format!("key{}", i),
                                ConfigValue::String(format!("value{}", i)),
                                Environment::Development,
                                "bench-user",
                            )
                            .unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_config_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_get");

    // Setup: Create a manager with pre-populated data
    let temp_dir = TempDir::new().unwrap();
    let manager = ConfigManager::new(temp_dir.path()).unwrap();

    for i in 0..1000 {
        manager
            .set(
                "benchmark/ns",
                format!("key{}", i),
                ConfigValue::String(format!("value{}", i)),
                Environment::Development,
                "bench-user",
            )
            .unwrap();
    }

    group.bench_function("single_get", |b| {
        b.iter(|| {
            manager
                .get(
                    black_box("benchmark/ns"),
                    black_box("key500"),
                    Environment::Development,
                )
                .unwrap()
        });
    });

    group.finish();
}

fn bench_config_get_with_overrides(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let manager = ConfigManager::new(temp_dir.path()).unwrap();

    // Setup base and override configs
    manager
        .set(
            "benchmark/ns",
            "key",
            ConfigValue::String("base".to_string()),
            Environment::Base,
            "user",
        )
        .unwrap();

    manager
        .set(
            "benchmark/ns",
            "key",
            ConfigValue::String("production".to_string()),
            Environment::Production,
            "user",
        )
        .unwrap();

    c.bench_function("get_with_overrides", |b| {
        b.iter(|| {
            manager
                .get_with_overrides(
                    black_box("benchmark/ns"),
                    black_box("key"),
                    Environment::Production,
                )
                .unwrap()
        });
    });
}

fn bench_config_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_list");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let manager = ConfigManager::new(temp_dir.path()).unwrap();

            for i in 0..size {
                manager
                    .set(
                        "benchmark/ns",
                        format!("key{}", i),
                        ConfigValue::String(format!("value{}", i)),
                        Environment::Development,
                        "bench-user",
                    )
                    .unwrap();
            }

            b.iter(|| {
                manager
                    .list(black_box("benchmark/ns"), Environment::Development)
                    .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_secret_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("secrets");

    let temp_dir = TempDir::new().unwrap();
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
    let manager = ConfigManager::new(temp_dir.path())
        .unwrap()
        .with_encryption_key(key);

    let secret_data = b"super-secret-api-key-12345";

    group.bench_function("set_secret", |b| {
        let mut counter = 0;
        b.iter(|| {
            manager
                .set_secret(
                    "benchmark/secrets",
                    format!("secret{}", counter),
                    black_box(secret_data),
                    Environment::Production,
                    "bench-user",
                )
                .unwrap();
            counter += 1;
        });
    });

    // Pre-populate for get benchmark
    manager
        .set_secret(
            "benchmark/secrets",
            "test-secret",
            secret_data,
            Environment::Production,
            "bench-user",
        )
        .unwrap();

    group.bench_function("get_secret", |b| {
        b.iter(|| {
            manager
                .get_secret(
                    black_box("benchmark/secrets"),
                    black_box("test-secret"),
                    Environment::Production,
                )
                .unwrap()
        });
    });

    group.finish();
}

fn bench_versioning(c: &mut Criterion) {
    let mut group = c.benchmark_group("versioning");

    let temp_dir = TempDir::new().unwrap();
    let manager = ConfigManager::new(temp_dir.path()).unwrap();

    // Create initial version
    manager
        .set(
            "benchmark/ns",
            "versioned-key",
            ConfigValue::String("v1".to_string()),
            Environment::Development,
            "user",
        )
        .unwrap();

    // Create multiple versions
    for i in 2..=10 {
        manager
            .set(
                "benchmark/ns",
                "versioned-key",
                ConfigValue::String(format!("v{}", i)),
                Environment::Development,
                "user",
            )
            .unwrap();
    }

    group.bench_function("get_history", |b| {
        b.iter(|| {
            manager
                .get_history(
                    black_box("benchmark/ns"),
                    black_box("versioned-key"),
                    Environment::Development,
                )
                .unwrap()
        });
    });

    group.bench_function("rollback", |b| {
        b.iter(|| {
            manager
                .rollback(
                    black_box("benchmark/ns"),
                    black_box("versioned-key"),
                    Environment::Development,
                    black_box(1),
                )
                .unwrap()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_config_set,
    bench_config_get,
    bench_config_get_with_overrides,
    bench_config_list,
    bench_secret_operations,
    bench_versioning
);
criterion_main!(benches);
