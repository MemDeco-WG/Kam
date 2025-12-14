use super::args::VerifyArgs;

use crate::errors::KamError;
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Verifier;
use std::fs;
use std::path::Path;

// 验证文件签名
// 这个函数会检查文件是否被正确签名，支持多种公钥来源
pub fn run(args: VerifyArgs) -> Result<(), KamError> {
    // 1. 确定源文件和签名文件路径
    let src_str = args.src.as_ref().ok_or_else(|| {
        KamError::CommandFailed("Source file is required for verification".to_string())
    })?;
    let src_path = Path::new(src_str);

    if !src_path.exists() {
        return Err(KamError::CommandFailed(trf!(
            "Source file not found: {}",
            src_path.display()
        )));
    }

    // 如果没有指定签名文件，就用源文件名+.sig
    let sig_path = if let Some(s) = &args.sig {
        Path::new(s).to_path_buf()
    } else {
        Path::new(&format!("{}.sig", src_str)).to_path_buf()
    };

    if !sig_path.exists() {
        return Err(KamError::CommandFailed(trf!(
            "Signature file not found: {}",
            sig_path.display()
        )));
    }

    // 2. 读取源文件和签名
    let data = fs::read(src_path).map_err(KamError::Io)?;
    let sig_b64 = fs::read_to_string(&sig_path).map_err(KamError::Io)?;
    // 签名是base64编码的，需要解码
    let sig_bytes = BASE64_ENGINE
        .decode(sig_b64.trim().as_bytes())
        .map_err(|e| KamError::CommandFailed(trf!("Failed to base64 decode signature: {}", e)))?;

    // 3. 获取公钥
    // 优先级：--key > --cert-chain > --cert-name > secret
    // 这样用户可以选择最方便的方式
    let pkey = if let Some(key_path_str) = &args.key {
        // Direct public key file
        let key_path = Path::new(key_path_str);
        let key_bytes = fs::read(key_path).map_err(KamError::Io)?;
        PKey::public_key_from_pem(&key_bytes).map_err(|e| {
            KamError::CommandFailed(trf!(
                "Failed to parse public key PEM from {}: {}",
                key_path.display(),
                e
            ))
        })?
    } else if let Some(cert_chain_path) = &args.cert_chain {
        // Certificate chain from file
        if args.verbose {
            use crate::utils::Utils;
            Utils::executing(&format!(
                "Loading certificate chain from {}...",
                cert_chain_path
            ));
        }
        let chain_pem = fs::read_to_string(cert_chain_path).map_err(KamError::Io)?;

        // Load trusted CAs
        let trusted_cas = crate::cmds::secret::cert::load_trusted_cas()?;
        if trusted_cas.is_empty() {
            return Err(KamError::CommandFailed(
                "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>".to_string()
            ));
        }

        // Verify chain and extract public key
        if args.verbose {
            use crate::utils::Utils;
            Utils::executing("Verifying certificate chain...");
        }
        let pub_key_pem = crate::cmds::secret::cert::verify_cert_chain(&chain_pem, &trusted_cas)?;

        if args.verbose {
            use crate::utils::Utils;
            Utils::success("Certificate chain verified successfully");
        }

        // Parse public key PEM
        PKey::public_key_from_pem(pub_key_pem.as_bytes()).map_err(|e| {
            KamError::CommandFailed(format!(
                "Failed to parse public key from certificate: {}",
                e
            ))
        })?
    } else if let Some(cert_name) = &args.cert_name {
        // Cached certificate
        if args.verbose {
            use crate::utils::Utils;
            Utils::executing(&format!("Loading cached certificate '{}'...", cert_name));
        }
        let chain_pem = crate::cmds::secret::cert::load_cert_chain(cert_name)?;

        // Load trusted CAs
        let trusted_cas = crate::cmds::secret::cert::load_trusted_cas()?;
        if trusted_cas.is_empty() {
            return Err(KamError::CommandFailed(
                "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>".to_string()
            ));
        }

        // Verify chain and extract public key
        if args.verbose {
            println!("Verifying certificate chain...");
        }
        let pub_key_pem = crate::cmds::secret::cert::verify_cert_chain(&chain_pem, &trusted_cas)?;

        if args.verbose {
            println!("Certificate chain verified successfully.");
        }

        // Parse public key PEM
        PKey::public_key_from_pem(pub_key_pem.as_bytes()).map_err(|e| {
            KamError::CommandFailed(trf!("Failed to parse public key from certificate: {}", e))
        })?
    } else {
        // Use helper to get/refresh public key from secret (handles caching and fallback)
        match crate::cmds::secret::utils::get_or_refresh_public_key(&args.secret, args.verbose) {
            Ok(pk) => pk,
            Err(e) => {
                return Err(KamError::CommandFailed(trf!(
                    "Failed to retrieve public key: {}",
                    e
                )));
            }
        }
    };

    if args.verbose {
        use crate::utils::Utils;
        Utils::executing(&trf!("Calculating hash for '{}'...", src_path.display()));
    }

    // 4. 验证签名
    // 用SHA-256哈希和公钥验证签名
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| KamError::CommandFailed(trf!("Failed to create verifier: {}", e)))?;

    if args.verbose {
        use crate::utils::Utils;
        Utils::executing(crate::i18n::tr_key("Verifying signature..."));
    }
    // 更新验证器（把文件内容加进去）
    verifier
        .update(&data)
        .map_err(|e| KamError::CommandFailed(trf!("Failed to update verifier: {}", e)))?;

    // 验证签名
    let result = verifier
        .verify(&sig_bytes)
        .map_err(|e| KamError::CommandFailed(trf!("Verification error: {}", e)))?;

    if result {
        // 验证成功！
        use crate::utils::Utils;
        if args.verbose {
            Utils::success(crate::i18n::tr_key("Verification successful"));
        } else {
            Utils::success(crate::i18n::tr_key("Verified"));
        }
        Ok(())
    } else {
        // 验证失败，文件可能被篡改或签名不对
        use crate::utils::Utils;
        let fail_msg = trf!("Verification FAILED for '{}'", src_path.display());
        Utils::error(&fail_msg);
        Err(KamError::CommandFailed(fail_msg))
    }
}
