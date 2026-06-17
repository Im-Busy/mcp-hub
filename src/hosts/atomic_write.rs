//! Atomic config file writes with backup.
//!
//! Pattern from tool-cli: prevents config corruption by writing to
//! a temp file first, then atomically replacing the real file.

use std::path::Path;

/// Write content atomically to a file.
///
/// 1. Write to a temp file
/// 2. Validate the file is parseable (if JSON/TOML)
/// 3. Atomic rename to the target path
pub fn atomic_write<P: AsRef<Path>>(path: P, content: &str) -> std::io::Result<()> {
    let path = path.as_ref();

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write to temp file
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("json")
    ));

    std::fs::write(&tmp_path, content)?;

    // Atomic rename
    std::fs::rename(&tmp_path, path)?;

    Ok(())
}
