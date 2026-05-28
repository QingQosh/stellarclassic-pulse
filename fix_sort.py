with open('src/handlers.rs', 'r') as f:
    c = f.read()

helpers = """fn encode_cursor_tagged(tag: &str, value: &str, id: uuid::Uuid) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{}|{}|{}", tag, value, id))
}

fn decode_cursor_tagged(cursor: &str) -> Result<(String, String, uuid::Uuid), crate::error::AppError> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| crate::error::AppError::Validation("invalid cursor".to_string()))?;
    let s = std::str::from_utf8(&bytes)
        .map_err(|_| crate::error::AppError::Validation("invalid cursor".to_string()))?;
    if s.contains('|') {
        let mut parts = s.splitn(3, '|');
        let tag = parts.next().unwrap().to_string();
        let val = parts.next().unwrap().to_string();
        let id = uuid::Uuid::parse_str(parts.next().unwrap()).map_err(|_| crate::error::AppError::Validation("invalid cursor".to_string()))?;
        Ok((tag, val, id))
    } else {
        // legacy ledger-only cursor decoding
        let (ledger_str, id_str) = s
            .rsplit_once(':')
            .ok_or_else(|| crate::error::AppError::Validation("invalid cursor".to_string()))?;
        let id = uuid::Uuid::parse_str(id_str)
            .map_err(|_| crate::error::AppError::Validation("invalid cursor".to_string()))?;
        Ok(("ledger".to_string(), ledger_str.to_string(), id))
    }
}
"""

c = c.replace("fn encode_cursor(ledger: i64, id: uuid::Uuid) -> String {", helpers + "\nfn encode_cursor(ledger: i64, id: uuid::Uuid) -> String {")

with open('src/handlers.rs', 'w') as f:
    f.write(c)

