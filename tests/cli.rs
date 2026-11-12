use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("rws").unwrap()
}

fn create_package(root: &std::path::Path, name: &str, with_justfile: bool) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir_all(path.join(".rws")).unwrap();
    fs::write(
        path.join("package.xml"),
        format!("<package><name>{name}</name></package>"),
    )
    .unwrap();
    if with_justfile {
        fs::write(path.join(".rws/justfile"), "start:\n  @echo start\n").unwrap();
    }
    path
}

fn create_package_with_ros_name(
    root: &std::path::Path,
    dir_name: &str,
    package_name: &str,
    with_justfile: bool,
) -> std::path::PathBuf {
    let path = root.join(dir_name);
    fs::create_dir_all(path.join(".rws")).unwrap();
    fs::write(
        path.join("package.xml"),
        format!("<package><name>{package_name}</name></package>"),
    )
    .unwrap();
    if with_justfile {
        fs::write(path.join(".rws/justfile"), "start:\n  @echo start\n").unwrap();
    }
    path
}

#[test]
fn completion_generates_bash_script() {
    bin()
        .args(["completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_rws"));
}

#[test]
fn completion_generates_zsh_script() {
    bin()
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef rws"));
}

#[test]
fn init_creates_workspace_and_generated_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    assert!(workspace.join("src").is_dir());
    assert!(workspace.join(".rws/rws.toml").is_file());
    assert!(workspace.join("justfile").is_file());
}

#[test]
fn init_copies_common_justfile_into_dot_rws() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    fs::write(&common, "build:\n  @echo build\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(workspace.join(".rws/justfile.common").is_file());
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("common_justfile = \".rws/justfile.common\""));
}

#[test]
fn init_copies_envrc_into_dot_rws_and_root() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let envrc = temp.path().join("envrc");
    fs::write(&envrc, "export DIRENV_DISTROBOX=debian\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--envrc",
            envrc.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/envrc")).unwrap(),
        fs::read_to_string(workspace.join(".envrc")).unwrap()
    );
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("envrc = \".rws/envrc\""));
}

#[test]
fn init_copies_dependencies_repos_into_dot_rws_and_root() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let dependencies_repos = temp.path().join("dependencies.repos");
    fs::write(&dependencies_repos, "repositories:\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--dependencies-repos",
            dependencies_repos.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/dependencies.repos")).unwrap(),
        fs::read_to_string(workspace.join("dependencies.repos")).unwrap()
    );
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("dependencies_repos = \".rws/dependencies.repos\""));
}

#[test]
fn init_rejects_existing_manifest_and_points_to_update() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    let manifest_path = workspace.join(".rws/rws.toml");
    fs::write(&manifest_path, "custom = true\n").unwrap();

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("use `rws update` to modify it"));

    assert_eq!(
        fs::read_to_string(&manifest_path).unwrap(),
        "custom = true\n"
    );
}

#[test]
fn workspace_update_adds_envrc_after_init() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let envrc = temp.path().join("envrc");
    fs::write(&envrc, "export DIRENV_DISTROBOX=debian\n").unwrap();

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--envrc", envrc.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("updated workspace at"));

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/envrc")).unwrap(),
        fs::read_to_string(workspace.join(".envrc")).unwrap()
    );
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("envrc = \".rws/envrc\""));
}

#[test]
fn workspace_update_adds_dependencies_repos_after_init() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let dependencies_repos = temp.path().join("dependencies.repos");
    fs::write(&dependencies_repos, "repositories:\n").unwrap();

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args([
            "update",
            "--dependencies-repos",
            dependencies_repos.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/dependencies.repos")).unwrap(),
        fs::read_to_string(workspace.join("dependencies.repos")).unwrap()
    );
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("dependencies_repos = \".rws/dependencies.repos\""));
}

#[test]
fn workspace_update_adds_common_justfile_after_init() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    fs::write(&common, "build:\n  @echo build\n").unwrap();

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--common-justfile", common.to_str().unwrap()])
        .assert()
        .success();

    assert!(workspace.join(".rws/justfile.common").is_file());
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("common_justfile = \".rws/justfile.common\""));
}

#[test]
fn workspace_update_removes_managed_envrc() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let envrc = temp.path().join("envrc");
    fs::write(&envrc, "export DIRENV_DISTROBOX=debian\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--envrc",
            envrc.to_str().unwrap(),
        ])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--no-envrc"])
        .assert()
        .success();

    assert!(!workspace.join(".rws/envrc").exists());
    assert!(!workspace.join(".envrc").exists());
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(!manifest.contains("envrc = \".rws/envrc\""));
}

#[test]
fn workspace_update_removes_managed_dependencies_repos() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let dependencies_repos = temp.path().join("dependencies.repos");
    fs::write(&dependencies_repos, "repositories:\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--dependencies-repos",
            dependencies_repos.to_str().unwrap(),
        ])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--no-dependencies-repos"])
        .assert()
        .success();

    assert!(!workspace.join(".rws/dependencies.repos").exists());
    assert!(!workspace.join("dependencies.repos").exists());
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(!manifest.contains("dependencies_repos = \".rws/dependencies.repos\""));
}

#[test]
fn workspace_update_removes_common_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    fs::write(&common, "build:\n  @echo build\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
        ])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--no-common-justfile"])
        .assert()
        .success();

    assert!(!workspace.join(".rws/justfile.common").exists());
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(!manifest.contains("common_justfile = \".rws/justfile.common\""));
    assert!(!manifest.contains("common_justfile_source"));
}

#[test]
fn workspace_update_refreshes_managed_files_from_recorded_sources() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    let envrc = temp.path().join("envrc");
    let dependencies_repos = temp.path().join("dependencies.repos");
    fs::write(&common, "build:\n  @echo build\n").unwrap();
    fs::write(&envrc, "export A=1\n").unwrap();
    fs::write(&dependencies_repos, "repositories:\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
            "--envrc",
            envrc.to_str().unwrap(),
            "--dependencies-repos",
            dependencies_repos.to_str().unwrap(),
        ])
        .assert()
        .success();

    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    let canonical_common = fs::canonicalize(&common).unwrap();
    assert!(manifest.contains(&format!(
        "common_justfile_source = \"{}\"",
        canonical_common.display()
    )));

    fs::write(&common, "test:\n  @echo test\n").unwrap();
    fs::write(&envrc, "export A=2\n").unwrap();
    fs::write(&dependencies_repos, "repositories:\n  foo:\n").unwrap();

    bin()
        .current_dir(&workspace)
        .arg("update")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/justfile.common")).unwrap(),
        "test:\n  @echo test\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join(".rws/envrc")).unwrap(),
        "export A=2\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join(".envrc")).unwrap(),
        "export A=2\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join(".rws/dependencies.repos")).unwrap(),
        "repositories:\n  foo:\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("dependencies.repos")).unwrap(),
        "repositories:\n  foo:\n"
    );
}

#[test]
fn workspace_update_refreshes_only_named_files() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    let envrc = temp.path().join("envrc");
    let new_envrc = temp.path().join("new_envrc");
    fs::write(&common, "build:\n  @echo build\n").unwrap();
    fs::write(&envrc, "export A=1\n").unwrap();
    fs::write(&new_envrc, "export B=1\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
            "--envrc",
            envrc.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(&common, "test:\n  @echo test\n").unwrap();
    fs::write(&envrc, "export A=2\n").unwrap();

    bin()
        .current_dir(&workspace)
        .args(["update", "--common-justfile"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/justfile.common")).unwrap(),
        "test:\n  @echo test\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join(".rws/envrc")).unwrap(),
        "export A=1\n"
    );

    fs::write(&common, "lint:\n  @echo lint\n").unwrap();

    bin()
        .current_dir(&workspace)
        .args(["update", "--envrc", new_envrc.to_str().unwrap()])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".envrc")).unwrap(),
        "export B=1\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join(".rws/justfile.common")).unwrap(),
        "test:\n  @echo test\n"
    );
}

#[test]
fn workspace_update_refresh_without_recorded_source_fails() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--common-justfile"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no recorded common justfile source; pass `--common-justfile <PATH>` to set one",
        ));
}

#[test]
fn workspace_update_fails_when_recorded_source_is_missing() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    fs::write(&common, "build:\n  @echo build\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::remove_file(&common).unwrap();

    bin()
        .current_dir(&workspace)
        .arg("update")
        .assert()
        .failure()
        .stderr(predicate::str::contains("path does not exist"));

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/justfile.common")).unwrap(),
        "build:\n  @echo build\n"
    );
}

#[test]
fn workspace_update_with_managed_file_as_source_keeps_contents() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    fs::write(&common, "build:\n  @echo build\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
        ])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["update", "--common-justfile", ".rws/justfile.common"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/justfile.common")).unwrap(),
        "build:\n  @echo build\n"
    );
    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("common_justfile = \".rws/justfile.common\""));
    assert!(!manifest.contains("common_justfile_source"));
}

#[test]
fn doctor_warns_when_managed_file_differs_from_recorded_source() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let common = temp.path().join("common.just");
    fs::write(&common, "build:\n  @echo build\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--common-justfile",
            common.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(&common, "test:\n  @echo test\n").unwrap();

    bin()
        .current_dir(&workspace)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "warning: managed common justfile differs from recorded source",
        ));
}

#[test]
fn add_sync_and_remove_manage_symlinks_and_root_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["package", "add", camera.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("added package `camera`"));

    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();

    let link = workspace.join("src/camera");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let justfile = fs::read_to_string(workspace.join("justfile")).unwrap();
    assert!(justfile.contains("mod camera 'src/camera/.rws/justfile'"));

    bin()
        .current_dir(&workspace)
        .args(["package", "remove", "camera"])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();
    assert!(!link.exists());
}

#[test]
fn add_accepts_multiple_paths_in_one_command() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);
    let lidar = create_package(&packages_root, "lidar", true);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args([
            "package",
            "add",
            camera.to_str().unwrap(),
            lidar.to_str().unwrap(),
        ])
        .assert()
        .success();

    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(manifest.contains("name = \"camera\""));
    assert!(manifest.contains("name = \"lidar\""));
}

#[test]
fn remove_missing_package_lists_available_package_names() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);
    let lidar = create_package(&packages_root, "lidar", true);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args([
            "package",
            "add",
            camera.to_str().unwrap(),
            lidar.to_str().unwrap(),
        ])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["package", "remove", "typo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "package `typo` not found in manifest; available packages: camera, lidar",
        ));
}

#[test]
fn remove_missing_package_reports_empty_manifest() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();

    bin()
        .current_dir(&workspace)
        .args(["package", "remove", "typo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "package `typo` not found in manifest; no packages are registered",
        ));
}

#[test]
fn add_rejects_duplicate_module_names() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let first = create_package_with_ros_name(&packages_root, "camera_a", "robot_cam", true);
    let second = create_package_with_ros_name(&packages_root, "camera_b", "robot_cam", true);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args([
            "package",
            "add",
            first.to_str().unwrap(),
            second.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "duplicate package name `robot_cam`",
        ));
}

#[test]
fn add_rejects_missing_source_path() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args([
            "package",
            "add",
            temp.path().join("missing").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("path does not exist"));
}

#[test]
fn add_is_atomic_when_one_source_is_missing() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);
    let missing = temp.path().join("missing");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args([
            "package",
            "add",
            camera.to_str().unwrap(),
            missing.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("path does not exist"));

    let manifest = fs::read_to_string(workspace.join(".rws/rws.toml")).unwrap();
    assert!(!manifest.contains("name = \"camera\""));
}

#[test]
fn sync_refuses_to_overwrite_real_directory() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["package", "add", camera.to_str().unwrap()])
        .assert()
        .success();
    fs::create_dir_all(workspace.join("src/camera")).unwrap();

    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to overwrite existing non-symlink path",
        ));
}

#[test]
fn doctor_returns_zero_for_warnings_only() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", false);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["package", "add", camera.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();

    bin()
        .current_dir(workspace.join("src"))
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "warning: missing package justfile",
        ));
}

#[test]
fn sync_skips_modules_for_packages_without_justfiles() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let nav = create_package(&packages_root, "visual_nav", false);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["package", "add", nav.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();

    let justfile = fs::read_to_string(workspace.join("justfile")).unwrap();
    assert!(!justfile.contains("mod visual_nav 'src/visual_nav/.rws/justfile'"));
    assert!(!justfile.contains("@echo visual_nav"));
}

#[test]
fn doctor_returns_non_zero_for_missing_source() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["package", "add", camera.to_str().unwrap()])
        .assert()
        .success();
    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();
    fs::remove_dir_all(&camera).unwrap();

    bin()
        .current_dir(&workspace)
        .args(["doctor"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error: missing source path"));
}

#[test]
fn sync_overwrites_root_envrc_from_managed_envrc() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let envrc = temp.path().join("envrc");
    fs::write(&envrc, "export DIRENV_DISTROBOX=debian\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--envrc",
            envrc.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(workspace.join(".envrc"), "changed\n").unwrap();

    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/envrc")).unwrap(),
        fs::read_to_string(workspace.join(".envrc")).unwrap()
    );
}

#[test]
fn sync_overwrites_root_dependencies_repos_from_managed_dependencies_repos() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let dependencies_repos = temp.path().join("dependencies.repos");
    fs::write(&dependencies_repos, "repositories:\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--dependencies-repos",
            dependencies_repos.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(workspace.join("dependencies.repos"), "changed\n").unwrap();

    bin()
        .current_dir(&workspace)
        .args(["sync"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(workspace.join(".rws/dependencies.repos")).unwrap(),
        fs::read_to_string(workspace.join("dependencies.repos")).unwrap()
    );
}

#[test]
fn doctor_warns_when_root_envrc_differs() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let envrc = temp.path().join("envrc");
    fs::write(&envrc, "export DIRENV_DISTROBOX=debian\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--envrc",
            envrc.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(workspace.join(".envrc"), "changed\n").unwrap();

    bin()
        .current_dir(&workspace)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "warning: root envrc differs from managed envrc source",
        ));
}

#[test]
fn doctor_warns_when_root_dependencies_repos_differs() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let dependencies_repos = temp.path().join("dependencies.repos");
    fs::write(&dependencies_repos, "repositories:\n").unwrap();

    bin()
        .args([
            "init",
            workspace.to_str().unwrap(),
            "--dependencies-repos",
            dependencies_repos.to_str().unwrap(),
        ])
        .assert()
        .success();

    fs::write(workspace.join("dependencies.repos"), "changed\n").unwrap();

    bin()
        .current_dir(&workspace)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "warning: root dependencies.repos differs from managed dependencies.repos source",
        ));
}

#[test]
fn non_init_commands_discover_workspace_from_nested_directory() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let nested = workspace.join("src/nested");

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .success();
    fs::create_dir_all(&nested).unwrap();

    bin()
        .current_dir(&nested)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("doctor checks passed"));
}

#[test]
fn package_dot_rws_without_manifest_is_not_a_workspace() {
    let temp = TempDir::new().unwrap();
    let package = create_package(temp.path(), "camera", true);

    bin()
        .current_dir(&package)
        .args(["doctor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rws workspace not found"));
}

#[test]
fn non_init_commands_fail_outside_workspace() {
    let temp = TempDir::new().unwrap();

    bin()
        .current_dir(temp.path())
        .args(["doctor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rws workspace not found"));
}

#[test]
fn init_refuses_to_overwrite_non_generated_root_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("justfile"), "build:\n  @echo custom\n").unwrap();

    bin()
        .args(["init", workspace.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to overwrite existing non-symlink path",
        ));
}
