use camino::Utf8Path;

use crate::manifest::Manifest;

pub fn generate_root_justfile(manifest: &Manifest, workspace_root: &Utf8Path) -> String {
    let mut lines = vec![
        "# GENERATED FILE - DO NOT EDIT".to_string(),
        "# Regenerate with: rws sync".to_string(),
        String::new(),
    ];

    if let Some(common) = &manifest.workspace.common_justfile {
        let common_path = workspace_root.join(common);
        if common_path.exists() {
            lines.push(format!("import '{}'", common.as_str()));
            lines.push(String::new());
        }
    }

    let mut packages = manifest
        .packages
        .iter()
        .filter(|package| package.enabled)
        .filter(|package| {
            workspace_root
                .join(&package.link)
                .join(&package.justfile)
                .exists()
        })
        .collect::<Vec<_>>();
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    for package in &packages {
        let module_path = package.link.join(&package.justfile);
        lines.push(format!("mod {} '{}'", package.name, module_path.as_str()));
    }

    if !packages.is_empty() {
        lines.push(String::new());
    }

    lines.push("default:".into());
    lines.push("  @just --list".into());
    lines.push(String::new());
    lines.push("modules:".into());
    if packages.is_empty() {
        lines.push("  @true".into());
    } else {
        for package in packages {
            lines.push(format!("  @echo {}", package.name));
        }
    }

    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Manifest, PackageEntry, WorkspaceConfig};

    #[test]
    fn generated_justfile_is_sorted_and_imports_common_file() {
        let manifest = Manifest {
            workspace: WorkspaceConfig {
                name: "robot".into(),
                src_dir: "src".into(),
                common_justfile: Some(".rws/justfile.common".into()),
                envrc: None,
            },
            packages: vec![
                PackageEntry {
                    name: "lidar".into(),
                    source: "/tmp/lidar".into(),
                    link: "src/lidar".into(),
                    justfile: ".rws/justfile".into(),
                    enabled: true,
                },
                PackageEntry {
                    name: "camera".into(),
                    source: "/tmp/camera".into(),
                    link: "src/camera".into(),
                    justfile: ".rws/justfile".into(),
                    enabled: true,
                },
            ],
        };

        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".rws")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/camera/.rws")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/lidar/.rws")).unwrap();
        std::fs::write(tmp.path().join(".rws/justfile.common"), "build:\n  true\n").unwrap();
        std::fs::write(
            tmp.path().join("src/camera/.rws/justfile"),
            "start:\n  true\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("src/lidar/.rws/justfile"),
            "start:\n  true\n",
        )
        .unwrap();
        let root = camino::Utf8Path::from_path(tmp.path()).unwrap();

        let generated = generate_root_justfile(&manifest, root);

        assert!(generated.contains("import '.rws/justfile.common'"));
        let camera_idx = generated
            .find("mod camera 'src/camera/.rws/justfile'")
            .unwrap();
        let lidar_idx = generated
            .find("mod lidar 'src/lidar/.rws/justfile'")
            .unwrap();
        assert!(camera_idx < lidar_idx);
    }

    #[test]
    fn generated_justfile_skips_packages_without_existing_justfiles() {
        let manifest = Manifest {
            workspace: WorkspaceConfig {
                name: "robot".into(),
                src_dir: "src".into(),
                common_justfile: None,
                envrc: None,
            },
            packages: vec![
                PackageEntry {
                    name: "camera".into(),
                    source: "/tmp/camera".into(),
                    link: "src/camera".into(),
                    justfile: ".rws/justfile".into(),
                    enabled: true,
                },
                PackageEntry {
                    name: "lidar".into(),
                    source: "/tmp/lidar".into(),
                    link: "src/lidar".into(),
                    justfile: ".rws/justfile".into(),
                    enabled: true,
                },
            ],
        };

        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/camera/.rws")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src/lidar")).unwrap();
        std::fs::write(
            tmp.path().join("src/camera/.rws/justfile"),
            "start:\n  true\n",
        )
        .unwrap();
        let root = camino::Utf8Path::from_path(tmp.path()).unwrap();

        let generated = generate_root_justfile(&manifest, root);

        assert!(generated.contains("mod camera 'src/camera/.rws/justfile'"));
        assert!(!generated.contains("mod lidar 'src/lidar/.rws/justfile'"));
        assert!(generated.contains("  @echo camera"));
        assert!(!generated.contains("  @echo lidar"));
    }
}
