use std::collections::BTreeSet;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;

use anyhow::{Result, bail};
use camino::Utf8Path;

use crate::errors::RwsError;
use crate::fs_utils::utf8_from_path_buf;
use crate::manifest::Manifest;

#[derive(Debug, Default)]
pub struct SyncReport {
    pub warnings: Vec<String>,
}

pub fn sync_links(workspace_root: &Utf8Path, manifest: &Manifest) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let src_dir = workspace_root.join(&manifest.workspace.src_dir);
    fs::create_dir_all(&src_dir)?;

    let mut active_names = BTreeSet::new();
    for package in manifest.packages.iter().filter(|package| package.enabled) {
        active_names.insert(package.name.clone());
        let link_path = workspace_root.join(&package.link);
        ensure_link(&link_path, &package.source)?;

        let justfile_path = package.source.join(&package.justfile);
        if !justfile_path.exists() {
            report.warnings.push(format!(
                "warning: package `{}` is missing justfile at {}",
                package.name, justfile_path
            ));
        }
    }

    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let path = utf8_from_path_buf(entry.path())?;
        let Some(name) = path.file_name() else {
            continue;
        };

        if active_names.contains(name) {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() && looks_like_managed_package_link(&path)? {
            fs::remove_file(&path)?;
        }
    }

    Ok(report)
}

fn looks_like_managed_package_link(link_path: &Utf8Path) -> Result<bool> {
    let target = utf8_from_path_buf(fs::read_link(link_path)?)?;
    let target_abs = if target.is_absolute() {
        target
    } else {
        link_path.parent().unwrap().join(target)
    };

    Ok(target_abs.join(".rws/justfile").exists())
}

fn ensure_link(link_path: &Utf8Path, expected_target: &Utf8Path) -> Result<()> {
    match fs::symlink_metadata(link_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let actual = utf8_from_path_buf(fs::read_link(link_path)?)?;
                let actual_abs = if actual.is_absolute() {
                    actual.clone()
                } else {
                    link_path.parent().unwrap().join(actual)
                };
                if actual_abs != expected_target {
                    bail!(RwsError::WrongSymlinkTarget {
                        path: link_path.to_path_buf(),
                        actual: actual_abs,
                        expected: expected_target.to_path_buf(),
                    });
                }
                Ok(())
            } else {
                bail!(RwsError::PathConflict(link_path.to_path_buf()));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                unix_fs::symlink(expected_target, link_path)?;
                Ok(())
            }
            #[cfg(not(unix))]
            {
                let _ = expected_target;
                let _ = link_path;
                bail!("symlink creation is only supported on unix in v1")
            }
        }
        Err(error) => Err(error.into()),
    }
}
