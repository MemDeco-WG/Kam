use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use walkdir::WalkDir;

use crate::assets::tmpl::TmplAssets;
use crate::cmds::tmpl::pull;
use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::{template::TemplateCacheManager, template::TemplateManager};
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use super::args::InitArgs;
use super::{impl_mod, post_init, pre_init};

fn prompt_input(prompt: &str, default: Option<&str>) -> Result<String, KamError> {
    let default_str = default.unwrap_or("").to_string();

    // Prefer dialoguer Input for a nicer interactive UI; fall back to stdio if it fails
    if let Ok(v) = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .allow_empty(true)
        .default(default_str.clone())
        .interact_text()
    {
        return Ok(v);
    }

    // Fallback: standard input (non-TTY or dialoguer failure)
    if default_str.is_empty() {
        print!("{}: ", prompt);
    } else {
        print!("{} [{}]: ", prompt, default_str);
    }
    io::stdout().flush().map_err(KamError::Io)?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(KamError::Io)?;
    let input = input.trim();
    if input.is_empty() {
        Ok(default_str)
    } else {
        Ok(input.to_string())
    }
}

fn prompt_confirm(prompt: &str, default: bool) -> Result<bool, KamError> {
    // Try the dialoguer Confirm for a nicer TTY experience; if it fails (non-TTY),
    // fall back to a simple text-based confirmation prompt.
    match Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default)
        .interact()
    {
        Ok(v) => Ok(v),
        Err(_) => {
            let suffix = if default { "[Y/n]" } else { "[y/N]" };
            loop {
                print!("{} {}: ", prompt, suffix);
                io::stdout().flush().map_err(KamError::Io)?;
                let mut input = String::new();
                io::stdin().read_line(&mut input).map_err(KamError::Io)?;
                let trimmed = input.trim().to_lowercase();
                if trimmed.is_empty() {
                    return Ok(default);
                }
                match trimmed.as_str() {
                    "y" | "yes" => return Ok(true),
                    "n" | "no" => return Ok(false),
                    _ => {
                        println!("Please enter 'y' or 'n'.");
                        continue;
                    }
                }
            }
        }
    }
}

/// Display a simplified list of candidate templates and ask the user to choose one
fn choose_template(default_template: &str) -> Result<String, KamError> {
    loop {
        let mut choices = TemplateManager::list_builtin_templates();
        choices.sort();
        choices.dedup();
        // Add a visible option for a local path
        choices.push("<local path or archive>".to_string());
        // Provide an explicit option to pull default templates online
        choices.push("<pull default templates>".to_string());

        // Compute a reasonable default index if possible
        let default_idx = choices
            .iter()
            .position(|t| t == default_template)
            .unwrap_or(0);

        // Try an interactive Select first (arrow keys). If it fails (non-TTY), fallback to text input.
        match Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose a template to use")
            .items(&choices[..])
            .default(default_idx)
            .interact()
        {
            Ok(idx) => {
                let sel = &choices[idx];
                if sel == "<pull default templates>" {
                    // Pull default templates from the configured URL (default behavior)
                    pull::run_pull(None, true)?;
                    // Refresh the choices and present the menu again
                    continue;
                }
                if sel == "<local path or archive>" {
                    // Ask for local path
                    let input = Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter local template path or archive file (leave empty to download default templates)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|e| KamError::Io(e.into()))?;
                    let input_trim = input.trim();
                    if input_trim.is_empty() {
                        // When left blank, download default templates and re-run the template selection flow
                        pull::run_pull(None, true)?;
                        // Refresh the local list and re-display to the user
                        continue;
                    }

                    let p = Path::new(&input);
                    if !p.exists() {
                        return Err(KamError::InvalidDirectory(format!(
                            "Local template path not found: {}",
                            input
                        )));
                    }
                    if p.is_file() {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            // If template already exists in cache, reuse it instead of reinstalling
                            if let Ok(Some(_existing)) =
                                TemplateCacheManager::resolve_template_path(stem)
                            {
                                return Ok(stem.to_string());
                            }
                            // Otherwise install the local archive into the cache
                            TemplateCacheManager::install_template(stem, p)?;
                            return Ok(stem.to_string());
                        } else {
                            // return the provided path string if we couldn't derive a stem
                            return Ok(input.clone());
                        }
                    }
                    // Directory path - use it directly
                    return Ok(input.clone());
                } else {
                    return Ok(choices[idx].clone());
                }
            }
            Err(_) => {
                // Fallback to previous text-based interaction
                println!("(Non-interactive mode detected: falling back to text input.)");
                loop {
                    let pick = prompt_input(
                        "Select a template by name or number (or provide a local path)",
                        Some(default_template),
                    )?;
                    if pick.trim().is_empty() {
                        // Empty text input: download the default templates and refresh choices
                        crate::cmds::tmpl::pull::run_pull(None, true)?;
                        break; // Exit the fallback prompt and let the outer loop refresh the choices
                    }
                    if let Ok(num) = pick.parse::<usize>() {
                        if num == 0 {
                            let p =
                                prompt_input("Enter path to local template (file or dir)", None)?;
                            if !p.trim().is_empty() {
                                return Ok(p);
                            }
                        } else if num > 0 && num <= choices.len() {
                            return Ok(choices[num - 1].clone());
                        } else {
                            println!("Invalid selection: {}", num);
                            continue;
                        }
                    } else {
                        let pick_trim = pick.trim();
                        if !pick_trim.is_empty() {
                            // If it's a path and exists, handle it as local template path
                            let p = Path::new(pick_trim);
                            if p.exists() {
                                // try to install to cache if path exists
                                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                                    TemplateCacheManager::install_template(stem, p)?;
                                    return Ok(stem.to_string());
                                }
                                return Ok(pick_trim.to_string());
                            }
                            // Otherwise, assume it's a template name
                            return Ok(pick_trim.to_string());
                        }
                    }
                }
            }
        }
    }
}

/// Extract an archive path (tar.gz, tgz, zip) into provided `dst` folder.
/// Returns Ok(()) on success, or KamError on failure.
fn extract_archive(path: &Path, dst: &Path) -> Result<(), KamError> {
    let file = fs::File::open(path).map_err(KamError::Io)?;
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive
            .unpack(dst)
            .map_err(|e| KamError::Io(std::io::Error::new(io::ErrorKind::Other, e)))?;
    } else if path_str.ends_with(".zip") {
        let mut zip_arch = zip::ZipArchive::new(file).map_err(KamError::Zip)?;
        zip_arch.extract(dst).map_err(KamError::Zip)?;
    } else {
        return Err(KamError::UnsupportedArchive(format!(
            "Unsupported archive: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Resolve the template source into a path containing a `kam.toml`.
///
/// Returns a tuple of (kam_toml_path, optional_tempdir).
/// If the template requires extraction to a tempdir, the TempDir is returned so it is kept alive.
/// Otherwise None returned and the kam_toml_path references an existing directory/file on disk.
fn resolve_template_kam_toml(template_spec: &str) -> Result<(PathBuf, Option<TempDir>), KamError> {
    let try_specs = {
        let mut v = vec![template_spec.to_string()];
        let is_archive_or_path = template_spec.contains('/')
            || template_spec.contains('\\')
            || template_spec.ends_with(".tar.gz")
            || template_spec.ends_with(".tgz")
            || template_spec.ends_with(".zip");
        if !template_spec.ends_with("_template") && !is_archive_or_path {
            v.push(format!("{}_template", template_spec));
        }
        v
    };

    for spec in try_specs {
        // 1. If raw input is a path
        let spec_path = Path::new(&spec);
        if spec_path.exists() {
            if spec_path.is_dir() {
                let k = spec_path.join("kam.toml");
                if k.exists() {
                    return Ok((k, None));
                } else {
                    // Could be a template root without kam.toml (unlikely) — treat as error
                    return Err(KamError::InvalidDirectory(format!(
                        "Template '{}' is a directory but no kam.toml found",
                        spec_path.display()
                    )));
                }
            } else if spec_path.is_file() {
                let tmp = tempfile::tempdir().map_err(KamError::Io)?;
                extract_archive(spec_path, tmp.path())?;
                // Find kam.toml in the extracted temp
                let kt_path = tmp.path().join("kam.toml");
                if kt_path.exists() {
                    return Ok((kt_path, Some(tmp)));
                } else {
                    // maybe templates are under single nested dir; attempt to descend
                    // find the first directory that contains kam.toml
                    for entry in fs::read_dir(tmp.path()).map_err(KamError::Io)? {
                        let entry = entry.map_err(KamError::Io)?;
                        let p = entry.path();
                        let candidate = p.join("kam.toml");
                        if candidate.exists() {
                            return Ok((candidate, Some(tmp)));
                        }
                    }
                    return Err(KamError::InvalidDirectory(format!(
                        "Archive '{}' did not contain kam.toml",
                        spec
                    )));
                }
            }
        }

        // 2. Check cached template path
        if let Ok(Some(cached_path)) = TemplateCacheManager::resolve_template_path(&spec) {
            if cached_path.is_dir() {
                let k = cached_path.join("kam.toml");
                if k.exists() {
                    return Ok((k, None));
                } else {
                    return Err(KamError::InvalidDirectory(format!(
                        "Cached template '{}' has no kam.toml",
                        spec
                    )));
                }
            } else if cached_path.is_file() {
                let tmp = tempfile::tempdir().map_err(KamError::Io)?;
                extract_archive(&cached_path, tmp.path())?;
                let kt_path = tmp.path().join("kam.toml");
                if kt_path.exists() {
                    return Ok((kt_path, Some(tmp)));
                }
            }
        }

        // 3. Check built-in assets (assets are .tar.gz)
        {
            let asset_name = format!("{}.tar.gz", spec);
            if let Some(asset) = TmplAssets::get(&asset_name) {
                // create temp file and extract
                let tmp = tempfile::tempdir().map_err(KamError::Io)?;
                let bin_path = tmp.path().join(asset_name);
                fs::write(&bin_path, &asset.data).map_err(KamError::Io)?;
                extract_archive(&bin_path, tmp.path())?;
                // Kam file could be in root or inside top-level dir
                let kt_path = tmp.path().join("kam.toml");
                if kt_path.exists() {
                    return Ok((kt_path, Some(tmp)));
                } else {
                    for entry in fs::read_dir(tmp.path()).map_err(KamError::Io)? {
                        let entry = entry.map_err(KamError::Io)?;
                        let p = entry.path();
                        let candidate = p.join("kam.toml");
                        if candidate.exists() {
                            return Ok((candidate, Some(tmp)));
                        }
                    }
                    return Err(KamError::InvalidDirectory(format!(
                        "Built-in asset '{}' did not contain kam.toml",
                        spec
                    )));
                }
            }
        }
    } // end for potentials

    Err(KamError::TemplateNotFound(format!(
        "Template '{}' not found in built-in assets, cache, or local path",
        template_spec
    )))
}

/// Prompt for template-defined variables using the `kam.toml` definition.
///
/// Returns a HashMap of variables to values (overrides).
fn prompt_template_variables(
    kam_toml_path: &Path,
    existing: &mut HashMap<String, String>,
) -> Result<(), KamError> {
    let kt = KamToml::load_from_file(kam_toml_path)?;
    if let Some(tmpl_section) = kt.tmpl {
        for (var_name, var_def) in tmpl_section.variables.iter() {
            let default = existing
                .get(var_name)
                .cloned()
                .or_else(|| var_def.default.clone())
                .unwrap_or_default();

            // Show notes/help/example where available
            if let Some(note) = &var_def.note {
                println!("\n{} - Note: {}", var_name, note);
            } else {
                println!("\nVariable: {}", var_name);
            }
            if let Some(help) = &var_def.help {
                println!("  Help: {}", help);
            }
            if let Some(example) = &var_def.example {
                println!("  Example: {}", example);
            }
            if !default.is_empty() {
                println!("  Default: {}", default);
            }

            // If the variable offers choices, prefer a visual Select and allow a custom value option.
            if let Some(choices) = &var_def.choices {
                // Offer existing choices plus a '<Custom value>' option to allow free-form input.
                let mut opts = choices.clone();
                opts.push("<Custom value>".to_string());

                // Default index: prefer a matching default value if present, otherwise 0
                let default_idx = choices.iter().position(|c| c == &default).unwrap_or(0);

                // Try dialoguer Select (arrow navigation). On failure fall back to textual input.
                match Select::with_theme(&ColorfulTheme::default())
                    .with_prompt(format!("Select a value for '{}'", var_name))
                    .items(&opts)
                    .default(default_idx)
                    .interact()
                {
                    Ok(idx) => {
                        if idx < choices.len() {
                            // Chosen one of the predefined choices
                            existing.insert(var_name.clone(), choices[idx].clone());
                        } else {
                            // User chose the '<Custom value>' option -> ask for free-form input
                            let custom = Input::<String>::with_theme(&ColorfulTheme::default())
                                .with_prompt(format!("Enter custom value for {}", var_name))
                                .allow_empty(!var_def.required)
                                .default(default.clone())
                                .interact_text()
                                .map_err(|e| KamError::Io(e.into()))?;
                            if custom.is_empty() && var_def.required {
                                return Err(KamError::InvalidConfig(format!(
                                    "Value required for {}",
                                    var_name
                                )));
                            }
                            existing.insert(var_name.clone(), custom);
                        }
                    }
                    Err(_) => {
                        // dialoguer failed (likely non-interactive): fallback to text-based flow
                        loop {
                            let response = prompt_input(
                                &format!("Enter value for {} (index or value)", var_name),
                                Some(&default),
                            )?;
                            if response.is_empty() && var_def.required {
                                println!("Value is required for {}", var_name);
                                continue;
                            }
                            // numeric index allowed (1-based)
                            if let Ok(idx) = response.parse::<usize>() {
                                if idx > 0 && idx <= choices.len() {
                                    existing.insert(var_name.clone(), choices[idx - 1].clone());
                                    break;
                                } else {
                                    println!("Invalid selection index");
                                    continue;
                                }
                            }
                            if !choices.contains(&response) {
                                if prompt_confirm(
                                    "Value is not one of the choices. Use raw value anyway?",
                                    false,
                                )? {
                                    existing.insert(var_name.clone(), response.clone());
                                    break;
                                } else {
                                    continue;
                                }
                            } else {
                                existing.insert(var_name.clone(), response.clone());
                                break;
                            }
                        }
                    }
                }
            } else {
                // No choices: handle bool as Confirm, otherwise use Input
                if var_def.var_type == "bool" {
                    let default_bool =
                        !default.is_empty() && (default == "1" || default.to_lowercase() == "true");
                    match Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(format!("{}?", var_name))
                        .default(default_bool)
                        .interact()
                    {
                        Ok(v) => {
                            existing.insert(
                                var_name.clone(),
                                if v { "1".to_string() } else { "0".to_string() },
                            );
                        }
                        Err(_) => {
                            // Fallback to text prompt for bool
                            loop {
                                let resp = prompt_input(
                                    &format!(
                                        "Enter true/false for {} (default: {})",
                                        var_name, default_bool
                                    ),
                                    Some(if default_bool { "true" } else { "false" }),
                                )?;
                                if resp.is_empty() && var_def.required {
                                    println!("Value is required for {}", var_name);
                                    continue;
                                }
                                let v = resp.to_lowercase();
                                if v == "true" || v == "1" {
                                    existing.insert(var_name.clone(), "1".to_string());
                                    break;
                                } else if v == "false" || v == "0" {
                                    existing.insert(var_name.clone(), "0".to_string());
                                    break;
                                } else {
                                    println!("Please enter 'true' or 'false'");
                                    continue;
                                }
                            }
                        }
                    }
                } else {
                    // String input via dialoguer Input, fallback to prompt_input
                    loop {
                        let resp = Input::<String>::with_theme(&ColorfulTheme::default())
                            .with_prompt(format!("Enter value for {}", var_name))
                            .allow_empty(true)
                            .default(default.clone())
                            .interact_text();
                        let response = match resp {
                            Ok(s) => s,
                            Err(_) => prompt_input(
                                &format!("Enter value for {}", var_name),
                                Some(&default),
                            )?,
                        };
                        if response.is_empty() && var_def.required {
                            println!("Value is required for {}", var_name);
                            continue;
                        }
                        existing.insert(var_name.clone(), response.clone());
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Build a list of files that would be copied by the template (visualize).
fn visualize_template(template_dir: &Path, limit: usize) -> Result<(), KamError> {
    // Build a tree-like list with indentation and provide a Select UI to scroll and preview files.
    // This is arrow-key friendly via `dialoguer::Select`.
    let mut entries: Vec<(String, PathBuf, bool)> = Vec::new();

    for entry in WalkDir::new(template_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        // Use metadata where possible; ignore broken entries
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        // only consider files and directories
        let path = entry.path();
        // relative path for display
        let rel = path
            .strip_prefix(template_dir)
            .unwrap_or(path)
            .to_path_buf();
        // compute depth for a tree prefix
        let depth = rel.components().count().saturating_sub(1);
        let mut prefix = String::new();
        for _ in 0..depth {
            prefix.push_str("  ");
        }
        // Display name (with last component only to make the tree compact)
        let display_name = if let Some(name) = rel.file_name().and_then(|s| s.to_str()) {
            format!("{}{}", prefix, name)
        } else {
            format!("{}{}", prefix, rel.display())
        };
        entries.push((display_name, rel, metadata.is_file()));
    }

    // Sort for deterministic order
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Nothing to show?
    if entries.is_empty() {
        println!("(Template directory empty)");
        return Ok(());
    }

    // Build the display items
    let mut display_items: Vec<String> = entries.iter().map(|(d, _, _)| d.clone()).collect();
    // Add control items at the end
    display_items.push("-- Continue --".to_string());
    display_items.push("-- Exit preview --".to_string());

    // Interactive selection loop
    loop {
        // Interactive select (arrow keys). If it fails (non TTY / CI), fallback to text listing.
        match Select::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Template preview ({} entries): (Enter to preview; select '-- Continue --' to proceed)",
                entries.len()
            ))
            .items(&display_items)
            .default(0)
            .interact()
        {
            Ok(idx) => {
                let choices_len = display_items.len();
                // If choose continue -> exit preview and return Ok
                if idx == choices_len - 2 {
                    break;
                }
                // If choose exit -> simply return Ok (cancel preview)
                if idx == choices_len - 1 {
                    println!("Preview cancelled.");
                    return Ok(());
                }
                // If an actual entry is selected
                if idx < entries.len() {
                    let (_label, rel_path, is_file) = &entries[idx];
                    let full_path = template_dir.join(rel_path);
                    if !is_file {
                        // Directory selected: list immediate children as a small preview
                        println!("\n--- Directory: {} ---", rel_path.display());
                        if let Ok(children) = fs::read_dir(&full_path) {
                            let mut cvec: Vec<_> = children.filter_map(|e| e.ok()).collect();
                            cvec.sort_by_key(|d| d.path());
                            for c in cvec.iter().take(50) {
                                if let Some(name) = c.path().file_name().and_then(|s| s.to_str()) {
                                    println!("  - {}", name);
                                }
                            }
                            println!("--- End of directory preview ---\n");
                        } else {
                            println!("  (unable to read directory contents)\n");
                        }
                        // Continue loop for further selection
                        continue;
                    } else {
                        // File selected -> display a small content preview (first N lines)
                        println!("\n--- Preview: {} ---", rel_path.display());
                        match fs::read_to_string(&full_path) {
                            Ok(content) => {
                                let mut count = 0usize;
                                for line in content.lines() {
                                    println!("{}", line);
                                    count += 1;
                                    if count >= 50 {
                                        println!("[... preview truncated after 50 lines ...]");
                                        break;
                                    }
                                }
                                if count == 0 {
                                    println!("(file is empty)");
                                }
                            }
                            Err(_) => {
                                println!("(failed to read file for preview)");
                            }
                        }
                        println!("--- End of preview: {} ---\n", rel_path.display());

                        // Ask whether to preview another file (interactive)
                        let cont = match Confirm::with_theme(&ColorfulTheme::default())
                            .with_prompt("Preview another file?")
                            .default(true)
                            .interact() {
                            Ok(v) => v,
                            Err(_) => prompt_confirm("Preview another file?", false)?,
                        };
                        if cont {
                            continue;
                        } else {
                            break;
                        }
                    }
                } else {
                    // Out-of-range index, just break
                    break;
                }
            }
            Err(_) => {
                // Non-interactive: fallback to simple listing (no arrows)
                println!("\nTemplate contents (showing up to {} files):", limit);
                for (i, (_, rel, is_file)) in entries.iter().enumerate().take(limit) {
                    let suffix = if *is_file { "" } else { "/" };
                    println!("  {}) {}{}", i + 1, rel.display(), suffix);
                }
                if entries.len() > limit {
                    println!("  ... and {} more files", entries.len() - limit);
                }
                println!();
                return Ok(());
            }
        }
    }

    Ok(())
}

fn save_global_config(
    author: Option<&str>,
    name: Option<&str>,
    version: Option<&str>,
) -> Result<(), KamError> {
    // Use `toml` to update or create a global config at ~/.kam/config.toml
    let home = dirs::home_dir()
        .ok_or_else(|| KamError::InvalidDirectory("Cannot determine home directory".to_string()))?;
    let config_dir = home.join(".kam");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(KamError::Io)?;
    }
    let cfg_path = config_dir.join("config.toml");

    // Parse existing TOML if present; else start with an empty table
    let mut doc_table: toml::value::Table = if cfg_path.exists() {
        let s = fs::read_to_string(&cfg_path).map_err(KamError::Io)?;
        let val: toml::Value = toml::from_str(&s).map_err(|e| {
            KamError::CommandFailed(format!("Failed to parse existing config: {}", e))
        })?;
        match val {
            toml::Value::Table(t) => t,
            _ => toml::value::Table::new(),
        }
    } else {
        toml::value::Table::new()
    };

    // Ensure we have a [prop] table and update keys using get_mut to avoid Entry type mismatches
    if let Some(val) = doc_table.get_mut("prop") {
        match val {
            toml::Value::Table(prop_table) => {
                if let Some(a) = author {
                    prop_table.insert("author".to_string(), toml::Value::String(a.to_string()));
                }
                if let Some(n) = name {
                    prop_table.insert("name".to_string(), toml::Value::String(n.to_string()));
                }
                if let Some(v) = version {
                    prop_table.insert("version".to_string(), toml::Value::String(v.to_string()));
                }
            }
            _ => {
                // Replace non-table 'prop' value with a proper table containing our values
                let mut prop_tbl = toml::value::Table::new();
                if let Some(a) = author {
                    prop_tbl.insert("author".to_string(), toml::Value::String(a.to_string()));
                }
                if let Some(n) = name {
                    prop_tbl.insert("name".to_string(), toml::Value::String(n.to_string()));
                }
                if let Some(v) = version {
                    prop_tbl.insert("version".to_string(), toml::Value::String(v.to_string()));
                }
                *val = toml::Value::Table(prop_tbl);
            }
        }
    } else {
        // Insert a new [prop] table with our fields
        let mut prop_tbl = toml::value::Table::new();
        if let Some(a) = author {
            prop_tbl.insert("author".to_string(), toml::Value::String(a.to_string()));
        }
        if let Some(n) = name {
            prop_tbl.insert("name".to_string(), toml::Value::String(n.to_string()));
        }
        if let Some(v) = version {
            prop_tbl.insert("version".to_string(), toml::Value::String(v.to_string()));
        }
        doc_table.insert("prop".to_string(), toml::Value::Table(prop_tbl));
    }

    // Serialize and write back to file
    let final_value = toml::Value::Table(doc_table);
    let out = toml::to_string_pretty(&final_value)
        .map_err(|e| KamError::CommandFailed(format!("Failed to serialize config: {}", e)))?;
    fs::write(cfg_path, out).map_err(KamError::Io)?;
    Ok(())
}

pub fn run(args: InitArgs) -> Result<(), KamError> {
    println!("Interactive Kam init");
    println!("(You may press Enter to accept a default value shown in brackets)\n");

    // Prepare defaults (non-interactive sanity pass)
    let mut data = match pre_init::prepare_init(&args) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to prepare initialization defaults: {}", e);
            return Err(e);
        }
    };

    // Template selection
    let chosen_template = choose_template(&data.impl_template)?;
    data.impl_template = chosen_template;

    // Path selection: allow changing target path
    let cur_path_str = data.path.to_string_lossy().to_string();
    let new_path_str = prompt_input("Path to create project", Some(&cur_path_str))?;
    if new_path_str != cur_path_str {
        // Normalize to an absolute path, respecting '.' and relative paths
        let final_path = if new_path_str.trim().is_empty() || new_path_str == "." {
            std::env::current_dir().map_err(KamError::Io)?
        } else {
            let candidate = PathBuf::from(&new_path_str);
            if candidate.is_relative() {
                std::env::current_dir()
                    .map_err(KamError::Io)?
                    .join(candidate)
            } else {
                candidate
            }
        };
        data.path = final_path;
    }

    // Ensure module id matches path name
    let path_basename = data
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&data.id)
        .to_string();
    if path_basename != data.id {
        println!(
            "Module id '{}' does not match folder basename '{}'.",
            data.id, path_basename
        );
        if prompt_confirm("Set module id to folder basename?", true)? {
            data.id = path_basename.clone();
        } else {
            // Let user input a custom id
            let id_input = prompt_input("Module ID", Some(&data.id))?;
            data.id = id_input;
            // re-validate
            if !data
                .id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
            {
                return Err(KamError::InvalidConfig(format!(
                    "Invalid module ID '{}': ID must contain only alphanumeric characters, dots, dashes, and underscores",
                    data.id
                )));
            }
        }
    }

    // Base params (name, version, author, description)
    data.name = prompt_input("Project name", Some(&data.name))?;
    data.version = prompt_input("Version", Some(&data.version))?;
    data.author = prompt_input("Author", Some(&data.author))?;
    data.description = prompt_input("Description", Some(&data.description))?;

    // Ask to persist defaults to global config (~/.kam/config.toml)
    if prompt_confirm(
        "Save these base values to global config (~/.kam/config.toml)?",
        false,
    )? {
        save_global_config(Some(&data.author), Some(&data.name), Some(&data.version))?;
        println!("Saved base configuration to ~/.kam/config.toml");
    }

    // Template variables: find template kam.toml then prompt for variables
    let (kt_path, maybe_tmp) = resolve_template_kam_toml(&data.impl_template)?;
    // `maybe_tmp` kept here to keep tempdir alive while we inspect its files
    if let Some(tmp) = &maybe_tmp {
        println!(
            "Loaded template from temporary extraction at: {}",
            tmp.path().display()
        );
    }

    // Ensure data.template_vars contains default fallbacks from pre_init
    // Now prompt for each variable defined in template
    prompt_template_variables(&kt_path, &mut data.template_vars)?;

    // Optional: visualize the template contents to the user
    let template_dir = kt_path.parent().unwrap_or_else(|| Path::new("."));
    let _ = visualize_template(template_dir, 50);

    // Check for gh and cz presence, ask to install if missing
    // GH
    let gh_present = std::process::Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !gh_present {
        if prompt_confirm(
            "GitHub CLI (gh) not found. Would you like to view instructions to install it?",
            true,
        )? {
            println!("Recommend: https://cli.github.com/manual/installation");
            println!("Or you may run the interactive helper script: Kam/KamModuleX/kam.sh");
        }
    }

    // cz (commitizen)
    let cz_present = std::process::Command::new("cz")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !cz_present {
        if prompt_confirm(
            "Commitizen (cz) not found. Would you like to view instructions to install it?",
            false,
        )? {
            println!(
                "Recommended: python -m pip install --user commitizen  (or npm/yarn global install)"
            );
            println!("Or you may run the interactive helper script: Kam/KamModuleX/kam.sh");
        }
    }

    // Present a summary and confirm before creating files
    println!("\nSummary:");
    println!("  Path: {}", data.path.display());
    println!("  Module ID: {}", data.id);
    println!("  Project name: {}", data.name);
    println!("  Version: {}", data.version);
    println!("  Author: {}", data.author);
    println!("  Template: {}", data.impl_template);
    println!("  Template variables:");
    for (k, v) in &data.template_vars {
        println!("    {} = {}", k, v);
    }

    if !prompt_confirm("Proceed and create project with these settings?", true)? {
        println!("Aborted.");
        return Err(KamError::CommandFailed(
            "User aborted interactive init".to_string(),
        ));
    }

    // Merge into `Vec<String>` for init_template
    let mut merged_var_vec: Vec<String> = data
        .template_vars
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    merged_var_vec.sort();

    // Call the same init_template as non-interactive run
    impl_mod::init_template(
        &data.path,
        &data.id,
        data.name.clone(),
        &data.version,
        &data.author,
        data.description.clone(),
        &merged_var_vec,
        Some(data.impl_template.clone()),
        args.force,
        data.module_type,
        data.update_json.clone(),
    )?;

    // Post-process
    post_init::post_process(&data.path)?;

    println!("Interactive initialization completed successfully.");
    println!("Suggestion:");
    println!("  cd {}", data.path.display());
    println!("  kam build");
    Ok(())
}
