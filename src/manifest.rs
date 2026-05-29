use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::errors::RwsError;
use crate::fs_utils::write_atomic;

pub const MANIFEST_DIR: &str = ".rws";
pub const MANIFEST_FILE: &str = "rws.toml";
pub const PACKAGE_JUSTFILE: &str = ".rws/justfile";
pub const COMMON_JUSTFILE: &str = ".rws/justfile.common";
pub const WORKSPACE_ENVRC: &str = ".rws/envrc";
pub const DEPENDENCIES_REPOS: &str = ".rws/dependencies.repos";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub packages: Vec<PackageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub name: String,
    pub src_dir: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_justfile: Option<Utf8PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envrc: Option<Utf8PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies_repos: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageEntry {
    pub name: String,
    pub source: Utf8PathBuf,
    pub link: Utf8PathBuf,
    pub justfile: Utf8PathBuf,
    pub enabled: bool,
}

impl Manifest {
    pub fn new(
        workspace_name: String,
        common_justfile: Option<Utf8PathBuf>,
        envrc: Option<Utf8PathBuf>,
        dependencies_repos: Option<Utf8PathBuf>,
    ) -> Self {
        Self {
            workspace: WorkspaceConfig {
                name: workspace_name,
                src_dir: Utf8PathBuf::from("src"),
                common_justfile,
                envrc,
                dependencies_repos,
            },
            packages: Vec::new(),
        }
    }

    pub fn load(path: &Utf8Path) -> Result<Self> {
        let contents =
            fs::read_to_string(path).with_context(|| format!("failed to read manifest {path}"))?;
        let manifest: Self = toml::from_str(&contents)
            .with_context(|| format!("failed to parse manifest {path}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn save(&self, path: &Utf8Path) -> Result<()> {
        self.validate()?;
        let mut manifest = self.clone();
        manifest.packages.sort_by(|a, b| a.name.cmp(&b.name));
        let contents =
            toml::to_string_pretty(&manifest).context("failed to serialize manifest")? + "\n";
        write_atomic(path, &contents)
    }

    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        for package in &self.packages {
            validate_package_name(&package.name)?;
            if !seen.insert(package.name.clone()) {
                bail!(RwsError::DuplicatePackageName(package.name.clone()));
            }
        }
        Ok(())
    }

    pub fn has_package(&self, name: &str) -> bool {
        self.packages.iter().any(|package| package.name == name)
    }

    pub fn remove_package(&mut self, name: &str) -> bool {
        let before = self.packages.len();
        self.packages.retain(|package| package.name != name);
        before != self.packages.len()
    }
}

pub fn manifest_path(workspace_root: &Utf8Path) -> Utf8PathBuf {
    workspace_root.join(MANIFEST_DIR).join(MANIFEST_FILE)
}

pub fn normalize_package_name(raw: &str) -> String {
    raw.trim().replace('-', "_")
}

pub fn validate_package_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        bail!(RwsError::InvalidPackageName {
            name: name.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip_preserves_common_justfile() {
        let manifest = Manifest {
            workspace: WorkspaceConfig {
                name: "robot".into(),
                src_dir: "src".into(),
                common_justfile: Some(COMMON_JUSTFILE.into()),
                envrc: Some(WORKSPACE_ENVRC.into()),
                dependencies_repos: Some(DEPENDENCIES_REPOS.into()),
            },
            packages: vec![PackageEntry {
                name: "camera".into(),
                source: "/tmp/camera".into(),
                link: "src/camera".into(),
                justfile: PACKAGE_JUSTFILE.into(),
                enabled: true,
            }],
        };

        let toml = toml::to_string_pretty(&manifest).unwrap();
        let parsed: Manifest = toml::from_str(&toml).unwrap();

        assert_eq!(manifest, parsed);
    }

    #[test]
    fn normalize_replaces_hyphens() {
        assert_eq!(normalize_package_name("my-robot"), "my_robot");
    }

    #[test]
    fn validate_rejects_invalid_names() {
        assert!(validate_package_name("bad name").is_err());
        assert!(validate_package_name("bad-name").is_err());
        assert!(validate_package_name("good_name1").is_ok());
    }
}
