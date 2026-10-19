use crate::{
    models::{validate_input, Project, ProjectInput, Settings},
    services::paths,
};
use chrono::Utc;
use rusqlite::{params, Connection, OpenFlags};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
pub(crate) mod assistant;
mod bulk;
pub mod development;
pub mod weekly_report;
mod tags;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS projects (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL,
 path_key TEXT NOT NULL UNIQUE, description TEXT NOT NULL DEFAULT '',
 category TEXT NOT NULL DEFAULT 'other', tags_json TEXT NOT NULL DEFAULT '[]',
 tech_stack_json TEXT NOT NULL DEFAULT '[]', favorite INTEGER NOT NULL DEFAULT 0,
 notes TEXT NOT NULL DEFAULT '', default_ide TEXT NOT NULL DEFAULT 'system',
 last_opened_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);
CREATE INDEX IF NOT EXISTS idx_projects_category ON projects(category);
CREATE INDEX IF NOT EXISTS idx_projects_last_opened ON projects(last_opened_at);
CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);
";
const COLUMNS: &str = "id,name,path,description,category,tags_json,tech_stack_json,favorite,notes,default_ide,last_opened_at,created_at,updated_at";
fn error(e: rusqlite::Error) -> String {
    if let rusqlite::Error::SqliteFailure(ref code, _) = e {
        if code.extended_code == 2067 {
            return "该路径已录入，请在项目库中查看已有项目".into();
        }
    }
    format!("项目数据库操作失败：{}", e)
}
#[derive(Clone)]
pub struct Database {
    pub path: PathBuf,
    epoch: std::sync::Arc<std::sync::atomic::AtomicU64>,
    request_epoch: Option<u64>,
}
impl Database {
    pub fn for_development_request(&self) -> Self {
        let mut snapshot = self.clone();
        snapshot.request_epoch = Some(self.development_epoch());
        snapshot
    }
    pub fn development_request_epoch(&self) -> u64 {
        self.request_epoch
            .unwrap_or_else(|| self.development_epoch())
    }
    pub fn development_epoch(&self) -> u64 {
        self.epoch.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn check_development_epoch(&self, expected: u64) -> Result<(), String> {
        if self.development_epoch() != expected {
            Err("项目库已恢复，旧开发记录请求已失效，请重新载入".into())
        } else {
            Ok(())
        }
    }
    pub fn new(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Self {
            path,
            epoch: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            request_epoch: None,
        };
        let mut connection = db.connect()?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(error)?;
        if version > 3 {
            return Err("数据库来自更新版本的软件，请先升级 ProjectHub".into());
        }
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        tx.execute_batch(SCHEMA).map_err(error)?;
        tx.execute_batch(development::SCHEMA).map_err(error)?;
        tx.execute_batch(weekly_report::SCHEMA).map_err(error)?;
        tx.execute_batch("PRAGMA user_version=3;").map_err(error)?;
        let invalid: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
                [],
                |r| r.get(0),
            )
            .map_err(error)?;
        if invalid {
            return Err("数据库关系校验失败，迁移未保存".into());
        }
        tx.commit().map_err(error)?;
        Ok(db)
    }
    fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path).map_err(error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(error)?;
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")
            .map_err(error)?;
        Ok(connection)
    }
    fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
        let path: String = row.get(2)?;
        let decode = |index| -> rusqlite::Result<Vec<String>> {
            let value: String = row.get(index)?;
            serde_json::from_str(&value).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        };
        Ok(Project {
            id: row.get(0)?,
            available: paths::available(&path),
            input: ProjectInput {
                name: row.get(1)?,
                path,
                description: row.get(3)?,
                category: row.get(4)?,
                tags: decode(5)?,
                tech_stack: decode(6)?,
                favorite: row.get::<_, i64>(7)? != 0,
                notes: row.get(8)?,
                default_ide: row.get(9)?,
            },
            last_opened_at: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    }
    pub fn list(&self) -> Result<Vec<Project>, String> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {} FROM projects ORDER BY updated_at DESC",
                COLUMNS
            ))
            .map_err(error)?;
        let result = statement
            .query_map([], Self::row)
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error);
        result
    }
    pub fn get(&self, id: &str) -> Result<Project, String> {
        self.connect()?
            .query_row(
                &format!("SELECT {} FROM projects WHERE id=?1", COLUMNS),
                [id],
                Self::row,
            )
            .map_err(|e| {
                if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                    "项目不存在或已被移除".into()
                } else {
                    error(e)
                }
            })
    }
    pub fn validate_path(&self, path: &str, except: Option<&str>) -> Result<String, String> {
        let (_, display) = paths::validate(path)?;
        let count: i64 = self
            .connect()?
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE path_key=?1 AND id<>?2",
                params![paths::path_key(&display), except.unwrap_or("")],
                |r| r.get(0),
            )
            .map_err(error)?;
        if count > 0 {
            return Err("该路径已录入，请在项目库中查看已有项目".into());
        }
        Ok(display)
    }
    pub fn create(&self, mut input: ProjectInput) -> Result<Project, String> {
        validate_input(&mut input)?;
        input.path = self.validate_path(&input.path, None)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.connect()?.execute("INSERT INTO projects (id,name,path,path_key,description,category,tags_json,tech_stack_json,favorite,notes,default_ide,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)", params![id,input.name,input.path,paths::path_key(&input.path),input.description,input.category,serde_json::to_string(&input.tags).unwrap(),serde_json::to_string(&input.tech_stack).unwrap(),input.favorite,input.notes,input.default_ide,now]).map_err(error)?;
        self.get(&id)
    }
    pub fn update(&self, id: &str, mut input: ProjectInput) -> Result<Project, String> {
        validate_input(&mut input)?;
        let original = self.get(id)?;
        // 磁盘离线时仍允许编辑元数据；修改路径必须重新校验和去重。
        input.path = if paths::path_key(&original.input.path) == paths::path_key(&input.path) {
            original.input.path
        } else {
            self.validate_path(&input.path, Some(id))?
        };
        let now = Utc::now().to_rfc3339();
        let changed = self.connect()?.execute("UPDATE projects SET name=?2,path=?3,path_key=?4,description=?5,category=?6,tags_json=?7,tech_stack_json=?8,favorite=?9,notes=?10,default_ide=?11,updated_at=?12 WHERE id=?1", params![id,input.name,input.path,paths::path_key(&input.path),input.description,input.category,serde_json::to_string(&input.tags).unwrap(),serde_json::to_string(&input.tech_stack).unwrap(),input.favorite,input.notes,input.default_ide,now]).map_err(error)?;
        if changed == 0 {
            return Err("项目已被移除".into());
        }
        self.get(id)
    }
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        development::archive_project(&tx, id)?;
        tx.execute("DELETE FROM projects WHERE id=?1", [id])
            .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(())
    }
    pub fn mark_opened(&self, id: &str) -> Result<(), String> {
        self.connect()?
            .execute(
                "UPDATE projects SET last_opened_at=?2 WHERE id=?1",
                params![id, Utc::now().to_rfc3339()],
            )
            .map_err(error)?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings, String> {
        let connection = self.connect()?;
        match connection.query_row::<String, _, _>(
            "SELECT json FROM settings WHERE id=1",
            [],
            |r| r.get(0),
        ) {
            Ok(value) => serde_json::from_str(&value).map_err(|e| format!("设置数据损坏：{}", e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Settings::default()),
            Err(e) => Err(error(e)),
        }
    }
    pub fn save_settings(&self, settings: Settings) -> Result<(), String> {
        if !crate::models::valid_ide(&settings.default_ide)
            || !crate::models::valid_category(&settings.default_category)
            || !["light", "dark", "system"].contains(&settings.theme.as_str())
            || !["grid", "list"].contains(&settings.view.as_str())
        {
            return Err("设置内容无效".into());
        }
        for (key, path) in &settings.ide_paths {
            if !["code", "idea", "cursor", "custom"].contains(&key.as_str()) {
                return Err("不支持的打开工具".into());
            }
            if !path.trim().is_empty() {
                crate::services::launcher::validate_executable(Path::new(path.trim()))?;
            }
        }
        self.connect()?.execute("INSERT INTO settings(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json", [serde_json::to_string(&settings).map_err(|e| e.to_string())?]).map_err(error)?;
        Ok(())
    }
    pub fn export(&self, target: &Path) -> Result<(), String> {
        if target.exists()
            && std::fs::canonicalize(target).ok() == std::fs::canonicalize(&self.path).ok()
        {
            return Err("备份不能覆盖正在使用的数据库".into());
        }
        self.connect()?
            .backup(rusqlite::DatabaseName::Main, target, None)
            .map_err(error)
    }
    pub fn import(&self, source: &Path) -> Result<String, String> {
        if !source.is_file() {
            return Err("所选备份不存在".into());
        }
        if std::fs::metadata(source).map_err(|e| e.to_string())?.len() > 100 * 1024 * 1024 {
            return Err("备份超过 100 MB，无法导入".into());
        }
        if std::fs::canonicalize(source).ok() == std::fs::canonicalize(&self.path).ok() {
            return Err("不能导入正在使用的数据库".into());
        }
        let mut backup =
            Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
        backup
            .execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")
            .map_err(error)?;
        let snapshot = backup.transaction().map_err(error)?;
        let integrity: String = snapshot
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(error)?;
        let version: i64 = snapshot
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(error)?;
        if integrity != "ok" || ![1, 2, 3].contains(&version) {
            return Err("备份损坏或版本不兼容".into());
        }
        let mut statement = snapshot
            .prepare(&format!("SELECT {} FROM projects", COLUMNS))
            .map_err(error)?;
        let mut projects = statement
            .query_map([], Self::row)
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)?;
        let mut keys = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for p in &mut projects {
            validate_input(&mut p.input)?;
            if !Path::new(&p.input.path).is_absolute() || p.input.path.contains('\0') {
                return Err("备份中含有无效项目路径".into());
            }
            if let Ok((_, canonical)) = paths::validate(&p.input.path) {
                p.input.path = canonical;
            }
            if !keys.insert(paths::path_key(&p.input.path)) || !ids.insert(p.id.clone()) {
                return Err("备份中含有重复项目路径或 ID".into());
            }
            if p.id.is_empty()
                || chrono::DateTime::parse_from_rfc3339(&p.created_at).is_err()
                || chrono::DateTime::parse_from_rfc3339(&p.updated_at).is_err()
                || p.last_opened_at
                    .as_ref()
                    .is_some_and(|v| chrono::DateTime::parse_from_rfc3339(v).is_err())
            {
                return Err("备份中含有无效项目标识或时间".into());
            }
        }
        let prefs = match snapshot.query_row::<String, _, _>(
            "SELECT json FROM settings WHERE id=1",
            [],
            |r| r.get(0),
        ) {
            Ok(json) => serde_json::from_str::<Settings>(&json)
                .map_err(|e| format!("备份设置无效：{}", e))?,
            Err(rusqlite::Error::QueryReturnedNoRows) => Settings::default(),
            Err(e) => return Err(error(e)),
        };
        if !crate::models::valid_ide(&prefs.default_ide)
            || !crate::models::valid_category(&prefs.default_category)
            || !["light", "dark", "system"].contains(&prefs.theme.as_str())
            || !["grid", "list"].contains(&prefs.view.as_str())
        {
            return Err("备份中含有无效设置".into());
        }
        for (ide, path) in &prefs.ide_paths {
            let path = Path::new(path.trim());
            if !["code", "idea", "cursor", "custom"].contains(&ide.as_str())
                || (!path.as_os_str().is_empty()
                    && (!path.is_absolute()
                        || !["exe", "com"].contains(
                            &path
                                .extension()
                                .and_then(|v| v.to_str())
                                .unwrap_or("")
                                .to_ascii_lowercase()
                                .as_str(),
                        )))
            {
                return Err("备份中含有无效可执行程序路径".into());
            }
        }
        let weekly_reports = if version == 3 { weekly_report::read_backup(&snapshot)? } else { Default::default() };
        let development = if version >= 2 {
            development::read_backup(&snapshot, &ids)?
        } else {
            vec![]
        };
        drop(statement);
        drop(snapshot);
        drop(backup);
        let automatic = self.path.with_file_name(format!(
            "projecthub-before-import-{}-{}.sqlite",
            Utc::now().format("%Y%m%d-%H%M%S"),
            uuid::Uuid::new_v4()
        ));
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        self.export(&automatic)?;
        // 只导入同一个只读快照里已校验的数据，用应用自己的 schema 重建唯一约束。
        // 不复制外来表结构、触发器、索引或未经验证的 path_key。
        transaction.execute_batch("DROP TABLE weekly_report_sources; DROP TABLE weekly_report_versions; DROP TABLE weekly_reports; DROP TABLE weekly_report_preferences; DROP TABLE development_commit_links; DROP TABLE development_logs; DROP TABLE development_tasks; DROP TABLE development_spaces; DROP TABLE projects; DROP TABLE settings;").map_err(error)?;
        transaction.execute_batch(SCHEMA).map_err(error)?;
        transaction
            .execute_batch(development::SCHEMA)
            .map_err(error)?;
        transaction
            .execute_batch("PRAGMA user_version=3;")
            .map_err(error)?;
        for p in projects {
            let input = p.input;
            transaction.execute("INSERT INTO projects (id,name,path,path_key,description,category,tags_json,tech_stack_json,favorite,notes,default_ide,last_opened_at,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)", params![p.id,input.name,input.path,paths::path_key(&input.path),input.description,input.category,serde_json::to_string(&input.tags).unwrap(),serde_json::to_string(&input.tech_stack).unwrap(),input.favorite,input.notes,input.default_ide,p.last_opened_at,p.created_at,p.updated_at]).map_err(error)?;
        }
        transaction
            .execute(
                "INSERT INTO settings(id,json) VALUES(1,?1)",
                [serde_json::to_string(&prefs).map_err(|e| e.to_string())?],
            )
            .map_err(error)?;
        transaction.execute_batch(weekly_report::SCHEMA).map_err(error)?;
        weekly_report::write_backup(&transaction, &weekly_reports)?;
        development::write_backup(&transaction, &development)?;
        let invalid: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
                [],
                |r| r.get(0),
            )
            .map_err(error)?;
        if invalid {
            return Err("备份关联关系无效，当前数据未改变".into());
        }
        // 在释放数据库写事务前使导入前的异步记录保存失效。
        self.epoch.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        transaction.commit().map_err(error)?;
        Ok(format!(
            "导入成功。旧版备份不含周报时，周报历史为空。{}原数据库已备份到 {}",
            if version == 1 {
                "旧版备份不含日志和任务，恢复后开发记录为空。"
            } else {
                ""
            },
            automatic.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(path: &Path) -> ProjectInput {
        ProjectInput {
            name: "测试项目".into(),
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
    fn duplicates_are_rejected_and_restart_preserves_data() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("projects.sqlite")).unwrap();
        let saved = db.create(input(project.path())).unwrap();
        assert!(db
            .create(input(&project.path().join(".")))
            .unwrap_err()
            .contains("已录入"));
        let reopened = Database::new(db.path.clone()).unwrap();
        assert_eq!(reopened.list().unwrap()[0].id, saved.id);
    }
    #[test]
    fn remove_never_touches_source_files() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let file = project.path().join("keep.txt");
        std::fs::write(&file, "keep").unwrap();
        let db = Database::new(dir.path().join("projects.sqlite")).unwrap();
        let saved = db.create(input(project.path())).unwrap();
        db.remove(&saved.id).unwrap();
        assert!(db.list().unwrap().is_empty());
        assert_eq!(std::fs::read_to_string(file).unwrap(), "keep");
    }
    #[test]
    fn offline_project_retains_index_and_can_edit_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("projects.sqlite")).unwrap();
        let mut saved = db.create(input(project.path())).unwrap();
        project.close().unwrap();
        saved.input.favorite = true;
        let updated = db.update(&saved.id, saved.input).unwrap();
        assert!(!updated.available);
        assert!(updated.input.favorite);
    }
    #[test]
    fn sqlite_backup_roundtrip_and_invalid_import_preserve_current_data() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("projects.sqlite")).unwrap();
        let saved = db.create(input(project.path())).unwrap();
        let file = dir.path().join("backup.sqlite");
        db.export(&file).unwrap();
        db.remove(&saved.id).unwrap();
        db.import(&file).unwrap();
        assert_eq!(db.list().unwrap()[0].id, saved.id);
        let bad = dir.path().join("bad.sqlite");
        std::fs::write(&bad, "not sqlite").unwrap();
        assert!(db.import(&bad).is_err());
        assert_eq!(db.list().unwrap().len(), 1);
    }
    #[test]
    fn import_recomputes_path_keys_and_rejects_duplicate_paths() {
        let dir = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("projects.sqlite")).unwrap();
        let saved = db.create(input(project.path())).unwrap();
        let file = dir.path().join("backup.sqlite");
        db.export(&file).unwrap();
        let backup = Connection::open(&file).unwrap();
        backup
            .execute("UPDATE projects SET path_key='wrong'", [])
            .unwrap();
        drop(backup);
        db.import(&file).unwrap();
        assert!(db
            .create(input(project.path()))
            .unwrap_err()
            .contains("已录入"));
        let backup = Connection::open(&file).unwrap();
        backup.execute("INSERT INTO projects SELECT 'different-id',name,path,'different-key',description,category,tags_json,tech_stack_json,favorite,notes,default_ide,last_opened_at,created_at,updated_at FROM projects", []).unwrap();
        drop(backup);
        assert!(db.import(&file).unwrap_err().contains("重复"));
        assert_eq!(db.list().unwrap()[0].id, saved.id);
    }
}

#[cfg(test)]
mod development_v2_tests {
    use super::*;
    #[test]
    fn development_migrates_v1_and_preserves_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("PRAGMA user_version=1;").unwrap();
        drop(conn);
        let db = Database::new(path).unwrap();
        let conn = db.connect().unwrap();
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM development_spaces", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
