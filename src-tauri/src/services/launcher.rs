use crate::{db::Database, models::Settings};
use std::{path::{Path, PathBuf}, process::{Command, Stdio}};

pub fn validate_executable(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() { return Err(format!("可执行程序不存在，请在设置中填写完整路径：{}", path.display())); }
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !["exe", "com"].contains(&extension.as_str()) { return Err("仅支持 .exe 或 .com 程序，不支持脚本或命令参数".into()); }
    Ok(())
}
fn locate(ide: &str, settings: &Settings) -> Result<PathBuf, String> {
    if let Some(configured) = settings.ide_paths.get(ide).filter(|s| !s.trim().is_empty()) {
        let path = PathBuf::from(configured.trim()); validate_executable(&path)?; return Ok(path);
    }
    let name = match ide { "code" => "Code.exe", "idea" => "idea64.exe", "cursor" => "Cursor.exe", "custom" => return Err("请先在设置中配置自定义程序路径".into()), _ => return Err("不支持的打开工具".into()) };
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") { for dir in std::env::split_paths(&path) { candidates.push(dir.join(name)); } }
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        if ide == "code" { candidates.push(PathBuf::from(&base).join("Programs/Microsoft VS Code/Code.exe")); }
        if ide == "cursor" { candidates.push(PathBuf::from(base).join("Programs/cursor/Cursor.exe")); }
    }
    if ide == "code" { if let Some(base) = std::env::var_os("ProgramFiles") { candidates.push(PathBuf::from(base).join("Microsoft VS Code/Code.exe")); } }
    candidates.into_iter().find(|p| p.is_file() && p.is_absolute()).ok_or_else(|| format!("未找到 {}，请在设置中配置它的 .exe 路径", name))
}
fn spawn(executable: &Path, directory: &str, arguments: &[&str]) -> Result<(), String> {
    let mut command = Command::new(executable);
    command.current_dir(directory).args(arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    command.spawn().map_err(|e| format!("无法启动 {}：{}", executable.display(), e))?;
    Ok(())
}
fn windows_program(relative: &str) -> Result<PathBuf, String> {
    let root = std::env::var_os("SystemRoot").ok_or("找不到 Windows 系统目录")?;
    let path = PathBuf::from(root).join(relative); validate_executable(&path)?; Ok(path)
}
pub fn open(db: &Database, id: &str, action: &str) -> Result<(), String> {
    let project = db.get(id)?;
    let (_, directory) = super::paths::validate(&project.input.path).map_err(|_| "项目目录暂时不可用，请检查磁盘或编辑项目重新定位路径".to_string())?;
    if action == "terminal" {
        // 不使用 -Command，不拼接用户路径为 PowerShell 代码。
        let executable = windows_program("System32/WindowsPowerShell/v1.0/powershell.exe")?;
        spawn(&executable, &directory, &["-NoLogo", "-NoExit"])?;
    } else if action == "readme" {
        let root = Path::new(&directory);
        let readme = ["README.md", "readme.md", "README.txt"].iter().map(|p| root.join(p)).find(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())).ok_or("项目根目录没有 README 文件")?;
        let executable = windows_program("System32/notepad.exe")?;
        spawn(&executable, &directory, &[&readme.to_string_lossy()])?;
    } else {
        let settings = db.settings()?;
        let ide = if action == "system" { if project.input.default_ide == "system" { &settings.default_ide } else { &project.input.default_ide } } else { action };
        let executable = if action == "explorer" || ide == "system" { windows_program("explorer.exe")? } else { locate(ide, &settings)? };
        spawn(&executable, &directory, &[&directory])?;
    }
    db.mark_opened(id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_scripts_and_nonexistent_programs() {
        let dir = tempfile::tempdir().unwrap(); let script = dir.path().join("unsafe.cmd"); std::fs::write(&script, "echo test").unwrap();
        assert!(validate_executable(&script).is_err());
        assert!(validate_executable(&dir.path().join("missing.exe")).is_err());
    }
}
