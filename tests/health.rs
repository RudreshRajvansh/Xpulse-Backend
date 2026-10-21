mod common;

use axum::http::{Method, StatusCode};
use common::TestApp;

#[tokio::test]
async fn health_check_is_public() {
    let app = TestApp::new();
    let reply = app.send(Method::GET, "/health", None, None).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["status"], "ok");
}

#[tokio::test]
async fn seeded_patient_can_read_own_medicines() {
    let app = TestApp::new();
    let token = app.login("patient@xpulse.in").await;
    let reply = app.get("/patients/u_patient/medicines", &token).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.body.as_array().is_some_and(|items| !items.is_empty()));
}
