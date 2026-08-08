use axum_server::tls_rustls::RustlsConfig;
use std::path::{Path, PathBuf};

pub async fn config(dir: &Path) -> RustlsConfig {
    let cert_path = dir.join("xpulse-cert.pem");
    let key_path = dir.join("xpulse-key.pem");

    if !cert_path.exists() || !key_path.exists() {
        generate(&cert_path, &key_path);
        println!("generated self-signed certificate at {}", cert_path.display());
    }

    RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .expect("failed to load tls certificate")
}

fn generate(cert_path: &PathBuf, key_path: &PathBuf) {
    let subject_alt_names = vec![
        "localhost".to_string(),
        "10.0.2.2".to_string(),
        "127.0.0.1".to_string(),
    ];
    let certified = rcgen::generate_simple_self_signed(subject_alt_names)
        .expect("failed to generate self-signed certificate");
    std::fs::write(cert_path, certified.cert.pem()).expect("failed to write certificate");
    std::fs::write(key_path, certified.key_pair.serialize_pem()).expect("failed to write key");
}
