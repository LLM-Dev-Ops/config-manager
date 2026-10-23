//! Performance benchmarks for cryptographic operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use llm_config_crypto::{decrypt, encrypt, Algorithm, SecretKey};

fn bench_key_generation(c: &mut Criterion) {
    c.bench_function("key_generation_aes256gcm", |b| {
        b.iter(|| SecretKey::generate(black_box(Algorithm::Aes256Gcm)).unwrap());
    });
}

fn bench_encryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("encryption");

    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();

    // Benchmark different payload sizes
    for size in [16, 256, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("aes256gcm", size), size, |b, _size| {
            b.iter(|| encrypt(&key, black_box(&data), None).unwrap());
        });
    }

    group.finish();
}

fn bench_decryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("decryption");

    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();

    // Benchmark different payload sizes
    for size in [16, 256, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        let encrypted = encrypt(&key, &data, None).unwrap();

        group.bench_with_input(BenchmarkId::new("aes256gcm", size), size, |b, _size| {
            b.iter(|| decrypt(&key, black_box(&encrypted)).unwrap());
        });
    }

    group.finish();
}

fn bench_encrypt_decrypt_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("encrypt_decrypt_roundtrip");

    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();

    for size in [256, 1024, 4096].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("aes256gcm", size), size, |b, _size| {
            b.iter(|| {
                let encrypted = encrypt(&key, black_box(&data), None).unwrap();
                decrypt(&key, &encrypted).unwrap()
            });
        });
    }

    group.finish();
}

fn bench_encryption_with_aad(c: &mut Criterion) {
    let mut group = c.benchmark_group("encryption_with_aad");

    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
    let data = vec![0u8; 1024];
    let aad = "additional authenticated data";

    group.throughput(Throughput::Bytes(1024));
    group.bench_function("with_aad", |b| {
        b.iter(|| encrypt(&key, black_box(&data), Some(black_box(aad))).unwrap());
    });

    group.bench_function("without_aad", |b| {
        b.iter(|| encrypt(&key, black_box(&data), None).unwrap());
    });

    group.finish();
}

fn bench_concurrent_encryption(c: &mut Criterion) {
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
    let data = vec![0u8; 1024];

    c.bench_function("sequential_10_encryptions", |b| {
        b.iter(|| {
            for _ in 0..10 {
                encrypt(&key, black_box(&data), None).unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_key_generation,
    bench_encryption,
    bench_decryption,
    bench_encrypt_decrypt_roundtrip,
    bench_encryption_with_aad,
    bench_concurrent_encryption
);
criterion_main!(benches);
