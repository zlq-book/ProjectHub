use super::{error, Database, COLUMNS};
use crate::models::Project;
use rusqlite::{params, TransactionBehavior};
use std::collections::HashSet;

// Unicode White_Space 与 BOM 的并集，与前端 trimTagName 保持一致。
fn trim_tag_name(value: &str) -> &str { value.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}') }

impl Database {
    pub fn change_tag(&self, ids: Vec<String>, source: String, target: Option<String>) -> Result<Vec<Project>, String> {
        let mut seen = HashSet::new();
        let ids: Vec<_> = ids.into_iter().filter(|id| seen.insert(id.clone())).collect();
        if ids.is_empty() || ids.len() > 500 { return Err("请选择 1–500 个项目".into()); }
        if trim_tag_name(&source).is_empty() || source.chars().count() > 80 { return Err("源标签无效".into()); }
        let target = target.map(|value| trim_tag_name(&value).to_string());
        if target.as_ref().is_some_and(|value| value.is_empty() || value.chars().count() > 80) {
            return Err("标签名称须为 1–80 个字符".into());
        }
        if target.as_ref() == Some(&source) { return Err("新标签名称与原名称相同".into()); }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut result = Vec::new();
        for id in ids {
            let mut project = transaction.query_row(&format!("SELECT {} FROM projects WHERE id=?1", COLUMNS), [&id], Database::row)
                .map_err(|e| if matches!(e, rusqlite::Error::QueryReturnedNoRows) { "部分项目不存在，请刷新后重试".into() } else { error(e) })?;
            if !project.input.tags.contains(&source) { return Err("部分项目的标签已变化，请返回标签列表后重试".into()); }
            let mut seen = HashSet::new();
            project.input.tags = project.input.tags.iter().filter_map(|tag| {
                let value = if tag == &source { target.as_ref()? } else { tag };
                seen.insert(value.clone()).then(|| value.clone())
            }).collect();
            project.updated_at = now.clone();
            let tags = serde_json::to_string(&project.input.tags).map_err(|e| e.to_string())?;
            transaction.execute("UPDATE projects SET tags_json=?2,updated_at=?3 WHERE id=?1", params![id,tags,now]).map_err(error)?;
            result.push(project);
        }
        transaction.commit().map_err(error)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProjectInput;
    fn input(path: &std::path::Path) -> ProjectInput {
        ProjectInput { name: "标签测试".into(), path: path.to_string_lossy().into_owned(), description: "".into(), category: "other".into(),
            tags: vec!["旧标签".into(), "已有标签".into()], tech_stack: vec!["旧标签".into()], favorite: false, notes: "".into(), default_ide: "system".into() }
    }
    #[test]
    fn merge_and_remove_preserve_tech_stack_and_source_files() {
        let data = tempfile::tempdir().unwrap(); let folder = tempfile::tempdir().unwrap();
        let db = Database::new(data.path().join("test.sqlite")).unwrap();
        let p = db.create(input(folder.path())).unwrap();
        let file = folder.path().join("source.txt"); std::fs::write(&file, "keep").unwrap();
        let result = db.change_tag(vec![p.id.clone(), p.id.clone()], "旧标签".into(), Some(" 已有标签 ".into())).unwrap();
        assert_eq!(result.len(), 1); assert_eq!(result[0].input.tags, vec!["已有标签"]);
        assert_eq!(result[0].input.tech_stack, vec!["旧标签"]);
        db.change_tag(vec![p.id.clone()], "已有标签".into(), None).unwrap();
        assert!(db.get(&p.id).unwrap().input.tags.is_empty());
        assert_eq!(std::fs::read_to_string(file).unwrap(), "keep");
    }
    #[test]
    fn stale_ids_and_tags_roll_back_entire_change() {
        let data = tempfile::tempdir().unwrap(); let a = tempfile::tempdir().unwrap(); let b = tempfile::tempdir().unwrap();
        let db = Database::new(data.path().join("test.sqlite")).unwrap();
        let p = db.create(input(a.path())).unwrap(); let mut other = input(b.path()); other.tags.clear();
        let q = db.create(other).unwrap();
        for ids in [vec![p.id.clone(), q.id], vec![p.id.clone(), "missing".into()]] {
            assert!(db.change_tag(ids, "旧标签".into(), None).is_err());
            let unchanged = db.get(&p.id).unwrap();
            assert_eq!(unchanged.input.tags, p.input.tags); assert_eq!(unchanged.updated_at, p.updated_at);
        }
        assert!(db.change_tag(vec![p.id], "旧标签".into(), Some(" ".into())).is_err());
    }
    #[test]
    fn unicode_whitespace_matches_frontend_contract() {
        assert_eq!(trim_tag_name("\u{0085}\u{feff}新标签\u{0085}"), "新标签");
        assert_eq!(trim_tag_name("\u{0085}\u{feff}"), "");
        let data = tempfile::tempdir().unwrap(); let folder = tempfile::tempdir().unwrap();
        let db = Database::new(data.path().join("test.sqlite")).unwrap(); let p = db.create(input(folder.path())).unwrap();
        assert!(db.change_tag(vec![p.id.clone()], "旧标签".into(), Some("\u{0085}\u{feff}".into())).is_err());
        let result = db.change_tag(vec![p.id], "旧标签".into(), Some("\u{0085}\u{feff}新标签\u{0085}".into())).unwrap();
        assert_eq!(result[0].input.tags, vec!["新标签", "已有标签"]);
    }
}
