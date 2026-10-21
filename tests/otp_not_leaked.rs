mod common;

use axum::http::StatusCode;
use common::{SEED_PASSWORD, TestApp, latest_code};
use serde_json::json;

const PATIENT: &str = "patient@xpulse.in";
const NEW_PASSWORD: &str = "fresh-pass-42";

#[tokio::test]
async fn reset_request_response_never_contains_the_code() {
    let (app, inbox) = TestApp::with_inbox();
    let reply = app.post("/auth/password/request", None, json!({ "email": PATIENT })).await;

    assert_eq!(reply.status, StatusCode::OK);
    let code = latest_code(&inbox, PATIENT).expect("code should be delivered out of band");
    assert!(reply.body.get("code").is_none(), "response has a code field: {}", reply.raw);
    assert!(!reply.raw.contains(&code), "response leaks the code: {}", reply.raw);
}

#[tokio::test]
async fn registered_and_unknown_emails_get_identical_responses() {
    let (app, inbox) = TestApp::with_inbox();
    let known = app.post("/auth/password/request", None, json!({ "email": PATIENT })).await;
    let unknown = app
        .post("/auth/password/request", None, json!({ "email": "nobody@xpulse.in" }))
        .await;

    assert_eq!(known.status, unknown.status);
    assert_eq!(known.body, unknown.body);
    assert!(latest_code(&inbox, "nobody@xpulse.in").is_none());
}

#[tokio::test]
async fn delivered_code_resets_the_password_exactly_once() {
    let (app, inbox) = TestApp::with_inbox();
    app.post("/auth/password/request", None, json!({ "email": PATIENT })).await;
    let code = latest_code(&inbox, PATIENT).unwrap();

    let reset = json!({ "email": PATIENT, "otp": code, "newPassword": NEW_PASSWORD });
    assert_eq!(app.post("/auth/password/reset", None, reset.clone()).await.status, StatusCode::OK);
    assert_eq!(app.try_login(PATIENT, NEW_PASSWORD).await.status, StatusCode::OK);
    assert_eq!(app.try_login(PATIENT, SEED_PASSWORD).await.status, StatusCode::UNAUTHORIZED);

    let replay = app.post("/auth/password/reset", None, reset).await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn repeated_wrong_guesses_burn_the_code() {
    let (app, inbox) = TestApp::with_inbox();
    app.post("/auth/password/request", None, json!({ "email": PATIENT })).await;
    let code = latest_code(&inbox, PATIENT).unwrap();

    for _ in 0..5 {
        let guess = json!({ "email": PATIENT, "otp": "000000", "newPassword": NEW_PASSWORD });
        assert_eq!(app.post("/auth/password/reset", None, guess).await.status, StatusCode::BAD_REQUEST);
    }

    let late = json!({ "email": PATIENT, "otp": code, "newPassword": NEW_PASSWORD });
    assert_eq!(app.post("/auth/password/reset", None, late).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(app.try_login(PATIENT, SEED_PASSWORD).await.status, StatusCode::OK);
}

#[tokio::test]
async fn successful_reset_signs_out_existing_sessions() {
    let (app, inbox) = TestApp::with_inbox();
    let old_token = app.login(PATIENT).await;

    app.post("/auth/password/request", None, json!({ "email": PATIENT })).await;
    let code = latest_code(&inbox, PATIENT).unwrap();
    let reset = json!({ "email": PATIENT, "otp": code, "newPassword": NEW_PASSWORD });
    assert_eq!(app.post("/auth/password/reset", None, reset).await.status, StatusCode::OK);

    let reply = app.get("/patients/u_patient/medicines", &old_token).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}
