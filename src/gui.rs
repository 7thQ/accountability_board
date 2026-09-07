use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::{fs, sync::Mutex, time};

const DATA_FILE: &str = "board_data.json";
const ART_DIR: &str = "art";
// Bump this to Duration::from_secs(30 * 60) once the 1-minute test period is done.
const ROTATE_INTERVAL: Duration = Duration::from_secs(60);
pub const PORT: u16 = 3000;

// Opening a UDP "connection" doesn't send any packets, it just asks the OS
// to pick the local interface it would use to reach that address — a
// standard trick for finding this machine's LAN IP without extra deps.
fn local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn lan_base_url() -> String {
    format!("http://{}:{}", local_ip(), PORT)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    id: u64,
    name: String,
    shift: String,
    status: String,
    description: String,
}

#[derive(Debug, Deserialize)]
pub struct RowInput {
    name: String,
    shift: String,
    status: String,
    description: String,
}

pub struct AppState {
    pub rows: Mutex<Vec<Row>>,
}

pub async fn load_rows() -> Vec<Row> {
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

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

pub async fn get_rows(State(state): State<std::sync::Arc<AppState>>) -> Json<Vec<Row>> {
    let rows = state.rows.lock().await;
    Json(rows.clone())
}

pub async fn create_row(
    State(state): State<std::sync::Arc<AppState>>,
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

pub async fn update_row(
    State(state): State<std::sync::Arc<AppState>>,
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

pub async fn delete_row(State(state): State<std::sync::Arc<AppState>>, Path(id): Path<u64>) -> Json<bool> {
    let mut rows = state.rows.lock().await;
    let len_before = rows.len();
    rows.retain(|r| r.id != id);
    let deleted = rows.len() != len_before;
    save_rows(&rows).await;
    Json(deleted)
}

#[derive(Debug, Serialize)]
pub struct ArtItem {
    name: String,
    kind: &'static str,
}

fn art_kind(name: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "gif" | "png" | "jpg" | "jpeg" | "webp" => Some("image"),
        "txt" => Some("text"),
        _ => None,
    }
}

fn art_content_type(name: &str) -> &'static str {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "gif" => "image/gif",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub async fn list_art() -> Json<Vec<ArtItem>> {
    let mut items = Vec::new();
    if let Ok(mut entries) = fs::read_dir(ART_DIR).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Some(name) = entry.file_name().to_str().map(String::from) else {
                continue;
            };
            if let Some(kind) = art_kind(&name) {
                items.push(ArtItem { name, kind });
            }
        }
    }
    Json(items)
}

pub async fn get_art(Path(name): Path<String>) -> impl IntoResponse {
    if name.contains('/') || name.contains("..") {
        return (StatusCode::BAD_REQUEST, "invalid art name").into_response();
    }
    let path = std::path::Path::new(ART_DIR).join(&name);
    match fs::read(&path).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, art_content_type(&name))], bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "art not found").into_response(),
    }
}

pub async fn lan_url() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "url": lan_base_url() }))
}

pub async fn qr_code() -> impl IntoResponse {
    let code = match qrcode::QrCode::new(lan_base_url().as_bytes()) {
        Ok(code) => code,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "failed to build qr code").into_response(),
    };
    let svg = code
        .render()
        .min_dimensions(220, 220)
        .dark_color(qrcode::render::svg::Color("#1f2328"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response()
}

pub async fn rotate_loop(state: std::sync::Arc<AppState>) {
    let mut ticker = time::interval(ROTATE_INTERVAL);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let mut rows = state.rows.lock().await;
        if rows.len() > 1 {
            let top = rows.remove(0);
            rows.push(top);
            save_rows(&rows).await;
        }
    }
}
