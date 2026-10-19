use serde_json::{json, Value};
use std::{collections::HashSet, fs, io::Read, path::Path};
const PER_FILE: u64 = 32 * 1024;
const TOTAL: usize = 64 * 1024;
pub fn collect(root: &Path) -> Result<Value, String> {
    if !root.is_dir() {
        return Err("项目目录不存在或无法访问，未读取资料".into());
    }
    let mut budget = TOTAL;
    let mut files = vec![];
    let mut warnings = vec![];
    let mut read_files = HashSet::new();
    for name in [
        "README.md",
        "readme.md",
        "README.txt",
        "package.json",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "requirements.txt",
        "pyproject.toml",
        "Cargo.toml",
        "go.mod",
    ] {
        let path = root.join(name);
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                warnings.push(format!("{name}：无法检查文件"));
                continue;
            }
        };
        if !meta.is_file() {
            warnings.push(format!("{name}：不是普通文件，已跳过"));
            continue;
        }
        // Windows 默认文件系统中两种 README 大小写指向同一文件，
        // 规范化后只计一次预算；其他平台保留不同文件的大小写。
        let canonical = match fs::canonicalize(&path) {
            Ok(path) => path,
            Err(_) => {
                warnings.push(format!("{name}：无法确定文件路径，已跳过"));
                continue;
            }
        };
        if !read_files.insert(crate::services::paths::path_key(
            &canonical.to_string_lossy(),
        )) {
            continue;
        }
        if budget == 0 {
            warnings.push(format!("{name}：总资料预算用尽，未读取"));
            continue;
        }
        let limit = PER_FILE.min(budget as u64);
        let mut bytes = vec![];
        let read = fs::File::open(&path).and_then(|file| file.take(limit).read_to_end(&mut bytes));
        if read.is_err() {
            warnings.push(format!("{name}：读取失败，内容未知"));
            continue;
        }
        budget -= bytes.len();
        let truncated = meta.len() > bytes.len() as u64;
        if truncated {
            warnings.push(format!("{name}：资料已截断"));
        }
        files.push(
            json!({"name":name,"content":String::from_utf8_lossy(&bytes),"truncated":truncated}),
        );
    }
    let mut names = vec![];
    for entry in fs::read_dir(root)
        .map_err(|_| "无法读取项目目录")?
        .take(300)
    {
        let Ok(entry) = entry else {
            warnings.push("部分子目录无法检查".into());
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.')
            || ["node_modules", "target", "dist", "build", "vendor", "venv"]
                .contains(&name.as_str())
        {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            names.push(name.chars().take(120).collect::<String>());
        }
        if names.len() >= 60 {
            warnings.push("子目录列表已截断".into());
            break;
        }
    }
    names.sort();
    if files.is_empty() {
        warnings.push("未发现可读取的白名单文档或依赖清单，资料不足".into());
    }
    Ok(
        json!({"folderName":root.file_name().map(|name|name.to_string_lossy().into_owned()).unwrap_or_default(),"directories":names,"files":files,"warnings":warnings}),
    )
}
