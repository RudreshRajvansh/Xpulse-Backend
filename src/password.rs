use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use std::sync::OnceLock;

pub fn hash(plain: &str) -> String {
    let seed = uuid::Uuid::new_v4();
    let salt = SaltString::encode_b64(seed.as_bytes()).expect("salt encoding failed");
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .expect("password hashing failed")
        .to_string()
}

pub fn verify(plain: &str, stored: &str) -> bool {
    PasswordHash::new(stored)
        .map(|parsed| Argon2::default().verify_password(plain.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

fn decoy_hash() -> &'static str {
    static DECOY: OnceLock<String> = OnceLock::new();
    DECOY.get_or_init(|| hash(&uuid::Uuid::new_v4().to_string()))
}

pub fn warm_up() {
    let _ = decoy_hash();
}

pub async fn hash_async(plain: String) -> String {
    tokio::task::spawn_blocking(move || hash(&plain))
        .await
        .expect("password hashing task failed")
}

pub async fn verify_async(plain: String, stored: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || match stored {
        Some(stored) => verify(&plain, &stored),
        None => {
            let _ = verify(&plain, decoy_hash());
            false
        }
    })
    .await
    .unwrap_or(false)
}
