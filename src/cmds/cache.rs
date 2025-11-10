/// # Kam Cache Command
/// 
/// Manage the global Kam cache.
/// 
/// ## Subcommands
/// 
/// - `info` - Show cache information and statistics
/// - `clear` - Clear all cache
/// - `clear-dir <dir>` - Clear specific directory (bin, lib, log, profile)
/// - `path` - Show cache root path

use clap::{Args, Subcommand};
use colored::Colorize;
use crate::cache::KamCache;
use crate::errors::KamError;

/// Arguments for the cache command
#[derive(Args, Debug)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommands,
}

/// Cache subcommands
#[derive(Subcommand, Debug)]
pub enum CacheCommands {
    /// Show cache information and statistics
    Info,
    
    /// Clear all cache
    Clear {
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    
    /// Clear a specific cache directory
    ClearDir {
        /// Directory to clear (bin, lib, log, profile)
        dir: String,
        
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    
    /// Show the cache root path
    Path,
}

/// Run the cache command
/// 
/// ## Example
/// 
/// ```bash
/// kam cache info
/// kam cache clear --yes
/// kam cache clear-dir log
/// kam cache path
/// ```
pub fn run(args: CacheArgs) -> Result<(), KamError> {
    match args.command {
        CacheCommands::Info => show_info(),
        CacheCommands::Clear { yes } => clear_cache(yes),
        CacheCommands::ClearDir { dir, yes } => clear_dir(&dir, yes),
        CacheCommands::Path => show_path(),
    }
}

/// Show cache information
fn show_info() -> Result<(), KamError> {
    let cache = KamCache::new()?;
    
    println!("{}", "Kam Cache Information".bold().cyan());
    println!();
    println!("  {}: {}", "Root".bold(), cache.root().display());
    println!();
    
    // Show directory paths
    println!("{}", "Directories:".bold());
    println!("  {}: {}", "bin".yellow(), cache.bin_dir().display());
    println!("  {}: {}", "lib".yellow(), cache.lib_dir().display());
    println!("  {}: {}", "log".yellow(), cache.log_dir().display());
    println!("  {}: {}", "profile".yellow(), cache.profile_dir().display());
    println!();
    
    // Show statistics
    let stats = cache.stats()?;
    println!("{}", "Statistics:".bold());
    println!("  {}: {}", "Total Size".bold(), stats.format_size().green());
    println!("  {}: {}", "File Count".bold(), format!("{}", stats.file_count).green());
    
    Ok(())
}

/// Clear all cache
fn clear_cache(skip_confirm: bool) -> Result<(), KamError> {
    let cache = KamCache::new()?;
    
    if !skip_confirm {
        println!("{}", "Warning: This will delete all cached data!".yellow().bold());
        println!("Cache location: {}", cache.root().display());
        print!("Are you sure? (y/N): ");
        
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", "Cancelled.".yellow());
            return Ok(());
        }
    }
    
    cache.clear_all()?;
    println!("{}", "✓ Cache cleared successfully".green().bold());
    
    Ok(())
}

/// Clear a specific cache directory
fn clear_dir(dir: &str, skip_confirm: bool) -> Result<(), KamError> {
    // Validate directory name
    const VALID_DIRS: &[&str] = &["bin", "lib", "log", "profile"];
    if !VALID_DIRS.contains(&dir) {
        return Err(KamError::Other(format!(
            "Invalid directory '{}'. Valid options: {}",
            dir,
            VALID_DIRS.join(", ")
        )));
    }
    
    let cache = KamCache::new()?;
    
    if !skip_confirm {
        println!("{}", format!("Warning: This will delete all data in the '{}' directory!", dir).yellow().bold());
        print!("Are you sure? (y/N): ");
        
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", "Cancelled.".yellow());
            return Ok(());
        }
    }
    
    cache.clear_dir(dir)?;
    println!("{}", format!("✓ Directory '{}' cleared successfully", dir).green().bold());
    
    Ok(())
}

/// Show the cache root path
fn show_path() -> Result<(), KamError> {
    let cache = KamCache::new()?;
    println!("{}", cache.root().display());
    Ok(())
}
