use crate::errors::KamError;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use base64::engine::Engine as _;

pub fn write_sigstore_bundle(
    out_dir: &Path,
    filename: &str,
    payload: &Value,
    signature: &[u8],
    cert_der: Option<&[u8]>,
) -> Result<PathBuf, KamError> {
    // Base64 encode payload
    let payload_bytes = serde_json::to_vec(payload).map_err(|e| KamError::CommandFailed(format!("Failed to serialize payload: {}", e)))?;
    let payload_b64 = BASE64_ENGINE.encode(&payload_bytes);
    // Base64 encode signature
    let sig_b64 = BASE64_ENGINE.encode(signature);
    // Prepare verificationMaterial
    let mut certificate_json = serde_json::Map::new();
    if let Some(cert) = cert_der {
        certificate_json.insert(
            "rawBytes".to_string(),
            serde_json::Value::String(BASE64_ENGINE.encode(cert)),
        );
    }
    let verification = if !certificate_json.is_empty() {
        let mut v = serde_json::Map::new();
        v.insert("certificate".to_string(), serde_json::Value::Object(certificate_json));
        serde_json::Value::Object(v)
    } else {
        serde_json::Value::Null
    };

    let bundle = serde_json::json!({
        "dsseEnvelope": {
            "payload": payload_b64,
            "payloadType": "application/vnd.in-toto+json",
            "signatures": [ { "sig": sig_b64 } ]
        },
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "verificationMaterial": verification
    });

    let bundle_path = out_dir.join(format!("{}.sigstore.json", filename));
    let mut f = fs::File::create(&bundle_path).map_err(KamError::Io)?;
    let v = serde_json::to_vec_pretty(&bundle).map_err(|e| KamError::CommandFailed(format!("Failed to serialize bundle JSON: {}", e)))?;
    f.write_all(&v).map_err(KamError::Io)?;
    Ok(bundle_path)
}
