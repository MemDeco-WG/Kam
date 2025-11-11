use std::collections::HashMap;
use crate::types::kam_toml::module::VariableDefinition;

pub fn parse_template_vars(vars: &[String]) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut template_vars = HashMap::new();
    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            template_vars.insert(key.to_string(), value.to_string());
        } else {
            return Err(format!("Invalid template variable format: {}", var).into());
        }
    }
    Ok(template_vars)
}

pub fn parse_template_variables(vars: &[String]) -> Result<HashMap<String, VariableDefinition>, Box<dyn std::error::Error>> {
    let mut variables = HashMap::new();
    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            // Accept an optional fourth field as a human-friendly note/message.
            // Format: type:required:default[:note]
            let mut parts_iter = value.splitn(4, ':');
            let var_type = parts_iter.next().unwrap_or("").to_string();
            let required = parts_iter.next().unwrap_or("") == "true";
            let default_part = parts_iter.next().unwrap_or("");
            let default = if default_part.is_empty() { None } else { Some(default_part.to_string()) };
            let note = parts_iter.next().map(|s| s.to_string());
            variables.insert(key.to_string(), VariableDefinition {
                var_type,
                required,
                default,
                note,
                help: None,
                example: None,
                choices: None,
            });
        } else {
            return Err(format!("Invalid template variable format: {}. Expected key=type:required:default", var).into());
        }
    }
    Ok(variables)
}
