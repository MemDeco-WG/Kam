use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct PropSection {
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub version: String,
    pub versionCode: i64,
    pub author: String,
    pub description: BTreeMap<String, String>,
    pub updateJson: Option<String>,
}

impl PropSection {
    pub fn get_name(&self) -> &str {
        if let Some(v) = self.name.get("en") {
            v.as_str()
        } else if let Some((_k, v)) = self.name.iter().next() {
            v.as_str()
        } else {
            ""
        }
    }

    pub fn get_description(&self) -> &str {
        if let Some(v) = self.description.get("en") {
            v.as_str()
        } else if let Some((_k, v)) = self.description.iter().next() {
            v.as_str()
        } else {
            ""
        }
    }
}

impl Default for PropSection {
    fn default() -> Self {
        let mut name = std::collections::BTreeMap::new();
        name.insert("en".to_string(), "My Module".to_string());
        let mut description = std::collections::BTreeMap::new();
        description.insert("en".to_string(), "A module description".to_string());
        PropSection {
            id: "my_module".to_string(),
            name,
            version: "0.1.0".to_string(),
            versionCode: 1i64,
            author: "Author".to_string(),
            description,
            updateJson: Some("https://example.com/update.json".to_string()),
        }
    }
}
