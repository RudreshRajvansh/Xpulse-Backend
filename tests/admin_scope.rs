mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;

fn emails(reply: &common::Reply) -> Vec<String> {
    reply
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["email"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn area_admin_only_lists_staff_from_its_own_area() {
    let app = TestApp::new();
    app.staff_at("PHARMACY", "sunrise-pharmacy@xpulse.in", "fac_2").await;
    app.staff_at("PHARMACY", "lakeview-pharmacy@xpulse.in", "fac_3").await;

    let admin = app.login("admin@xpulse.in").await;
    let listed = emails(&app.get("/admin/users", &admin).await);
    assert!(listed.contains(&"sunrise-pharmacy@xpulse.in".to_string()));
    assert!(!listed.contains(&"lakeview-pharmacy@xpulse.in".to_string()));
    assert!(!listed.contains(&"patient@xpulse.in".to_string()));
    assert!(!listed.contains(&"superadmin@xpulse.in".to_string()));

    let super_admin = app.login("superadmin@xpulse.in").await;
    let everyone = emails(&app.get("/admin/users", &super_admin).await);
    assert!(everyone.contains(&"lakeview-pharmacy@xpulse.in".to_string()));
    assert!(everyone.contains(&"patient@xpulse.in".to_string()));
}

#[tokio::test]
async fn area_admin_stats_ignore_activity_in_other_areas() {
    let app = TestApp::new();
    let admin = app.login("admin@xpulse.in").await;
    let super_admin = app.login("superadmin@xpulse.in").await;
    let area_before = app.get("/admin/stats", &admin).await.body["prescriptionsIssued"].as_i64().unwrap();
    let platform_before = app.get("/admin/stats", &super_admin).await.body["prescriptionsIssued"].as_i64().unwrap();

    let (doctor_id, lakeview_doctor) = app.create_staff("DOCTOR", "lakeview-doctor@xpulse.in", "fac_3").await;
    let patient = app.login("patient@xpulse.in").await;
    app.grant(&patient, "u_patient", &doctor_id, &["Medicines"]).await;
    let issued = app
        .post(
            "/prescriptions",
            Some(&lakeview_doctor),
            json!({ "patientId": "u_patient", "items": [{ "name": "Aspirin 75mg" }] }),
        )
        .await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.raw);

    let area_after = app.get("/admin/stats", &admin).await.body["prescriptionsIssued"].as_i64().unwrap();
    let platform_after = app.get("/admin/stats", &super_admin).await.body["prescriptionsIssued"].as_i64().unwrap();
    assert_eq!(area_after, area_before);
    assert_eq!(platform_after, platform_before + 1);
}

#[tokio::test]
async fn staff_must_be_placed_in_a_real_facility_or_area() {
    let app = TestApp::new();
    let super_admin = app.login("superadmin@xpulse.in").await;
    let create = |role: &str, email: &str, scope: &str| {
        json!({ "name": "New Hire", "email": email, "password": "staff-pass-1", "role": role, "scopeId": scope })
    };

    let rejected = [
        create("DOCTOR", "a@xpulse.in", "fac_99"),
        create("PHARMACY", "b@xpulse.in", ""),
        create("DIAGNOSTIC", "c@xpulse.in", "area_1"),
        create("ADMIN", "d@xpulse.in", "fac_1"),
        create("ADMIN", "e@xpulse.in", "area_9"),
    ];
    for body in rejected {
        let reply = app.post("/admin/staff", Some(&super_admin), body.clone()).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "accepted {body}");
    }

    let admin = app
        .post("/admin/staff", Some(&super_admin), create("ADMIN", "area2-admin@xpulse.in", "area_2"))
        .await;
    assert_eq!(admin.status, StatusCode::OK);
    assert_eq!(admin.body["scopeId"], "area_2");

    let care = app
        .post("/admin/staff", Some(&super_admin), create("CUSTOMER_CARE", "care2@xpulse.in", "fac_1"))
        .await;
    assert_eq!(care.status, StatusCode::OK);
    assert!(care.body["scopeId"].is_null());
}
