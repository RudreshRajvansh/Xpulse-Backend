mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{Value, json};

async fn audit_of(app: &TestApp, patient_token: &str) -> Vec<Value> {
    let reply = app.get("/patients/u_patient/audit", patient_token).await;
    assert_eq!(reply.status, StatusCode::OK);
    reply.body.as_array().unwrap().clone()
}

#[tokio::test]
async fn every_entry_names_the_signed_in_user() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;
    let pharmacy = app.login("pharmacy@xpulse.in").await;

    let grant = app.grant(&patient, "u_patient", "u_doctor", &["Medicines"]).await;
    let grant_id = grant.body["id"].as_str().unwrap().to_string();
    app.get(&format!("/grants/{grant_id}/verify"), &doctor).await;
    let issued = app
        .post(
            "/prescriptions",
            Some(&doctor),
            json!({ "patientId": "u_patient", "items": [{ "name": "Aspirin 75mg" }] }),
        )
        .await;
    let prescription_id = issued.body["id"].as_str().unwrap().to_string();
    app.post(&format!("/prescriptions/{prescription_id}/fulfill"), Some(&pharmacy), json!({}))
        .await;

    let entries = audit_of(&app, &patient).await;
    let actors: Vec<&str> = entries.iter().map(|e| e["actor"].as_str().unwrap()).collect();
    for expected in ["u_patient", "u_doctor", "u_pharmacy"] {
        assert!(actors.contains(&expected), "{expected} missing from {actors:?}");
    }
    for placeholder in ["You", "Pharmacy", "Reception", "Diagnostics", "Support"] {
        assert!(!actors.contains(&placeholder), "placeholder actor {placeholder} still written");
    }

    let doctor_entry = entries.iter().find(|e| e["actor"] == "u_doctor").unwrap();
    assert_eq!(doctor_entry["actorName"], "Dr. Mehta");
}

#[tokio::test]
async fn body_supplied_identities_are_ignored() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;

    let ticket = app
        .post(
            "/tickets",
            Some(&patient),
            json!({ "userId": "u_doctor", "subject": "Sync", "detail": "Stuck" }),
        )
        .await;
    assert_eq!(ticket.status, StatusCode::OK);
    assert_eq!(ticket.body["raisedBy"], "u_patient");

    let reception = app.login("reception@xpulse.in").await;
    let code = app.get("/reception/token", &reception).await.body["token"]
        .as_str()
        .unwrap()
        .to_string();
    let appointment = app
        .post(
            "/appointments/checkin",
            Some(&patient),
            json!({ "patientId": "u_doctor", "token": code }),
        )
        .await;
    assert_eq!(appointment.status, StatusCode::OK, "{}", appointment.raw);
    assert_eq!(appointment.body["patientId"], "u_patient");
}
