use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_metamodule<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum MetamoduleValue {
        Bool(bool),
        Int(i64),
        String(String),
    }

    let value = MetamoduleValue::deserialize(deserializer)?;
    match value {
        MetamoduleValue::Bool(b) => Ok(b),
        MetamoduleValue::Int(i) => match i {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(D::Error::custom(format!(
                "Invalid integer value for metamodule: {}. Expected 0 or 1.",
                i
            ))),
        },
        MetamoduleValue::String(s) => match s.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(D::Error::custom(format!(
                "Invalid string value for metamodule: {}. Expected 'true', 'false', '0', or '1'.",
                s
            ))),
        },
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct PropSection {
    pub id: String,
    pub name: String,
    pub version: String,
    pub versionCode: i64,
    pub author: Option<String>,
    pub description: String,
    pub updateJson: Option<String>,
    #[serde(default, deserialize_with = "deserialize_metamodule")]
    pub metamodule: bool,
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
            author: Some("Your Name".to_string()),
            description: "Describe your module here".to_string(),
            updateJson: Some(
                "https://raw.githubusercontent.com/user/repo/branch/update.json".to_string(),
            ),
            metamodule: false,
        }
    }
}
