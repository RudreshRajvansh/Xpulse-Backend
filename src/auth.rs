use crate::error::ApiError;
use crate::store::{AppState, now_millis};
use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

pub const ROLE_PATIENT: &str = "PATIENT";
pub const ROLE_DOCTOR: &str = "DOCTOR";
pub const ROLE_RECEPTIONIST: &str = "RECEPTIONIST";
pub const ROLE_PHARMACY: &str = "PHARMACY";
pub const ROLE_DIAGNOSTIC: &str = "DIAGNOSTIC";
pub const ROLE_ADMIN: &str = "ADMIN";
pub const ROLE_SUPER_ADMIN: &str = "SUPER_ADMIN";
pub const ROLE_CUSTOMER_CARE: &str = "CUSTOMER_CARE";

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
    pub token: String,
}

impl AuthUser {
    pub fn has_any(&self, allowed: &[&str]) -> bool {
        self.roles.iter().any(|r| allowed.contains(&r.as_str()))
    }

    pub fn require(&self, allowed: &[&str]) -> Result<(), ApiError> {
        if self.has_any(allowed) {
            Ok(())
        } else {
            Err(ApiError::Forbidden("Your role cannot perform this action".into()))
        }
    }

    pub fn require_self(&self, patient_id: &str) -> Result<(), ApiError> {
        if self.id == patient_id {
            Ok(())
        } else {
            Err(ApiError::Forbidden("You can only act on your own record".into()))
        }
    }
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::Unauthorized("Missing session token".into()))?;

    let auth_user = state.write_volatile(|db| {
        let session = db.sessions.get(&token).cloned()?;
        if session.expires_at <= now_millis() {
            db.sessions.remove(&token);
            return None;
        }
        db.accounts
            .iter()
            .find(|a| a.user.id == session.user_id)
            .map(|a| AuthUser {
                id: a.user.id.clone(),
                name: a.user.name.clone(),
                roles: a.user.roles.clone(),
                token: token.clone(),
            })
    });

    let auth_user = auth_user.ok_or_else(|| ApiError::Unauthorized("Session expired, sign in again".into()))?;

    request.extensions_mut().insert(auth_user);
    Ok(next.run(request).await)
}
