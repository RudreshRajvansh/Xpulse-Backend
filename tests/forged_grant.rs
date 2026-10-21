mod common;

use axum::http::StatusCode;
use common::{TestApp, now_millis};
use serde_json::json;

async fn post_grant(app: &TestApp, token: &str, patient: &str, grantee: &str, scope: serde_json::Value, expires_at: i64) -> common::Reply {
    app.post(
        "/grants",
        Some(token),
        json!({ "patientId": patient, "granteeId": grantee, "scope": scope, "expiresAt": expires_at }),
    )
    .await
}

#[tokio::test]
async fn staff_cannot_forge_a_grant_on_a_patients_behalf() {
    let app = TestApp::new();
    let expires = now_millis() + 600_000;

    let pharmacy = app.login("pharmacy@xpulse.in").await;
    let forged = post_grant(&app, &pharmacy, "u_patient", "u_pharmacy", json!(["Medicines"]), expires).await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);

    let doctor = app.login("doctor@xpulse.in").await;
    let self_granted = post_grant(&app, &doctor, "u_patient", "u_doctor", json!(["Medicines"]), expires).await;
    assert_eq!(self_granted.status, StatusCode::FORBIDDEN);

    assert_eq!(app.get("/patients/u_patient/medicines", &doctor).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn patient_grants_are_validated() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let expires = now_millis() + 600_000;

    let to_pharmacy = post_grant(&app, &patient, "u_patient", "u_pharmacy", json!(["Medicines"]), expires).await;
    assert_eq!(to_pharmacy.status, StatusCode::BAD_REQUEST);

    let to_nobody = post_grant(&app, &patient, "u_patient", "u_ghost", json!(["Medicines"]), expires).await;
    assert_eq!(to_nobody.status, StatusCode::BAD_REQUEST);

    let bad_scope = post_grant(&app, &patient, "u_patient", "u_doctor", json!(["Everything"]), expires).await;
    assert_eq!(bad_scope.status, StatusCode::BAD_REQUEST);

    let no_scope = post_grant(&app, &patient, "u_patient", "u_doctor", json!([]), expires).await;
    assert_eq!(no_scope.status, StatusCode::BAD_REQUEST);

    let already_expired = post_grant(&app, &patient, "u_patient", "u_doctor", json!(["Medicines"]), now_millis() - 1).await;
    assert_eq!(already_expired.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn long_lived_grants_are_capped_at_a_day() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let one_year = now_millis() + 365 * 24 * 60 * 60 * 1000;

    let reply = post_grant(&app, &patient, "u_patient", "u_doctor", json!(["Medicines"]), one_year).await;
    assert_eq!(reply.status, StatusCode::OK);
    let expires_at = reply.body["expiresAt"].as_i64().unwrap();
    assert!(expires_at <= now_millis() + 24 * 60 * 60 * 1000);
}

#[tokio::test]
async fn only_the_named_grantee_can_open_a_grant() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let grant = app.grant(&patient, "u_patient", "u_doctor", &["Medicines", "Allergies"]).await;
    let grant_id = grant.body["id"].as_str().unwrap().to_string();

    let pharmacy = app.login("pharmacy@xpulse.in").await;
    let stolen = app.get(&format!("/grants/{grant_id}/verify"), &pharmacy).await;
    assert_eq!(stolen.status, StatusCode::FORBIDDEN);

    let listing = app.get("/grants?granteeId=u_doctor", &pharmacy).await;
    assert_eq!(listing.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn only_the_patient_can_revoke() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;
    let grant = app.grant(&patient, "u_patient", "u_doctor", &["Medicines"]).await;
    let grant_id = grant.body["id"].as_str().unwrap().to_string();

    let hijack = app.post(&format!("/grants/{grant_id}/revoke"), Some(&doctor), json!({})).await;
    assert_eq!(hijack.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get("/patients/u_patient/medicines", &doctor).await.status, StatusCode::OK);
}

#[tokio::test]
async fn opened_grant_returns_only_the_chosen_scope() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;
    let grant = app.grant(&patient, "u_patient", "u_doctor", &["Medicines"]).await;
    let grant_id = grant.body["id"].as_str().unwrap().to_string();

    let scoped = app.get(&format!("/grants/{grant_id}/verify"), &doctor).await;
    assert_eq!(scoped.status, StatusCode::OK);
    assert!(!scoped.body["medicines"].as_array().unwrap().is_empty());
    assert!(scoped.body["allergies"].as_array().unwrap().is_empty());
    assert!(scoped.body["labOrders"].as_array().unwrap().is_empty());
    assert_eq!(scoped.body["profile"]["age"], "");
    assert_eq!(scoped.body["scope"], json!(["Medicines"]));
}

#[tokio::test]
async fn doctor_needs_matching_consent_to_prescribe_or_order_labs() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;
    let prescription = json!({
        "patientId": "u_patient",
        "doctorId": "u_someone_else",
        "items": [{ "name": "Aspirin 75mg", "dose": "1 tablet", "frequency": "Once daily" }],
    });
    let lab = json!({ "patientId": "u_patient", "testName": "CBC", "orderedBy": "u_someone_else" });

    assert_eq!(app.post("/prescriptions", Some(&doctor), prescription.clone()).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.post("/lab-orders", Some(&doctor), lab.clone()).await.status, StatusCode::FORBIDDEN);

    app.grant(&patient, "u_patient", "u_doctor", &["Medicines"]).await;
    let issued = app.post("/prescriptions", Some(&doctor), prescription).await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.raw);
    assert_eq!(issued.body["doctorId"], "u_doctor");
    assert_eq!(app.post("/lab-orders", Some(&doctor), lab).await.status, StatusCode::FORBIDDEN);
}
