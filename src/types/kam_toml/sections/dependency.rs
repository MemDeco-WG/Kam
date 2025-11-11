use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct Dependency {
    pub id: String,
    pub version: Option<String>,
    pub versionCode: Option<i64>,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct DependencySection {
    pub kam: Option<Vec<Dependency>>,
    pub dev: Option<Vec<Dependency>>,
}

impl Default for Dependency {
    fn default() -> Self {
        Dependency {
            id: String::new(),
            version: None,
            versionCode: None,
            source: None,
        }
    }
}

impl Default for DependencySection {
    fn default() -> Self {
        DependencySection {
            kam: Some(Vec::new()),
            dev: Some(Vec::new()),
        }
    }
}
