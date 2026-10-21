pub mod api;
pub mod auth;
pub mod error;
pub mod models;
pub mod password;
pub mod recommend;
pub mod storage;
pub mod store;
pub mod tls;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use tower_http::cors::{Any, CorsLayer};

pub use store::{AppState, Config, ResetCodeSink, SeedCredential, SeedPasswords};

pub fn create_app(state: AppState) -> Router {
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
