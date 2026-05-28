with open('src/handlers.rs', 'r') as f:
    c = f.read()

# 1. replace `dir` and Add `sort_by` processing
c = c.replace(
"""    let dir = params
        .sort
        .unwrap_or(crate::models::SortOrder::Desc)
        .as_sql();

    // Cursor-based path
    if let Some(ref cursor_str) = params.cursor {
        let (cursor_ledger, cursor_id) = decode_cursor(cursor_str)?;

        // For DESC (default): rows where (ledger, id) < cursor
        // For ASC: rows where (ledger, id) > cursor
        let cursor_op = if params.sort == Some(crate::models::SortOrder::Asc) {
            ">"
        } else {
            "<"
        };

        let mut conditions: Vec<String> = vec![format!("(ledger, id) {cursor_op} ($1, $2)")];
        let mut bind_idx: i32 = 3;""",
"""    let sort_order = params.sort.unwrap_or(crate::models::SortOrder::Desc);
    let dir = sort_order.as_sql();
    let sort_by = params.sort_by.unwrap_or(crate::models::SortBy::Ledger);
    let sort_col = sort_by.as_sql_col();

    // Cursor-based path
    if let Some(ref cursor_str) = params.cursor {
        let (cursor_tag, cursor_val_text, cursor_id) = decode_cursor_tagged(cursor_str)?;
        if cursor_tag != sort_by.as_tag() {
            return Err(crate::error::AppError::Validation("cursor sort column does not match sort_by".to_string()));
        }

        let cursor_op = if sort_order == crate::models::SortOrder::Asc {
            ">"
        } else {
            "<"
        };

        let mut conditions: Vec<String> = vec![format!("({col}, id) {op} ($1, $2)", col = sort_col, op = cursor_op)];
        let mut bind_idx: i32 = 3;""")

# 2. replace select_cols additions (ledger -> sort_col)
c = c.replace(
"""        let mut select_cols = columns.to_vec();
        if !select_cols.contains(&"ledger") {
            select_cols.push("ledger");
        }
        if !select_cols.contains(&"id") {
            select_cols.push("id");
        }
        if !select_cols.contains(&"created_at") {
            select_cols.push("created_at");
        }

        let query_str = format!(
            "SELECT {} FROM events {} ORDER BY ledger {dir}, id {dir} LIMIT ${}",
            select_cols.join(", "),
            where_clause,
            bind_idx,
        );

        let mut q = sqlx::query(&query_str).bind(cursor_ledger).bind(cursor_id);""",
"""        let mut select_cols = columns.to_vec();
        if !select_cols.contains(&sort_col) {
            select_cols.push(sort_col);
        }
        if !select_cols.contains(&"id") {
            select_cols.push("id");
        }
        if !select_cols.contains(&"created_at") {
            select_cols.push("created_at");
        }

        let query_str = format!(
            "SELECT {} FROM events {} ORDER BY {col} {dir}, id {dir} LIMIT ${}",
            select_cols.join(", "),
            where_clause,
            col = sort_col,
            dir = dir,
            bind_idx,
        );

        let mut q = sqlx::query(&query_str);
        match sort_by {
            crate::models::SortBy::Ledger => {
                let val = cursor_val_text.parse::<i64>().map_err(|_| crate::error::AppError::Validation("invalid ledger cursor".to_string()))?;
                q = q.bind(val).bind(cursor_id);
            }
            crate::models::SortBy::Timestamp | crate::models::SortBy::CreatedAt => {
                let ts = cursor_val_text.parse::<chrono::DateTime<chrono::Utc>>().map_err(|_| crate::error::AppError::Validation("invalid timestamp cursor".to_string()))?;
                q = q.bind(ts).bind(cursor_id);
            }
        }""")

# 3. replace next_cursor formatting
c = c.replace(
"""        let next_cursor = if has_more {
            let last = rows.last().unwrap();
            let last_ledger: i64 = last.try_get("ledger")?;
            let last_id: Uuid = last.try_get("id")?;
            Some(encode_cursor(last_ledger, last_id))
        } else {""",
"""        let next_cursor = if has_more {
            let last = rows.last().unwrap();
            let last_id: uuid::Uuid = last.try_get("id")?;
            let last_val_text = match sort_by {
                crate::models::SortBy::Ledger => {
                    let v: i64 = last.try_get("ledger")?;
                    v.to_string()
                }
                crate::models::SortBy::Timestamp => {
                    let v: chrono::DateTime<chrono::Utc> = last.try_get("timestamp")?;
                    v.to_rfc3339()
                }
                crate::models::SortBy::CreatedAt => {
                    let v: chrono::DateTime<chrono::Utc> = last.try_get("created_at")?;
                    v.to_rfc3339()
                }
            };
            Some(encode_cursor_tagged(sort_by.as_tag(), &last_val_text, last_id))
        } else {""")

# 4. update offset path query execution
c = c.replace(
"""    let mut conditions: Vec<String> = Vec::new();
    let mut bind_idx: i32 = 1;""",
"""    let mut conditions: Vec<String> = Vec::new();
    let mut bind_idx: i32 = 1;""")

c = c.replace(
"""    let mut select_cols = columns.to_vec();
    if !select_cols.contains(&"ledger") {
        select_cols.push("ledger");
    }
    if !select_cols.contains(&"id") {
        select_cols.push("id");
    }
    // Always fetch created_at for ETag computation
    if !select_cols.contains(&"created_at") {
        select_cols.push("created_at");
    }

    let query_str = format!(
        "SELECT {} FROM events {} ORDER BY ledger {dir}, id {dir} LIMIT ${} OFFSET ${}",
        select_cols.join(", "),
        where_clause,
        bind_idx,
        bind_idx + 1,
    );""",
"""    let mut select_cols = columns.to_vec();
    if !select_cols.contains(&sort_col) {
        select_cols.push(sort_col);
    }
    if !select_cols.contains(&"id") {
        select_cols.push("id");
    }
    // Always fetch created_at for ETag computation
    if !select_cols.contains(&"created_at") {
        select_cols.push("created_at");
    }

    let query_str = format!(
        "SELECT {} FROM events {} ORDER BY {col} {dir}, id {dir} LIMIT ${} OFFSET ${}",
        select_cols.join(", "),
        where_clause,
        col = sort_col,
        dir = dir,
        bind_idx,
        bind_idx + 1,
    );""")

c = c.replace(
"""    let next_cursor = if has_more {
        let last = rows.last().unwrap();
        let last_ledger: i64 = last.try_get("ledger")?;
        let last_id: Uuid = last.try_get("id")?;
        Some(encode_cursor(last_ledger, last_id))
    } else {""",
"""    let next_cursor = if has_more {
        let last = rows.last().unwrap();
        let last_id: uuid::Uuid = last.try_get("id")?;
        let last_val_text = match sort_by {
            crate::models::SortBy::Ledger => {
                let v: i64 = last.try_get("ledger")?;
                v.to_string()
            }
            crate::models::SortBy::Timestamp => {
                let v: chrono::DateTime<chrono::Utc> = last.try_get("timestamp")?;
                v.to_rfc3339()
            }
            crate::models::SortBy::CreatedAt => {
                let v: chrono::DateTime<chrono::Utc> = last.try_get("created_at")?;
                v.to_rfc3339()
            }
        };
        Some(encode_cursor_tagged(sort_by.as_tag(), &last_val_text, last_id))
    } else {""")

with open('src/handlers.rs', 'w') as f:
    f.write(c)
