with open('src/handlers.rs', 'r') as f:
    c = f.read()

# 1. get_events -> add created_at
c = c.replace(
"""        let mut select_cols = columns.to_vec();
        if !select_cols.contains(&"ledger") {
            select_cols.push("ledger");
        }
        if !select_cols.contains(&"id") {
            select_cols.push("id");
        }""",
"""        let mut select_cols = columns.to_vec();
        if !select_cols.contains(&"ledger") {
            select_cols.push("ledger");
        }
        if !select_cols.contains(&"id") {
            select_cols.push("id");
        }
        if !select_cols.contains(&"created_at") {
            select_cols.push("created_at");
        }""")

# 2. get_events -> ETag generation in cursor path
c = c.replace(
"""        let events = rows_to_json(
            &rows,
            &columns,
            state.encryption_key.as_ref(),
            state.encryption_key_old.as_ref(),
            params.compact.unwrap_or(false),
        )?;

        let want_ndjson = accepts_ndjson(&headers);
        if want_ndjson {
            return Ok(ndjson_response(events.into_iter()));
        }

        let compact_mode = params.compact.unwrap_or(false);
        let mut resp = json_response(json!({
            "data": events,
            "next_cursor": next_cursor,
            "limit": limit,
        }));
        if compact_mode {
            resp.headers_mut().insert(
                "X-Event-Data-Encoding",
                axum::http::HeaderValue::from_static("gzip+base64"),
            );
        }
        return Ok(resp.into_response());
    }

    // Offset-based path (deprecated fallback)""",
"""        let events = rows_to_json(
            &rows,
            &columns,
            state.encryption_key.as_ref(),
            state.encryption_key_old.as_ref(),
            params.compact.unwrap_or(false),
        )?;

        let etag = rows.first().and_then(|row| {
            let id: Option<uuid::Uuid> = row.try_get("id").ok();
            let created_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("created_at").ok();
            id.zip(created_at).map(|(id, ca)| compute_etag(&id, &ca, None))
        });

        if let Some(ref tag) = etag {
            if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
                if inm == tag {
                    let resp = axum::http::Response::builder()
                        .status(axum::http::StatusCode::NOT_MODIFIED)
                        .header("ETag", tag.as_str())
                        .header("Cache-Control", "no-cache")
                        .body(axum::body::Body::empty())
                        .unwrap();
                    return Ok(resp.into_response());
                }
            }
        }

        let want_ndjson = accepts_ndjson(&headers);
        if want_ndjson {
            let mut resp = ndjson_response(events.into_iter()).into_response();
            if let Some(ref tag) = etag {
                resp.headers_mut().insert("ETag", tag.parse().unwrap());
                resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());
            }
            return Ok(resp);
        }

        let compact_mode = params.compact.unwrap_or(false);
        let mut resp = json_response(json!({
            "data": events,
            "next_cursor": next_cursor,
            "limit": limit,
        }));
        if compact_mode {
            resp.headers_mut().insert(
                "X-Event-Data-Encoding",
                axum::http::HeaderValue::from_static("gzip+base64"),
            );
        }
        if let Some(ref tag) = etag {
            resp.headers_mut().insert("ETag", tag.parse().unwrap());
            resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());
        }
        return Ok(resp.into_response());
    }

    // Offset-based path (deprecated fallback)""")

# 3. get_events_by_contract signature + created_at
c = c.replace(
"""pub async fn get_events_by_contract(
    State(state): State<AppState>,
    Path(contract_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, AppError> {
    validate_contract_id(&contract_id)?;""",
"""pub async fn get_events_by_contract(
    State(state): State<AppState>,
    Path(contract_id): Path<String>,
    Query(params): Query<PaginationParams>,
    headers: axum::http::HeaderMap,
) -> Result<impl axum::response::IntoResponse, AppError> {
    validate_contract_id(&contract_id)?;""")

c = c.replace(
"""        let mut select_cols: Vec<&str> = vec!["id", "contract_id", "event_type", "tx_hash", "ledger", "timestamp", "event_data"];""",
"""        let mut select_cols: Vec<&str> = vec!["id", "contract_id", "event_type", "tx_hash", "ledger", "timestamp", "event_data", "created_at"];""")

# 4. get_events_by_contract cursor path -> ETag handling
c = c.replace(
"""        let columns = resolve_columns(&params)?;
        let events = rows_to_json(
            &rows,
            &columns,
            state.encryption_key.as_ref(),
            state.encryption_key_old.as_ref(),
            params.compact.unwrap_or(false),
        )?;

        let mut response = json!({
            "data": events,
            "contract_id": contract_id,
            "next_cursor": next_cursor,
            "limit": limit,
        });

        if let Some(fl) = params.from_ledger {
            response["from_ledger"] = json!(fl);
        }
        if let Some(tl) = params.to_ledger {
            response["to_ledger"] = json!(tl);
        }

        return Ok(Json(response));
    }

    // Offset-based path (deprecated fallback)""",
"""        let columns = resolve_columns(&params)?;
        let events = rows_to_json(
            &rows,
            &columns,
            state.encryption_key.as_ref(),
            state.encryption_key_old.as_ref(),
            params.compact.unwrap_or(false),
        )?;

        let etag = rows.first().and_then(|row| {
            let id: Option<uuid::Uuid> = row.try_get("id").ok();
            let created_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("created_at").ok();
            id.zip(created_at).map(|(id, ca)| compute_etag(&id, &ca, None))
        });

        if let Some(ref tag) = etag {
            if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
                if inm == tag {
                    let resp = axum::http::Response::builder()
                        .status(axum::http::StatusCode::NOT_MODIFIED)
                        .header("ETag", tag.as_str())
                        .header("Cache-Control", "no-cache")
                        .body(axum::body::Body::empty())
                        .unwrap();
                    return Ok(resp.into_response());
                }
            }
        }

        let mut response = json!({
            "data": events,
            "contract_id": contract_id,
            "next_cursor": next_cursor,
            "limit": limit,
        });

        if let Some(fl) = params.from_ledger {
            response["from_ledger"] = json!(fl);
        }
        if let Some(tl) = params.to_ledger {
            response["to_ledger"] = json!(tl);
        }

        let mut resp = Json(response).into_response();
        if let Some(ref tag) = etag {
            resp.headers_mut().insert("ETag", tag.parse().unwrap());
            resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());
        }
        return Ok(resp);
    }

    // Offset-based path (deprecated fallback)""")

# 5. get_events_by_contract offset path -> ETag handling
c = c.replace(
"""        let count_str = format!("SELECT COUNT(*) FROM events {}", where_clause);
        let mut cq = sqlx::query_scalar::<_, i64>(&count_str).bind(&contract_id);
        if let Some(fl) = params.from_ledger {
            cq = cq.bind(fl);
        }
        if let Some(tl) = params.to_ledger {
            cq = cq.bind(tl);
        }
        cq.fetch_one(&state.read_pool).await?
    };

    let mut response = json!({
        "data": events,
        "contract_id": contract_id,
        "next_cursor": next_cursor,
        "total": total,
        "limit": limit,
        "approximate": false, // we did an exact count above
        "pagination": "offset — migrate to cursor parameter for better performance",
    });

    if let Some(fl) = params.from_ledger {
        response["from_ledger"] = json!(fl);
    }
    if let Some(tl) = params.to_ledger {
        response["to_ledger"] = json!(tl);
    }

    return Ok(Json(response));
}""",
"""        let count_str = format!("SELECT COUNT(*) FROM events {}", where_clause);
        let mut cq = sqlx::query_scalar::<_, i64>(&count_str).bind(&contract_id);
        if let Some(fl) = params.from_ledger {
            cq = cq.bind(fl);
        }
        if let Some(tl) = params.to_ledger {
            cq = cq.bind(tl);
        }
        cq.fetch_one(&state.read_pool).await?
    };

    let etag = rows.first().and_then(|row| {
        let id: Option<uuid::Uuid> = row.try_get("id").ok();
        let created_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("created_at").ok();
        id.zip(created_at).map(|(id, ca)| compute_etag(&id, &ca, Some(total)))
    });

    if let Some(ref tag) = etag {
        if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
            if inm == tag {
                let resp = axum::http::Response::builder()
                    .status(axum::http::StatusCode::NOT_MODIFIED)
                    .header("ETag", tag.as_str())
                    .header("Cache-Control", "no-cache")
                    .body(axum::body::Body::empty())
                    .unwrap();
                return Ok(resp.into_response());
            }
        }
    }

    let mut response = json!({
        "data": events,
        "contract_id": contract_id,
        "next_cursor": next_cursor,
        "total": total,
        "limit": limit,
        "approximate": false, // we did an exact count above
        "pagination": "offset — migrate to cursor parameter for better performance",
    });

    if let Some(fl) = params.from_ledger {
        response["from_ledger"] = json!(fl);
    }
    if let Some(tl) = params.to_ledger {
        response["to_ledger"] = json!(tl);
    }

    let mut resp = Json(response).into_response();
    if let Some(ref tag) = etag {
        resp.headers_mut().insert("ETag", tag.parse().unwrap());
        resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());
    }
    return Ok(resp);
}""")

with open('src/handlers.rs', 'w') as f:
    f.write(c)

