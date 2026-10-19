use crate::models::Detection;
use super::paths;
use std::{fs, io::Read, path::Path};

const MAX_BYTES: u64 = 128 * 1024;
fn read_limited(root: &Path, filename: &str) -> Option<String> {
    let path = root.join(filename);
    // 不跟随根目录中特征文件的符号链接，也不递归读取子目录。
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES { return None; }
    let mut data = String::new();
    fs::File::open(path).ok()?.take(MAX_BYTES).read_to_string(&mut data).ok()?;
    Some(data)
}
fn add(stack: &mut Vec<String>, value: &str) { if !stack.iter().any(|v| v == value) { stack.push(value.into()); } }
pub fn detect(path: &str) -> Result<Detection, String> {
    let (root, display) = paths::validate(path)?;
    let mut name = root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "本地项目".into());
    let mut stack = Vec::new();
    if let Some(data) = read_limited(&root, "pom.xml") {
        add(&mut stack, "Java"); add(&mut stack, "Maven");
        if let Ok(doc) = roxmltree::Document::parse(&data) {
            let project = doc.root_element();
            if let Some(value) = project.children().find(|n| n.has_tag_name("name")).and_then(|n| n.text()).filter(|v| !v.contains("${") && !v.trim().is_empty()) { name = value.trim().into(); }
        }
        if data.contains("spring-boot") { add(&mut stack, "Spring Boot"); }
        if data.contains("spring-ai-alibaba") { add(&mut stack, "Spring AI Alibaba"); }
        if data.contains("mysql") { add(&mut stack, "MySQL"); }
        if data.contains("mybatis") { add(&mut stack, "MyBatis"); }
    }
    for filename in ["build.gradle", "build.gradle.kts"] {
        if let Some(data) = read_limited(&root, filename) {
            add(&mut stack, "Gradle"); add(&mut stack, if filename.ends_with("kts") || data.contains("kotlin") { "Kotlin" } else { "Java" });
            if data.contains("org.springframework.boot") { add(&mut stack, "Spring Boot"); }
        }
    }
    if let Some(data) = read_limited(&root, "package.json") {
        add(&mut stack, "Node.js");
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(value) = json["name"].as_str().filter(|s| !s.is_empty()) { name = value.into(); }
            for group in ["dependencies", "devDependencies"] {
                if let Some(deps) = json[group].as_object() {
                    if let Some(version) = deps.get("vue").and_then(|v| v.as_str()) {
                        let major = version.trim_start_matches(|c: char| !c.is_ascii_digit()).chars().next();
                        add(&mut stack, match major { Some('3') => "Vue3", Some('2') => "Vue2", _ => "Vue" });
                    }
                    for (key, value) in [("react", "React"), ("vite", "Vite"), ("next", "Next.js"), ("typescript", "TypeScript"), ("element-plus", "Element Plus"), ("@tauri-apps/api", "Tauri")] {
                        if deps.contains_key(key) { add(&mut stack, value); }
                    }
                }
            }
        }
    }
    for filename in ["requirements.txt", "pyproject.toml"] {
        if let Some(data) = read_limited(&root, filename) {
            add(&mut stack, "Python"); let lower = data.to_lowercase();
            for (key, value) in [("fastapi", "FastAPI"), ("django", "Django"), ("flask", "Flask"), ("torch", "PyTorch"), ("langchain", "LangChain")] { if lower.contains(key) { add(&mut stack, value); } }
        }
    }
    if read_limited(&root, "Cargo.toml").is_some() { add(&mut stack, "Rust"); }
    if read_limited(&root, "go.mod").is_some() { add(&mut stack, "Go"); }
    if let Ok(entries) = fs::read_dir(&root) {
        if entries.take(300).filter_map(Result::ok).any(|e| e.path().extension().is_some_and(|ext| ext == "sln" || ext == "csproj")) { add(&mut stack, "C#"); add(&mut stack, ".NET"); }
    }
    let mut description = String::new();
    if let Some(readme) = read_limited(&root, "README.md").or_else(|| read_limited(&root, "readme.md")) {
        let mut fenced = false; let mut lines = Vec::new();
        for line in readme.lines().take(200) {
            let line = line.trim();
            if line.starts_with("```") || line.starts_with("~~~") { fenced = !fenced; continue; }
            if fenced || line.is_empty() || line.starts_with('#') || line.starts_with('!') || line.starts_with('<') || line.starts_with('|') { continue; }
            lines.push(line.trim_start_matches(['-', '*', '>']).trim().to_string());
            if lines.join(" ").chars().count() >= 300 { break; }
        }
        description = lines.join(" ").chars().take(300).collect();
    }
    name = name.chars().take(120).collect();
    Ok(Detection { path: display, name, description, tech_stack: stack })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_directory_is_valid_without_readme() {
        let dir = tempfile::tempdir().unwrap();
        let detected = detect(dir.path().to_str().unwrap()).unwrap();
        assert!(detected.tech_stack.is_empty()); assert!(detected.description.is_empty());
    }
    #[test]
    fn detects_multiple_stacks_without_executing_scripts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), r#"{"name":"test-ui","dependencies":{"react":"19","vite":"7"},"scripts":{"postinstall":"dangerous command"}}"#).unwrap();
        fs::write(dir.path().join("requirements.txt"), "fastapi\n").unwrap();
        fs::write(dir.path().join("README.md"), "# Test\n\n测试说明\n```sh\ndelete everything\n```\n").unwrap();
        let detected = detect(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(detected.name, "test-ui"); assert_eq!(detected.description, "测试说明");
        for value in ["React", "Vite", "Python", "FastAPI"] { assert!(detected.tech_stack.contains(&value.to_string())); }
    }
    #[test]
    fn ignores_oversized_feature_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "a".repeat(MAX_BYTES as usize + 1)).unwrap();
        assert!(detect(dir.path().to_str().unwrap()).unwrap().description.is_empty());
    }
}
