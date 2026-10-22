#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use xpulse_backend::{AppState, Config, SeedCredential, SeedPasswords, create_app};

pub type Inbox = Arc<Mutex<Vec<(String, String)>>>;

pub fn latest_code(inbox: &Inbox, email: &str) -> Option<String> {
    inbox
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(to, _)| to == email)
        .map(|(_, code)| code.clone())
}

pub const SEED_PASSWORD: &str = "seed-pass-123";
pub const PATIENT_PASSWORD: &str = "patient-pass-1";
pub const STAFF_PASSWORD: &str = "staff-pass-1";

pub struct Reply {
    pub status: StatusCode,
    pub body: Value,
    pub raw: String,
}

pub struct TestApp {
    router: Router,
}

impl TestApp {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Self {
        let state = AppState::ephemeral(SeedPasswords::Shared(SEED_PASSWORD.to_string())).with_config(config);
        TestApp {
            router: create_app(state),
        }
    }

    pub fn with_session_ttl(ttl_ms: i64) -> Self {
        Self::with_config(Config {
            session_ttl_ms: ttl_ms,
            ..Config::default()
        })
    }

    pub fn with_inbox() -> (Self, Inbox) {
        let inbox: Inbox = Arc::new(Mutex::new(Vec::new()));
        let delivered = inbox.clone();
        let config = Config {
            reset_code_sink: Some(Arc::new(move |email: &str, code: &str| {
                delivered.lock().unwrap().push((email.to_string(), code.to_string()));
            })),
            ..Config::default()
        };
        (Self::with_config(config), inbox)
    }

    pub fn with_random_seed() -> (Self, Vec<SeedCredential>) {
        let (state, generated) = AppState::seeded(SeedPasswords::Random);
        (TestApp { router: create_app(state) }, generated)
    }

    pub async fn send(&self, method: Method, path: &str, token: Option<&str>, body: Option<Value>) -> Reply {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(value) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let raw = String::from_utf8_lossy(&bytes).to_string();
        let body = serde_json::from_str(&raw).unwrap_or(Value::Null);
        Reply { status, body, raw }
    }

    pub async fn get(&self, path: &str, token: &str) -> Reply {
        self.send(Method::GET, path, Some(token), None).await
    }

    pub async fn post(&self, path: &str, token: Option<&str>, body: Value) -> Reply {
        self.send(Method::POST, path, token, Some(body)).await
    }

    pub async fn put(&self, path: &str, token: &str, body: Value) -> Reply {
        self.send(Method::PUT, path, Some(token), Some(body)).await
    }

    pub async fn try_login(&self, email: &str, password: &str) -> Reply {
        self.post("/auth/login", None, json!({ "email": email, "password": password }))
            .await
    }

    pub async fn login(&self, email: &str) -> String {
        let reply = self.try_login(email, SEED_PASSWORD).await;
        assert_eq!(reply.status, StatusCode::OK, "login failed for {email}: {}", reply.raw);
        reply.body["token"].as_str().unwrap().to_string()
    }

    pub async fn staff_at(&self, role: &str, email: &str, scope: &str) -> String {
        self.create_staff(role, email, scope).await.1
    }

    pub async fn create_staff(&self, role: &str, email: &str, scope: &str) -> (String, String) {
        let super_admin = self.login("superadmin@xpulse.in").await;
        let reply = self
            .post(
                "/admin/staff",
                Some(&super_admin),
                json!({
                    "name": format!("{role} {scope}"),
                    "email": email,
                    "password": STAFF_PASSWORD,
                    "role": role,
                    "scopeId": scope,
                }),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "staff creation failed: {}", reply.raw);
        let login = self.try_login(email, STAFF_PASSWORD).await;
        (
            login.body["user"]["id"].as_str().unwrap().to_string(),
            login.body["token"].as_str().unwrap().to_string(),
        )
    }

    pub async fn register_patient(&self, name: &str, email: &str) -> (String, String) {
        let reply = self
            .post(
                "/auth/register",
                None,
                json!({
                    "name": name,
                    "email": email,
                    "phone": "9876543210",
                    "password": PATIENT_PASSWORD,
                }),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "register failed: {}", reply.raw);
        (
            reply.body["user"]["id"].as_str().unwrap().to_string(),
            reply.body["token"].as_str().unwrap().to_string(),
        )
    }

    pub async fn grant(&self, patient_token: &str, patient_id: &str, grantee_id: &str, scope: &[&str]) -> Reply {
        let expires_at = now_millis() + 10 * 60 * 1000;
        self.post(
            "/grants",
            Some(patient_token),
            json!({
                "patientId": patient_id,
                "granteeId": grantee_id,
                "scope": scope,
                "expiresAt": expires_at,
            }),
        )
        .await
    }
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
