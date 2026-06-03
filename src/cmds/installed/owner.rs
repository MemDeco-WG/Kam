use crate::errors::KamError;

use super::metadata::run_root_script;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerRequest {
    pub paths: Vec<String>,
    pub device: Option<String>,
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerRecord {
    pub module_id: String,
    pub module_path: String,
    pub queried_path: String,
}

/// Resolve device paths to installed module owners.
///
/// # Errors
///
/// Returns an error when adb/root queries fail, no path is supplied, or one of
/// the requested paths is not owned by an installed module.
pub fn handle_owner(request: &OwnerRequest) -> Result<(), KamError> {
    if request.paths.is_empty() {
        return Err(KamError::CommandFailed(
            "Owner query requires a device path, e.g. `kam -Qo /data/adb/modules/MagicNet/cli`"
                .to_string(),
        ));
    }

    let mut records = Vec::new();
    for path in &request.paths {
        records.push(query_owner(path, request.device.as_deref())?);
    }

    for record in records {
        if request.quiet {
            println!("{}", record.module_id);
        } else {
            println!(
                "{} is owned by {} ({})",
                record.queried_path, record.module_id, record.module_path
            );
        }
    }
    Ok(())
}

fn query_owner(path: &str, device: Option<&str>) -> Result<OwnerRecord, KamError> {
    let script = owner_script(path);
    let output = run_root_script(device, &script)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(KamError::PackageNotFound(format!(
            "{}: {}",
            path,
            stderr.trim()
        )));
    }
    parse_owner_record(&stdout).ok_or_else(|| {
        KamError::CommandFailed(format!("Failed to parse owner query output for {path}"))
    })
}

#[must_use]
pub fn parse_owner_record(input: &str) -> Option<OwnerRecord> {
    let mut module_id = String::new();
    let mut module_path = String::new();
    let mut queried_path = String::new();

    for line in input.lines() {
        if let Some(value) = line.strip_prefix("module_id=") {
            module_id = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("module_path=") {
            module_path = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("queried_path=") {
            queried_path = value.trim().to_string();
        }
    }

    if module_id.is_empty() || module_path.is_empty() || queried_path.is_empty() {
        None
    } else {
        Some(OwnerRecord {
            module_id,
            module_path,
            queried_path,
        })
    }
}

fn owner_script(path: &str) -> String {
    format!(
        r#"query={query}
best_id=
best_path=
best_len=0

for d in /data/adb/modules/*; do
  [ -d "$d" ] || continue
  case "$query" in
    "$d"|"$d"/*)
      len=${{#d}}
      if [ "$len" -gt "$best_len" ]; then
        best_id="${{d##*/}}"
        best_path="$d"
        best_len="$len"
      fi
      ;;
  esac
done

if [ -z "$best_id" ]; then
  printf 'no installed module owns %s\n' "$query" >&2
  exit 1
fi

printf 'module_id=%s\n' "$best_id"
printf 'module_path=%s\n' "$best_path"
printf 'queried_path=%s\n' "$query"
"#,
        query = shell_quote(path)
    )
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_owner_record, shell_quote};

    #[test]
    fn parses_owner_record() {
        let record = parse_owner_record(
            "module_id=MagicNet\n\
             module_path=/data/adb/modules/MagicNet\n\
             queried_path=/data/adb/modules/MagicNet/cli\n",
        )
        .expect("owner record");

        assert_eq!(record.module_id, "MagicNet");
        assert_eq!(record.queried_path, "/data/adb/modules/MagicNet/cli");
    }

    #[test]
    fn shell_quote_wraps_spaces() {
        assert_eq!(
            shell_quote("/data/adb/modules/MagicNet/cli"),
            "/data/adb/modules/MagicNet/cli"
        );
        assert_eq!(
            shell_quote("/data/adb/modules/demo module/file"),
            "'/data/adb/modules/demo module/file'"
        );
    }
}
