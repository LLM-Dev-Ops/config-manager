use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use llm_config_crypto::{decrypt, encrypt, Algorithm, SecretKey};

fn encryption_benchmark(c: &mut Criterion) {
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();

    let mut group = c.benchmark_group("encryption");

    for size in [100, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let encrypted = encrypt(&key, black_box(&data), None).unwrap();
                black_box(encrypted);
            });
        });
    }

    group.finish();
}

fn decryption_benchmark(c: &mut Criterion) {
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();

    let mut group = c.benchmark_group("decryption");

    for size in [100, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];
        let encrypted = encrypt(&key, &data, None).unwrap();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let decrypted = decrypt(&key, black_box(&encrypted)).unwrap();
                black_box(decrypted);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, encryption_benchmark, decryption_benchmark);
criterion_main!(benches);
