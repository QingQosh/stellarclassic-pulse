import re

with open('src/handlers.rs', 'r') as f:
    c = f.read()

c = c.replace(
"""        ("sort" = Option<String>, Query, description = "Sort order: asc (oldest first) or desc (newest first, default)"),
        ("topic_sym" = Option<String>, Query, description = "Filter by first topic symbol (uses topic_0_sym generated column index)"),""",
"""        ("sort" = Option<String>, Query, description = "Sort order: asc (oldest first) or desc (newest first, default)"),
        ("sort_by" = Option<crate::models::SortBy>, Query, description = "Sort column: ledger (default), timestamp, or created_at"),
        ("topic_sym" = Option<String>, Query, description = "Filter by first topic symbol (uses topic_0_sym generated column index)"),""")

with open('src/handlers.rs', 'w') as f:
    f.write(c)
