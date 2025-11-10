use std::collections::HashMap;

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

pub fn parse_template_variables(vars: &[String]) -> Result<HashMap<String, crate::types::kam_toml::VariableDefinition>, Box<dyn std::error::Error>> {
    let mut variables = HashMap::new();
    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            let parts: Vec<&str> = value.split(':').collect();
            if parts.len() == 3 {
                let var_type = parts[0].to_string();
                let required = parts[1] == "true";
                let default = if parts[2].is_empty() { None } else { Some(parts[2].to_string()) };
                variables.insert(key.to_string(), crate::types::kam_toml::VariableDefinition {
                    var_type,
                    required,
                    default,
                });
            } else {
                return Err(format!("Invalid template variable format: {}. Expected type:required:default", var).into());
            }
        } else {
            return Err(format!("Invalid template variable format: {}. Expected key=type:required:default", var).into());
        }
    }
    Ok(variables)
}
