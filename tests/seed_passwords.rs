mod common;

use axum::http::StatusCode;
use common::TestApp;
use std::collections::HashSet;

#[tokio::test]
async fn old_default_password_no_longer_works() {
    let (app, _) = TestApp::with_random_seed();
    for email in ["patient@xpulse.in", "doctor@xpulse.in", "superadmin@xpulse.in"] {
        let reply = app.try_login(email, "password").await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{email} still accepts the old default");
    }
}

#[tokio::test]
async fn random_seed_gives_each_account_its_own_working_password() {
    let (app, generated) = TestApp::with_random_seed();
    assert_eq!(generated.len(), 8);

    let unique: HashSet<&str> = generated.iter().map(|c| c.password.as_str()).collect();
    assert_eq!(unique.len(), generated.len(), "seed passwords must not repeat");

    for credential in &generated {
        assert!(credential.password.len() >= 16);
        let reply = app.try_login(&credential.email, &credential.password).await;
        assert_eq!(reply.status, StatusCode::OK, "{} cannot sign in", credential.email);
    }
}
