use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub name: String,
    pub path: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub tech_stack: Vec<String>,
    pub favorite: bool,
    pub notes: String,
    pub default_ide: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    #[serde(flatten)]
    pub input: ProjectInput,
    pub last_opened_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub path: String,
    pub name: String,
    pub description: String,
    pub tech_stack: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCandidate {
    #[serde(flatten)]
    pub detection: Detection,
    pub existing: bool,
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub projects: Vec<ScanCandidate>,
    pub warnings: Vec<String>,
    pub truncated: bool,
    pub scanned_directories: usize,
}
#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub projects: Vec<Project>,
    pub skipped: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub default_ide: String,
    pub default_category: String,
    pub theme: String,
    pub view: String,
    pub ide_paths: HashMap<String, String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self { default_ide: "code".into(), default_category: "other".into(), theme: "light".into(), view: "list".into(), ide_paths: HashMap::new() }
    }
}
pub fn valid_ide(value: &str) -> bool { ["system", "code", "idea", "cursor", "custom"].contains(&value) }
pub fn valid_category(value: &str) -> bool { ["backend", "frontend", "ai", "tool", "learning", "other"].contains(&value) }
pub fn validate_input(input: &mut ProjectInput) -> Result<(), String> {
    input.name = input.name.trim().into();
    input.path = input.path.trim().into();
    if input.name.is_empty() || input.name.chars().count() > 120 { return Err("项目名称须为 1–120 个字符".into()); }
    if input.description.chars().count() > 4000 || input.notes.chars().count() > 8000 { return Err("简介或备注过长".into()); }
    if !valid_category(&input.category) || !valid_ide(&input.default_ide) { return Err("分类或打开工具无效".into()); }
    for tags in [&mut input.tags, &mut input.tech_stack] {
        if tags.len() > 50 || tags.iter().any(|t| t.chars().count() > 80) { return Err("最多支持 50 个标签，每个标签最多 80 个字符".into()); }
        *tags = tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        let mut seen = std::collections::HashSet::new();
        tags.retain(|t| seen.insert(t.clone()));
    }
    Ok(())
}
