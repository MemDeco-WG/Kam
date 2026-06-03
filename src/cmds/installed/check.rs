use crate::errors::KamError;
use crate::utils::Utils;

use super::metadata::run_root_script;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRequest {
    pub modules: Vec<String>,
    pub device: Option<String>,
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCheckReport {
    pub id: String,
    pub path: String,
    pub state: String,
    pub problems: Vec<String>,
}

/// Check installed Magisk/KernelSU/APatch module directories.
///
/// # Errors
///
/// Returns an error when adb/root queries fail, requested modules are missing,
/// or one or more checked modules fail integrity checks.
pub fn handle_check(request: &CheckRequest) -> Result<(), KamError> {
    let output = run_root_script(request.device.as_deref(), check_script())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(KamError::CommandFailed(format!(
            "Failed to check installed modules: {}",
            stderr.trim()
        )));
    }

    let mut reports = parse_check_reports(&stdout);
    reports.sort_by_key(|report| report.id.to_ascii_lowercase());
    let reports = filter_reports(&reports, &request.modules)?;
    let problem_count = reports
        .iter()
        .filter(|report| !report.problems.is_empty())
        .count();

    for report in &reports {
        if report.problems.is_empty() {
            if !request.quiet {
                println!("{}: 0 problems [{}]", report.id, report.state);
            }
        } else {
            println!(
                "{}: {} problem(s) [{}]",
                report.id,
                report.problems.len(),
                report.state
            );
            for problem in &report.problems {
                println!("  - {problem}");
            }
        }
    }

    if problem_count == 0 {
        if !request.quiet {
            Utils::success(format!("Checked {} installed module(s).", reports.len()));
        }
        Ok(())
    } else {
        Err(KamError::CommandFailed(format!(
            "{problem_count} installed module(s) failed integrity checks"
        )))
    }
}

#[must_use]
pub fn parse_check_reports(input: &str) -> Vec<ModuleCheckReport> {
    let mut reports = Vec::new();
    let mut id = String::new();
    let mut path = String::new();
    let mut state = String::new();
    let mut problems = Vec::new();
    let mut in_report = false;

    for line in input.lines() {
        match line.trim() {
            "__kam_check_begin__" => {
                id.clear();
                path.clear();
                state.clear();
                problems.clear();
                in_report = true;
            }
            "__kam_check_end__" => {
                if in_report {
                    reports.push(ModuleCheckReport {
                        id: id.clone(),
                        path: path.clone(),
                        state: state.clone(),
                        problems: problems.clone(),
                    });
                }
                in_report = false;
            }
            _ if in_report => {
                if let Some(value) = line.strip_prefix("id=") {
                    id = value.trim().to_string();
                } else if let Some(value) = line.strip_prefix("path=") {
                    path = value.trim().to_string();
                } else if let Some(value) = line.strip_prefix("state=") {
                    state = value.trim().to_string();
                } else if let Some(value) = line.strip_prefix("problem=") {
                    problems.push(value.trim().to_string());
                }
            }
            _ => {}
        }
    }
    reports
}

fn filter_reports(
    reports: &[ModuleCheckReport],
    modules: &[String],
) -> Result<Vec<ModuleCheckReport>, KamError> {
    if modules.is_empty() {
        return Ok(reports.to_vec());
    }
    let mut selected = Vec::new();
    for requested in modules {
        let Some(report) = reports
            .iter()
            .find(|report| report.id.eq_ignore_ascii_case(requested))
        else {
            return Err(KamError::PackageNotFound(format!(
                "Installed module not found: {requested}"
            )));
        };
        selected.push(report.clone());
    }
    Ok(selected)
}

fn check_script() -> &'static str {
    r#"for d in /data/adb/modules/*; do
  [ -d "$d" ] || continue
  base="${d##*/}"
  prop="$d/module.prop"
  state=enabled
  [ -e "$d/disable" ] && state=disabled
  [ -e "$d/remove" ] && state=remove-pending

  printf '__kam_check_begin__\n'
  printf 'id=%s\n' "$base"
  printf 'path=%s\n' "$d"
  printf 'state=%s\n' "$state"

  if [ ! -f "$prop" ]; then
    printf 'problem=%s\n' 'missing module.prop'
  elif [ ! -r "$prop" ]; then
    printf 'problem=%s\n' 'module.prop is not readable'
  else
    prop_id="$(sed -n 's/\r$//;/^[[:space:]]*#/d;s/^[[:space:]]*id[[:space:]]*=[[:space:]]*//p' "$prop" | head -n 1)"
    prop_name="$(sed -n 's/\r$//;/^[[:space:]]*#/d;s/^[[:space:]]*name[[:space:]]*=[[:space:]]*//p' "$prop" | head -n 1)"
    prop_version="$(sed -n 's/\r$//;/^[[:space:]]*#/d;s/^[[:space:]]*version[[:space:]]*=[[:space:]]*//p' "$prop" | head -n 1)"
    prop_code="$(sed -n 's/\r$//;/^[[:space:]]*#/d;s/^[[:space:]]*versionCode[[:space:]]*=[[:space:]]*//p' "$prop" | head -n 1)"

    [ -n "$prop_id" ] || printf 'problem=%s\n' 'missing id in module.prop'
    [ -n "$prop_name" ] || printf 'problem=%s\n' 'missing name in module.prop'
    [ -n "$prop_version" ] || printf 'problem=%s\n' 'missing version in module.prop'
    [ -n "$prop_code" ] || printf 'problem=%s\n' 'missing versionCode in module.prop'
    if [ -n "$prop_id" ] && [ "$prop_id" != "$base" ]; then
      printf 'problem=id mismatch: directory is %s but module.prop id is %s\n' "$base" "$prop_id"
    fi
  fi

  printf '__kam_check_end__\n'
done"#
}

#[cfg(test)]
mod tests {
    use super::parse_check_reports;

    #[test]
    fn parses_check_report_blocks() {
        let reports = parse_check_reports(
            "__kam_check_begin__\n\
             id=MagicNet\n\
             path=/data/adb/modules/MagicNet\n\
             state=enabled\n\
             problem=missing versionCode in module.prop\n\
             __kam_check_end__\n",
        );

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].id, "MagicNet");
        assert_eq!(reports[0].problems, ["missing versionCode in module.prop"]);
    }
}
