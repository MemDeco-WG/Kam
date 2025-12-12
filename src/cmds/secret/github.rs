use crate::errors::KamError;
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct Issue {
    body: Option<String>,
    comments: u32,
    comments_url: String,
}

#[derive(Deserialize)]
struct Comment {
    body: String,
}

/// Fetch certificate chain from a GitHub issue
///
/// This looks for certificate chains in both the issue body and comments.
/// Certificate chains should be in PEM format between markers.
pub fn fetch_cert_from_issue(user: &str, repo: &str, issue_num: u32) -> Result<String, KamError> {
    let client = Client::new();

    // Fetch the issue
    let issue_url = format!("https://api.github.com/repos/{}/{}/issues/{}", user, repo, issue_num);
    let issue: Issue = client
        .get(&issue_url)
        .header("User-Agent", "kam-cli")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .map_err(|e| KamError::CommandFailed(format!("Failed to fetch issue: {}", e)))?
        .json()
        .map_err(|e| KamError::CommandFailed(format!("Failed to parse issue JSON: {}", e)))?;

    // Try to extract certificate from issue body
    if let Some(body) = &issue.body {
        if let Some(cert) = extract_cert_chain(body) {
            return Ok(cert);
        }
    }

    // If not found in body, check comments
    if issue.comments > 0 {
        let comments: Vec<Comment> = client
            .get(&issue.comments_url)
            .header("User-Agent", "kam-cli")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| KamError::CommandFailed(format!("Failed to fetch comments: {}", e)))?
            .json()
            .map_err(|e| KamError::CommandFailed(format!("Failed to parse comments JSON: {}", e)))?;

        for comment in comments {
            if let Some(cert) = extract_cert_chain(&comment.body) {
                return Ok(cert);
            }
        }
    }

    Err(KamError::CommandFailed("No certificate chain found in issue or comments".to_string()))
}

/// Extract certificate chain from markdown text
///
/// Looks for PEM-formatted certificates between -----BEGIN CERTIFICATE----- and -----END CERTIFICATE----- markers.
/// Can handle multiple certificates in a chain.
pub fn extract_cert_chain(text: &str) -> Option<String> {
    let mut chain = String::new();
    let mut in_cert = false;
    let mut current_cert = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("-----BEGIN CERTIFICATE-----") {
            in_cert = true;
            current_cert.clear();
            current_cert.push_str(trimmed);
            current_cert.push('\n');
        } else if trimmed.starts_with("-----END CERTIFICATE-----") {
            if in_cert {
                current_cert.push_str(trimmed);
                current_cert.push('\n');
                chain.push_str(&current_cert);
                in_cert = false;
            }
        } else if in_cert {
            current_cert.push_str(trimmed);
            current_cert.push('\n');
        }
    }

    if chain.is_empty() {
        None
    } else {
        Some(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_cert() {
        let text = r#"
Here is my certificate:

```
-----BEGIN CERTIFICATE-----
MIIBkTCB+wIJAKHHCgVZU6KRMA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBlRl
c3RDQTAeFw0yMDAxMDEwMDAwMDBaFw0zMDAxMDEwMDAwMDBaMBExDzANBgNVBAMM
BlRlc3RDQTCBnzANBgkqhkiG9w0BAQEFAAOBjQAwgYkCgYEAwRQ0LqtgXK+h8tnN
-----END CERTIFICATE-----
```

Thanks!
"#;

        let result = extract_cert_chain(text);
        assert!(result.is_some());
        let chain = result.unwrap();
        assert!(chain.contains("-----BEGIN CERTIFICATE-----"));
        assert!(chain.contains("-----END CERTIFICATE-----"));
    }

    #[test]
    fn test_extract_multiple_certs() {
        let text = r#"
-----BEGIN CERTIFICATE-----
CERT1DATA
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
CERT2DATA
-----END CERTIFICATE-----
"#;

        let result = extract_cert_chain(text);
        assert!(result.is_some());
        let chain = result.unwrap();
        assert_eq!(chain.matches("-----BEGIN CERTIFICATE-----").count(), 2);
        assert_eq!(chain.matches("-----END CERTIFICATE-----").count(), 2);
    }
}
