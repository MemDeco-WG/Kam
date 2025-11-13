use crate::errors::KamError;
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Arguments for the dev command
#[derive(Args, Debug)]
pub struct DevArgs {
    #[command(subcommand)]
    command: DevCommands,
}

#[derive(Subcommand, Debug)]
enum DevCommands {
    /// Collect modules from index into modules.json
    Collect(CollectArgs),
    /// Create index directory structure
    Mkindex(MkindexArgs),
    /// Sync modules.json to index
    Sync(SyncArgs),
}

#[derive(Args, Debug)]
pub struct CollectArgs {
    /// Path to the index directory
    index_path: String,
    /// Output file
    #[arg(short, long, default_value = "modules.json")]
    output: String,
}

#[derive(Args, Debug)]
pub struct MkindexArgs {
    /// Path to the index directory
    index_path: String,
    /// Ensure directory structure exists
    #[arg(short = 'p', long)]
    ensure: bool,
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Path to modules.json
    input: String,
    /// Path to the index directory
    #[arg(short, long)]
    output: String,
}

/// Run the dev command
pub fn run(args: DevArgs) -> Result<(), KamError> {
    match args.command {
        DevCommands::Collect(a) => collect(a),
        DevCommands::Mkindex(a) => mkindex(a),
        DevCommands::Sync(a) => sync(a),
    }
}

fn collect(args: CollectArgs) -> Result<(), KamError> {
    let index_path = Path::new(&args.index_path);
    let mut modules_map: HashMap<String, Vec<IndexEntry>> = HashMap::new();

    for entry in WalkDir::new(index_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(parent) = path.parent() {
                if parent.starts_with(index_path) {
                    let content = fs::read_to_string(path)?;
                    for line in content.lines() {
                        if let Ok(entry) = serde_json::from_str::<IndexEntry>(line) {
                            modules_map.entry(entry.name.clone()).or_insert_with(Vec::new).push(entry);
                        }
                    }
                }
            }
        }
    }

    let mut modules = Vec::new();
    for (id, mut entries) in modules_map {
        entries.sort_by_key(|e| e.versionCode.unwrap_or(0));
        let versions: Vec<Version> = entries.iter().map(|e| Version {
            timestamp: e.timestamp,
            version: e.vers.clone(),
            versionCode: e.versionCode,
            zipUrl: e.zipUrl.clone(),
            changelog: e.changelog.clone(),
            size: e.size,
        }).collect();
        if let Some(latest) = entries.last() {
            let module = Module {
                id: id.clone(),
                name: latest.name.clone(),
                version: latest.vers.clone(),
                versionCode: latest.versionCode,
                author: latest.author.clone(),
                description: latest.description.clone(),
                added: latest.added,
                require: latest.require.clone(),
                categories: latest.categories.clone(),
                support: latest.support.clone(),
                license: latest.license.clone(),
                readme: latest.readme.clone(),
                verified: latest.verified,
                timestamp: latest.timestamp,
                size: latest.size,
                features: latest.features.clone(),
                track: latest.track.clone(),
                versions,
            };
            modules.push(module);
        }
    }

    let len = modules.len();
    let modules_json = ModulesJson { modules };
    let json = serde_json::to_string_pretty(&modules_json)?;
    fs::write(&args.output, json)?;
    println!("Collected {} modules to {}", len, args.output);
    Ok(())
}

fn mkindex(args: MkindexArgs) -> Result<(), KamError> {
    let index_path = Path::new(&args.index_path);
    if args.ensure {
        fs::create_dir_all(index_path)?;
    }

    // Create single char dirs
    for char in '0'..='9' {
        fs::create_dir_all(index_path.join(char.to_string()))?;
    }
    for char in 'a'..='z' {
        fs::create_dir_all(index_path.join(char.to_string()))?;
    }

    // Create two char dirs
    for first in 'a'..='z' {
        for second in 'a'..='z' {
            fs::create_dir_all(index_path.join(format!("{}{}", first, second)))?;
        }
    }

    println!("Index directories created in {}", args.index_path);
    Ok(())
}

fn sync(args: SyncArgs) -> Result<(), KamError> {
    let content = fs::read_to_string(&args.input)?;
    let modules_json: ModulesJson = serde_json::from_str(&content)?;
    let index_path = Path::new(&args.output);

    for module in modules_json.modules {
        let prefix = get_prefix(&module.id);
        let file_path = index_path.join(prefix).join(&module.id);
        fs::create_dir_all(file_path.parent().unwrap())?;

        let mut content = String::new();
        for version in module.versions {
            let mut hasher = Sha256::new();
            hasher.update(version.zipUrl.as_bytes());
            let cksum = format!("{:x}", hasher.finalize());

            let entry = IndexEntry {
                name: module.id.clone(),
                vers: version.version,
                versionCode: version.versionCode,
                zipUrl: version.zipUrl,
                changelog: version.changelog,
                size: version.size,
                timestamp: version.timestamp,
                author: module.author.clone(),
                description: module.description.clone(),
                added: module.added,
                require: module.require.clone(),
                categories: module.categories.clone(),
                support: module.support.clone(),
                license: module.license.clone(),
                readme: module.readme.clone(),
                verified: module.verified,
                features: module.features.clone(),
                track: module.track.clone(),
                cksum,
                yanked: false,
            };
            content.push_str(&serde_json::to_string(&entry)?);
            content.push('\n');
        }
        fs::write(&file_path, content)?;
    }

    println!("Synced to index {}", args.output);
    Ok(())
}

fn get_prefix(id: &str) -> String {
    if id.len() == 1 {
        format!("{}{}", id, id)
    } else {
        id[0..2].to_string()
    }
}

#[derive(Serialize, Deserialize)]
struct ModulesJson {
    modules: Vec<Module>,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct Module {
    id: String,
    name: String,
    version: String,
    versionCode: Option<u32>,
    author: String,
    description: String,
    added: Option<f64>,
    #[serde(default)]
    require: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
    support: Option<String>,
    license: Option<String>,
    readme: Option<String>,
    #[serde(default)]
    verified: bool,
    timestamp: Option<f64>,
    size: Option<u64>,
    features: Option<Features>,
    track: Option<Track>,
    versions: Vec<Version>,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct Version {
    timestamp: Option<f64>,
    version: String,
    versionCode: Option<u32>,
    zipUrl: String,
    changelog: Option<String>,
    size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Features {
    service: Option<bool>,
    post_fs_data: Option<bool>,
    resetprop: Option<bool>,
    zygisk: Option<bool>,
    webroot: Option<bool>,
    apks: Option<bool>,
    sepolicy: Option<bool>,
    action: Option<bool>,
    boot_completed: Option<bool>,
    modconf: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone)]
#[allow(non_snake_case)]
struct Track {
    #[serde(rename = "type")]
    r#type: String,
    added: Option<f64>,
    source: String,
    antifeatures: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
struct IndexEntry {
    name: String,
    vers: String,
    versionCode: Option<u32>,
    zipUrl: String,
    changelog: Option<String>,
    size: Option<u64>,
    timestamp: Option<f64>,
    author: String,
    description: String,
    added: Option<f64>,
    #[serde(default)]
    require: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
    support: Option<String>,
    license: Option<String>,
    readme: Option<String>,
    #[serde(default)]
    verified: bool,
    features: Option<Features>,
    track: Option<Track>,
    cksum: String,
    yanked: bool,
}
