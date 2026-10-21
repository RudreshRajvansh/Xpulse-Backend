mod common;

use axum::http::{Method, StatusCode};
use common::TestApp;

#[tokio::test]
async fn expired_session_token_is_rejected() {
    let app = TestApp::with_session_ttl(0);
    let token = app.login("patient@xpulse.in").await;

    let reply = app.get("/patients/u_patient/medicines", &token).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{}", reply.raw);
}

#[tokio::test]
async fn expired_token_stays_rejected_on_retry() {
    let app = TestApp::with_session_ttl(0);
    let token = app.login("doctor@xpulse.in").await;

    for _ in 0..2 {
        let reply = app.get("/grants?granteeId=u_doctor", &token).await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn token_within_lifetime_is_accepted() {
    let app = TestApp::with_session_ttl(60_000);
    let token = app.login("patient@xpulse.in").await;

    let reply = app.get("/patients/u_patient/medicines", &token).await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn missing_and_forged_tokens_are_rejected() {
    let app = TestApp::new();

    let missing = app.send(Method::GET, "/patients/u_patient/medicines", None, None).await;
    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);

    let forged = app.get("/patients/u_patient/medicines", "00000000-0000-4000-8000-000000000000").await;
    assert_eq!(forged.status, StatusCode::UNAUTHORIZED);
}
