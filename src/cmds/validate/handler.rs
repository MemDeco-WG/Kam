use colored::*;
use std::path::Path;

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;

use super::args::ValidateArgs;

pub fn run(args: ValidateArgs) -> Result<(), KamError> {
    let project_path = Path::new(&args.path);
    let kam_toml_path = project_path.join("kam.toml");

    if !kam_toml_path.exists() {
        println!(
            "{} kam.toml not found at {}",
            "✕".red(),
            kam_toml_path.display()
        );
        return Ok(());
    }

    println!("Validating {}...", kam_toml_path.display());

    let kam_toml = match KamToml::load_from_file(&kam_toml_path) {
        Ok(kt) => kt,
        Err(e) => {
            println!("{} Failed to parse kam.toml: {}", "✕".red(), e);
            return Ok(());
        }
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // --- [prop] Section ---
    if kam_toml.prop.id.trim().is_empty() {
        errors.push("[prop] id is required".to_string());
    } else if !kam_toml
        .prop
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        errors.push(
            "[prop] id contains invalid characters (allowed: a-z, A-Z, 0-9, _, -, .)".to_string(),
        );
    }

    if kam_toml.prop.version.trim().is_empty() {
        errors.push("[prop] version is required".to_string());
    }

    if kam_toml.prop.versionCode <= 0 {
        errors.push("[prop] versionCode must be a positive integer".to_string());
    }

    if kam_toml.prop.author.trim().is_empty() {
        warnings.push("[prop] author is empty".to_string());
    }

    // --- [mmrl.repo] Section ---
    if let Some(mmrl) = &kam_toml.mmrl {
        if let Some(repo) = &mmrl.repo {
            // Check for recommended fields
            if repo.license.as_deref().unwrap_or("").is_empty() {
                warnings.push("[mmrl.repo] license is recommended".to_string());
            }

            // Check file existence
            check_file_exists(
                project_path,
                &repo.license_file,
                "[mmrl.repo] license_file",
                &mut errors,
            );
            check_file_exists(
                project_path,
                &repo.readme_file,
                "[mmrl.repo] readme_file",
                &mut errors,
            );
            check_file_exists(
                project_path,
                &repo.changelog_file,
                "[mmrl.repo] changelog_file",
                &mut errors,
            );
        }
    }

    // --- [kam.build] Section ---
    if let Some(build) = &kam_toml.kam.build {
        // Check source dir
        let src_dir = if let Some(custom) = &build.source_dir {
            project_path.join(custom)
        } else {
            project_path.join("src").join(&kam_toml.prop.id)
        };

        if !src_dir.exists() {
            warnings.push(format!(
                "Source directory '{}' does not exist. Build might fail or produce empty module.",
                src_dir.display()
            ));
        }

        // Check hooks dir
        if let Some(hooks) = &build.hooks_dir {
            let hooks_path = project_path.join(hooks);
            if !hooks_path.exists() && hooks != "hooks" {
                // Only warn if user explicitly set a non-default path that doesn't exist
                warnings.push(format!(
                    "Hooks directory '{}' specified but does not exist",
                    hooks
                ));
            }
        }
    } else {
        // Default source dir check
        let default_src = project_path.join("src").join(&kam_toml.prop.id);
        if !default_src.exists() {
            warnings.push(format!(
                "Default source directory '{}' does not exist.",
                default_src.display()
            ));
        }
    }

    // --- Output ---
    println!();
    if errors.is_empty() && warnings.is_empty() {
        println!("{} No issues found. kam.toml is valid.", "✓".green());
    } else {
        if !errors.is_empty() {
            println!("{}", "Errors:".red().bold());
            for e in &errors {
                println!("  {} {}", "x".red(), e);
            }
        }
        if !warnings.is_empty() {
            if !errors.is_empty() {
                println!();
            }
            println!("{}", "Warnings:".yellow().bold());
            for w in &warnings {
                println!("  {} {}", "!".yellow(), w);
            }
        }

        println!();
        if !errors.is_empty() {
            println!(
                "{}",
                "Validation failed. Please fix the errors above.".red()
            );
        } else {
            println!("{}", "Validation passed with warnings.".yellow());
        }
    }

    Ok(())
}

fn check_file_exists(base: &Path, file: &Option<String>, name: &str, errors: &mut Vec<String>) {
    if let Some(f) = file {
        if !f.is_empty() && !base.join(f).exists() {
            errors.push(format!("{} '{}' not found", name, f));
        }
    }
}
