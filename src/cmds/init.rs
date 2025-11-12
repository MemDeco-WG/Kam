use std::collections::HashMap;
use std::path::Path;

use crate::errors::KamError;

pub mod args;
pub mod impl_mod;
pub mod kam;
pub mod post_init;
pub mod repo;
pub mod tmpl_mod;
pub use args::InitArgs;

/// Run the init command
pub fn run(args: InitArgs) -> Result<(), KamError> {
    let path = Path::new(&args.path);

    // Ensure cache is initialized early so templates and builtins are available.
    // Try automatic initialization; if it fails, print a helpful hint and continue.
    if let Err(e) = crate::cache::KamCache::new().and_then(|c| c.ensure_dirs()) {
        println!("Note: failed to initialize Kam cache: {}", e);
        println!("You can initialize the cache by running: 'kam cache info' or 'kam sync'.");
        println!(
            "Continuing init without a cache - some templates or modules may not be available."
        );
    }

    // Validate conflicting flags
    let type_flags = [args.lib, args.tmpl, args.repo]
        .iter()
        .filter(|&&x| x)
        .count();
    if type_flags > 1 {
        return Err(KamError::InvalidModuleType(
            "Cannot specify multiple module types: --lib, --tmpl, --repo".to_string(),
        ));
    }

    // Module type determination will be handled in the main logic below

    // Environment variables used in this project (collected):
    // - GITHUB_TOKEN       : used by `publish` as a default auth token when --token is not provided
    // - KAM_PUBLISH_TOKEN  : alternative token for publish (fallback)
    // - KAM_LOCAL_REPO     : used by `sync` as a candidate local repository path
    // - HOME / USERPROFILE : used by cache code to locate the user's home directory
    //
    // Determine ID from folder name if not provided. If the provided `path` is a
    // relative marker like `.` then `path.file_name()` may be None. In that case
    // fall back to the current working directory's name. If that also fails,
    // fall back to the literal "my_module" to preserve previous behaviour.
    let id = if let Some(id) = args.id.as_deref() {
        id.to_string()
    } else {
        let name_from_path = path
            .file_name()
            .and_then(|n| n.to_str().map(|s| s.to_string()));
        let name = name_from_path.or_else(|| {
            std::env::current_dir().ok().and_then(|p| {
                p.file_name()
                    .and_then(|n| n.to_str().map(|s| s.to_string()))
            })
        });
        name.unwrap_or_else(|| "my_module".to_string())
    };

    // Parse template variables
    let mut template_vars = crate::types::modules::parse_template_vars(&args.var)?;

    let name = args.name.as_deref().unwrap_or("My Module");
    let version = args.version.as_deref().unwrap_or("1.0.0");
    let author = args.author.as_deref().unwrap_or("Author");
    let description = args
        .description
        .as_deref()
        .unwrap_or("A module description");

    // Create name and description maps (English only for simplicity)
    let mut name_map = HashMap::new();
    name_map.insert("en".to_string(), name.to_string());

    let mut description_map = HashMap::new();
    description_map.insert("en".to_string(), description.to_string());

    // Handle different initialization types based on flags
    if args.repo {
        repo::init_repo(
            &path,
            &id,
            name_map,
            &version,
            &author,
            description_map,
            &args.var,
            args.force,
        )?;
    } else if args.tmpl {
        tmpl_mod::init_template(
            &path,
            &id,
            name_map,
            &version,
            &author,
            description_map,
            &args.var,
            args.r#impl.clone(),
            args.force,
        )?;
    } else if args.lib {
        kam::init_kam(
            &path,
            &id,
            name_map,
            &version,
            &author,
            description_map,
            &template_vars,
            args.force,
            "library",
        )?;
    } else if let Some(impl_zip) = &args.r#impl {
        // Implement from template zip
        impl_mod::init_impl(
            &path,
            &id,
            name_map,
            &version,
            &author,
            description_map,
            impl_zip,
            &mut template_vars,
            args.force,
        )?;
    } else {
        // Initialize kam module (default)
        kam::init_kam(
            &path,
            &id,
            name_map,
            &version,
            &author,
            description_map,
            &template_vars,
            args.force,
            "kam",
        )?;
    }

    post_init::post_process(
        &path,
        &args,
        &mut template_vars,
        &id,
        &name,
        &version,
        &author,
        &description,
    )?;

    Ok(())
}
