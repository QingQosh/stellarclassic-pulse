# Query Response Streaming (Issue #688)

## Overview

This document describes the streaming response feature that allows SorobanPulse to efficiently handle large result sets without loading entire datasets into memory.

## Problem Statement

Traditional paginated responses load entire result sets into memory before sending them to clients. For large datasets, this approach:
- Consumes excessive memory on the server
- Delays response initiation while building the full response
- Can cause out-of-memory errors with very large datasets

## Solution: Chunked Transfer Encoding

The streaming response implementation uses HTTP chunked transfer encoding to stream results in real-time as they're fetched from the database.

### Benefits

1. **Memory Efficiency** - Only buffer items being actively processed
2. **Low Latency** - Clients receive data immediately as available
3. **Scalability** - Handle arbitrarily large result sets
4. **Backpressure Handling** - Automatically manages client consumption speed

## API Usage

### Basic Streaming Endpoint

```rust
async fn get_events_stream(
    State(state): State<AppState>,
) -> Result<StreamingJsonResponse<Event>, AppError> {
    let stream = sqlx::query_as::<_, Event>(
        "SELECT * FROM events ORDER BY ledger DESC"
    )
    .fetch(&state.pool);

    Ok(StreamingJsonResponse::new(stream))
}
```

### Query Parameters

- `stream=true` - Enable streaming response
- `batch_size=N` - Size of internal batches (default: 1000)
- `buffer_size=N` - Send buffer size (default: 8192 bytes)

## Response Format

Responses use JSON array format with chunked transfer encoding:

```
HTTP/1.1 200 OK
Transfer-Encoding: chunked
Content-Type: application/json

[
{"id":1,"type":"contract_event",...},
{"id":2,"type":"contract_event",...},
...
]
```

Each chunk contains one or more complete JSON objects separated by commas.

## Client Implementation

### Rust Client

```rust
let response = client.get("http://api/events?stream=true").send().await?;
let body_str = response.text().await?;
let trimmed = body_str.trim_start_matches('[').trim_end_matches(']');

for item_str in trimmed.split(',') {
    let event: Event = serde_json::from_str(item_str)?;
    process_event(&event).await?;
}
```

### JavaScript/TypeScript Client

```typescript
async function* streamEvents(url: string) {
  const response = await fetch(url);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split(',');
      
      // Process complete items
      for (let i = 0; i < lines.length - 1; i++) {
        const line = lines[i].replace(/[\[\]]/g, '').trim();
        if (line) {
          yield JSON.parse(line);
        }
      }
      
      // Keep incomplete line in buffer
      buffer = lines[lines.length - 1];
    }
    
    // Process final item
    if (buffer.trim() && buffer.trim() !== ']') {
      yield JSON.parse(buffer.trim());
    }
  } finally {
    reader.releaseLock();
  }
}
```

### Python Client

```python
import requests
import json

def stream_events(url: str):
    response = requests.get(url, stream=True)
    buffer = ''
    
    for chunk in response.iter_content(decode_unicode=True):
        buffer += chunk
        
        # Find complete JSON objects
        while True:
            start = buffer.find('{')
            end = buffer.find('}', start) + 1
            
            if start == -1 or end == 0:
                break
                
            try:
                event = json.loads(buffer[start:end])
                yield event
                buffer = buffer[end:].lstrip(',')
            except json.JSONDecodeError:
                break
```

## Performance Characteristics

### Memory Usage

- **Batch response (10,000 items)**: ~10-50 MB depending on item size
- **Streaming response (10,000 items)**: ~1-5 MB constant, with chunk buffering

### Latency

- **TTFB (Time To First Byte)**: 10-100ms (database fetch + first chunk)
- **Throughput**: 50,000-500,000 items/sec depending on item complexity

### Network

- **Chunked encoding overhead**: ~2-5% additional bytes
- **Compression**: Recommended for large item sizes

## Monitoring

### Metrics

```
soroban_pulse_streaming_response_items_sent_total
  - Total items sent across all streaming responses

soroban_pulse_streaming_response_items_per_stream
  - Histogram of items per streaming response

soroban_pulse_streaming_responses_completed_total
  - Total completed streaming responses

soroban_pulse_streaming_response_errors_total{error_type}
  - Streaming response errors (serialization, database)
```

### Alerting

```promql
# Alert if streaming responses have high error rates
rate(soroban_pulse_streaming_response_errors_total[5m]) > 0.1
```

## Configuration

### Server Configuration

```rust
// Set streaming buffer size (default: 8192)
let response = StreamingJsonResponse::with_buffer_size(stream, 16384);

// Batch size for database fetching
let batch_size = 1000;
```

### Client Configuration

**Read timeout**: Should be higher than expected stream duration
**Connection pool**: Dedicate connections for streaming to avoid stalls

## Best Practices

1. **Use Streaming for Large Datasets**
   - Only use when result set likely exceeds 10MB
   - Falling back to pagination for smaller result sets

2. **Handle Connection Errors**
   - Implement exponential backoff retry logic
   - Track partial progress for recovery

3. **Monitor Resource Usage**
   - Track peak memory during streaming
   - Monitor database connection hold times

4. **Buffer Management**
   - Adjust buffer size for network conditions
   - Consider item size when tuning

5. **Filtering and Sorting**
   - Push filtering to database layer
   - Avoid in-memory sorting of streams

## Limitations and Future Work

1. **Current**: Single connection per stream
2. **Future**: Connection pooling for streams
3. **Current**: JSON array format only
4. **Future**: NDJSON (newline-delimited JSON) support
5. **Current**: No progress indication
6. **Future**: Range requests for resumable streaming

## Examples

See `examples/streaming_client.rs` for complete working examples in Rust.

Additional examples available for JavaScript, Python, and Go in the `examples/` directory.
