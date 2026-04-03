use camino::Utf8PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RwsError {
    #[error("workspace manifest already exists at {0}; use `rws update` to modify it")]
    WorkspaceAlreadyInitialized(Utf8PathBuf),
    #[error("workspace manifest not found at {0}")]
    MissingManifest(Utf8PathBuf),
    #[error("invalid package name `{name}`: only letters, digits, and underscores are allowed")]
    InvalidPackageName { name: String },
    #[error("duplicate package name `{0}`")]
    DuplicatePackageName(String),
    #[error("package `{0}` not found in manifest")]
    MissingPackage(String),
    #[error("path does not exist: {0}")]
    MissingPath(Utf8PathBuf),
    #[error("refusing to overwrite existing non-symlink path: {0}")]
    PathConflict(Utf8PathBuf),
    #[error("symlink at {path} points to {actual}, expected {expected}")]
    WrongSymlinkTarget {
        path: Utf8PathBuf,
        actual: Utf8PathBuf,
        expected: Utf8PathBuf,
    },
}
