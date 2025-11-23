use crate::errors::KamError;
use clap::{Args, Subcommand};
use std::fs;
use std::path::Path;
use std::env;

#[derive(Args, Debug)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoCommands,
}

#[derive(Subcommand, Debug)]
pub enum RepoCommands {
    /// Add new repositories
    Add {
        /// Repository URLs to add
        urls: Vec<String>,
    },
    /// List configured repositories
    List,
    /// Remove a repository
    Remove {
        /// Repository URL to remove
        url: String,
    },
}

pub fn run(args: RepoArgs) -> Result<(), KamError> {
    match args.command {
        RepoCommands::Add { urls } => {
            add_repos(urls)?;
            println!("Repositories added successfully.");
        }
        RepoCommands::List => {
            list_repos()?;
        }
        RepoCommands::Remove { url } => {
            remove_repo(url)?;
            println!("Repository removed successfully.");
        }
    }
    Ok(())
}

fn get_repos_path() -> String {
    // Use a path that works in both Android and regular systems
    if cfg!(target_os = "android") {
        "/data/adb/mmrl/repos.json".to_string()
    } else {
        // Use a path in the user's home directory for non-Android systems
        let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/.kam/repos.json", home_dir)
    }
}

fn ensure_repos_dir() -> Result<(), KamError> {
    let repos_path = get_repos_path();
    let file_path = Path::new(&repos_path);
    
    if let Some(parent_dir) = file_path.parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| KamError::Io(e))?;
        }
    }
    
    Ok(())
}

fn add_repos(urls: Vec<String>) -> Result<(), KamError> {
    ensure_repos_dir()?;
    
    let file_path = get_repos_path();
    let mut existing_urls: Vec<String> = if Path::new(&file_path).exists() {
        let file = fs::File::open(&file_path)?;
        serde_json::from_reader(file).unwrap_or_else(|_| vec![])
    } else {
        vec![]
    };

    for url in urls {
        let formatted_url = if url.starts_with("http://") || url.starts_with("https://") {
            // If it's already a full URL, use it as-is
            url
        } else {
            // Apply smart completion based on common patterns
            if url == "gmr" {
                "https://gr.dergoogler.com/gmr/json/modules.json".to_string()
            } else if url == "magisk-alt" {
                "https://magisk-modules-alt-repo.github.io/json-v2/json/modules.json".to_string()
            } else if url.contains('/') && !url.contains('.') {
                // Handle "username/repo" format - this could be GitHub repo
                // For now, we'll make a simple assumption, but this could be extended
                // to check different common patterns like GitHub Pages
                format!("https://{}/json/modules.json", url)
            } else if url.contains("magisk-modules-alt-repo") && url.contains("/json-v2") {
                // If it looks like a base path for the alt repo, append modules.json
                format!("https://{}/json/modules.json", url)
            } else if url.contains("gr.dergoogler.com") && url.contains("/gmr") {
                // If it looks like gmr base path, append json/modules.json
                format!("https://{}/json/modules.json", url)
            } else if url.ends_with("/json/modules.json") {
                // If already points to modules.json, just add https://
                format!("https://{}", url)
            } else if url.ends_with("/json-v2") {
                // If ends with json-v2, append modules.json
                format!("https://{}/json/modules.json", url)
            } else if url.ends_with("/json") {
                // If ends with json, append modules.json
                format!("https://{}/modules.json", url)
            } else {
                // Default: append /json/modules.json
                format!("https://{}/json/modules.json", url)
            }
        };
        
        if !existing_urls.contains(&formatted_url) {
            existing_urls.push(formatted_url);
        }
    }

    let json_content = serde_json::to_string_pretty(&existing_urls)?;
    fs::write(&file_path, json_content)?;
    
    Ok(())
}

fn list_repos() -> Result<(), KamError> {
    let file_path = get_repos_path();
    if !Path::new(&file_path).exists() {
        println!("No repositories configured.");
        return Ok(());
    }
    
    let file = fs::File::open(&file_path)?;
    let urls: Vec<String> = serde_json::from_reader(file)?;
    
    println!("Configured repositories:");
    for url in urls {
        println!("  - {}", url);
    }
    
    Ok(())
}

fn remove_repo(url_to_remove: String) -> Result<(), KamError> {
    let file_path = get_repos_path();
    if !Path::new(&file_path).exists() {
        return Err(KamError::CommandFailed("No repositories file found.".to_string()));
    }
    
    let file = fs::File::open(&file_path)?;
    let mut urls: Vec<String> = serde_json::from_reader(file)?;
    
    // Also check for URL with and without scheme for removal
    urls.retain(|url| {
        url != &url_to_remove 
        && url != &format!("https://{}", url_to_remove)
        && url != &format!("http://{}", url_to_remove)
    });
    
    let json_content = serde_json::to_string_pretty(&urls)?;
    fs::write(&file_path, json_content)?;
    
    Ok(())
}