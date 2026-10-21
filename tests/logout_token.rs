mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn token_is_rejected_after_logout() {
    let app = TestApp::new();
    let token = app.login("patient@xpulse.in").await;
    assert_eq!(app.get("/patients/u_patient/medicines", &token).await.status, StatusCode::OK);

    let logout = app.post("/auth/logout", Some(&token), json!({})).await;
    assert_eq!(logout.status, StatusCode::OK);

    let reply = app.get("/patients/u_patient/medicines", &token).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_only_revokes_the_presented_token() {
    let app = TestApp::new();
    let phone = app.login("patient@xpulse.in").await;
    let tablet = app.login("patient@xpulse.in").await;

    app.post("/auth/logout", Some(&phone), json!({})).await;

    assert_eq!(app.get("/patients/u_patient/medicines", &phone).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.get("/patients/u_patient/medicines", &tablet).await.status, StatusCode::OK);
}

#[tokio::test]
async fn logging_out_twice_does_not_reopen_the_session() {
    let app = TestApp::new();
    let token = app.login("doctor@xpulse.in").await;

    assert_eq!(app.post("/auth/logout", Some(&token), json!({})).await.status, StatusCode::OK);
    assert_eq!(app.post("/auth/logout", Some(&token), json!({})).await.status, StatusCode::UNAUTHORIZED);
}
