use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::json;

fn memory_footprint_batch_response(c: &mut Criterion) {
    c.bench_function("memory_footprint_batch_1000_events", |b| {
        b.iter(|| {
            let events: Vec<_> = (0..1000)
                .map(|i| {
                    json!({
                        "type": "contract_event",
                        "ledger": 1000 + i,
                        "tx_hash": format!("hash_{}", i),
                        "data": {"field": "value"}
                    })
                })
                .collect();

            let _serialized = serde_json::to_vec(black_box(&events)).unwrap();
        });
    });
}

fn memory_footprint_large_events(c: &mut Criterion) {
    c.bench_function("memory_footprint_large_events", |b| {
        b.iter(|| {
            let events: Vec<_> = (0..100)
                .map(|i| {
                    json!({
                        "type": "contract_event",
                        "ledger": 1000 + i,
                        "tx_hash": format!("hash_{}", i),
                        "data": {
                            "large_field": "x".repeat(10000),
                            "nested": {
                                "field1": "value1",
                                "field2": "value2",
                            }
                        }
                    })
                })
                .collect();

            let _serialized = serde_json::to_vec(black_box(&events)).unwrap();
        });
    });
}

fn chunked_encoding_overhead(c: &mut Criterion) {
    c.bench_function("chunked_encoding_overhead_10000_items", |b| {
        b.iter(|| {
            let mut buffer = Vec::with_capacity(100_000);
            buffer.push(b'[');

            for i in 0..10000 {
                let item = json!({"id": i});
                if i > 0 {
                    buffer.push(b',');
                }
                let json_bytes = serde_json::to_vec(black_box(&item)).unwrap();
                buffer.extend_from_slice(&json_bytes);
            }

            buffer.push(b']');
            let _ = black_box(buffer);
        });
    });
}

criterion_group!(
    benches,
    memory_footprint_batch_response,
    memory_footprint_large_events,
    chunked_encoding_overhead
);
criterion_main!(benches);
