import re

with open('src/handlers.rs', 'r') as f:
    content = f.read()

# Make sure all paginated handlers return impl IntoResponse and accept headers
handlers = ['get_events_by_contract', 'get_events_by_tx', 'get_events_by_tx_batch', 'get_events_by_ledger_hash', 'get_events_diff']

for h in handlers:
    # 1. Update signature to include `headers: HeaderMap` and return `impl IntoResponse`
    # Warning: this regex might need careful tuning
    sig_pattern = re.compile(rf'pub async fn {h}\b[^{{]*\)\s*->\s*Result<Json<Value>,\s*AppError>\s*{{')
    
    def sig_repl(m):
        orig = m.group(0)
        if 'headers: HeaderMap' not in orig:
            # inject headers before `) ->`
            orig = re.sub(r'\)', ',\n    headers: axum::http::HeaderMap,\n)', orig)
        return orig.replace('Result<Json<Value>, AppError>', 'Result<impl IntoResponse, AppError>')
    
    content = sig_pattern.sub(sig_repl, content)

    # 2. Add ETag computation and checking before returning Ok(Json(...))
    # We'll look for `Ok(Json(json!({...})))` or similar.
    # Note: this is risky if there are multiple return points.

with open('src/handlers.rs.new', 'w') as f:
    f.write(content)

