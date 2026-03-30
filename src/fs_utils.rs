use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

pub fn ensure_dir(path: &Utf8Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create directory {path}"))
}

pub fn write_atomic(path: &Utf8Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    let tmp_name = format!(".{}.tmp", path.file_name().unwrap_or("tmp"));
    let tmp_path = path.parent().unwrap_or(Utf8Path::new(".")).join(tmp_name);
    fs::write(&tmp_path, contents)
        .with_context(|| format!("failed to write temporary file {tmp_path}"))?;
    fs::rename(&tmp_path, path).with_context(|| format!("failed to replace file {path}"))?;
    Ok(())
}

pub fn copy_file(from: &Utf8Path, to: &Utf8Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        ensure_dir(parent)?;
    }
    fs::copy(from, to).with_context(|| format!("failed to copy {from} to {to}"))?;
    Ok(())
}

pub fn canonicalize_utf8(path: &Utf8Path) -> Result<Utf8PathBuf> {
    let canonical =
        fs::canonicalize(path).with_context(|| format!("failed to canonicalize {path}"))?;
    utf8_from_path_buf(canonical)
}

pub fn utf8_from_path_buf(path: std::path::PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|p| anyhow::anyhow!("path is not valid UTF-8: {}", p.display()))
}

pub fn path_exists(path: &Utf8Path) -> bool {
    Path::new(path.as_str()).exists()
}
