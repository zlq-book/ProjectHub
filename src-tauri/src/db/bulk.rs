use super::{error, Database, COLUMNS};
use crate::{
    models::{valid_category, validate_input, ImportResult, Project, ProjectInput},
    services::paths,
};
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::HashSet;

fn unique_ids(ids: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let ids: Vec<_> = ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect();
    if ids.is_empty() || ids.len() > 500 {
        return Err("请选择 1–500 个项目".into());
    }
    Ok(ids)
}
fn insert(connection: &Connection, project: &Project) -> Result<(), String> {
    let input = &project.input;
    connection.execute("INSERT INTO projects (id,name,path,path_key,description,category,tags_json,tech_stack_json,favorite,notes,default_ide,last_opened_at,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)", params![project.id,input.name,input.path,paths::path_key(&input.path),input.description,input.category,serde_json::to_string(&input.tags).map_err(|e| e.to_string())?,serde_json::to_string(&input.tech_stack).map_err(|e| e.to_string())?,input.favorite,input.notes,input.default_ide,project.last_opened_at,project.created_at,project.updated_at]).map_err(error)?;
    Ok(())
}
impl Database {
    pub fn known_paths(&self) -> Result<HashSet<String>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT path_key FROM projects")
            .map_err(error)?;
        let result = statement
            .query_map([], |row| row.get(0))
            .map_err(error)?
            .collect::<rusqlite::Result<HashSet<String>>>()
            .map_err(error);
        result
    }
    pub fn batch_update(
        &self,
        ids: Vec<String>,
        favorite: Option<bool>,
        category: Option<String>,
    ) -> Result<Vec<Project>, String> {
        let ids = unique_ids(ids)?;
        if favorite.is_none() && category.is_none() {
            return Err("没有要更新的资料".into());
        }
        if category.as_ref().is_some_and(|c| !valid_category(c)) {
            return Err("项目分类无效".into());
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut result = Vec::new();
        for id in ids {
            let mut project = transaction
                .query_row(
                    &format!("SELECT {} FROM projects WHERE id=?1", COLUMNS),
                    [&id],
                    Database::row,
                )
                .map_err(|e| {
                    if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                        "部分项目不存在，请刷新后重试".to_string()
                    } else {
                        error(e)
                    }
                })?;
            if let Some(favorite) = favorite {
                project.input.favorite = favorite;
            }
            if let Some(category) = &category {
                project.input.category = category.clone();
            }
            project.updated_at = now.clone();
            transaction
                .execute(
                    "UPDATE projects SET favorite=?2,category=?3,updated_at=?4 WHERE id=?1",
                    params![id, project.input.favorite, project.input.category, now],
                )
                .map_err(error)?;
            result.push(project);
        }
        transaction.commit().map_err(error)?;
        Ok(result)
    }
    pub fn batch_remove(&self, ids: Vec<String>) -> Result<(), String> {
        let ids = unique_ids(ids)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        for id in ids {
            super::development::archive_project(&transaction, &id)?;
            if transaction
                .execute("DELETE FROM projects WHERE id=?1", [id])
                .map_err(error)?
                != 1
            {
                return Err("部分项目不存在，请刷新后重试".into());
            }
        }
        transaction.commit().map_err(error)?;
        Ok(())
    }
    pub fn import_projects(&self, inputs: Vec<ProjectInput>) -> Result<ImportResult, String> {
        if inputs.is_empty() || inputs.len() > 500 {
            return Err("请选择 1–500 个候选项目".into());
        }
        let mut prepared = Vec::new();
        let mut skipped = Vec::new();
        for mut input in inputs {
            validate_input(&mut input)?;
            match paths::validate(&input.path) {
                Ok((_, display)) => {
                    input.path = display;
                    prepared.push(input);
                }
                Err(message) => skipped.push(format!("{}：{}", input.path, message)),
            }
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        let mut result = Vec::new();
        let now = chrono::Utc::now().to_rfc3339();
        for input in prepared {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE path_key=?1)",
                    [paths::path_key(&input.path)],
                    |r| r.get(0),
                )
                .map_err(error)?;
            if exists {
                skipped.push(format!("{}：已录入", input.path));
                continue;
            }
            let project = Project {
                id: uuid::Uuid::new_v4().to_string(),
                input,
                last_opened_at: None,
                created_at: now.clone(),
                updated_at: now.clone(),
                available: true,
            };
            insert(&transaction, &project)?;
            result.push(project);
        }
        transaction.commit().map_err(error)?;
        Ok(ImportResult {
            projects: result,
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(path: &std::path::Path) -> ProjectInput {
        ProjectInput {
            name: "批量测试".into(),
            path: path.to_string_lossy().into_owned(),
            description: "".into(),
            category: "other".into(),
            tags: vec![],
            tech_stack: vec![],
            favorite: false,
            notes: "".into(),
            default_ide: "system".into(),
        }
    }
    #[test]
    fn duplicate_imports_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("test.sqlite")).unwrap();
        let result = db
            .import_projects(vec![
                input(project.path()),
                input(&project.path().join(".")),
            ])
            .unwrap();
        assert_eq!(result.projects.len(), 1);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(db.list().unwrap().len(), 1);
    }
    #[test]
    fn missing_id_rolls_back_batch_update_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("test.sqlite")).unwrap();
        let p = db.create(input(project.path())).unwrap();
        assert!(db
            .batch_update(vec![p.id.clone(), "missing".into()], Some(true), None)
            .is_err());
        assert!(!db.get(&p.id).unwrap().input.favorite);
        assert!(db
            .batch_remove(vec![p.id.clone(), "missing".into()])
            .is_err());
        assert!(db.get(&p.id).is_ok());
    }
    #[test]
    fn invalid_metadata_rolls_back_import_and_remove_preserves_files() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("test.sqlite")).unwrap();
        let mut bad = input(project.path());
        bad.name.clear();
        assert!(db
            .import_projects(vec![input(project.path()), bad])
            .is_err());
        assert!(db.list().unwrap().is_empty());
        let file = project.path().join("source.txt");
        std::fs::write(&file, "keep").unwrap();
        let p = db.create(input(project.path())).unwrap();
        db.batch_remove(vec![p.id]).unwrap();
        assert_eq!(std::fs::read_to_string(file).unwrap(), "keep");
    }
}
