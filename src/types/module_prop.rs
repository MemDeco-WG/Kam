

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct ModuleProp {
  pub id: String,
  pub name: String,
  pub version: String,
  pub versionCode: u64,
  pub author: String,
  pub description: String,
  pub updateJson: Option<String>,
}


#[allow(non_snake_case)]
impl ModuleProp {

    /// Serialize to properties string format
    pub fn to_properties(&self) -> String {
        let mut props = format!(
            "id={}\nname={}\nversion={}\nversionCode={}\nauthor={}\ndescription={}",
            self.id, self.name, self.version, self.versionCode, self.author, self.description
        );
        if let Some(update_json) = &self.updateJson {
            props.push_str(&format!("\nupdateJson={}", update_json));
        }
        props
    }

    /// Deserialize from properties string format
    pub fn from_properties(content: &str) -> Result<ModuleProp, Box<dyn std::error::Error>> {
        let mut map = std::collections::HashMap::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        Ok(ModuleProp {
            id: map.get("id").ok_or("missing id")?.clone(),
            name: map.get("name").ok_or("missing name")?.clone(),
            version: map.get("version").ok_or("missing version")?.clone(),
            versionCode: map.get("versionCode").ok_or("missing versionCode")?.parse()?,
            author: map.get("author").ok_or("missing author")?.clone(),
            description: map.get("description").ok_or("missing description")?.clone(),
            updateJson: map.get("updateJson").cloned(),
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::traits::KamConvertible;



    #[test]
    fn test_new() {
        let prop = ModuleProp {
            id: "test_module".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            versionCode: 123,
            author: "Author".to_string(),
            description: "Desc".to_string(),
            updateJson: Some("https://example.com/update.json".to_string()),
        };
        assert_eq!(prop.id, "test_module");
        assert_eq!(prop.name, "Test");
        assert_eq!(prop.updateJson, Some("https://example.com/update.json".to_string()));
    }

    #[test]
    fn test_to_properties() {
        let prop = ModuleProp {
            id: "test_module".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            versionCode: 123,
            author: "Author".to_string(),
            description: "Desc".to_string(),
            updateJson: Some("https://example.com/update.json".to_string()),
        };
        let props = prop.to_properties();
        assert!(props.contains("id=test_module"));
        assert!(props.contains("updateJson=https://example.com/update.json"));
    }

    #[test]
    fn test_from_properties() {
        let content = "id=test_module\nname=Test\nversion=1.0.0\nversionCode=123\nauthor=Author\ndescription=Desc\nupdateJson=https://example.com/update.json";
        let prop = ModuleProp::from_properties(content).unwrap();
        assert_eq!(prop.id, "test_module");
        assert_eq!(prop.updateJson, Some("https://example.com/update.json".to_string()));
    }

    #[test]
    fn test_from_kam() {
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), "Test Module".to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), "Test Description".to_string());
        let kt = crate::types::kam_toml::KamToml::new(
            "test_id".to_string(),
            name,
            "1.0.0".to_string(),
            123,
            "Test Author".to_string(),
            description,
            Some("https://example.com/update.json".to_string()),
        );
        let prop = ModuleProp::from_kam(&kt);
        assert_eq!(prop.id, "test_id");
        assert_eq!(prop.name, "Test Module");
        assert_eq!(prop.version, "1.0.0");
        assert_eq!(prop.versionCode, 123);
        assert_eq!(prop.author, "Test Author");
        assert_eq!(prop.description, "Test Description");
        assert_eq!(prop.updateJson, Some("https://example.com/update.json".to_string()));
    }

    #[test]
    fn test_to_kam() {
        let prop = ModuleProp {
            id: "test_id".to_string(),
            name: "Test Module".to_string(),
            version: "1.0.0".to_string(),
            versionCode: 123,
            author: "Test Author".to_string(),
            description: "Test Description".to_string(),
            updateJson: Some("https://example.com/update.json".to_string()),
        };
        let kt = prop.to_kam();
        assert_eq!(kt.prop.id, "test_id");
        assert_eq!(kt.prop.get_name(), "Test Module");
        assert_eq!(kt.prop.version, "1.0.0");
        assert_eq!(kt.prop.versionCode, 123);
        assert_eq!(kt.prop.author, "Test Author");
        assert_eq!(kt.prop.get_description(), "Test Description");
        assert_eq!(kt.prop.updateJson, Some("https://example.com/update.json".to_string()));
    }

}

#[allow(non_snake_case)]
impl<'a> crate::types::traits::KamConvertible<'a> for ModuleProp {
    fn from_kam(kam: &'a crate::types::kam_toml::KamToml) -> Self {
        Self {
            id: kam.prop.id.clone(),
            name: kam.prop.get_name().to_string(),
            version: kam.prop.version.clone(),
            versionCode: kam.prop.versionCode,
            author: kam.prop.author.clone(),
            description: kam.prop.get_description().to_string(),
            updateJson: kam.prop.updateJson.clone(),
        }
    }

    fn to_kam(&self) -> crate::types::kam_toml::KamToml {
        // Create a minimal KamToml with prop section
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), self.name.clone());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), self.description.clone());

        crate::types::kam_toml::KamToml::new(
            self.id.clone(),
            name,
            self.version.clone(),
            self.versionCode,
            self.author.clone(),
            description,
            self.updateJson.clone(),
        )
    }
}
