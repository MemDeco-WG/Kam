use std::path::Path;

use crate::errors::KamError;

/// Perform post-initialization steps for a newly created project at `path`.
///
/// # Errors
/// Returns `KamError` when underlying I/O or other post-processing operations fail.

pub fn post_process(path: &Path) -> Result<(), KamError> {
    // Template variable validation is handled by the template engine and `init_template`.
    // Folders
    use crate::utils::Utils;
    Utils::success(format!("Initialized Kam project in {}", path.display()));

    Ok(())
}
