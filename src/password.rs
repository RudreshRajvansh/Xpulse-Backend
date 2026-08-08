use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

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
