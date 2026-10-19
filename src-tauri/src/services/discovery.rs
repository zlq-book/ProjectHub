use super::{detection, paths};
use crate::models::{ScanCandidate, ScanResult};
use std::{collections::{HashSet, VecDeque}, fs, path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

const MAX_DIRECTORIES: usize = 5000;
const MAX_ENTRIES: usize = 20000;
const MAX_CANDIDATES: usize = 500;
const SKIP: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".venv", "venv", "__pycache__", ".idea", ".next", ".nuxt", ".cache", ".pytest_cache", "vendor", "bin", "obj", "$recycle.bin", "system volume information"];

#[derive(Clone, Default)]
pub struct Scanner { current: Arc<Mutex<Option<(String, Arc<AtomicBool>)>>> }
pub struct ScanLease { scanner: Scanner, id: String, pub token: Arc<AtomicBool> }
impl Drop for ScanLease { fn drop(&mut self) { self.scanner.finish(&self.id); } }
impl Scanner {
    pub fn lease(&self, id: &str) -> Result<ScanLease, String> {
        Ok(ScanLease { token: self.begin(id)?, scanner: self.clone(), id: id.into() })
    }
    pub fn begin(&self, id: &str) -> Result<Arc<AtomicBool>, String> {
        if id.is_empty() || id.len() > 128 { return Err("扫描请求无效".into()); }
        let mut active = self.current.lock().map_err(|_| "扫描状态异常".to_string())?;
        if active.is_some() { return Err("已有扫描正在执行，请取消或等待它结束".into()); }
        let token = Arc::new(AtomicBool::new(false));
        *active = Some((id.to_string(), token.clone())); Ok(token)
    }
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let active = self.current.lock().map_err(|_| "扫描状态异常".to_string())?;
        if let Some((current, token)) = active.as_ref() { if current == id { token.store(true, Ordering::Relaxed); } }
        Ok(())
    }
    pub fn finish(&self, id: &str) {
        if let Ok(mut active) = self.current.lock() { if active.as_ref().is_some_and(|(current, _)| current == id) { *active = None; } }
    }
}
fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() { return true; }
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; if metadata.file_attributes() & 0x400 != 0 { return true; } }
    false
}
fn feature(name: &str) -> bool {
    ["pom.xml", "build.gradle", "build.gradle.kts", "package.json", "requirements.txt", "pyproject.toml", "cargo.toml", "go.mod", ".git"].contains(&name) || name.ends_with(".sln") || name.ends_with(".csproj")
}
fn warning(result: &mut ScanResult, message: String) { if result.warnings.len() < 50 { result.warnings.push(message); } }
pub fn scan(roots: Vec<String>, depth: usize, token: &AtomicBool, known: &HashSet<String>) -> Result<ScanResult, String> {
    if roots.is_empty() || roots.len() > 20 || !(1..=4).contains(&depth) { return Err("请选择 1–20 个根目录，扫描深度须为 1–4".into()); }
    let started = Instant::now(); let mut result = ScanResult::default();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new(); let mut seen = HashSet::new(); let mut entries_count = 0;
    for root in roots {
        if token.load(Ordering::Relaxed) { return Err("扫描已取消".into()); }
        if started.elapsed() >= Duration::from_secs(15) { result.truncated = true; break; }
        match paths::validate(&root) { Ok((path, _)) => queue.push_back((path, 0)), Err(e) => warning(&mut result, format!("{}：{}", root, e)) }
    }
    while let Some((directory, level)) = queue.pop_front() {
        if token.load(Ordering::Relaxed) { return Err("扫描已取消".into()); }
        if result.scanned_directories >= MAX_DIRECTORIES || entries_count >= MAX_ENTRIES || result.projects.len() >= MAX_CANDIDATES || started.elapsed() >= Duration::from_secs(15) { result.truncated = true; break; }
        let key = paths::path_key(&paths::display_path(&directory)); if !seen.insert(key) { continue; }
        result.scanned_directories += 1;
        let entries = match fs::read_dir(&directory) { Ok(entries) => entries, Err(e) => { warning(&mut result, format!("{}：{}", paths::display_path(&directory), e)); continue; } };
        let mut potential = false;
        for entry in entries {
            if token.load(Ordering::Relaxed) { return Err("扫描已取消".into()); }
            if entries_count >= MAX_ENTRIES || started.elapsed() >= Duration::from_secs(15) { result.truncated = true; break; }
            entries_count += 1;
            let entry = match entry { Ok(entry) => entry, Err(e) => { warning(&mut result, e.to_string()); continue; } };
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let metadata = match fs::symlink_metadata(entry.path()) { Ok(m) => m, Err(_) => continue };
            if linked(&metadata) { continue; }
            if feature(&name) && (metadata.is_file() || name == ".git" && metadata.is_dir()) { potential = true; }
            if metadata.is_dir() && level < depth && !SKIP.contains(&name.as_str()) { queue.push_back((entry.path(), level + 1)); }
        }
        if potential {
            if token.load(Ordering::Relaxed) { return Err("扫描已取消".into()); }
            if result.truncated || started.elapsed() >= Duration::from_secs(15) { result.truncated = true; break; }
            let detected = detection::detect(&paths::display_path(&directory));
            if token.load(Ordering::Relaxed) { return Err("扫描已取消".into()); }
            match detected {
                Ok(detection) => result.projects.push(ScanCandidate { existing: known.contains(&paths::path_key(&detection.path)), detection }),
                Err(e) => warning(&mut result, format!("{}：{}", paths::display_path(&directory), e)),
            }
            if started.elapsed() >= Duration::from_secs(15) { result.truncated = true; break; }
        }
        if result.truncated { break; }
    }
    if started.elapsed() >= Duration::from_secs(15) { result.truncated = true; }
    if result.truncated { warning(&mut result, "扫描达到数量或时间上限，结果可能不完整；请缩小根目录范围后重试。".into()); }
    result.projects.sort_by(|a, b| a.detection.path.cmp(&b.detection.path));
    if token.load(Ordering::Relaxed) { return Err("扫描已取消".into()); }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_multiple_roots_deduplicates_and_skips_dependencies() {
        let root = tempfile::tempdir().unwrap();
        for name in ["app", "another", "node_modules/ignored", "target/ignored"] {
            let directory = root.path().join(name); fs::create_dir_all(&directory).unwrap(); fs::write(directory.join("package.json"), "{}").unwrap();
        }
        let roots = vec![root.path().to_string_lossy().into_owned(), root.path().join("app").to_string_lossy().into_owned()];
        let result = scan(roots, 2, &AtomicBool::new(false), &HashSet::new()).unwrap();
        assert_eq!(result.projects.len(), 2); assert!(!result.truncated);
    }
    #[test]
    fn cancellation_and_depth_are_enforced() {
        let root = tempfile::tempdir().unwrap(); let nested = root.path().join("one/two"); fs::create_dir_all(&nested).unwrap(); fs::write(nested.join("go.mod"), "module test").unwrap();
        assert!(scan(vec![root.path().to_string_lossy().into_owned()], 1, &AtomicBool::new(false), &HashSet::new()).unwrap().projects.is_empty());
        assert!(scan(vec![root.path().to_string_lossy().into_owned()], 2, &AtomicBool::new(true), &HashSet::new()).unwrap_err().contains("取消"));
    }
    #[test]
    fn scanner_does_not_cancel_an_unrelated_request() {
        let scanner = Scanner::default(); let token = scanner.begin("first").unwrap();
        assert!(scanner.begin("second").is_err()); scanner.cancel("second").unwrap(); assert!(!token.load(Ordering::Relaxed));
        scanner.cancel("first").unwrap(); assert!(token.load(Ordering::Relaxed)); scanner.finish("first"); assert!(scanner.begin("second").is_ok());
    }
    #[test]
    fn dropping_scan_lease_releases_active_request() {
        let scanner = Scanner::default();
        { let _lease = scanner.lease("first").unwrap(); assert!(scanner.begin("second").is_err()); }
        assert!(scanner.begin("second").is_ok());
    }
}
