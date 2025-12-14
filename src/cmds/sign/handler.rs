use crate::cmds::secret::read_secret_plaintext;
use crate::cmds::sign::args::SignArgs;
use crate::errors::KamError;
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// 签名单个文件
// 支持多种私钥来源：文件、环境变量、keyring
fn sign_single_file(src_path: &Path, args: &SignArgs) -> Result<(), KamError> {
    // 确保输出目录存在
    let out_dir = Path::new(&args.out);
    if !out_dir.exists() {
        fs::create_dir_all(out_dir).map_err(KamError::Io)?;
    }

    // 读取私钥：优先级 --key-path > 环境变量 KAM_SIGN_KEY > keyring里的secret
    let pem_bytes = if let Some(kp) = args.key_path.as_ref() {
        fs::read(kp).map_err(KamError::Io)?
    } else if let Ok(env_key) = env::var("KAM_SIGN_KEY") {
        if env_key.trim().is_empty() {
            // 环境变量是空的，当作不存在，继续用secret
             read_secret_plaintext(&args.secret, true)?
        } else {
             env_key.into_bytes()
        }
    } else {
        read_secret_plaintext(&args.secret, true)?
    };

    // 尝试解析PEM，如果失败且提供了密码，就用密码再试一次
    // 这样支持加密的私钥
    let pkey = match PKey::private_key_from_pem(&pem_bytes) {
        Ok(pk) => pk,
        Err(orig_err) => {
            if let Ok(pass) = std::env::var("KAM_SIGN_PASSPHRASE") {
                // 用密码尝试解析加密的PEM
                PKey::private_key_from_pem_passphrase(&pem_bytes, pass.as_bytes()).map_err(|e| {
                    KamError::CommandFailed(format!(
                        "Failed to parse private key PEM with passphrase: {}",
                        e
                    ))
                })?
            } else {
                return Err(KamError::CommandFailed(format!(
                    "Failed to parse private key PEM: {}",
                    orig_err
                )));
            }
        }
    };

    // 读取要签名的文件
    let data = fs::read(src_path).map_err(KamError::Io)?;

    // 签名（用SHA-256哈希）
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(format!("Failed to create signer: {}", e)))?;
    signer
        .update(&data)
        .map_err(|e| KamError::CommandFailed(format!("Failed to update signer: {}", e)))?;
    let sig_der = signer
        .sign_to_vec()
        .map_err(|e| KamError::CommandFailed(format!("Failed to sign: {}", e)))?;

    let filename = src_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| KamError::InvalidFilename("Invalid source filename".to_string()))?;

    // 可选：包含证书链（如果提供了的话）
    // 会写一个.cert.pem文件，和签名文件放在一起
    if let Some(cert_path) = args.cert.as_ref() {
        let cert_data = fs::read_to_string(cert_path).map_err(KamError::Io)?;
        let cert_file = out_dir.join(format!("{}.cert.pem", filename));
        fs::write(&cert_file, cert_data).map_err(KamError::Io)?;
    }

    // 写签名文件（.sig），base64编码
    let path = out_dir.join(format!("{}.sig", filename));
    let sig_b64 = BASE64_ENGINE.encode(&sig_der);
    fs::write(&path, sig_b64.as_bytes()).map_err(KamError::Io)?;
    use crate::utils::Utils;
    Utils::success(&trf!("sign.signed", filename, path.display()));

    Ok(())
}

// 签名命令的主入口
pub fn run(args: SignArgs) -> Result<(), KamError> {
    // 如果指定了src，就签名单个文件
    if let Some(src_str) = args.src.as_ref() {
        let src_path = Path::new(src_str);
        return sign_single_file(src_path, &args);
    }

    // decide dist dir from args.dist or --all
    let dist_dir: PathBuf = if let Some(d) = args.dist.clone() {
        d
    } else if args.all {
        PathBuf::from(&args.out)
    } else {
        return Err(KamError::CommandFailed(trf!("sign.no_src_or_dist")));
    }
    };
    for entry in std::fs::read_dir(dist_dir).map_err(KamError::Io)? {
        let entry = entry.map_err(KamError::Io)?;
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            match ext {
                "sig" | "tsr" | "json" => continue,
                _ => (),
            }
        }
        if let Err(e) = sign_single_file(&p, &args) {
            use crate::utils::Utils;
            Utils::error(&trf!("sign.failed_to_sign", p.display(), e));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::rsa::Rsa;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn sign_creates_sig_basic() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("artifact.zip");
        let mut f = File::create(&src_path).unwrap();
        writeln!(f, "hello").unwrap();

        // Generate a private key PEM for testing
        let rsa = Rsa::generate(2048).unwrap();
        let key_pem = rsa.private_key_to_pem().unwrap();
        let key_path = dir.path().join("key.pem");
        std::fs::write(&key_path, &key_pem).unwrap();

        let out_dir = dir.path().join("out");
        let args = SignArgs {
            src: Some(src_path.to_string_lossy().to_string()),
            secret: "main".to_string(),
            out: out_dir.to_string_lossy().to_string(),
            cert: None,
            key_path: Some(key_path.to_string_lossy().to_string()),
            dist: None,
            all: false,
        };
        let res = run(args);
        assert!(res.is_ok());
        // Check .sig exists
        let sig = out_dir.join("artifact.zip.sig");
        assert!(sig.exists());
    }
}
