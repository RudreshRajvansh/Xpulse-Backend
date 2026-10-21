mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;

fn staff(role: &str, email: &str) -> serde_json::Value {
    json!({ "name": "New Hire", "email": email, "password": "staff-pass-1", "role": role, "scopeId": "fac_1" })
}

#[tokio::test]
async fn only_super_admin_can_create_staff() {
    let app = TestApp::new();
    for email in ["patient@xpulse.in", "doctor@xpulse.in", "admin@xpulse.in", "care@xpulse.in"] {
        let token = app.login(email).await;
        let reply = app.post("/admin/staff", Some(&token), staff("DOCTOR", "hire@xpulse.in")).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{email} created staff");
    }
}

#[tokio::test]
async fn super_admin_cannot_mint_privileged_or_unknown_roles() {
    let app = TestApp::new();
    let super_admin = app.login("superadmin@xpulse.in").await;

    for role in ["SUPER_ADMIN", "PATIENT", "GOD", ""] {
        let reply = app.post("/admin/staff", Some(&super_admin), staff(role, "hire@xpulse.in")).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "role {role:?} was accepted");
    }

    let doctor = app.post("/admin/staff", Some(&super_admin), staff("DOCTOR", "hire@xpulse.in")).await;
    assert_eq!(doctor.status, StatusCode::OK, "{}", doctor.raw);
    assert_eq!(app.try_login("hire@xpulse.in", "staff-pass-1").await.status, StatusCode::OK);
}

#[tokio::test]
async fn self_registration_always_yields_a_patient() {
    let app = TestApp::new();
    let reply = app
        .post(
            "/auth/register",
            None,
            json!({
                "name": "Sneaky",
                "email": "sneaky@example.com",
                "phone": "9876543210",
                "password": "sneaky-pass-1",
                "roles": ["SUPER_ADMIN"],
                "role": "SUPER_ADMIN",
            }),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["user"]["roles"], json!(["PATIENT"]));

    let token = reply.body["token"].as_str().unwrap();
    assert_eq!(app.get("/admin/users", token).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn roles_cannot_borrow_each_others_actions() {
    let app = TestApp::new();
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;
    let pharmacy = app.login("pharmacy@xpulse.in").await;
    let reception = app.login("reception@xpulse.in").await;
    let care = app.login("care@xpulse.in").await;

    let attempts = [
        ("patient reads users", app.get("/admin/users", &patient).await.status),
        ("patient reads stats", app.get("/admin/stats", &patient).await.status),
        ("doctor reads pharmacy queue", app.get("/prescriptions/pending", &doctor).await.status),
        ("pharmacy reads lab queue", app.get("/lab-orders", &pharmacy).await.status),
        ("care reads reception code", app.get("/reception/token", &care).await.status),
        ("patient reads all tickets", app.get("/tickets", &patient).await.status),
        (
            "doctor collects a sample",
            app.post("/lab-orders/lab_seed_2/collect", Some(&doctor), json!({})).await.status,
        ),
        (
            "reception publishes a result",
            app.post("/lab-orders/lab_seed_2/complete", Some(&reception), json!({ "resultSummary": "x" }))
                .await
                .status,
        ),
        (
            "pharmacy orders a lab",
            app.post("/lab-orders", Some(&pharmacy), json!({ "patientId": "u_patient", "testName": "CBC" }))
                .await
                .status,
        ),
        (
            "doctor checks a patient in",
            app.post("/appointments/checkin", Some(&doctor), json!({ "token": "000000" })).await.status,
        ),
    ];
    for (label, status) in attempts {
        assert_eq!(status, StatusCode::FORBIDDEN, "{label}");
    }
}
