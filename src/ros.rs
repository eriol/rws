use std::fs;

use anyhow::Result;
use camino::Utf8Path;

pub fn detect_package_name(package_root: &Utf8Path) -> Result<Option<String>> {
    let package_xml = package_root.join("package.xml");
    if !package_xml.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&package_xml)?;
    Ok(extract_name(&contents))
}

fn extract_name(contents: &str) -> Option<String> {
    let start = contents.find("<name>")? + "<name>".len();
    let end = contents[start..].find("</name>")? + start;
    let name = contents[start..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_package_name_from_xml() {
        assert_eq!(
            extract_name("<package><name>camera_node</name></package>"),
            Some("camera_node".into())
        );
    }

    #[test]
    fn returns_none_when_name_missing() {
        assert_eq!(extract_name("<package></package>"), None);
    }
}
