use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct Dependency {
    pub id: String,
    /// versionCode may be either a single integer (exact version code)
    /// or a range expressed as a string, e.g. "[1000,2000)".
    pub versionCode: Option<VersionSpec>,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_camel_case_types)]
#[serde(untagged)]
pub enum VersionSpec {
    Exact(i64),
    Range(String),
}

impl VersionSpec {
    pub fn as_display(&self) -> String {
        match self {
            VersionSpec::Exact(v) => v.to_string(),
            VersionSpec::Range(s) => s.clone(),
        }
    }
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
