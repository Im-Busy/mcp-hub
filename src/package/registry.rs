//! Package registry index with JSON persistence.
//!
//! Tracks installed packages, their versions, checksums, and install timestamps.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{HubError, HubResult};

/// An installed package tracked in the registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub installed_at: String,
    pub checksum: String,
    pub manifest_path: PathBuf,
}

/// Persistent index of installed packages.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageIndex {
    pub installed: Vec<InstalledPackage>,
}

impl PackageIndex {
    /// Load the package index from a JSON file.
    ///
    /// Returns a default empty index if the file does not exist.
    pub fn load(path: &PathBuf) -> HubResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| HubError::Package(format!("Failed to read index: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| HubError::Package(format!("Invalid index JSON: {}", e)))
    }

    /// Persist the package index to a JSON file.
    ///
    /// Creates parent directories if they do not exist.
    pub fn save(&self, path: &PathBuf) -> HubResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| HubError::Package(format!("mkdir: {}", e)))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| HubError::Package(format!("serialize: {}", e)))?;

        std::fs::write(path, &content)
            .map_err(|e| HubError::Package(format!("write: {}", e)))
    }

    /// Register a newly installed package.
    pub fn add_package(&mut self, pkg: InstalledPackage) {
        self.installed.push(pkg);
    }

    /// Check whether a specific package version is already installed.
    pub fn is_installed(&self, namespace: &str, name: &str, version: &str) -> bool {
        self.installed
            .iter()
            .any(|p| p.namespace == namespace && p.name == name && p.version == version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_add_and_lookup() {
        let mut index = PackageIndex::default();
        let pkg = InstalledPackage {
            namespace: "imbusy".to_string(),
            name: "test-mcp".to_string(),
            version: "1.0.0".to_string(),
            installed_at: "2025-01-01T00:00:00Z".to_string(),
            checksum: "abc123".to_string(),
            manifest_path: PathBuf::from("/fake/path/mcpbx.json"),
        };

        index.add_package(pkg);

        assert!(index.is_installed("imbusy", "test-mcp", "1.0.0"));
        assert!(!index.is_installed("imbusy", "test-mcp", "2.0.0"));
        assert!(!index.is_installed("other", "test-mcp", "1.0.0"));
    }

    #[test]
    fn test_index_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.json");

        let mut index = PackageIndex::default();
        let pkg = InstalledPackage {
            namespace: "library".to_string(),
            name: "bash".to_string(),
            version: "0.5.0".to_string(),
            installed_at: "2025-06-01T12:00:00Z".to_string(),
            checksum: "def456".to_string(),
            manifest_path: PathBuf::from("/fake/bash/mcpbx.json"),
        };
        index.add_package(pkg);

        index.save(&path).unwrap();

        let loaded = PackageIndex::load(&path).unwrap();
        assert_eq!(loaded.installed.len(), 1);
        assert_eq!(loaded.installed[0].namespace, "library");
        assert_eq!(loaded.installed[0].name, "bash");
        assert_eq!(loaded.installed[0].version, "0.5.0");
        assert_eq!(loaded.installed[0].checksum, "def456");
    }

    #[test]
    fn test_index_load_missing_file() {
        let path = PathBuf::from("/nonexistent/path/index.json");
        let index = PackageIndex::load(&path).unwrap();
        assert!(index.installed.is_empty());
    }
}
