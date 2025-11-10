use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct MmrlSection {
    pub repo: Option<RepoSection>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct RepoSection {
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub readme: Option<String>,
    pub screenshots: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub maintainers: Option<Vec<String>>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub issues: Option<String>,
    pub funding: Option<String>,
    pub support: Option<String>,
    pub donate: Option<String>,
    pub cover: Option<String>,
    pub icon: Option<String>,
    pub devices: Option<Vec<String>>,
    pub arch: Option<Vec<String>>,
    pub require: Option<Vec<String>>,
    pub note: Option<NoteSection>,
    pub manager: Option<ManagerSection>,
    pub antifeatures: Option<Vec<String>>,
    pub options: Option<OptionsSection>,
    pub max_num: Option<u64>,
    pub min_api: Option<u32>,
    pub max_api: Option<u32>,
    pub verified: Option<bool>,
    pub features: Option<Vec<String>>,
}



#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct NoteSection {
    pub title: String,
    pub message: String,
    pub color: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct ManagerConfig {
    pub min: Option<String>,
    pub devices: Option<Vec<String>>,
    pub arch: Option<Vec<String>>,
    pub require: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct ManagerSection {
    pub magisk: Option<ManagerConfig>,
    pub kernelsu: Option<ManagerConfig>,
    pub apatch: Option<ManagerConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct OptionsSection {
    pub archive: Option<ArchiveOptions>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct ArchiveOptions {
    pub compression: Option<String>,
}
