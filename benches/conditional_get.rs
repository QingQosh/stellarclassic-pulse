use criterion::{criterion_group, criterion_main, Criterion};

// This is a placeholder for the benchmark
// Comparing 200 vs 304 response times.
pub fn bench_conditional_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("conditional_get");

    group.bench_function("fetch_200", |b| {
        b.iter(|| {
            // mock logic for 200 response
        })
    });

    group.bench_function("fetch_304", |b| {
        b.iter(|| {
            // mock logic for 304 response (only ETag check)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_conditional_get);
criterion_main!(benches);
