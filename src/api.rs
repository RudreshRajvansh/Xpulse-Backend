use crate::auth::*;
use crate::error::{ApiError, ApiResult};
use crate::models::*;
use crate::password;
use crate::store::{AppState, ResetCode, new_id, now_millis, random_code};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    middleware,
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(login))
        .route("/auth/register", post(register))
        .route("/auth/password/request", post(request_password_reset))
        .route("/auth/password/reset", post(reset_password))
}

pub fn protected_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/auth/logout", post(logout))
        .route("/patients/{id}/medicines", get(get_medicines))
        .route("/patients/{id}/audit", get(get_audit))
        .route("/patients/{id}/lab-orders", get(patient_lab_orders))
        .route("/patients/{id}/tickets", get(patient_tickets))
        .route("/patients/{id}/recommendations", get(recommendations))
        .route("/patients/{id}/profile", get(get_profile).put(save_profile))
        .route("/patients/{id}/reports", get(get_reports).post(add_report))
        .route("/reports/{id}", delete(remove_report))
        .route("/reports/{id}/file", get(download_report))
        .route("/prescriptions", post(issue_prescription))
        .route("/prescriptions/pending", get(pending_prescriptions))
        .route("/prescriptions/{id}/fulfill", post(fulfill_prescription))
        .route("/grants", get(active_grants).post(grant_access))
        .route("/grants/{id}/revoke", post(revoke_grant))
        .route("/grants/{id}/verify", get(verify_grant))
        .route("/reception/token", get(reception_token))
        .route("/appointments/checkin", post(check_in))
        .route("/appointments/queue", get(queue))
        .route("/appointments/{id}/admit", post(admit))
        .route("/lab-orders", get(all_lab_orders).post(order_lab_test))
        .route("/lab-orders/{id}/collect", post(collect_sample))
        .route("/lab-orders/{id}/complete", post(complete_lab_order))
        .route("/admin/stats", get(platform_stats))
        .route("/admin/users", get(list_users))
        .route("/admin/staff", post(create_staff))
        .route("/tickets", get(all_tickets).post(raise_ticket))
        .route("/tickets/{id}/resolve", post(resolve_ticket))
        .layer(middleware::from_fn_with_state(state, require_auth))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "xpulse-backend" }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginBody {
    email: String,
    password: String,
}

async fn login(State(state): State<AppState>, Json(body): Json<LoginBody>) -> ApiResult<Json<Value>> {
    let account = state.read(|db| db.account_by_email(&body.email).cloned());
    let stored = account.as_ref().map(|a| a.password_hash.clone());
    let matched = password::verify_async(body.password, stored).await;
    let account = match account {
        Some(account) if matched => account,
        _ => return Err(ApiError::Unauthorized("Invalid email or password".into())),
    };
    let token = state.issue_session(&account.user.id);
    Ok(Json(json!({ "token": token, "user": account.user })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterBody {
    name: String,
    email: String,
    phone: String,
    password: String,
}

async fn register(State(state): State<AppState>, Json(body): Json<RegisterBody>) -> ApiResult<Json<Value>> {
    if state.read(|db| db.account_by_email(&body.email).is_some()) {
        return Err(ApiError::Conflict("An account with this email already exists".into()));
    }
    let password_hash = password::hash_async(body.password.clone()).await;
    let user = state.write(|db| {
        if db.account_by_email(&body.email).is_some() {
            return Err(ApiError::Conflict("An account with this email already exists".into()));
        }
        let user = User {
            id: format!("u_{}", new_id()),
            name: body.name.clone(),
            email: body.email.clone(),
            roles: vec![ROLE_PATIENT.to_string()],
            scope_id: None,
            phone: Some(body.phone.clone()),
        };
        db.accounts.push(Account {
            user: user.clone(),
            password_hash,
        });
        Ok(user)
    })?;
    let token = state.issue_session(&user.id);
    Ok(Json(json!({ "token": token, "user": user })))
}

async fn logout(State(state): State<AppState>, Extension(auth): Extension<AuthUser>) -> Json<Value> {
    state.write_volatile(|db| {
        db.sessions.remove(&auth.token);
    });
    Json(json!({ "ok": true }))
}

const RESET_CODE_TTL_MS: i64 = 10 * 60 * 1000;
const RESET_MAX_ATTEMPTS: u32 = 5;
const RESET_REQUESTED: &str = "If an account exists for that email, a reset code has been sent.";
const RESET_REJECTED: &str = "That code is invalid or has expired";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmailBody {
    email: String,
}

async fn request_password_reset(State(state): State<AppState>, Json(body): Json<EmailBody>) -> Json<Value> {
    let email = body.email.trim().to_lowercase();
    let issued = state.write_volatile(|db| {
        db.account_by_email(&email)?;
        let code = random_code();
        db.reset_codes.insert(
            email.clone(),
            ResetCode {
                code: code.clone(),
                expires_at: now_millis() + RESET_CODE_TTL_MS,
                attempts: 0,
            },
        );
        Some(code)
    });
    if let (Some(code), Some(sink)) = (issued, &state.config().reset_code_sink) {
        sink(&email, &code);
    }
    Json(json!({ "ok": true, "message": RESET_REQUESTED }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetBody {
    email: String,
    otp: String,
    new_password: String,
}

async fn reset_password(State(state): State<AppState>, Json(body): Json<ResetBody>) -> ApiResult<Json<Value>> {
    let email = body.email.trim().to_lowercase();
    let otp = body.otp.trim().to_string();
    let user_id = state.write_volatile(|db| {
        let now = now_millis();
        let matched = match db.reset_codes.get_mut(&email) {
            Some(entry) if entry.expires_at > now && entry.attempts < RESET_MAX_ATTEMPTS => {
                if entry.code == otp {
                    true
                } else {
                    entry.attempts += 1;
                    false
                }
            }
            _ => false,
        };
        let spent = db
            .reset_codes
            .get(&email)
            .is_some_and(|entry| entry.expires_at <= now || entry.attempts >= RESET_MAX_ATTEMPTS);
        if matched || spent {
            db.reset_codes.remove(&email);
        }
        if !matched {
            return Err(ApiError::InvalidInput(RESET_REJECTED.into()));
        }
        db.account_by_email(&email)
            .map(|a| a.user.id.clone())
            .ok_or_else(|| ApiError::InvalidInput(RESET_REJECTED.into()))
    })?;
    let password_hash = password::hash_async(body.new_password).await;
    state.write(|db| {
        if let Some(account) = db.accounts.iter_mut().find(|a| a.user.id == user_id) {
            account.password_hash = password_hash;
        }
        db.sessions.retain(|_, session| session.user_id != user_id);
    });
    Ok(Json(json!({ "ok": true })))
}

async fn get_medicines(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Medicine>>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| db.medicines.get(&id).cloned().unwrap_or_default())))
}

async fn get_audit(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<AuditEntry>>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| {
        let mut items: Vec<AuditEntry> = db.audit.iter().filter(|e| e.subject_id == id).cloned().collect();
        items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        items
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssuePrescriptionBody {
    patient_id: String,
    doctor_id: String,
    items: Vec<Medicine>,
}

async fn issue_prescription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<IssuePrescriptionBody>,
) -> ApiResult<Json<Prescription>> {
    auth.require(&[ROLE_DOCTOR])?;
    Ok(Json(state.write(|db| {
        db.medicines
            .entry(body.patient_id.clone())
            .or_default()
            .extend(body.items.clone());
        db.push_audit(&body.doctor_id, "Issued a prescription", &body.patient_id);
        let prescription = Prescription {
            id: new_id(),
            patient_id: body.patient_id.clone(),
            patient_name: db.name_of(&body.patient_id),
            doctor_id: body.doctor_id.clone(),
            items: body.items.clone(),
            issued_at: now_millis(),
            fulfilled: false,
        };
        db.prescriptions.push(prescription.clone());
        prescription
    })))
}

async fn pending_prescriptions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<Vec<Prescription>>> {
    auth.require(&[ROLE_PHARMACY])?;
    Ok(Json(state.read(|db| {
        let mut items: Vec<Prescription> = db.prescriptions.iter().filter(|p| !p.fulfilled).cloned().collect();
        items.sort_by(|a, b| b.issued_at.cmp(&a.issued_at));
        items
    })))
}

async fn fulfill_prescription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Prescription>> {
    auth.require(&[ROLE_PHARMACY])?;
    state.write(|db| {
        let index = db
            .prescriptions
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| ApiError::NotFound("Prescription not found".into()))?;
        db.prescriptions[index].fulfilled = true;
        let updated = db.prescriptions[index].clone();
        db.push_audit("Pharmacy", "Fulfilled a prescription", &updated.patient_id);
        Ok(Json(updated))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantBody {
    patient_id: String,
    grantee_id: String,
    scope: Vec<String>,
    expires_at: i64,
}

async fn grant_access(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<GrantBody>,
) -> ApiResult<Json<AccessGrant>> {
    auth.require_self(&body.patient_id)?;
    Ok(Json(state.write(|db| {
        let grant = AccessGrant {
            id: new_id(),
            patient_id: body.patient_id.clone(),
            grantee_id: body.grantee_id.clone(),
            scope: body.scope.clone(),
            issued_at: now_millis(),
            expires_at: body.expires_at,
            revoked: false,
        };
        db.grants.push(grant.clone());
        db.push_audit("You", "Granted access", &body.patient_id);
        grant
    })))
}

async fn revoke_grant(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<AccessGrant>> {
    state.write(|db| {
        let index = db
            .grants
            .iter()
            .position(|g| g.id == id)
            .ok_or_else(|| ApiError::NotFound("Grant not found".into()))?;
        if db.grants[index].patient_id != auth.id {
            return Err(ApiError::Forbidden("Only the patient can revoke this consent".into()));
        }
        db.grants[index].revoked = true;
        let updated = db.grants[index].clone();
        db.push_audit("You", "Revoked access", &updated.patient_id);
        Ok(Json(updated))
    })
}

async fn verify_grant(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<ScopedData>> {
    state.write(|db| {
        let grant = db
            .grants
            .iter()
            .find(|g| g.id == id)
            .cloned()
            .ok_or_else(|| ApiError::NotFound("Grant not found".into()))?;
        if grant.grantee_id != auth.id {
            return Err(ApiError::Forbidden("This consent was not granted to you".into()));
        }
        if grant.revoked || now_millis() > grant.expires_at {
            db.push_audit(&grant.grantee_id, "Access denied", &grant.patient_id);
            return Err(ApiError::Unauthorized("Access denied".into()));
        }
        db.push_audit(&grant.grantee_id, "Viewed shared health record", &grant.patient_id);
        let mut lab_orders: Vec<LabOrder> = db
            .lab_orders
            .iter()
            .filter(|o| o.patient_id == grant.patient_id)
            .cloned()
            .collect();
        lab_orders.sort_by(|a, b| b.ordered_at.cmp(&a.ordered_at));
        let mut reports: Vec<MedicalReport> = db
            .reports
            .iter()
            .filter(|r| r.patient_id == grant.patient_id)
            .cloned()
            .collect();
        reports.sort_by(|a, b| b.added_at.cmp(&a.added_at));
        Ok(Json(ScopedData {
            patient_name: db.name_of(&grant.patient_id),
            medicines: db.medicines.get(&grant.patient_id).cloned().unwrap_or_default(),
            allergies: db.allergies.get(&grant.patient_id).cloned().unwrap_or_default(),
            profile: db.profiles.get(&grant.patient_id).cloned().unwrap_or_default(),
            lab_orders,
            reports,
        }))
    })
}

async fn active_grants(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Json<Vec<AccessGrant>>> {
    let grantee = params.get("granteeId").cloned().unwrap_or_default();
    if grantee != auth.id {
        return Err(ApiError::Forbidden("You can only list consents granted to you".into()));
    }
    Ok(Json(state.read(|db| {
        let now = now_millis();
        let mut items: Vec<AccessGrant> = db
            .grants
            .iter()
            .filter(|g| g.grantee_id == grantee && !g.revoked && g.expires_at > now)
            .cloned()
            .collect();
        items.sort_by(|a, b| b.issued_at.cmp(&a.issued_at));
        items
    })))
}

async fn reception_token(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<Value>> {
    auth.require(&[ROLE_RECEPTIONIST])?;
    Ok(Json(state.write(|db| {
        let token = db.reception_token.clone().unwrap_or_else(|| {
            let fresh = random_code();
            db.reception_token = Some(fresh.clone());
            fresh
        });
        json!({ "token": token })
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckInBody {
    patient_id: String,
    token: String,
}

async fn check_in(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CheckInBody>,
) -> ApiResult<Json<Appointment>> {
    auth.require_self(&body.patient_id)?;
    state.write(|db| {
        let active = db
            .reception_token
            .clone()
            .ok_or_else(|| ApiError::InvalidInput("Reception is not accepting check-ins right now".into()))?;
        if body.token.trim() != active {
            return Err(ApiError::InvalidInput("That code does not match the reception desk".into()));
        }
        if db.appointments.iter().any(|a| a.patient_id == body.patient_id) {
            return Err(ApiError::Conflict("You are already in the queue".into()));
        }
        let appointment = Appointment {
            id: new_id(),
            patient_id: body.patient_id.clone(),
            patient_name: db.name_of(&body.patient_id),
            checked_in_at: now_millis(),
        };
        db.appointments.push(appointment.clone());
        db.push_audit("Reception", "Checked in at the front desk", &body.patient_id);
        Ok(Json(appointment))
    })
}

async fn queue(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<Vec<Appointment>>> {
    auth.require(&[ROLE_RECEPTIONIST, ROLE_DOCTOR])?;
    Ok(Json(state.read(|db| {
        let mut items = db.appointments.clone();
        items.sort_by(|a, b| a.checked_in_at.cmp(&b.checked_in_at));
        items
    })))
}

async fn admit(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Appointment>> {
    auth.require(&[ROLE_RECEPTIONIST])?;
    state.write(|db| {
        let index = db
            .appointments
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| ApiError::NotFound("Appointment not found".into()))?;
        let appointment = db.appointments.remove(index);
        db.push_audit("Reception", "Admitted for consultation", &appointment.patient_id);
        Ok(Json(appointment))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderLabBody {
    patient_id: String,
    test_name: String,
    ordered_by: String,
}

async fn order_lab_test(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<OrderLabBody>,
) -> ApiResult<Json<LabOrder>> {
    auth.require(&[ROLE_DOCTOR])?;
    Ok(Json(state.write(|db| {
        let order = LabOrder {
            id: new_id(),
            patient_id: body.patient_id.clone(),
            patient_name: db.name_of(&body.patient_id),
            test_name: body.test_name.clone(),
            ordered_by: body.ordered_by.clone(),
            ordered_at: now_millis(),
            status: "ORDERED".to_string(),
            result_summary: None,
            completed_at: None,
        };
        db.lab_orders.push(order.clone());
        db.push_audit(
            &body.ordered_by,
            &format!("Ordered lab test: {}", body.test_name),
            &body.patient_id,
        );
        order
    })))
}

async fn all_lab_orders(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<Vec<LabOrder>>> {
    auth.require(&[ROLE_DIAGNOSTIC])?;
    Ok(Json(state.read(|db| {
        let mut items = db.lab_orders.clone();
        items.sort_by(|a, b| {
            let done_a = a.status == "COMPLETED";
            let done_b = b.status == "COMPLETED";
            done_a.cmp(&done_b).then(b.ordered_at.cmp(&a.ordered_at))
        });
        items
    })))
}

async fn patient_lab_orders(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<LabOrder>>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| {
        let mut items: Vec<LabOrder> = db.lab_orders.iter().filter(|o| o.patient_id == id).cloned().collect();
        items.sort_by(|a, b| b.ordered_at.cmp(&a.ordered_at));
        items
    })))
}

async fn collect_sample(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<LabOrder>> {
    auth.require(&[ROLE_DIAGNOSTIC])?;
    state.write(|db| {
        let index = db
            .lab_orders
            .iter()
            .position(|o| o.id == id)
            .ok_or_else(|| ApiError::NotFound("Lab order not found".into()))?;
        if db.lab_orders[index].status != "ORDERED" {
            return Err(ApiError::Conflict("Sample already collected".into()));
        }
        db.lab_orders[index].status = "SAMPLE_COLLECTED".to_string();
        let updated = db.lab_orders[index].clone();
        db.push_audit(
            "Diagnostics",
            &format!("Collected sample for {}", updated.test_name),
            &updated.patient_id,
        );
        Ok(Json(updated))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompleteLabBody {
    result_summary: String,
}

async fn complete_lab_order(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<CompleteLabBody>,
) -> ApiResult<Json<LabOrder>> {
    auth.require(&[ROLE_DIAGNOSTIC])?;
    state.write(|db| {
        let index = db
            .lab_orders
            .iter()
            .position(|o| o.id == id)
            .ok_or_else(|| ApiError::NotFound("Lab order not found".into()))?;
        if db.lab_orders[index].status == "COMPLETED" {
            return Err(ApiError::Conflict("Report already published".into()));
        }
        db.lab_orders[index].status = "COMPLETED".to_string();
        db.lab_orders[index].result_summary = Some(body.result_summary.clone());
        db.lab_orders[index].completed_at = Some(now_millis());
        let updated = db.lab_orders[index].clone();
        db.push_audit(
            "Diagnostics",
            &format!("Published report for {}", updated.test_name),
            &updated.patient_id,
        );
        Ok(Json(updated))
    })
}

async fn platform_stats(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<PlatformStats>> {
    auth.require(&[ROLE_ADMIN, ROLE_SUPER_ADMIN])?;
    Ok(Json(state.read(|db| {
        let now = now_millis();
        PlatformStats {
            total_users: db.accounts.len(),
            patients: db
                .accounts
                .iter()
                .filter(|a| a.user.roles.iter().any(|r| r == ROLE_PATIENT))
                .count(),
            staff: db
                .accounts
                .iter()
                .filter(|a| !a.user.roles.iter().any(|r| r == ROLE_PATIENT))
                .count(),
            prescriptions_issued: db.prescriptions.len(),
            prescriptions_pending: db.prescriptions.iter().filter(|p| !p.fulfilled).count(),
            lab_orders_open: db.lab_orders.iter().filter(|o| o.status != "COMPLETED").count(),
            lab_orders_completed: db.lab_orders.iter().filter(|o| o.status == "COMPLETED").count(),
            active_grants: db
                .grants
                .iter()
                .filter(|g| !g.revoked && g.expires_at > now)
                .count(),
            revoked_grants: db.grants.iter().filter(|g| g.revoked).count(),
            waiting_in_queue: db.appointments.len(),
            open_tickets: db.tickets.iter().filter(|t| t.status == "OPEN").count(),
        }
    })))
}

async fn list_users(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<Vec<User>>> {
    auth.require(&[ROLE_ADMIN, ROLE_SUPER_ADMIN])?;
    Ok(Json(state.read(|db| db.accounts.iter().map(|a| a.user.clone()).collect())))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateStaffBody {
    name: String,
    email: String,
    password: String,
    role: String,
    scope_id: Option<String>,
}

async fn create_staff(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateStaffBody>,
) -> ApiResult<Json<User>> {
    auth.require(&[ROLE_SUPER_ADMIN])?;
    if state.read(|db| db.account_by_email(&body.email).is_some()) {
        return Err(ApiError::Conflict("An account with this email already exists".into()));
    }
    let password_hash = password::hash_async(body.password.clone()).await;
    state.write(|db| {
        if db.account_by_email(&body.email).is_some() {
            return Err(ApiError::Conflict("An account with this email already exists".into()));
        }
        let user = User {
            id: format!("u_{}", new_id()),
            name: body.name.clone(),
            email: body.email.clone(),
            roles: vec![body.role.clone()],
            scope_id: body.scope_id.clone(),
            phone: None,
        };
        db.accounts.push(Account {
            user: user.clone(),
            password_hash,
        });
        Ok(Json(user))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RaiseTicketBody {
    user_id: String,
    subject: String,
    detail: String,
}

async fn raise_ticket(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<RaiseTicketBody>,
) -> ApiResult<Json<SupportTicket>> {
    auth.require_self(&body.user_id)?;
    Ok(Json(state.write(|db| {
        let ticket = SupportTicket {
            id: new_id(),
            raised_by: body.user_id.clone(),
            raised_by_name: db.name_of(&body.user_id),
            subject: body.subject.clone(),
            detail: body.detail.clone(),
            status: "OPEN".to_string(),
            created_at: now_millis(),
            resolution: None,
            resolved_at: None,
        };
        db.tickets.push(ticket.clone());
        db.push_audit("You", "Raised a support ticket", &body.user_id);
        ticket
    })))
}

async fn all_tickets(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> ApiResult<Json<Vec<SupportTicket>>> {
    auth.require(&[ROLE_CUSTOMER_CARE])?;
    Ok(Json(state.read(|db| {
        let mut items = db.tickets.clone();
        items.sort_by(|a, b| {
            let done_a = a.status == "RESOLVED";
            let done_b = b.status == "RESOLVED";
            done_a.cmp(&done_b).then(b.created_at.cmp(&a.created_at))
        });
        items
    })))
}

async fn patient_tickets(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<SupportTicket>>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| {
        let mut items: Vec<SupportTicket> = db.tickets.iter().filter(|t| t.raised_by == id).cloned().collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveTicketBody {
    resolution: String,
}

async fn resolve_ticket(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<ResolveTicketBody>,
) -> ApiResult<Json<SupportTicket>> {
    auth.require(&[ROLE_CUSTOMER_CARE])?;
    state.write(|db| {
        let index = db
            .tickets
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| ApiError::NotFound("Ticket not found".into()))?;
        if db.tickets[index].status == "RESOLVED" {
            return Err(ApiError::Conflict("Ticket already resolved".into()));
        }
        db.tickets[index].status = "RESOLVED".to_string();
        db.tickets[index].resolution = Some(body.resolution.clone());
        db.tickets[index].resolved_at = Some(now_millis());
        let updated = db.tickets[index].clone();
        db.push_audit("Support", "Resolved your support ticket", &updated.raised_by);
        Ok(Json(updated))
    })
}

async fn recommendations(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Recommendation>>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| {
        let profile = db.profiles.get(&id).cloned().unwrap_or_default();
        let medicines = db.medicines.get(&id).cloned().unwrap_or_default();
        let labs: Vec<LabOrder> = db
            .lab_orders
            .iter()
            .filter(|o| o.patient_id == id)
            .cloned()
            .collect();
        crate::recommend::build(&profile, &medicines, &labs)
    })))
}

async fn get_profile(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<PatientProfile>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| db.profiles.get(&id).cloned().unwrap_or_default())))
}

async fn save_profile(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<PatientProfile>,
) -> ApiResult<Json<PatientProfile>> {
    auth.require_self(&id)?;
    Ok(Json(state.write(|db| {
        db.profiles.insert(id.clone(), body.clone());
        body
    })))
}

async fn get_reports(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<MedicalReport>>> {
    auth.require_self_or_staff(&id)?;
    Ok(Json(state.read(|db| {
        let mut items: Vec<MedicalReport> = db.reports.iter().filter(|r| r.patient_id == id).cloned().collect();
        items.sort_by(|a, b| b.added_at.cmp(&a.added_at));
        items
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddReportBody {
    category: String,
    label: String,
    #[serde(default)]
    uri: String,
    #[serde(default)]
    data: String,
}

async fn add_report(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<AddReportBody>,
) -> ApiResult<Json<MedicalReport>> {
    auth.require_self(&id)?;
    let report_id = new_id();
    let mut file_bytes = 0usize;
    let mut stored = false;
    if !body.data.is_empty() {
        file_bytes = crate::storage::save_upload(&report_id, &body.data).map_err(ApiError::InvalidInput)?;
        stored = true;
    }
    Ok(Json(state.write(|db| {
        let report = MedicalReport {
            id: report_id.clone(),
            patient_id: id.clone(),
            file_bytes,
            stored,
            category: body.category.clone(),
            label: if body.label.trim().is_empty() {
                body.category.clone()
            } else {
                body.label.clone()
            },
            uri: body.uri.clone(),
            added_at: now_millis(),
        };
        db.reports.push(report.clone());
        db.push_audit("You", &format!("Uploaded a {} report", body.category), &id);
        report
    })))
}

async fn download_report(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Vec<u8>> {
    let owner = state.read(|db| db.reports.iter().find(|r| r.id == id).map(|r| r.patient_id.clone()));
    let patient_id = owner.ok_or_else(|| ApiError::NotFound("Report not found".into()))?;
    auth.require_self_or_staff(&patient_id)?;
    crate::storage::read_upload(&id).ok_or_else(|| ApiError::NotFound("No file stored for this report".into()))
}

async fn remove_report(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    state.write(|db| {
        let owner = db.reports.iter().find(|r| r.id == id).map(|r| r.patient_id.clone());
        match owner {
            Some(patient_id) if patient_id == auth.id => {
                db.reports.retain(|r| r.id != id);
                crate::storage::delete_upload(&id);
                Ok(Json(json!({ "ok": true })))
            }
            Some(_) => Err(ApiError::Forbidden("You can only delete your own reports".into())),
            None => Err(ApiError::NotFound("Report not found".into())),
        }
    })
}
