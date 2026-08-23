use axum::{
    extract::{Path, State},
    response::Html,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{fs, sync::Mutex};

const DATA_FILE: &str = "board_data.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Row {
    id: u64,
    name: String,
    shift: String,
    status: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct RowInput {
    name: String,
    shift: String,
    status: String,
    description: String,
}

struct AppState {
    rows: Mutex<Vec<Row>>,
}

async fn load_rows() -> Vec<Row> {
    match fs::read_to_string(DATA_FILE).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn save_rows(rows: &[Row]) {
    if let Ok(json) = serde_json::to_string_pretty(rows) {
        let _ = fs::write(DATA_FILE, json).await;
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn get_rows(State(state): State<Arc<AppState>>) -> Json<Vec<Row>> {
    let rows = state.rows.lock().await;
    Json(rows.clone())
}

async fn create_row(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RowInput>,
) -> Json<Row> {
    let mut rows = state.rows.lock().await;
    let next_id = rows.iter().map(|r| r.id).max().unwrap_or(0) + 1;
    let row = Row {
        id: next_id,
        name: input.name,
        shift: input.shift,
        status: input.status,
        description: input.description,
    };
    rows.push(row.clone());
    save_rows(&rows).await;
    Json(row)
}

async fn update_row(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
    Json(input): Json<RowInput>,
) -> Json<Option<Row>> {
    let mut rows = state.rows.lock().await;
    let updated = rows.iter_mut().find(|r| r.id == id).map(|row| {
        row.name = input.name;
        row.shift = input.shift;
        row.status = input.status;
        row.description = input.description;
        row.clone()
    });
    save_rows(&rows).await;
    Json(updated)
}

async fn delete_row(State(state): State<Arc<AppState>>, Path(id): Path<u64>) -> Json<bool> {
    let mut rows = state.rows.lock().await;
    let len_before = rows.len();
    rows.retain(|r| r.id != id);
    let deleted = rows.len() != len_before;
    save_rows(&rows).await;
    Json(deleted)
}

#[tokio::main]
async fn main() {
    let rows = load_rows().await;
    let state = Arc::new(AppState {
        rows: Mutex::new(rows),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/api/rows", get(get_rows).post(create_row))
        .route("/api/rows/{id}", axum::routing::put(update_row).delete(delete_row))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
