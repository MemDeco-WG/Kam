use crate::errors::Result;
use std::path::PathBuf;

/// Flexible source specification for a Kam module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Git repository URL with optional revision (branch/tag/commit)
    Git { url: String, rev: Option<String> },
    /// Local filesystem path
    Local { path: PathBuf },
    /// HTTP(S) URL pointing to an archive or raw source
    Url { url: String },
}

impl Source {
    /// Parse a flexible ``pip``-like spec into a Source.
    ///
    /// Supported forms (examples):
    /// - git+https://github.com/org/repo.git@v1.2.3
    /// - https://example.com/module.tar.gz
    /// - /path/to/local/module
    /// - file:///C:/path/to/module.tar.gz
    pub fn parse(spec: &str) -> Result<Self> {
        let s = spec.trim();

        // git+...@rev
        if let Some(rest) = s.strip_prefix("git+") {
            // split on last '@' to allow @ in URLs (rare) but handle rev
            if let Some(idx) = rest.rfind('@') {
                let (url_part, rev_part) = rest.split_at(idx);
                let rev = rev_part.trim_start_matches('@').to_string();
                return Ok(Source::Git {
                    url: url_part.to_string(),
                    rev: Some(rev),
                });
            }
            return Ok(Source::Git {
                url: rest.to_string(),
                rev: None,
            });
        }

        // file:// local path
        if let Some(rest) = s.strip_prefix("file://") {
            return Ok(Source::Local {
                path: PathBuf::from(rest),
            });
        }

        // http(s) URL
        if s.starts_with("http://") || s.starts_with("https://") {
            return Ok(Source::Url { url: s.to_string() });
        }

        // otherwise treat as local path if it exists or looks like a path
        let p = PathBuf::from(s);
        if p.exists() {
            return Ok(Source::Local { path: p });
        }

        // If it contains a scheme-like prefix (://) treat as URL
        if s.contains("://") {
            return Ok(Source::Url { url: s.to_string() });
        }

        // Fallback: treat as a Git URL if it ends with .git or contains ':' (scp-like)
        if s.ends_with(".git") || s.contains(':') {
            return Ok(Source::Git {
                url: s.to_string(),
                rev: None,
            });
        }

        // As last resort, treat as local path (may not exist yet)
        Ok(Source::Local { path: p })
    }
}
