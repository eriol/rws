use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};

use crate::errors::{MissingPackageError, RwsError};
use crate::fs_utils::{canonicalize_utf8, copy_file, ensure_dir, path_exists, write_atomic};
use crate::justfile::generate_root_justfile;
use crate::linker::{SyncReport, sync_links};
use crate::manifest::{
    COMMON_JUSTFILE, MANIFEST_DIR, Manifest, PACKAGE_JUSTFILE, PackageEntry, WORKSPACE_ENVRC,
    manifest_path, normalize_package_name, validate_package_name,
};
use crate::ros::detect_package_name;

#[derive(Debug)]
pub struct ListRow {
    pub name: String,
    pub source: Utf8PathBuf,
    pub link: Utf8PathBuf,
    pub has_justfile: bool,
}

#[derive(Debug, Default)]
pub struct DoctorReport {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub struct WorkspaceUpdate {
    pub common_justfile: Option<Utf8PathBuf>,
    pub remove_common_justfile: bool,
    pub envrc: Option<Utf8PathBuf>,
    pub remove_envrc: bool,
}

pub fn init_workspace(
    workspace_root: &Utf8Path,
    common_justfile: Option<&Utf8Path>,
    envrc: Option<&Utf8Path>,
) -> Result<()> {
    ensure_dir(workspace_root)?;
    ensure_dir(&workspace_root.join("src"))?;
    ensure_dir(&workspace_root.join(MANIFEST_DIR))?;

    let manifest_path = manifest_path(workspace_root);
    if path_exists(&manifest_path) {
        bail!(RwsError::WorkspaceAlreadyInitialized(manifest_path));
    }

    let root_justfile = workspace_root.join("justfile");
    ensure_generated_justfile_can_be_written(&root_justfile)?;

    let name = workspace_root
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace")
        .to_string();
    let mut manifest = Manifest::new(name, None, None);
    apply_workspace_file_updates(
        workspace_root,
        &mut manifest,
        WorkspaceUpdate {
            common_justfile: common_justfile.map(Utf8Path::to_path_buf),
            remove_common_justfile: false,
            envrc: envrc.map(Utf8Path::to_path_buf),
            remove_envrc: false,
        },
    )?;
    manifest.save(&manifest_path)?;
    generate_and_write_justfile(workspace_root, &manifest)
}

pub fn update_workspace(workspace_root: &Utf8Path, update: WorkspaceUpdate) -> Result<()> {
    let mut manifest = load_manifest(workspace_root)?;
    apply_workspace_file_updates(workspace_root, &mut manifest, update)?;
    manifest.save(&manifest_path(workspace_root))?;
    generate_and_write_justfile(workspace_root, &manifest)
}

pub fn load_manifest(workspace_root: &Utf8Path) -> Result<Manifest> {
    let path = manifest_path(workspace_root);
    if !path_exists(&path) {
        bail!(RwsError::MissingManifest(path));
    }
    Manifest::load(&path)
}

pub fn add_packages(
    workspace_root: &Utf8Path,
    sources: &[Utf8PathBuf],
) -> Result<Vec<PackageEntry>> {
    let mut manifest = load_manifest(workspace_root)?;
    let mut seen_names = manifest
        .packages
        .iter()
        .map(|package| package.name.clone())
        .collect::<BTreeSet<_>>();
    let mut new_packages = Vec::with_capacity(sources.len());

    for source in sources {
        if !path_exists(source) {
            bail!(RwsError::MissingPath(source.to_path_buf()));
        }

        let canonical_source = canonicalize_utf8(source)?;
        let resolved_name = resolve_package_name(&canonical_source)?;
        validate_package_name(&resolved_name)?;
        if !seen_names.insert(resolved_name.clone()) {
            bail!(RwsError::DuplicatePackageName(resolved_name));
        }

        new_packages.push(PackageEntry {
            name: resolved_name.clone(),
            source: canonical_source,
            link: Utf8PathBuf::from(format!("src/{resolved_name}")),
            justfile: Utf8PathBuf::from(PACKAGE_JUSTFILE),
            enabled: true,
        });
    }

    manifest.packages.extend(new_packages.iter().cloned());
    manifest.save(&manifest_path(workspace_root))?;
    Ok(new_packages)
}

pub fn remove_package(workspace_root: &Utf8Path, name: &str) -> Result<()> {
    let mut manifest = load_manifest(workspace_root)?;
    let normalized = normalize_package_name(name);
    if !manifest.remove_package(&normalized) {
        let mut available = manifest
            .packages
            .iter()
            .map(|package| package.name.clone())
            .collect::<Vec<_>>();
        available.sort();

        bail!(RwsError::MissingPackage(MissingPackageError {
            name: normalized,
            available,
        }));
    }
    manifest.save(&manifest_path(workspace_root))
}

pub fn list_packages(workspace_root: &Utf8Path) -> Result<Vec<ListRow>> {
    let manifest = load_manifest(workspace_root)?;
    let mut rows = manifest
        .packages
        .into_iter()
        .map(|package| {
            let justfile_path = package.source.join(&package.justfile);
            ListRow {
                name: package.name,
                source: package.source,
                link: package.link,
                has_justfile: justfile_path.exists(),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

pub fn sync_workspace(workspace_root: &Utf8Path) -> Result<SyncReport> {
    let manifest = load_manifest(workspace_root)?;
    let report = sync_links(workspace_root, &manifest)?;
    sync_envrc(workspace_root, &manifest)?;
    generate_and_write_justfile(workspace_root, &manifest)?;
    Ok(report)
}

pub fn doctor_workspace(workspace_root: &Utf8Path) -> Result<DoctorReport> {
    let mut report = DoctorReport::default();
    let manifest = load_manifest(workspace_root)?;

    let src_dir = workspace_root.join(&manifest.workspace.src_dir);
    let rws_dir = workspace_root.join(MANIFEST_DIR);
    if !src_dir.is_dir() {
        report
            .errors
            .push(format!("missing src directory: {src_dir}"));
    }
    if !rws_dir.is_dir() {
        report
            .errors
            .push(format!("missing metadata directory: {rws_dir}"));
    }

    if let Some(common) = &manifest.workspace.common_justfile {
        let path = workspace_root.join(common);
        if !path.exists() {
            report
                .warnings
                .push(format!("missing common justfile: {path}"));
        }
    }

    if let Some(envrc) = &manifest.workspace.envrc {
        let stored_envrc = workspace_root.join(envrc);
        if !stored_envrc.exists() {
            report
                .warnings
                .push(format!("missing managed envrc source: {stored_envrc}"));
        } else {
            let root_envrc = workspace_root.join(".envrc");
            if !root_envrc.exists() {
                report
                    .warnings
                    .push(format!("missing root envrc: {root_envrc}"));
            } else {
                let stored_contents = fs::read_to_string(&stored_envrc)
                    .with_context(|| format!("failed to read {stored_envrc}"))?;
                let root_contents = fs::read_to_string(&root_envrc)
                    .with_context(|| format!("failed to read {root_envrc}"))?;
                if stored_contents != root_contents {
                    report.warnings.push(format!(
                        "root envrc differs from managed envrc source: {}",
                        root_envrc
                    ));
                }
            }
        }
    }

    for package in &manifest.packages {
        if !package.source.exists() {
            report.errors.push(format!(
                "missing source path for `{}`: {}",
                package.name, package.source
            ));
        }

        let link_path = workspace_root.join(&package.link);
        match fs::symlink_metadata(&link_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    let target = fs::read_link(&link_path)
                        .with_context(|| format!("failed to read symlink {link_path}"))?;
                    let target = crate::fs_utils::utf8_from_path_buf(target)?;
                    let target_abs = if target.is_absolute() {
                        target
                    } else {
                        link_path.parent().unwrap().join(target)
                    };
                    if target_abs != package.source {
                        report.errors.push(format!(
                            "wrong symlink target for `{}`: {} -> {} (expected {})",
                            package.name, link_path, target_abs, package.source
                        ));
                    }
                } else {
                    report
                        .errors
                        .push(format!("managed link path is not a symlink: {link_path}"));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.errors.push(format!(
                    "missing symlink for `{}`: {}",
                    package.name, link_path
                ));
            }
            Err(error) => return Err(error.into()),
        }

        let package_justfile = package.source.join(&package.justfile);
        if !package_justfile.exists() {
            report.warnings.push(format!(
                "missing package justfile for `{}`: {}",
                package.name, package_justfile
            ));
        }
    }

    Ok(report)
}

pub fn generate_and_write_justfile(workspace_root: &Utf8Path, manifest: &Manifest) -> Result<()> {
    let justfile_path = workspace_root.join("justfile");
    ensure_generated_justfile_can_be_written(&justfile_path)?;
    let contents = generate_root_justfile(manifest, workspace_root);
    write_atomic(&justfile_path, &contents)
}

fn resolve_package_name(source: &Utf8Path) -> Result<String> {
    let name = if let Some(name) = detect_package_name(source)? {
        name
    } else {
        source
            .file_name()
            .context("package source path must have a final path component")?
            .to_string()
    };

    let normalized = normalize_package_name(&name);
    validate_package_name(&normalized)?;
    Ok(normalized)
}

fn ensure_generated_justfile_can_be_written(path: &Utf8Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    if contents.starts_with("# GENERATED FILE - DO NOT EDIT\n") {
        Ok(())
    } else {
        bail!(RwsError::PathConflict(path.to_path_buf()));
    }
}

fn apply_workspace_file_updates(
    workspace_root: &Utf8Path,
    manifest: &mut Manifest,
    update: WorkspaceUpdate,
) -> Result<()> {
    if let Some(common_source) = update.common_justfile.as_deref() {
        copy_managed_file(workspace_root, common_source, COMMON_JUSTFILE)?;
        manifest.workspace.common_justfile = Some(Utf8PathBuf::from(COMMON_JUSTFILE));
    } else if update.remove_common_justfile {
        remove_managed_file(workspace_root, COMMON_JUSTFILE)?;
        manifest.workspace.common_justfile = None;
    }

    if let Some(envrc_source) = update.envrc.as_deref() {
        copy_managed_file(workspace_root, envrc_source, WORKSPACE_ENVRC)?;
        copy_file(
            &workspace_root.join(WORKSPACE_ENVRC),
            &workspace_root.join(".envrc"),
        )?;
        manifest.workspace.envrc = Some(Utf8PathBuf::from(WORKSPACE_ENVRC));
    } else if update.remove_envrc {
        remove_managed_file(workspace_root, WORKSPACE_ENVRC)?;
        remove_root_envrc(workspace_root)?;
        manifest.workspace.envrc = None;
    }

    Ok(())
}

fn copy_managed_file(
    workspace_root: &Utf8Path,
    source: &Utf8Path,
    destination: &str,
) -> Result<()> {
    if !path_exists(source) {
        bail!(RwsError::MissingPath(source.to_path_buf()));
    }

    copy_file(source, &workspace_root.join(destination))
}

fn remove_managed_file(workspace_root: &Utf8Path, path: &str) -> Result<()> {
    let path = workspace_root.join(path);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove managed file {path}")),
    }
}

fn remove_root_envrc(workspace_root: &Utf8Path) -> Result<()> {
    let path = workspace_root.join(".envrc");
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove root envrc {path}")),
    }
}

fn sync_envrc(workspace_root: &Utf8Path, manifest: &Manifest) -> Result<()> {
    let Some(envrc) = &manifest.workspace.envrc else {
        return Ok(());
    };

    let source = workspace_root.join(envrc);
    if !source.exists() {
        return Ok(());
    }

    copy_file(&source, &workspace_root.join(".envrc"))
}
