mod common;

use axum::http::StatusCode;
use common::TestApp;

const RECORD_PATHS: [&str; 7] = [
    "medicines",
    "profile",
    "lab-orders",
    "reports",
    "audit",
    "tickets",
    "recommendations",
];

#[tokio::test]
async fn patient_cannot_read_another_patients_record() {
    let app = TestApp::new();
    let (_, intruder) = app.register_patient("Riya Patel", "riya@example.com").await;

    for path in RECORD_PATHS {
        let reply = app.get(&format!("/patients/u_patient/{path}"), &intruder).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{path} leaked: {}", reply.raw);
    }
}

#[tokio::test]
async fn isolation_holds_in_both_directions() {
    let app = TestApp::new();
    let (riya_id, riya_token) = app.register_patient("Riya Patel", "riya@example.com").await;
    let aarav = app.login("patient@xpulse.in").await;

    for path in RECORD_PATHS {
        let reply = app.get(&format!("/patients/{riya_id}/{path}"), &aarav).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{path} leaked: {}", reply.raw);

        let own = app.get(&format!("/patients/{riya_id}/{path}"), &riya_token).await;
        assert_eq!(own.status, StatusCode::OK, "patient blocked from own {path}");
    }
}

#[tokio::test]
async fn patient_cannot_write_to_another_patients_record() {
    let app = TestApp::new();
    let (_, intruder) = app.register_patient("Riya Patel", "riya@example.com").await;

    let profile = app
        .put(
            "/patients/u_patient/profile",
            &intruder,
            serde_json::json!({ "age": "99", "city": "Nowhere" }),
        )
        .await;
    assert_eq!(profile.status, StatusCode::FORBIDDEN);

    let report = app
        .post(
            "/patients/u_patient/reports",
            Some(&intruder),
            serde_json::json!({ "category": "X-ray", "label": "planted" }),
        )
        .await;
    assert_eq!(report.status, StatusCode::FORBIDDEN);
}
