use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub scope_id: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub user: User,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Medicine {
    pub id: String,
    pub name: String,
    pub dose: String,
    pub frequency: String,
    pub is_prn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prescription {
    pub id: String,
    pub patient_id: String,
    pub patient_name: String,
    pub doctor_id: String,
    pub items: Vec<Medicine>,
    pub issued_at: i64,
    pub fulfilled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessGrant {
    pub id: String,
    pub patient_id: String,
    pub grantee_id: String,
    pub scope: Vec<String>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: String,
    pub actor: String,
    pub action: String,
    pub subject_id: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appointment {
    pub id: String,
    pub patient_id: String,
    pub patient_name: String,
    pub checked_in_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabOrder {
    pub id: String,
    pub patient_id: String,
    pub patient_name: String,
    pub test_name: String,
    pub ordered_by: String,
    pub ordered_at: i64,
    pub status: String,
    pub result_summary: Option<String>,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportTicket {
    pub id: String,
    pub raised_by: String,
    pub raised_by_name: String,
    pub subject: String,
    pub detail: String,
    pub status: String,
    pub created_at: i64,
    pub resolution: Option<String>,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatientProfile {
    #[serde(default)]
    pub age: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub height_cm: String,
    #[serde(default)]
    pub weight_kg: String,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(default)]
    pub abha_connected: bool,
    #[serde(default)]
    pub insurance_connected: bool,
    #[serde(default)]
    pub onboarded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicalReport {
    pub id: String,
    pub patient_id: String,
    pub category: String,
    pub label: String,
    pub uri: String,
    pub added_at: i64,
    #[serde(default)]
    pub file_bytes: usize,
    #[serde(default)]
    pub stored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedData {
    pub patient_name: String,
    pub scope: Vec<String>,
    pub medicines: Vec<Medicine>,
    pub allergies: Vec<String>,
    pub profile: PatientProfile,
    pub lab_orders: Vec<LabOrder>,
    pub reports: Vec<MedicalReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub title: String,
    pub specialty: String,
    pub reason: String,
    pub urgency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformStats {
    pub total_users: usize,
    pub patients: usize,
    pub staff: usize,
    pub prescriptions_issued: usize,
    pub prescriptions_pending: usize,
    pub lab_orders_open: usize,
    pub lab_orders_completed: usize,
    pub active_grants: usize,
    pub revoked_grants: usize,
    pub waiting_in_queue: usize,
    pub open_tickets: usize,
}
