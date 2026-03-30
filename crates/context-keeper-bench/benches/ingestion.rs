use criterion::{criterion_group, criterion_main, Criterion};

fn ingestion_benchmark(_c: &mut Criterion) {
    // TODO: Implement ingestion benchmarks
}

criterion_group!(benches, ingestion_benchmark);
criterion_main!(benches);
