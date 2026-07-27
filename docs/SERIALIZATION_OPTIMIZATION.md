# JSON Serialization Optimization (Issue #687)

## Overview

This document describes the JSON serialization optimizations implemented to improve event data serialization performance in SorobanPulse.

## Optimization Strategies

### 1. Serialization Caching

The `SerializedEventCache` provides an in-memory cache of pre-serialized event data with a configurable TTL. This eliminates repeated serialization of the same event data across multiple requests.

**Benefits:**
- Reduces CPU overhead by avoiding redundant serialization operations
- Faster response times for frequently accessed events
- Configurable cache size and TTL to balance memory usage

**Usage:**
```rust
let cache = SerializedEventCache::new(max_capacity, ttl_secs);
let serialized = cache
    .get_or_serialize(key, &event, |v| serde_json::to_vec(v))
    .await?;
```

### 2. Compact Serialization

The `serialize_compact()` function uses pre-allocated buffers to reduce memory allocations during serialization.

**Benefits:**
- Fewer memory allocations
- Predictable performance characteristics
- Lower memory fragmentation

### 3. Metrics and Monitoring

Comprehensive metrics are collected for serialization operations:

- `soroban_pulse_serialization_cache_hits_total` - Number of cache hits
- `soroban_pulse_serialization_cache_misses_total` - Number of cache misses  
- `soroban_pulse_serialization_time_us` - Serialization time histogram

## Performance Benchmarks

Run serialization benchmarks with:
```bash
cargo bench --bench serialization
```

### Benchmark Results

The following operations are benchmarked:
1. **Simple Event Serialization** - Small event with basic fields
2. **Complex Event Serialization** - Nested objects and large strings
3. **Batch Serialization** - 100 events serialized sequentially
4. **Pretty vs Compact** - Performance comparison of formatting styles

## Cache Configuration

The `SerializedEventCache` should be configured based on:

- **max_capacity**: Number of events to cache (default: 10,000)
- **ttl_secs**: Cache time-to-live in seconds (default: 300)

Recommended settings:
- For high-volume endpoints: 10,000-100,000 capacity, 60-300s TTL
- For low-volume endpoints: 1,000-10,000 capacity, 300-3600s TTL

## Monitoring

Monitor serialization performance via Prometheus:

```
# Cache hit ratio
soroban_pulse_serialization_cache_hits_total / 
  (soroban_pulse_serialization_cache_hits_total + soroban_pulse_serialization_cache_misses_total)

# P95 serialization time
histogram_quantile(0.95, rate(soroban_pulse_serialization_time_us[5m]))
```

## Future Optimizations

1. **SIMD-JSON Integration** - Evaluate simd-json for faster serialization on compatible CPUs
2. **Adaptive Caching** - Automatically adjust cache size based on hit rates
3. **Compression** - Add optional gzip compression for large responses
4. **Custom Serializers** - Implement custom serializers for specific event types

## Implementation Notes

- Cache invalidation is automatic via TTL
- Thread-safe implementation using `moka::future::Cache`
- Metrics are recorded asynchronously without blocking
- All serialization errors are propagated to callers

## Testing

Run unit tests with:
```bash
cargo test serialization_cache
```

The test suite covers:
- Cache hit/miss scenarios
- Serialization correctness
- Metric recording
- Compact serialization efficiency
