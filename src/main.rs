mod api;
mod auth;
mod error;
mod models;
mod password;
mod recommend;
mod storage;
mod store;
mod tls;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use std::net::SocketAddr;
use std::path::PathBuf;
use store::AppState;
use tower_http::cors::{Any, CorsLayer};

fn create_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(api::public_routes())
        .merge(api::protected_routes(state.clone()))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(cors)
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let data_path = std::env::var("XPULSE_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("xpulse-data.json"));

    let state = AppState::load(data_path.clone());
    let app = create_app(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("invalid bind address");

    let tls_enabled = std::env::var("XPULSE_TLS").map(|v| v == "1").unwrap_or(false);

    println!("data file: {}", data_path.display());

    if tls_enabled {
        let cert_dir = data_path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
        let tls_config = tls::config(&cert_dir).await;
        println!("xpulse-backend listening on https://{addr}");
        println!("emulator base url: https://10.0.2.2:{port}");
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await
            .expect("failed to start tls server");
    } else {
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .expect("failed to bind tcp listener");
        println!("xpulse-backend listening on http://{addr}");
        println!("emulator base url: http://10.0.2.2:{port}");
        println!("set XPULSE_TLS=1 to serve over https");
        axum::serve(listener, app)
            .await
            .expect("failed to start server");
    }
}
