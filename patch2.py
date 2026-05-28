with open('src/handlers.rs', 'r') as f:
    c = f.read()

# ETag injection into cursor path of get_events_by_contract
c = c.replace(
"""        let columns = resolve_columns(&params)?;
        let events = rows_to_json(
            &rows,
            &columns,
            state.encryption_key.as_ref(),
            state.encryption_key_old.as_ref(),
            params.compact.unwrap_or(false),
        )?;

        let mut response = json!({""",
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
            id.zip(created_at)
                .map(|(id, ca)| compute_etag(&id, &ca, None))
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

        let mut response = json!({""")

# Modify return statement of cursor path
c = c.replace(
"""        if let Some(tl) = params.to_ledger {
            response["to_ledger"] = json!(tl);
        }

        return Ok(Json(response));
    }""",
"""        if let Some(tl) = params.to_ledger {
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
