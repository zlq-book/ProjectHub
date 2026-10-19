use std::{fs, path::{Path, PathBuf}};

pub fn path_key(path: &str) -> String {
    #[cfg(windows)]
    { path.replace('/', "\\").trim_end_matches('\\').to_lowercase() }
    #[cfg(not(windows))]
    { path.trim_end_matches('/').to_string() }
}
pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") { format!("\\\\{}", unc) }
    else { text.strip_prefix("\\\\?\\").unwrap_or(&text).to_string() }
}
pub fn validate(path: &str) -> Result<(PathBuf, String), String> {
    let text = path.trim();
    if text.is_empty() || text.contains('\0') { return Err("请输入有效的本地项目路径".into()); }
    #[cfg(windows)]
    if text.starts_with("\\\\.\\") || text.starts_with("\\\\?\\") { return Err("请使用普通 Windows 绝对目录路径".into()); }
    let raw = Path::new(text);
    if !raw.is_absolute() { return Err("请输入绝对目录路径，例如 D:\\Projects\\my-project".into()); }
    let canonical = fs::canonicalize(raw).map_err(|e| format!("路径不存在或无法访问：{}", e))?;
    if !canonical.is_dir() { return Err("所选路径不是文件夹".into()); }
    fs::read_dir(&canonical).map_err(|e| format!("没有读取此文件夹的权限：{}", e))?;
    let display = display_path(&canonical);
    Ok((canonical, display))
}
pub fn available(path: &str) -> bool { Path::new(path).is_dir() && fs::read_dir(path).is_ok() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_and_file_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory"); fs::write(&file, "test").unwrap();
        assert!(validate(file.to_str().unwrap()).is_err());
        assert!(validate(dir.path().join("missing").to_str().unwrap()).is_err());
        assert!(validate("relative/folder").is_err());
    }
    #[test]
    fn canonicalizes_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = validate(dir.path().join(".").to_str().unwrap()).unwrap();
        assert_eq!(path, fs::canonicalize(dir.path()).unwrap());
    }
    #[cfg(windows)]
    #[test]
    fn compares_case_and_separators() { assert_eq!(path_key("D:\\Projects\\Demo\\"), path_key("d:/projects/DEMO")); }
}
