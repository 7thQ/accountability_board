use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::sync::Mutex;

mod firewall;
mod gui;

use gui::{
    create_row, delete_row, get_art, get_rows, index, lan_url, list_art, load_rows, qr_code,
    rotate_loop, update_row, AppState, PORT,
};

#[tokio::main]
async fn main() {
    let password = firewall::prompt_password();
    let firewall_opened = password.as_deref().is_some_and(firewall::allow_port);

    let rows = load_rows().await;
    let state = Arc::new(AppState {
        rows: Mutex::new(rows),
    });

    tokio::spawn(rotate_loop(state.clone()));

    let app = Router::new()
        .route("/", get(index))
        .route("/api/rows", get(get_rows).post(create_row))
        .route("/api/rows/{id}", axum::routing::put(update_row).delete(delete_row))
        .route("/api/art", get(list_art))
        .route("/art/{name}", get(get_art))
        .route("/api/lan-url", get(lan_url))
        .route("/api/qr", get(qr_code))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", PORT))
        .await
        .unwrap();
    println!("Listening on http://0.0.0.0:{PORT}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen for Ctrl+C");
            println!("\nShutting down...");
            if firewall_opened {
                firewall::revert_port(&password.unwrap());
            }
        })
        .await
        .unwrap();

    println!("Server stopped.");
}
