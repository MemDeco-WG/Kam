use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct PropSection {
    pub id: String,
    pub name: String,
    pub version: String,
    pub versionCode: i64,
    pub author: String,
    pub description: String,
    pub updateJson: Option<String>,
}

impl PropSection {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_description(&self) -> &str {
        &self.description
    }
}

impl Default for PropSection {
    fn default() -> Self {
        PropSection {
            id: "example_module_id".to_string(),
            name: "Example Module Name".to_string(),
            version: "1.0.0".to_string(),
            versionCode: 1,
            author: "Your Name".to_string(),
            description: "Describe your module here".to_string(),
            updateJson: Some(
                "https://raw.githubusercontent.com/user/repo/branch/update.json".to_string(),
            ),
        }
    }
}
