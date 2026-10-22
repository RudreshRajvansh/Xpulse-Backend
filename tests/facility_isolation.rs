mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{Value, json};

fn ids(reply: &common::Reply) -> Vec<String> {
    reply
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|item: &Value| item["id"].as_str().unwrap().to_string())
        .collect()
}

async fn prescription_from_city_care(app: &TestApp) -> String {
    let patient = app.login("patient@xpulse.in").await;
    let doctor = app.login("doctor@xpulse.in").await;
    app.grant(&patient, "u_patient", "u_doctor", &["Medicines"]).await;
    let issued = app
        .post(
            "/prescriptions",
            Some(&doctor),
            json!({ "patientId": "u_patient", "items": [{ "name": "Aspirin 75mg" }] }),
        )
        .await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.raw);
    assert_eq!(issued.body["facilityId"], "fac_1");
    issued.body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn pharmacy_only_sees_and_dispenses_its_own_facilitys_prescriptions() {
    let app = TestApp::new();
    let prescription = prescription_from_city_care(&app).await;
    let home = app.login("pharmacy@xpulse.in").await;
    let other = app.staff_at("PHARMACY", "sunrise-pharmacy@xpulse.in", "fac_2").await;

    assert!(ids(&app.get("/prescriptions/pending", &home).await).contains(&prescription));
    assert!(ids(&app.get("/prescriptions/pending", &other).await).is_empty());

    let path = format!("/prescriptions/{prescription}/fulfill");
    assert_eq!(app.post(&path, Some(&other), json!({})).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.post(&path, Some(&home), json!({})).await.status, StatusCode::OK);
    assert_eq!(app.post(&path, Some(&home), json!({})).await.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn diagnostics_only_sees_and_works_its_own_facilitys_lab_orders() {
    let app = TestApp::new();
    let home = app.login("diagnostic@xpulse.in").await;
    let other = app.staff_at("DIAGNOSTIC", "lakeview-lab@xpulse.in", "fac_3").await;

    assert!(ids(&app.get("/lab-orders", &home).await).contains(&"lab_seed_2".to_string()));
    assert!(ids(&app.get("/lab-orders", &other).await).is_empty());

    let collect = app.post("/lab-orders/lab_seed_2/collect", Some(&other), json!({})).await;
    assert_eq!(collect.status, StatusCode::NOT_FOUND);
    let publish = app
        .post("/lab-orders/lab_seed_1/complete", Some(&other), json!({ "resultSummary": "Tampered" }))
        .await;
    assert_eq!(publish.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reception_codes_and_queues_are_per_facility() {
    let app = TestApp::new();
    let city = app.login("reception@xpulse.in").await;
    let sunrise = app.staff_at("RECEPTIONIST", "sunrise-desk@xpulse.in", "fac_2").await;
    let city_code = app.get("/reception/token", &city).await.body["token"].as_str().unwrap().to_string();
    let sunrise_code = app.get("/reception/token", &sunrise).await.body["token"].as_str().unwrap().to_string();
    assert_ne!(city_code, sunrise_code);

    let patient = app.login("patient@xpulse.in").await;
    let visit = app
        .post("/appointments/checkin", Some(&patient), json!({ "token": sunrise_code }))
        .await;
    assert_eq!(visit.status, StatusCode::OK, "{}", visit.raw);
    assert_eq!(visit.body["facilityId"], "fac_2");
    let visit_id = visit.body["id"].as_str().unwrap().to_string();

    assert!(ids(&app.get("/appointments/queue", &sunrise).await).contains(&visit_id));
    assert!(!ids(&app.get("/appointments/queue", &city).await).contains(&visit_id));
    let city_doctor = app.login("doctor@xpulse.in").await;
    assert!(!ids(&app.get("/appointments/queue", &city_doctor).await).contains(&visit_id));

    let path = format!("/appointments/{visit_id}/admit");
    assert_eq!(app.post(&path, Some(&city), json!({})).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.post(&path, Some(&sunrise), json!({})).await.status, StatusCode::OK);
}
