use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::store::{AppState, Db};

pub const SCOPE_MEDICINES: &str = "Medicines";
pub const SCOPE_ALLERGIES: &str = "Allergies";
pub const SCOPE_PROFILE: &str = "Profile";
pub const SCOPE_LAB_RESULTS: &str = "LabResults";
pub const SCOPE_REPORTS: &str = "Reports";

pub const ALL_SCOPES: [&str; 5] = [
    SCOPE_MEDICINES,
    SCOPE_ALLERGIES,
    SCOPE_PROFILE,
    SCOPE_LAB_RESULTS,
    SCOPE_REPORTS,
];

pub const MAX_GRANT_MS: i64 = 24 * 60 * 60 * 1000;

pub fn label(scope: &str) -> &'static str {
    match scope {
        SCOPE_MEDICINES => "medicines",
        SCOPE_ALLERGIES => "allergies",
        SCOPE_PROFILE => "vitals and history",
        SCOPE_LAB_RESULTS => "lab results",
        SCOPE_REPORTS => "reports",
        _ => "records",
    }
}

pub fn describe(scope: &[String]) -> String {
    scope.iter().map(|s| label(s)).collect::<Vec<_>>().join(", ")
}

pub fn normalise_scope(requested: &[String]) -> ApiResult<Vec<String>> {
    let mut accepted: Vec<String> = Vec::new();
    for item in requested {
        let item = item.trim();
        if !ALL_SCOPES.contains(&item) {
            return Err(ApiError::InvalidInput(format!("Unknown consent scope: {item}")));
        }
        if !accepted.iter().any(|existing| existing == item) {
            accepted.push(item.to_string());
        }
    }
    if accepted.is_empty() {
        return Err(ApiError::InvalidInput("Choose at least one record to share".into()));
    }
    Ok(accepted)
}

pub fn require_consent(db: &Db, auth: &AuthUser, patient_id: &str, scope: &str) -> ApiResult<()> {
    if db.active_grant(patient_id, &auth.id, scope).is_some() {
        Ok(())
    } else {
        Err(ApiError::Forbidden(format!(
            "No active consent from this patient covers {}",
            label(scope)
        )))
    }
}

pub fn authorize_record(state: &AppState, auth: &AuthUser, patient_id: &str, scope: &str) -> ApiResult<()> {
    if auth.id == patient_id {
        return Ok(());
    }
    state.write(|db| {
        let patient_exists = db.accounts.iter().any(|a| a.user.id == patient_id);
        let granted = db.active_grant(patient_id, &auth.id, scope).is_some();
        if patient_exists {
            let action = if granted {
                format!("Read {} under consent", label(scope))
            } else {
                format!("Was denied {}: no active consent", label(scope))
            };
            db.push_audit(auth, &action, patient_id);
        }
        if granted {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!(
                "No active consent from this patient covers {}",
                label(scope)
            )))
        }
    })
}
