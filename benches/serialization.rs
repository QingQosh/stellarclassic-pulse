use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::{json, Value};

fn serialize_simple_event(c: &mut Criterion) {
    c.bench_function("serialize_simple_event", |b| {
        let event = json!({
            "type": "contract_event",
            "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABC",
            "ledger": 1000,
            "tx_hash": "abc123"
        });

        b.iter(|| {
            serde_json::to_vec(black_box(&event))
        });
    });
}

fn serialize_complex_event(c: &mut Criterion) {
    c.bench_function("serialize_complex_event", |b| {
        let event = json!({
            "type": "contract_event",
            "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABC",
            "ledger": 1000,
            "tx_hash": "abc123",
            "data": {
                "field1": "value1",
                "field2": "value2",
                "field3": "value3",
                "nested": {
                    "inner1": "data1",
                    "inner2": "data2",
                    "inner3": "data3"
                },
                "array": [1, 2, 3, 4, 5],
                "large_string": "a".repeat(1000)
            }
        });

        b.iter(|| {
            serde_json::to_vec(black_box(&event))
        });
    });
}

fn serialize_batch_events(c: &mut Criterion) {
    c.bench_function("serialize_batch_events", |b| {
        let events: Vec<Value> = (0..100)
            .map(|i| {
                json!({
                    "type": "contract_event",
                    "contract_id": format!("C{:0>52}", i),
                    "ledger": 1000 + i,
                    "tx_hash": format!("hash_{}", i)
                })
            })
            .collect();

        b.iter(|| {
            for event in black_box(&events) {
                let _ = serde_json::to_vec(event);
            }
        });
    });
}

fn serialize_pretty_vs_compact(c: &mut Criterion) {
    let event = json!({
        "type": "contract_event",
        "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABC",
        "data": {
            "field1": "value1",
            "nested": {
                "inner1": "data1",
                "inner2": "data2"
            }
        }
    });

    c.bench_function("serialize_pretty", |b| {
        b.iter(|| {
            serde_json::to_string_pretty(black_box(&event))
        });
    });

    c.bench_function("serialize_compact", |b| {
        b.iter(|| {
            serde_json::to_vec(black_box(&event))
        });
    });
}

criterion_group!(
    benches,
    serialize_simple_event,
    serialize_complex_event,
    serialize_batch_events,
    serialize_pretty_vs_compact
);
criterion_main!(benches);
