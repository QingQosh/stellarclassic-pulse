with open('src/handlers.rs', 'r') as f:
    c = f.read()

# 1. Update signature of get_events_by_contract
c = c.replace(
"""pub async fn get_events_by_contract(
    State(state): State<AppState>,
    Path(contract_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, AppError> {""",
"""pub async fn get_events_by_contract(
    State(state): State<AppState>,
    Path(contract_id): Path<String>,
    Query(params): Query<PaginationParams>,
    headers: axum::http::HeaderMap,
) -> Result<impl axum::response::IntoResponse, AppError> {""")

print("File patched successfully.")
with open('src/handlers.rs', 'w') as f:
    f.write(c)

