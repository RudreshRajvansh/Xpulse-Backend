mod common;

use axum::http::StatusCode;
use common::TestApp;

const STAFF: [&str; 7] = [
    "pharmacy@xpulse.in",
    "care@xpulse.in",
    "reception@xpulse.in",
    "diagnostic@xpulse.in",
    "admin@xpulse.in",
    "superadmin@xpulse.in",
    "doctor@xpulse.in",
];

const CONSENT_PATHS: [&str; 4] = ["medicines", "profile", "lab-orders", "reports"];

#[tokio::test]
async fn pharmacy_cannot_read_patient_records_without_consent() {
    let app = TestApp::new();
    let pharmacy = app.login("pharmacy@xpulse.in").await;

    for path in CONSENT_PATHS {
        let reply = app.get(&format!("/patients/u_patient/{path}"), &pharmacy).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "pharmacy read {path}: {}", reply.raw);
    }
}

#[tokio::test]
async fn no_staff_role_reads_records_without_consent() {
    let app = TestApp::new();

    for email in STAFF {
        let token = app.login(email).await;
        for path in CONSENT_PATHS {
            let reply = app.get(&format!("/patients/u_patient/{path}"), &token).await;
            assert_eq!(reply.status, StatusCode::FORBIDDEN, "{email} read {path}");
        }
    }
}

#[tokio::test]
async fn pharmacy_still_sees_its_own_work_queue() {
    let app = TestApp::new();
    let pharmacy = app.login("pharmacy@xpulse.in").await;

    let reply = app.get("/prescriptions/pending", &pharmacy).await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn grant_opens_only_its_scope_and_closes_on_revoke() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;

    let grant = app.grant(&patient, "u_patient", "u_doctor", &["Medicines"]).await;
    assert_eq!(grant.status, StatusCode::OK, "{}", grant.raw);
    let grant_id = grant.body["id"].as_str().unwrap().to_string();

    assert_eq!(app.get("/patients/u_patient/medicines", &doctor).await.status, StatusCode::OK);
    assert_eq!(app.get("/patients/u_patient/profile", &doctor).await.status, StatusCode::FORBIDDEN);
    assert_eq!(app.get("/patients/u_patient/lab-orders", &doctor).await.status, StatusCode::FORBIDDEN);

    let pharmacy = app.login("pharmacy@xpulse.in").await;
    assert_eq!(app.get("/patients/u_patient/medicines", &pharmacy).await.status, StatusCode::FORBIDDEN);

    let revoke = app
        .post(&format!("/grants/{grant_id}/revoke"), Some(&patient), serde_json::json!({}))
        .await;
    assert_eq!(revoke.status, StatusCode::OK);
    assert_eq!(app.get("/patients/u_patient/medicines", &doctor).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn refused_attempts_show_up_in_the_patients_audit_log() {
    let app = TestApp::new();
    let pharmacy = app.login("pharmacy@xpulse.in").await;
    app.get("/patients/u_patient/medicines", &pharmacy).await;

    let patient = app.login("patient@xpulse.in").await;
    let audit = app.get("/patients/u_patient/audit", &patient).await;
    let entries = audit.body.as_array().unwrap();
    assert!(
        entries.iter().any(|e| e["actor"] == "u_pharmacy" && e["action"].as_str().unwrap().contains("denied")),
        "refusal not audited: {}",
        audit.raw
    );
}
