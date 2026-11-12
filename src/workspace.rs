use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};

use crate::errors::{MissingPackageError, RwsError};
use crate::fs_utils::{canonicalize_utf8, copy_file, ensure_dir, path_exists, write_atomic};
use crate::justfile::generate_root_justfile;
use crate::linker::{SyncReport, sync_links};
use crate::manifest::{
    COMMON_JUSTFILE, DEPENDENCIES_REPOS, MANIFEST_DIR, Manifest, PACKAGE_JUSTFILE, PackageEntry,
    WORKSPACE_ENVRC, manifest_path, normalize_package_name, validate_package_name,
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
    pub dependencies_repos: Option<Utf8PathBuf>,
    pub remove_dependencies_repos: bool,
}

pub fn init_workspace(
    workspace_root: &Utf8Path,
    common_justfile: Option<&Utf8Path>,
    envrc: Option<&Utf8Path>,
    dependencies_repos: Option<&Utf8Path>,
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
    let mut manifest = Manifest::new(name, None, None, None);
    apply_workspace_file_updates(
        workspace_root,
        &mut manifest,
        WorkspaceUpdate {
            common_justfile: common_justfile.map(Utf8Path::to_path_buf),
            remove_common_justfile: false,
            envrc: envrc.map(Utf8Path::to_path_buf),
            remove_envrc: false,
            dependencies_repos: dependencies_repos.map(Utf8Path::to_path_buf),
            remove_dependencies_repos: false,
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
    sync_dependencies_repos(workspace_root, &manifest)?;
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

    if let Some(dependencies_repos) = &manifest.workspace.dependencies_repos {
        let stored_dependencies_repos = workspace_root.join(dependencies_repos);
        if !stored_dependencies_repos.exists() {
            report.warnings.push(format!(
                "missing managed dependencies.repos source: {stored_dependencies_repos}"
            ));
        } else {
            let root_dependencies_repos = workspace_root.join("dependencies.repos");
            if !root_dependencies_repos.exists() {
                report.warnings.push(format!(
                    "missing root dependencies.repos: {root_dependencies_repos}"
                ));
            } else {
                let stored_contents = fs::read_to_string(&stored_dependencies_repos)
                    .with_context(|| format!("failed to read {stored_dependencies_repos}"))?;
                let root_contents = fs::read_to_string(&root_dependencies_repos)
                    .with_context(|| format!("failed to read {root_dependencies_repos}"))?;
                if stored_contents != root_contents {
                    report.warnings.push(format!(
                        "root dependencies.repos differs from managed dependencies.repos source: {}",
                        root_dependencies_repos
                    ));
                }
            }
        }
    }

    let config = &manifest.workspace;
    for (label, managed, source) in [
        (
            "common justfile",
            &config.common_justfile,
            &config.common_justfile_source,
        ),
        ("envrc", &config.envrc, &config.envrc_source),
        (
            "dependencies.repos",
            &config.dependencies_repos,
            &config.dependencies_repos_source,
        ),
    ] {
        if let (Some(managed), Some(source)) = (managed, source) {
            check_recorded_source(&mut report, label, &workspace_root.join(managed), source)?;
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

/// Warns when `source` is missing or its contents differ from `managed`.
/// Skips the comparison when `managed` is missing; that case is reported separately.
fn check_recorded_source(
    report: &mut DoctorReport,
    label: &str,
    managed: &Utf8Path,
    source: &Utf8Path,
) -> Result<()> {
    if !source.exists() {
        report
            .warnings
            .push(format!("missing recorded {label} source: {source}"));
        return Ok(());
    }
    if !managed.exists() {
        return Ok(());
    }

    let managed_contents =
        fs::read_to_string(managed).with_context(|| format!("failed to read {managed}"))?;
    let source_contents =
        fs::read_to_string(source).with_context(|| format!("failed to read {source}"))?;
    if managed_contents != source_contents {
        report.warnings.push(format!(
            "managed {label} differs from recorded source {source}; run `rws update` to refresh it"
        ));
    }
    Ok(())
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
    // Without an explicit path, each managed file is refreshed from its recorded source.
    let config = &mut manifest.workspace;

    if update.remove_common_justfile {
        remove_managed_file(workspace_root, COMMON_JUSTFILE)?;
        config.common_justfile = None;
        config.common_justfile_source = None;
    } else if let Some(source) = update
        .common_justfile
        .or_else(|| config.common_justfile_source.clone())
    {
        config.common_justfile_source =
            install_managed_file(workspace_root, &source, COMMON_JUSTFILE)?;
        config.common_justfile = Some(Utf8PathBuf::from(COMMON_JUSTFILE));
    }

    if update.remove_envrc {
        remove_managed_file(workspace_root, WORKSPACE_ENVRC)?;
        remove_root_envrc(workspace_root)?;
        config.envrc = None;
        config.envrc_source = None;
    } else if let Some(source) = update.envrc.or_else(|| config.envrc_source.clone()) {
        config.envrc_source = install_managed_file(workspace_root, &source, WORKSPACE_ENVRC)?;
        copy_file(
            &workspace_root.join(WORKSPACE_ENVRC),
            &workspace_root.join(".envrc"),
        )?;
        config.envrc = Some(Utf8PathBuf::from(WORKSPACE_ENVRC));
    }

    if update.remove_dependencies_repos {
        remove_managed_file(workspace_root, DEPENDENCIES_REPOS)?;
        remove_root_dependencies_repos(workspace_root)?;
        config.dependencies_repos = None;
        config.dependencies_repos_source = None;
    } else if let Some(source) = update
        .dependencies_repos
        .or_else(|| config.dependencies_repos_source.clone())
    {
        config.dependencies_repos_source =
            install_managed_file(workspace_root, &source, DEPENDENCIES_REPOS)?;
        copy_file(
            &workspace_root.join(DEPENDENCIES_REPOS),
            &workspace_root.join("dependencies.repos"),
        )?;
        config.dependencies_repos = Some(Utf8PathBuf::from(DEPENDENCIES_REPOS));
    }

    Ok(())
}

/// Copies `source` to `destination` under `workspace_root` and returns the canonical `source`
/// to record in the manifest. Returns `None` without copying when `source` is `destination`
/// itself, since copying a file onto itself truncates it.
fn install_managed_file(
    workspace_root: &Utf8Path,
    source: &Utf8Path,
    destination: &str,
) -> Result<Option<Utf8PathBuf>> {
    if !path_exists(source) {
        bail!(RwsError::MissingPath(source.to_path_buf()));
    }

    let source = canonicalize_utf8(source)?;
    let destination = workspace_root.join(destination);
    if path_exists(&destination) && canonicalize_utf8(&destination)? == source {
        return Ok(None);
    }

    copy_file(&source, &destination)?;
    Ok(Some(source))
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

fn remove_root_dependencies_repos(workspace_root: &Utf8Path) -> Result<()> {
    let path = workspace_root.join("dependencies.repos");
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove root dependencies.repos {path}"))
        }
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

fn sync_dependencies_repos(workspace_root: &Utf8Path, manifest: &Manifest) -> Result<()> {
    let Some(dependencies_repos) = &manifest.workspace.dependencies_repos else {
        return Ok(());
    };

    let source = workspace_root.join(dependencies_repos);
    if !source.exists() {
        return Ok(());
    }

    copy_file(&source, &workspace_root.join("dependencies.repos"))
}
