mod common;

use axum::http::StatusCode;
use common::TestApp;

#[tokio::test]
async fn unknown_email_and_wrong_password_look_identical() {
    let app = TestApp::new();
    let wrong_password = app.try_login("patient@xpulse.in", "not-the-password-1").await;
    let unknown_email = app.try_login("nobody@xpulse.in", "not-the-password-1").await;

    assert_eq!(wrong_password.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_email.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password.body, unknown_email.body);
}

#[tokio::test]
async fn email_lookup_ignores_case_and_whitespace() {
    let app = TestApp::new();
    let reply = app.try_login("  Patient@XPulse.in ", common::SEED_PASSWORD).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.raw);
}
