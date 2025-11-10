use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct PropSection {
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub version: String,
    pub versionCode: u64,
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
