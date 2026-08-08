use std::path::PathBuf;

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn uploads_dir() -> PathBuf {
    let dir = std::env::var("XPULSE_UPLOADS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("uploads"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn save_upload(report_id: &str, base64_data: &str) -> Result<usize, String> {
    let bytes = decode_base64(base64_data)?;
    if bytes.is_empty() {
        return Err("Uploaded file was empty".to_string());
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err("File is larger than the 8 MB limit".to_string());
    }
    let path = uploads_dir().join(format!("{report_id}.bin"));
    std::fs::write(path, &bytes).map_err(|e| format!("Could not store file: {e}"))?;
    Ok(bytes.len())
}

pub fn read_upload(report_id: &str) -> Option<Vec<u8>> {
    std::fs::read(uploads_dir().join(format!("{report_id}.bin"))).ok()
}

pub fn delete_upload(report_id: &str) {
    let _ = std::fs::remove_file(uploads_dir().join(format!("{report_id}.bin")));
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();

    let mut lookup = [255u8; 256];
    for (index, value) in B64.iter().enumerate() {
        lookup[*value as usize] = index as u8;
    }

    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in cleaned {
        let value = lookup[byte as usize];
        if value == 255 {
            return Err("Upload was not valid base64".to_string());
        }
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Ok(out)
}

const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
