//! Package installer using git clone for fetching packages.
//!
//! Pattern from tool-cli: git-based install with manifest validation,
//! SHA-256 checksumming, and local cache management.

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::error::{HubError, HubResult};
use crate::package::manifest::McpbxManifest;

/// Package installer using git clone for fetching packages.
pub struct Installer {
    cache_dir: PathBuf,
}

impl Installer {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Install a package from a git reference.
    ///
    /// `package_ref` format: `"namespace/name"` or `"namespace/name@version"`.
    /// If no registry_url is provided, defaults to GitHub (`github.com/namespace/name.git`).
    pub fn install(
        &self,
        package_ref: &str,
        registry_url: Option<&str>,
    ) -> HubResult<McpbxManifest> {
        let (namespace, name, _version) = parse_package_ref(package_ref)?;

        let repo_url = registry_url
            .map(|u| format!("{}/{}/{}", u.trim_end_matches('/'), namespace, name))
            .unwrap_or_else(|| format!("https://github.com/{}/{}.git", namespace, name));

        let temp_dir = std::env::temp_dir().join(format!("mcp-hub-install-{}", uuid::Uuid::new_v4()));
        let clone_result = clone_repo(&repo_url, &temp_dir);
        if let Err(e) = &clone_result {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(HubError::Package(format!(
                "Failed to clone {}: {}",
                repo_url, e
            )));
        }

        let manifest_path = find_manifest(&temp_dir).ok_or_else(|| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            HubError::Package("No mcpbx.json or mcp-hub.json manifest found in repository".to_string())
        })?;

        let manifest_content =
            std::fs::read_to_string(&manifest_path).map_err(|e| HubError::Package(format!("Failed to read manifest: {}", e)))?;

        let manifest: McpbxManifest =
            serde_json::from_str(&manifest_content).map_err(|e| HubError::Package(format!("Invalid manifest: {}", e)))?;

        let repo_hash = compute_repo_hash(&temp_dir)?;

        let install_dir = self
            .cache_dir
            .join(&namespace)
            .join(&name)
            .join(&manifest.version);
        std::fs::create_dir_all(&install_dir)
            .map_err(|e| HubError::Package(format!("Failed to create install dir: {}", e)))?;

        copy_dir_contents(&temp_dir, &install_dir)?;

        let _ = std::fs::remove_dir_all(&temp_dir);

        tracing::info!(
            namespace = %namespace,
            name = %name,
            version = %manifest.version,
            hash = %repo_hash,
            "Package installed successfully"
        );

        Ok(manifest)
    }

    /// Validate a package at a local path for publishing.
    ///
    /// Checks that a manifest exists, parses it, and computes a directory hash.
    pub fn publish(&self, path: &Path, _registry_url: Option<&str>) -> HubResult<()> {
        let manifest_path = find_manifest(path).ok_or_else(|| {
            HubError::Package("No mcpbx.json or mcp-hub.json manifest found".to_string())
        })?;

        let manifest_content =
            std::fs::read_to_string(&manifest_path).map_err(|e| HubError::Package(format!("Failed to read manifest: {}", e)))?;

        let manifest: McpbxManifest =
            serde_json::from_str(&manifest_content).map_err(|e| HubError::Package(format!("Invalid manifest: {}", e)))?;

        let checksum = compute_dir_hash(path)?;

        tracing::info!(
            name = %manifest.name,
            version = %manifest.version,
            checksum = %checksum,
            "Package validated for publishing"
        );

        Ok(())
    }
}

/// Parse a package reference like `"imbusy/searxng-mcp@1.2.0"`.
fn parse_package_ref(package_ref: &str) -> HubResult<(String, String, String)> {
    let (ns_name, version) = if let Some(pos) = package_ref.find('@') {
        let (prefix, ver) = package_ref.split_at(pos);
        (prefix.to_string(), ver[1..].to_string())
    } else {
        (package_ref.to_string(), "latest".to_string())
    };

    let parts: Vec<&str> = ns_name.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(HubError::Package(format!(
            "Invalid package reference '{}'. Expected format: namespace/name[@version]",
            package_ref
        )));
    }

    Ok((parts[0].to_string(), parts[1].to_string(), version))
}

/// Search a directory for a known manifest file.
fn find_manifest(dir: &Path) -> Option<PathBuf> {
    for name in &["mcpbx.json", "mcp-hub.json"] {
        let path = dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Clone a git repository with depth=1 into the target directory.
fn clone_repo(url: &str, dest: &Path) -> HubResult<()> {
    let output = Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(dest)
        .output()
        .map_err(|e| HubError::Package(format!("git clone failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HubError::Package(format!("git clone failed: {}", stderr)));
    }

    Ok(())
}

/// Compute a short SHA-256 hash of the repository contents (first 16 chars).
fn compute_repo_hash(dir: &Path) -> HubResult<String> {
    compute_dir_hash_inner(dir, 16)
}

/// Compute the full SHA-256 hex string of directory contents.
fn compute_dir_hash(dir: &Path) -> HubResult<String> {
    compute_dir_hash_inner(dir, 64)
}

/// Compute a deterministic SHA-256 hash of all non-dotfile entries in a directory.
fn compute_dir_hash_inner(dir: &Path, hex_len: usize) -> HubResult<String> {
    let mut hasher = Sha256::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| HubError::Package(format!("read_dir failed: {}", e)))?
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_file() {
            let content = std::fs::read(&path)
                .map_err(|e| HubError::Package(format!("read file failed: {}", e)))?;
            hasher.update(entry.file_name().to_string_lossy().as_bytes());
            hasher.update(&content);
        }
    }

    let result = format!("{:x}", hasher.finalize());
    Ok(result.chars().take(hex_len).collect())
}

/// Recursively copy directory contents from src to dest.
fn copy_dir_contents(src: &Path, dest: &Path) -> HubResult<()> {
    for entry in
        std::fs::read_dir(src).map_err(|e| HubError::Package(format!("read_dir: {}", e)))?
    {
        let entry = entry.map_err(|e| HubError::Package(format!("entry error: {}", e)))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            std::fs::create_dir_all(&dest_path)
                .map_err(|e| HubError::Package(format!("mkdir: {}", e)))?;
            copy_dir_contents(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)
                .map_err(|e| HubError::Package(format!("copy: {}", e)))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_package_ref_with_version() {
        let (ns, name, ver) = parse_package_ref("foo/bar@1.0").unwrap();
        assert_eq!(ns, "foo");
        assert_eq!(name, "bar");
        assert_eq!(ver, "1.0");
    }

    #[test]
    fn test_parse_package_ref_no_version() {
        let (ns, name, ver) = parse_package_ref("foo/bar").unwrap();
        assert_eq!(ns, "foo");
        assert_eq!(name, "bar");
        assert_eq!(ver, "latest");
    }

    #[test]
    fn test_parse_package_ref_invalid() {
        let result = parse_package_ref("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_manifest_mcpbx() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mcpbx.json"), "{}").unwrap();
        let found = find_manifest(dir.path());
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("mcpbx.json"));
    }

    #[test]
    fn test_find_manifest_mcp_hub() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mcp-hub.json"), "{}").unwrap();
        let found = find_manifest(dir.path());
        assert!(found.is_some());
        assert!(found.unwrap().ends_with("mcp-hub.json"));
    }

    #[test]
    fn test_find_manifest_none() {
        let dir = tempfile::tempdir().unwrap();
        let found = find_manifest(dir.path());
        assert!(found.is_none());
    }

    #[test]
    fn test_compute_dir_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.txt");
        let mut file = std::fs::File::create(&f1).unwrap();
        writeln!(file, "hello").unwrap();

        let hash1 = compute_dir_hash(dir.path()).unwrap();
        let hash2 = compute_dir_hash(dir.path()).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_compute_repo_hash_length() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.txt");
        let mut file = std::fs::File::create(&f1).unwrap();
        writeln!(file, "test").unwrap();

        let hash = compute_repo_hash(dir.path()).unwrap();
        assert_eq!(hash.len(), 16);
    }
}
