with open('src/handlers.rs', 'r') as f:
    c = f.read()

# ETag injection into offset path of get_events_by_contract
c = c.replace(
"""            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE contract_id = $1")
                    .bind(&contract_id)
                    .fetch_one(&state.pool)
                    .await?;
            state.contract_count_cache.insert(contract_id.clone(), count).await;
            count
        }
    } else {
        let count_str = format!("SELECT COUNT(*) FROM events {}", where_clause);
        let mut cq = sqlx::query_scalar::<_, i64>(&count_str).bind(&contract_id);
        if let Some(fl) = params.from_ledger {
            cq = cq.bind(fl);
        }
        if let Some(tl) = params.to_ledger {
            cq = cq.bind(tl);
        }
        cq.fetch_one(&state.read_pool).await?
    };

    let mut response = json!({""",
"""            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE contract_id = $1")
                    .bind(&contract_id)
                    .fetch_one(&state.pool)
                    .await?;
            state.contract_count_cache.insert(contract_id.clone(), count).await;
            count
        }
    } else {
        let count_str = format!("SELECT COUNT(*) FROM events {}", where_clause);
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
        id.zip(created_at)
            .map(|(id, ca)| compute_etag(&id, &ca, Some(total)))
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

# Modify return statement of offset path
c = c.replace(
"""        if let Some(tl) = params.to_ledger {
            response["to_ledger"] = json!(tl);
        }

        return Ok(Json(response));
}
""",
"""        if let Some(tl) = params.to_ledger {
            response["to_ledger"] = json!(tl);
        }

        let mut resp = Json(response).into_response();
        if let Some(ref tag) = etag {
            resp.headers_mut().insert("ETag", tag.parse().unwrap());
            resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());
        }
        return Ok(resp);
}
""")

with open('src/handlers.rs', 'w') as f:
    f.write(c)
