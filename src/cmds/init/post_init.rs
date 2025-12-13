use std::path::Path;

use crate::errors::KamError;

pub fn post_process(path: &Path) -> Result<(), KamError> {
    // Template variable validation is handled by the template engine and `init_template`.
    // Folders
    use crate::utils::Utils;
    Utils::success(&format!("Initialized Kam project in {}", path.display()));

    Ok(())
}
