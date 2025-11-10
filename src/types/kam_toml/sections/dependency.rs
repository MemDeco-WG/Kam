use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct Dependency {
    pub id: String,
    pub version: Option<String>,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct DependencySection {
    pub normal: Option<Vec<Dependency>>,
    pub dev: Option<Vec<Dependency>>,
}
