with open('src/handlers.rs', 'r') as f:
    c = f.read()

# 1. get_events OpenAPI macro
c = c.replace(
"""        ("topic_sym" = Option<String>, Query, description = "Filter by top-level topic symbol (from topic 0)"),
        ("search" = Option<String>, Query, description = "Full-text search on normalized event payload"),
    ),
    responses(""",
"""        ("topic_sym" = Option<String>, Query, description = "Filter by top-level topic symbol (from topic 0)"),
        ("search" = Option<String>, Query, description = "Full-text search on normalized event payload"),
        ("If-None-Match" = Option<String>, Header, description = "Conditional GET: Return 304 if ETag matches"),
    ),
    responses(
        (status = 304, description = "Not Modified (ETag matched)", headers(
            ("ETag" = String, description = "Computed ETag"),
            ("Cache-Control" = String, description = "No cache")
        )),""")

# 2. get_events_by_contract OpenAPI macro
c = c.replace(
"""        ("to_ledger" = Option<i64>, Query, description = "Return events at or before this ledger"),
        ("sort" = Option<String>, Query, description = "Sort order: asc (oldest first) or desc (newest first, default)"),
        ("sort_by" = Option<crate::models::SortBy>, Query, description = "Sort column: ledger (default), timestamp, or created_at"),
    ),
    responses(""",
"""        ("to_ledger" = Option<i64>, Query, description = "Return events at or before this ledger"),
        ("sort" = Option<String>, Query, description = "Sort order: asc (oldest first) or desc (newest first, default)"),
        ("sort_by" = Option<crate::models::SortBy>, Query, description = "Sort column: ledger (default), timestamp, or created_at"),
        ("If-None-Match" = Option<String>, Header, description = "Conditional GET: Return 304 if ETag matches"),
    ),
    responses(
        (status = 304, description = "Not Modified (ETag matched)", headers(
            ("ETag" = String, description = "Computed ETag"),
            ("Cache-Control" = String, description = "No cache")
        )),""")

with open('src/handlers.rs', 'w') as f:
    f.write(c)
