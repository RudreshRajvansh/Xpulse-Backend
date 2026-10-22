use crate::auth::AuthUser;
use crate::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn random_code() -> String {
    let source = uuid::Uuid::new_v4();
    let bytes = source.as_bytes();
    let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    format!("{:06}", 100_000 + value % 900_000)
}

pub type ResetCodeSink = Arc<dyn Fn(&str, &str) + Send + Sync>;

pub const DEFAULT_SESSION_TTL_MS: i64 = 12 * 60 * 60 * 1000;

#[derive(Clone)]
pub struct Config {
    pub reset_code_sink: Option<ResetCodeSink>,
    pub session_ttl_ms: i64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            reset_code_sink: None,
            session_ttl_ms: DEFAULT_SESSION_TTL_MS,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let session_ttl_ms = std::env::var("XPULSE_SESSION_TTL_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|secs| *secs > 0)
            .map(|secs| secs * 1000)
            .unwrap_or(DEFAULT_SESSION_TTL_MS);
        let dev_mode = std::env::var("XPULSE_DEV").map(|v| v == "1").unwrap_or(false);
        let sink: Option<ResetCodeSink> = if dev_mode {
            Some(Arc::new(|email: &str, code: &str| {
                println!("password reset code for {email}: {code}");
            }))
        } else {
            None
        };
        Config {
            reset_code_sink: sink,
            session_ttl_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct ResetCode {
    pub code: String,
    pub expires_at: i64,
    pub attempts: u32,
}

pub enum SeedPasswords {
    Shared(String),
    Random,
}

impl SeedPasswords {
    pub fn from_env() -> Self {
        match std::env::var("XPULSE_SEED_PASSWORD") {
            Ok(value) if !value.trim().is_empty() => SeedPasswords::Shared(value),
            _ => SeedPasswords::Random,
        }
    }

    fn next(&self) -> String {
        match self {
            SeedPasswords::Shared(value) => value.clone(),
            SeedPasswords::Random => uuid::Uuid::new_v4().simple().to_string()[..16].to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeedCredential {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Db {
    pub accounts: Vec<Account>,
    pub medicines: HashMap<String, Vec<Medicine>>,
    pub allergies: HashMap<String, Vec<String>>,
    pub grants: Vec<AccessGrant>,
    pub audit: Vec<AuditEntry>,
    pub prescriptions: Vec<Prescription>,
    pub appointments: Vec<Appointment>,
    pub lab_orders: Vec<LabOrder>,
    pub tickets: Vec<SupportTicket>,
    pub profiles: HashMap<String, PatientProfile>,
    pub reports: Vec<MedicalReport>,
    #[serde(skip)]
    pub reset_codes: HashMap<String, ResetCode>,
    #[serde(skip)]
    pub sessions: HashMap<String, Session>,
    pub facilities: Vec<Facility>,
    pub reception_codes: HashMap<String, String>,
}

impl Db {
    pub fn facility_exists(&self, facility_id: &str) -> bool {
        self.facilities.iter().any(|f| f.id == facility_id)
    }

    pub fn area_exists(&self, area_id: &str) -> bool {
        self.facilities.iter().any(|f| f.area_id == area_id)
    }

    pub fn facilities_in_area(&self, area_id: &str) -> Vec<String> {
        self.facilities
            .iter()
            .filter(|f| f.area_id == area_id)
            .map(|f| f.id.clone())
            .collect()
    }

    pub fn facility_of(&self, user_id: &str) -> Option<String> {
        self.accounts
            .iter()
            .find(|a| a.user.id == user_id)
            .and_then(|a| a.user.scope_id.clone())
            .filter(|scope| self.facility_exists(scope))
    }
    pub fn active_grant(&self, patient_id: &str, grantee_id: &str, scope: &str) -> Option<&AccessGrant> {
        let now = now_millis();
        self.grants.iter().find(|g| {
            g.patient_id == patient_id
                && g.grantee_id == grantee_id
                && !g.revoked
                && g.expires_at > now
                && g.scope.iter().any(|s| s == scope)
        })
    }

    pub fn account_by_email(&self, email: &str) -> Option<&Account> {
        let wanted = email.trim();
        self.accounts.iter().find(|a| a.user.email.eq_ignore_ascii_case(wanted))
    }

    pub fn name_of(&self, user_id: &str) -> String {
        self.accounts
            .iter()
            .find(|a| a.user.id == user_id)
            .map(|a| a.user.name.clone())
            .unwrap_or_else(|| "Patient".to_string())
    }

    pub fn push_audit(&mut self, actor: &AuthUser, action: &str, subject_id: &str) {
        self.audit.push(AuditEntry {
            id: new_id(),
            actor: actor.id.clone(),
            actor_name: actor.name.clone(),
            action: action.to_string(),
            subject_id: subject_id.to_string(),
            timestamp: now_millis(),
        });
    }

    fn seed(passwords: &SeedPasswords) -> (Self, Vec<SeedCredential>) {
        let mut db = Db::default();
        let mut generated = Vec::new();
        for (id, name, area) in [
            ("fac_1", "City Care Clinic", "area_1"),
            ("fac_2", "Sunrise Clinic", "area_1"),
            ("fac_3", "Lakeview Hospital", "area_2"),
        ] {
            db.facilities.push(Facility {
                id: id.to_string(),
                name: name.to_string(),
                area_id: area.to_string(),
            });
        }
        let staff = [
            ("u_patient", "Aarav Sharma", "patient@xpulse.in", "PATIENT", None),
            ("u_doctor", "Dr. Mehta", "doctor@xpulse.in", "DOCTOR", Some("fac_1")),
            ("u_reception", "Priya Nair", "reception@xpulse.in", "RECEPTIONIST", Some("fac_1")),
            ("u_pharmacy", "MedPlus Pharmacy", "pharmacy@xpulse.in", "PHARMACY", Some("fac_1")),
            ("u_diagnostic", "City Diagnostics", "diagnostic@xpulse.in", "DIAGNOSTIC", Some("fac_1")),
            ("u_admin", "Area Admin", "admin@xpulse.in", "ADMIN", Some("area_1")),
            ("u_super_admin", "Super Admin", "superadmin@xpulse.in", "SUPER_ADMIN", None),
            ("u_customer_care", "Support Desk", "care@xpulse.in", "CUSTOMER_CARE", None),
        ];
        for (id, name, email, role, scope) in staff {
            let password = passwords.next();
            if matches!(passwords, SeedPasswords::Random) {
                generated.push(SeedCredential {
                    email: email.to_string(),
                    password: password.clone(),
                });
            }
            db.accounts.push(Account {
                user: User {
                    id: id.to_string(),
                    name: name.to_string(),
                    email: email.to_string(),
                    roles: vec![role.to_string()],
                    scope_id: scope.map(|s| s.to_string()),
                    phone: None,
                },
                password_hash: crate::password::hash(&password),
            });
        }

        db.medicines.insert(
            "u_patient".to_string(),
            vec![
                Medicine {
                    id: "m1".to_string(),
                    name: "Metformin 500mg".to_string(),
                    dose: "1 tablet".to_string(),
                    frequency: "Twice daily".to_string(),
                    is_prn: false,
                },
                Medicine {
                    id: "m2".to_string(),
                    name: "Telmisartan 40mg".to_string(),
                    dose: "1 tablet".to_string(),
                    frequency: "Once daily".to_string(),
                    is_prn: false,
                },
            ],
        );

        db.allergies.insert(
            "u_patient".to_string(),
            vec!["Penicillin".to_string(), "Sulfa drugs".to_string()],
        );

        db.profiles.insert(
            "u_patient".to_string(),
            PatientProfile {
                age: "34".to_string(),
                city: "Pune".to_string(),
                height_cm: "174".to_string(),
                weight_kg: "78".to_string(),
                conditions: vec!["Diabetes".to_string()],
                abha_connected: false,
                insurance_connected: false,
                onboarded: true,
            },
        );

        let now = now_millis();
        db.lab_orders.push(LabOrder {
            id: "lab_seed_1".to_string(),
            patient_id: "u_patient".to_string(),
            patient_name: "Aarav Sharma".to_string(),
            test_name: "HbA1c".to_string(),
            ordered_by: "u_doctor".to_string(),
            facility_id: "fac_1".to_string(),
            ordered_at: now - 86_400_000,
            status: "COMPLETED".to_string(),
            result_summary: Some("6.8% · slightly above target range".to_string()),
            completed_at: Some(now - 43_200_000),
        });
        db.lab_orders.push(LabOrder {
            id: "lab_seed_2".to_string(),
            patient_id: "u_patient".to_string(),
            patient_name: "Aarav Sharma".to_string(),
            test_name: "Lipid Profile".to_string(),
            ordered_at: now - 3_600_000,
            ordered_by: "u_doctor".to_string(),
            facility_id: "fac_1".to_string(),
            status: "ORDERED".to_string(),
            result_summary: None,
            completed_at: None,
        });

        (db, generated)
    }
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<Db>>,
    path: Option<PathBuf>,
    config: Arc<Config>,
}

impl AppState {
    pub fn load(path: PathBuf, passwords: SeedPasswords) -> (Self, Vec<SeedCredential>) {
        let existing = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Db>(&raw).ok());
        let (db, generated) = match existing {
            Some(db) => (db, Vec::new()),
            None => Db::seed(&passwords),
        };
        let state = AppState {
            inner: Arc::new(Mutex::new(db)),
            path: Some(path),
            config: Arc::new(Config::default()),
        };
        (state, generated)
    }

    pub fn seeded(passwords: SeedPasswords) -> (Self, Vec<SeedCredential>) {
        let (db, generated) = Db::seed(&passwords);
        let state = AppState {
            inner: Arc::new(Mutex::new(db)),
            path: None,
            config: Arc::new(Config::default()),
        };
        (state, generated)
    }

    pub fn ephemeral(passwords: SeedPasswords) -> Self {
        Self::seeded(passwords).0
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Arc::new(config);
        self
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn write_volatile<T>(&self, f: impl FnOnce(&mut Db) -> T) -> T {
        let mut guard = self.inner.lock().expect("store poisoned");
        f(&mut guard)
    }

    pub fn issue_session(&self, user_id: &str) -> String {
        let ttl = self.config.session_ttl_ms;
        self.write_volatile(|db| {
            let now = now_millis();
            db.sessions.retain(|_, session| session.expires_at > now);
            let token = new_id();
            db.sessions.insert(
                token.clone(),
                Session {
                    user_id: user_id.to_string(),
                    expires_at: now + ttl,
                },
            );
            token
        })
    }

    pub fn read<T>(&self, f: impl FnOnce(&Db) -> T) -> T {
        let guard = self.inner.lock().expect("store poisoned");
        f(&guard)
    }

    pub fn write<T>(&self, f: impl FnOnce(&mut Db) -> T) -> T {
        let mut guard = self.inner.lock().expect("store poisoned");
        let out = f(&mut guard);
        if let Some(path) = &self.path {
            if let Ok(raw) = serde_json::to_string_pretty(&*guard) {
                let _ = std::fs::write(path, raw);
            }
        }
        out
    }
}
