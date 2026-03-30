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

#[test]
fn init_creates_workspace_and_generated_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
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
            "--workspace",
            workspace.to_str().unwrap(),
            "init",
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
fn add_sync_and_remove_manage_symlinks_and_root_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            camera.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("added package `camera`"));

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "sync"])
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
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "remove",
            "camera",
        ])
        .assert()
        .success();
    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "sync"])
        .assert()
        .success();
    assert!(!link.exists());
}

#[test]
fn add_rejects_duplicate_module_names() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let first = create_package(&packages_root, "camera", true);
    let second = create_package(&packages_root, "another", true);

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            first.to_str().unwrap(),
            "--name",
            "robot_cam",
        ])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            second.to_str().unwrap(),
            "--name",
            "robot_cam",
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
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            temp.path().join("missing").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("path does not exist"));
}

#[test]
fn sync_refuses_to_overwrite_real_directory() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            camera.to_str().unwrap(),
        ])
        .assert()
        .success();
    fs::create_dir_all(workspace.join("src/camera")).unwrap();

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "sync"])
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
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            camera.to_str().unwrap(),
        ])
        .assert()
        .success();
    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "sync"])
        .assert()
        .success();

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "warning: missing package justfile",
        ));
}

#[test]
fn doctor_returns_non_zero_for_missing_source() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    let packages_root = temp.path().join("pkgs");
    fs::create_dir_all(&packages_root).unwrap();
    let camera = create_package(&packages_root, "camera", true);

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .success();
    bin()
        .args([
            "--workspace",
            workspace.to_str().unwrap(),
            "add",
            camera.to_str().unwrap(),
        ])
        .assert()
        .success();
    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "sync"])
        .assert()
        .success();
    fs::remove_dir_all(&camera).unwrap();

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "doctor"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error: missing source path"));
}

#[test]
fn init_refuses_to_overwrite_non_generated_root_justfile() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("ws");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(workspace.join("justfile"), "build:\n  @echo custom\n").unwrap();

    bin()
        .args(["--workspace", workspace.to_str().unwrap(), "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to overwrite existing non-symlink path",
        ));
}
