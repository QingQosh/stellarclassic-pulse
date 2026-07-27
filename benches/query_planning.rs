use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn query_planning_simple_query(c: &mut Criterion) {
    c.bench_function("query_planning_simple_select", |b| {
        b.iter(|| {
            let query = black_box("SELECT id, type, ledger FROM events WHERE id = $1");
            let hash = compute_query_hash(query);
            let _ = black_box(hash);
        });
    });
}

fn query_planning_complex_query(c: &mut Criterion) {
    c.bench_function("query_planning_complex_join", |b| {
        b.iter(|| {
            let query = black_box(
                "SELECT e.id, e.type, c.name FROM events e \
                 JOIN contracts c ON e.contract_id = c.id \
                 WHERE e.ledger > $1 AND c.status = $2 \
                 ORDER BY e.ledger DESC LIMIT $3"
            );
            let hash = compute_query_hash(query);
            let _ = black_box(hash);
        });
    });
}

fn query_planning_parameterized_queries(c: &mut Criterion) {
    c.bench_function("query_planning_1000_parameterized", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let query = format!(
                    "SELECT * FROM events WHERE id = {} AND ledger > {} AND status = '{}'",
                    i, 1000 + i, "active"
                );
                let _ = black_box(compute_query_hash(&query));
            }
        });
    });
}

fn query_cache_lookup_performance(c: &mut Criterion) {
    c.bench_function("query_cache_hashmap_lookup", |b| {
        let mut cache = std::collections::HashMap::new();

        for i in 0..1000 {
            let query = format!("SELECT * FROM events WHERE id = {}", i);
            cache.insert(compute_query_hash(&query), format!("plan_{}", i));
        }

        b.iter(|| {
            for i in 0..1000 {
                let query = format!("SELECT * FROM events WHERE id = {}", i);
                let _ = black_box(cache.get(&compute_query_hash(black_box(&query))));
            }
        });
    });
}

fn query_explain_parsing(c: &mut Criterion) {
    c.bench_function("query_explain_json_parsing", |b| {
        let json = black_box(r#"
[
  {
    "Plan": {
      "Node Type": "Seq Scan",
      "Relation Name": "events",
      "Total Cost": 100.50,
      "Startup Cost": 0.00,
      "Estimated Rows": 1000,
      "Filter": "(status = 'active')"
    },
    "Planning Time": 0.542
  }
]
"#);

        b.iter(|| {
            let _: serde_json::Value = serde_json::from_str(black_box(json)).unwrap();
        });
    });
}

fn prepared_statement_overhead(c: &mut Criterion) {
    c.bench_function("prepared_statement_creation", |b| {
        b.iter(|| {
            let query = black_box("SELECT * FROM events WHERE id = $1");
            let name = format!("stmt_{}", compute_query_hash(query));
            let _ = black_box(name);
        });
    });
}

fn compute_query_hash(query: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    format!("{:x}", hasher.finalize())[0..8].to_string()
}

criterion_group!(
    benches,
    query_planning_simple_query,
    query_planning_complex_query,
    query_planning_parameterized_queries,
    query_cache_lookup_performance,
    query_explain_parsing,
    prepared_statement_overhead
);
criterion_main!(benches);
