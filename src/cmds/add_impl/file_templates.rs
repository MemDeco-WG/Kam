fn set_executable_if_shell(path: &Path) -> Result<(), KamError> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("sh") {
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).map_err(KamError::Io)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(KamError::Io)?;
    }

    Ok(())
}

fn validate_slug(value: &str, label: &str) -> Result<(), KamError> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(KamError::InvalidConfig(format!(
            "Invalid {label} '{value}'. Use only letters, digits, '-' and '_'."
        )))
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn success_or_plan(dry_run: bool, msg: String) {
    if dry_run {
        Utils::info(format!("Plan: {msg}"));
    } else {
        Utils::success(msg);
    }
}

fn webui_index(module_id: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{module_id}</title>
    <link rel="stylesheet" href="./style.css">
  </head>
  <body>
    <main>
      <h1>{module_id}</h1>
      <p>Kam module WebUI</p>
      <button id="refresh" type="button">Refresh</button>
      <pre id="status">ready</pre>
    </main>
    <script src="./main.js"></script>
  </body>
</html>
"#
    )
}

const WEBUI_CSS: &str = r"body {
  margin: 0;
  font-family: system-ui, sans-serif;
  color: #f5f7fa;
  background: #101418;
}

main {
  max-width: 720px;
  margin: 0 auto;
  padding: 24px;
}

button {
  min-height: 40px;
  padding: 0 14px;
}

pre {
  overflow: auto;
  padding: 12px;
  background: #1b2229;
}
";

const WEBUI_JS: &str = r#"document.getElementById("refresh")?.addEventListener("click", () => {
  document.getElementById("status").textContent = new Date().toISOString();
});
"#;

#[cfg(test)]
mod tests {
    use super::add_import_to_script;

    #[test]
    fn add_import_after_kamfwrc_source() {
        let script = "#!/system/bin/sh\n. \"$MODDIR/lib/kamfw/.kamfwrc\" || exit 1\n";
        let updated = add_import_to_script(script, "watchdog");
        assert!(updated.contains(".kamfwrc\" || exit 1\nimport watchdog\n"));
    }

    #[test]
    fn add_import_is_idempotent() {
        let script = "#!/system/bin/sh\nimport notify\n";
        assert_eq!(add_import_to_script(script, "notify"), script);
    }
}
