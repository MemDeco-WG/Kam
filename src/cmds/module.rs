use crate::errors::KamError;
use crate::repo::{find_module, find_version, get_id_details, Module};
use crate::utils_ext::{confirm, is_url};
use clap::{Args, Subcommand};
use reqwest;
use std::fs;
use std::path::Path;
use std::process::exit;

#[derive(Args, Debug)]
pub struct ModuleArgs {
    #[command(subcommand)]
    pub command: ModuleCommands,
}

#[derive(Subcommand, Debug)]
pub enum ModuleCommands {
    /// View module information
    #[command(aliases = &["view"])]
    Info {
        /// Module IDs to get info for
        ids: Vec<String>,
    },
    /// Search through modules
    #[command(aliases = &["lookup", "find"])]
    Search {
        /// Search query
        query: String,
    },
    /// Download any module
    #[command(aliases = &["dl"])]
    Download {
        /// Module IDs to download
        ids: Vec<String>,
    },
    /// Install any module
    #[command(aliases = &["add", "get", "fetch"])]
    Install {
        /// Skip confirm
        #[arg(short, long)]
        yes: bool,
        /// Module IDs to install
        ids: Vec<String>,
    },
}

pub async fn run_async(args: ModuleArgs) -> Result<(), KamError> {
    let client = reqwest::Client::builder().build().unwrap();
    let modules = fetch_modules().await;

    match args.command {
        ModuleCommands::Info { ids } => {
            for id in ids {
                info(&modules, id)?;
            }
            exit(0);
        }
        ModuleCommands::Search { query } => {
            search(modules, query)?;
            exit(0);
        }
        ModuleCommands::Download { ids } => {
            for id in ids {
                download(client.clone(), &modules, id).await?;
            }
            exit(0);
        }
        ModuleCommands::Install { yes, ids } => {
            for id in ids {
                install(client.clone(), yes, &modules, id).await?;
            }
            exit(0);
        }
    }
}

pub fn run(args: ModuleArgs) -> Result<(), KamError> {
    // Use tokio runtime to run async function
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(run_async(args))
}

async fn fetch_modules() -> Vec<Module> {
    let repos_source = "/data/adb/mmrl/repos.json";
    let mut modules: Vec<Module> = vec![];

    // Check if repos file exists, if not create default
    let file_path = Path::new(repos_source);
    if !file_path.exists() {
        if let Some(parent_dir) = file_path.parent() {
            if !parent_dir.exists() {
                let _ = fs::create_dir_all(parent_dir);
            }
        }
        
        let default_repos = r#"[\"https://gr.dergoogler.com/gmr/json/modules.json\", \"https://magisk-modules-alt-repo.github.io/json-v2/json/modules.json\"]"#;
        let _ = fs::write(file_path, default_repos);
    }

    let file = match fs::File::open(repos_source) {
        Ok(f) => f,
        Err(_) => {
            // Return empty modules if file cannot be opened
            return vec![];
        }
    };
    
    let contents: Result<Vec<String>, _> = serde_json::from_reader(file);
    let urls = match contents {
        Ok(urls) => urls,
        Err(_) => {
            // Return empty modules if file content cannot be parsed
            return vec![]
        }
    };

    let client = reqwest::Client::builder().build().unwrap();
    let mut repos = vec![];

    for url in urls {
        let response = client.get(&url).send().await;
        if let Ok(response) = response {
            if let Ok(repo) = response.json::<crate::repo::Repo>().await {
                repos.push(repo);
            } else {
                eprintln!("! Unable to fetch \"{}\", pushed empty data", url);
                // Create an empty repo to continue
                let empty_repo: crate::repo::Repo = serde_json::from_str(r#"{ \"name\": \"\", \"metadata\": { \"version\": 666, \"timestamp\": 666 }, \"modules\": [] }"#).unwrap();
                repos.push(empty_repo);
            }
        } else {
            eprintln!("! Unable to fetch \"{}\", pushed empty data", url);
            let empty_repo: crate::repo::Repo = serde_json::from_str(r#"{ \"name\": \"\", \"metadata\": { \"version\": 666, \"timestamp\": 666 }, \"modules\": [] }"#).unwrap();
            repos.push(empty_repo);
        }
    }

    for mut repo in repos {
        modules.append(&mut repo.modules);
    }

    modules
}

fn info(modules: &Vec<Module>, id: String) -> Result<(), KamError> {
    let module = find_module(modules, id);
                
    let _id = &module.id;

    let moduleprop = Path::new("/data/adb/modules/").join(_id).join("module.prop");
    println!("\x1B[1mName:\x1B[0m {}", module.name);
    println!("\x1B[1mAuthor:\x1B[0m {}", module.author);
    if moduleprop.exists() {
        if let Ok(content) = fs::read_to_string(moduleprop) {
            // Simple parsing of module.prop file
            for line in content.lines() {
                if line.starts_with("version=") {
                    let version = line.trim_start_matches("version=");
                    if let Some(code_pos) = content.find("versionCode=") {
                        let next_line = &content[code_pos..];
                        let end_pos = next_line.find('\n').unwrap_or(next_line.len());
                        let version_code = &next_line[12..end_pos]; // Skip "versionCode="
                        println!("\x1B[4m\x1B[1mInstalled version: \x1B[34m{} \x1B[33m(\x1B[32m{}\x1B[33m)\x1B[0m", version, version_code) 
                    }
                }
            }
        }
    }
    println!(
        "\x1B[1mLatest version (Cloud):\x1B[0m \x1B[4m\x1B[34m{}\x1B[0m \x1B[33m(\x1B[32m{}\x1B[33m)\x1B[0m",
        module.version,
        module.version_code.to_string()    
    );
    println!("\x1B[1mDescription:\x1B[0m {}", module.description);
    println!("\x1B[1mLicense:\x1B[0m \x1B[36m{}\x1B[0m", module.track.license);
    println!("\x1B[2mModule id: {}\x1B[0m", _id);
    
    Ok(())
}

fn search(modules: Vec<Module>, query: String) -> Result<(), KamError> {
    println!("\x1B[1mFound these modules:\x1B[0m\n\n");
    for module in modules {
        let m = module.clone();
        if m.id.to_lowercase().contains(&query.to_lowercase())
            || m.name.to_lowercase().contains(&query.to_lowercase())
            || m.description.to_lowercase().contains(&query.to_lowercase())
            || m.author.to_lowercase().contains(&query.to_lowercase())
            || m.version.to_lowercase().contains(&query.to_lowercase()) {
                println!(
                    "\x1B[36m\x1B[4m{}\x1B[0m {}\n",
                    m.name,
                    [
                        "\x1B[34m".to_string(),
                        m.version,
                        "\x1B[34m".to_string(),
                        " \x1B[34m(".to_string(),
                        "\x1B[33m".to_string(),
                        m.version_code.to_string(),
                        "\x1B[0m".to_string(),
                        ")\x1B[0m".to_string(),
                        " \x1B[94m[".to_string(),
                        m.track.license,
                        "]\x1B[0m".to_string(),
                        "\n".to_string(),
                        "\x1B[2mId: ".to_string(),
                        m.id,
                        "\x1B[0m".to_string()
                    ]
                    .join("")
                );
            }
    }
    
    Ok(())
}

async fn download(_client: reqwest::Client, modules: &Vec<Module>, id: String) -> Result<(), KamError> {
    let _url = &id.to_owned()[..];
    if is_url(_url) {
        // Handle URL download
        println!("Downloading from URL: {}", id);
        // This would require more implementation for actual download
        Ok(())
    } else {
        let (_id, _ver) = get_id_details(id);
        let module = find_module(&modules, _id.clone());
        let version = find_version(module.versions.clone(), _ver);
        
        println!("Downloading module: {} version: {}", module.name, version.version);
        // This would require more implementation for actual download
        Ok(())
    }
}

async fn install(_client: reqwest::Client, yes: bool, modules: &Vec<Module>, id: String) -> Result<(), KamError> {
    let _url = &id.to_owned()[..];
    if is_url(_url) {
        println!("Installing from URL: {}", id);
        // This would require more implementation for actual install
        Ok(())
    } else {
        let (_id, _ver) = get_id_details(id);
        let module = find_module(&modules, _id.clone());
        let version = find_version(module.versions.clone(), _ver);
        
        let success = yes || confirm("Do you want to continue [y/N] ");
        
        if success {
            println!("Installing module: {} version: {}", module.name, version.version);
            // This would require more implementation for actual install
        } else {
            exit(0);
        }
        
        Ok(())
    }
}