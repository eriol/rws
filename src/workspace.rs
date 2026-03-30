use std::fs;

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};

use crate::errors::RwsError;
use crate::fs_utils::{canonicalize_utf8, copy_file, ensure_dir, path_exists, write_atomic};
use crate::justfile::generate_root_justfile;
use crate::linker::{SyncReport, sync_links};
use crate::manifest::{
    COMMON_JUSTFILE, MANIFEST_DIR, Manifest, PACKAGE_JUSTFILE, PackageEntry, manifest_path,
    normalize_package_name, validate_package_name,
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

pub fn init_workspace(workspace_root: &Utf8Path, common_justfile: Option<&Utf8Path>) -> Result<()> {
    ensure_dir(workspace_root)?;
    ensure_dir(&workspace_root.join("src"))?;
    ensure_dir(&workspace_root.join(MANIFEST_DIR))?;

    let root_justfile = workspace_root.join("justfile");
    ensure_generated_justfile_can_be_written(&root_justfile)?;

    let common_manifest_path = if let Some(common_source) = common_justfile {
        if !path_exists(common_source) {
            bail!(RwsError::MissingPath(common_source.to_path_buf()));
        }
        let destination = workspace_root.join(COMMON_JUSTFILE);
        copy_file(common_source, &destination)?;
        Some(Utf8PathBuf::from(COMMON_JUSTFILE))
    } else {
        None
    };

    let name = workspace_root
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace")
        .to_string();
    let manifest = Manifest::new(name, common_manifest_path);
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

pub fn add_package(
    workspace_root: &Utf8Path,
    source: &Utf8Path,
    explicit_name: Option<&str>,
) -> Result<PackageEntry> {
    if !path_exists(source) {
        bail!(RwsError::MissingPath(source.to_path_buf()));
    }

    let mut manifest = load_manifest(workspace_root)?;
    let canonical_source = canonicalize_utf8(source)?;
    let resolved_name = resolve_package_name(&canonical_source, explicit_name)?;
    validate_package_name(&resolved_name)?;
    if manifest.has_package(&resolved_name) {
        bail!(RwsError::DuplicatePackageName(resolved_name));
    }

    let package = PackageEntry {
        name: resolved_name.clone(),
        source: canonical_source,
        link: Utf8PathBuf::from(format!("src/{resolved_name}")),
        justfile: Utf8PathBuf::from(PACKAGE_JUSTFILE),
        enabled: true,
    };
    manifest.packages.push(package.clone());
    manifest.save(&manifest_path(workspace_root))?;
    Ok(package)
}

pub fn remove_package(workspace_root: &Utf8Path, name: &str) -> Result<()> {
    let mut manifest = load_manifest(workspace_root)?;
    let normalized = normalize_package_name(name);
    if !manifest.remove_package(&normalized) {
        bail!(RwsError::MissingPackage(normalized));
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

fn resolve_package_name(source: &Utf8Path, explicit_name: Option<&str>) -> Result<String> {
    let name = if let Some(name) = explicit_name {
        name.to_string()
    } else if let Some(name) = detect_package_name(source)? {
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
