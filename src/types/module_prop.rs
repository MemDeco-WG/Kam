use std::borrow::Cow;

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct ModuleProp<'a> {
  pub id: Cow<'a, str>,
  pub name: Cow<'a, str>,
  pub version: Cow<'a, str>,
  pub versionCode: u64,
  pub author: Cow<'a, str>,
  pub description: Cow<'a, str>,
  pub updateJson: Option<Cow<'a, str>>,
}


#[allow(non_snake_case)]
impl<'a> ModuleProp<'a> {

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
    pub fn from_properties(content: &str) -> Result<ModuleProp<'static>, Box<dyn std::error::Error>> {
        let mut map = std::collections::HashMap::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        Ok(ModuleProp {
            id: Cow::Owned(map.get("id").ok_or("missing id")?.clone()),
            name: Cow::Owned(map.get("name").ok_or("missing name")?.clone()),
            version: Cow::Owned(map.get("version").ok_or("missing version")?.clone()),
            versionCode: map.get("versionCode").ok_or("missing versionCode")?.parse()?,
            author: Cow::Owned(map.get("author").ok_or("missing author")?.clone()),
            description: Cow::Owned(map.get("description").ok_or("missing description")?.clone()),
            updateJson: map.get("updateJson").map(|s| Cow::Owned(s.clone())),
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
            id: Cow::Borrowed("test_module"),
            name: Cow::Borrowed("Test"),
            version: Cow::Borrowed("1.0.0"),
            versionCode: 123,
            author: Cow::Borrowed("Author"),
            description: Cow::Borrowed("Desc"),
            updateJson: Some(Cow::Borrowed("https://example.com/update.json")),
        };
        assert_eq!(prop.id, "test_module");
        assert_eq!(prop.name, "Test");
        assert_eq!(prop.updateJson.as_deref(), Some("https://example.com/update.json"));
    }

    #[test]
    fn test_to_properties() {
        let prop = ModuleProp {
            id: Cow::Borrowed("test_module"),
            name: Cow::Borrowed("Test"),
            version: Cow::Borrowed("1.0.0"),
            versionCode: 123,
            author: Cow::Borrowed("Author"),
            description: Cow::Borrowed("Desc"),
            updateJson: Some(Cow::Borrowed("https://example.com/update.json")),
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
        assert_eq!(prop.updateJson.as_deref(), Some("https://example.com/update.json"));
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
        assert_eq!(prop.updateJson.as_deref(), Some("https://example.com/update.json"));
    }

    #[test]
    fn test_to_kam() {
        let prop = ModuleProp {
            id: Cow::Borrowed("test_id"),
            name: Cow::Borrowed("Test Module"),
            version: Cow::Borrowed("1.0.0"),
            versionCode: 123,
            author: Cow::Borrowed("Test Author"),
            description: Cow::Borrowed("Test Description"),
            updateJson: Some(Cow::Borrowed("https://example.com/update.json")),
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
impl<'a> crate::types::traits::KamConvertible<'a> for ModuleProp<'a> {
    fn from_kam(kam: &'a crate::types::kam_toml::KamToml) -> Self {
        Self {
            id: Cow::Borrowed(&kam.prop.id),
            name: Cow::Borrowed(kam.prop.get_name()),
            version: Cow::Borrowed(&kam.prop.version),
            versionCode: kam.prop.versionCode,
            author: Cow::Borrowed(&kam.prop.author),
            description: Cow::Borrowed(kam.prop.get_description()),
            updateJson: kam.prop.updateJson.as_deref().map(Cow::Borrowed),
        }
    }

    fn to_kam(&self) -> crate::types::kam_toml::KamToml {
        // Create a minimal KamToml with prop section
        let mut name = std::collections::HashMap::new();
        name.insert("en".to_string(), self.name.to_string());
        let mut description = std::collections::HashMap::new();
        description.insert("en".to_string(), self.description.to_string());

        crate::types::kam_toml::KamToml::new(
            self.id.to_string(),
            name,
            self.version.to_string(),
            self.versionCode,
            self.author.to_string(),
            description,
            self.updateJson.as_ref().map(|s| s.to_string()),
        )
    }
}
