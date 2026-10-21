mod common;

use axum::http::StatusCode;
use common::{TestApp, latest_code};
use serde_json::json;

fn registration(name: &str, email: &str, phone: &str, password: &str) -> serde_json::Value {
    json!({ "name": name, "email": email, "phone": phone, "password": password })
}

#[tokio::test]
async fn registration_rejects_what_the_app_would_reject() {
    let app = TestApp::new();
    let cases = [
        registration("Riya", "riya@example.com", "9876543210", "short1"),
        registration("Riya", "riya@example.com", "9876543210", "no-digits-here"),
        registration("Riya", "riya@example.com", "9876543210", &format!("{}1", "a".repeat(200))),
        registration("Riya", "not-an-email", "9876543210", "good-pass-1"),
        registration("Riya", "riya@example.com", "12345", "good-pass-1"),
        registration("   ", "riya@example.com", "9876543210", "good-pass-1"),
    ];
    for body in cases {
        let reply = app.post("/auth/register", None, body.clone()).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "accepted {body}");
    }
}

#[tokio::test]
async fn weak_reset_password_is_rejected_without_burning_the_code() {
    let (app, inbox) = TestApp::with_inbox();
    app.post("/auth/password/request", None, json!({ "email": "patient@xpulse.in" })).await;
    let code = latest_code(&inbox, "patient@xpulse.in").unwrap();

    let weak = json!({ "email": "patient@xpulse.in", "otp": code, "newPassword": "weak" });
    assert_eq!(app.post("/auth/password/reset", None, weak).await.status, StatusCode::BAD_REQUEST);

    let strong = json!({ "email": "patient@xpulse.in", "otp": code, "newPassword": "strong-pass-9" });
    assert_eq!(app.post("/auth/password/reset", None, strong).await.status, StatusCode::OK);
}

#[tokio::test]
async fn empty_free_text_is_rejected() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;

    let ticket = app
        .post("/tickets", Some(&patient), json!({ "subject": "  ", "detail": "Something" }))
        .await;
    assert_eq!(ticket.status, StatusCode::BAD_REQUEST);

    let report = app
        .post("/patients/u_patient/reports", Some(&patient), json!({ "category": "", "label": "" }))
        .await;
    assert_eq!(report.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn lab_result_cannot_skip_sample_collection() {
    let app = TestApp::new();
    let lab = app.login("diagnostic@xpulse.in").await;

    let premature = app
        .post("/lab-orders/lab_seed_2/complete", Some(&lab), json!({ "resultSummary": "Normal" }))
        .await;
    assert_eq!(premature.status, StatusCode::CONFLICT);

    app.post("/lab-orders/lab_seed_2/collect", Some(&lab), json!({})).await;
    let published = app
        .post("/lab-orders/lab_seed_2/complete", Some(&lab), json!({ "resultSummary": "Normal" }))
        .await;
    assert_eq!(published.status, StatusCode::OK);
}
