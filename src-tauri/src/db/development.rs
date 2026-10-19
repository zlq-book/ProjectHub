use super::{error, Database};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS development_spaces (
 id TEXT PRIMARY KEY, project_id TEXT UNIQUE REFERENCES projects(id) ON DELETE SET NULL,
 project_name TEXT NOT NULL, project_path TEXT NOT NULL, repository_id TEXT
);
CREATE TABLE IF NOT EXISTS development_logs (
 id TEXT PRIMARY KEY, space_id TEXT NOT NULL REFERENCES development_spaces(id) ON DELETE CASCADE,
 title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 200), date TEXT NOT NULL,
 body TEXT NOT NULL CHECK(length(body)<=16000), revision INTEGER NOT NULL CHECK(revision>=1),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS development_tasks (
 id TEXT PRIMARY KEY, space_id TEXT NOT NULL REFERENCES development_spaces(id) ON DELETE CASCADE,
 title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 200), description TEXT NOT NULL CHECK(length(description)<=8000),
 status TEXT NOT NULL CHECK(status IN ('todo','inProgress','done')), completed_at TEXT,
 revision INTEGER NOT NULL CHECK(revision>=1), created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 CHECK((status='done' AND completed_at IS NOT NULL) OR (status<>'done' AND completed_at IS NULL))
);
CREATE TABLE IF NOT EXISTS development_commit_links (
 id TEXT PRIMARY KEY, log_id TEXT REFERENCES development_logs(id) ON DELETE CASCADE,
 task_id TEXT REFERENCES development_tasks(id) ON DELETE CASCADE,
 repository_id TEXT NOT NULL, sha TEXT NOT NULL, subject TEXT NOT NULL, author TEXT NOT NULL, commit_time TEXT NOT NULL,
 CHECK((log_id IS NOT NULL AND task_id IS NULL) OR (log_id IS NULL AND task_id IS NOT NULL)),
 UNIQUE(log_id,repository_id,sha), UNIQUE(task_id,repository_id,sha)
);
CREATE INDEX IF NOT EXISTS idx_development_logs_space ON development_logs(space_id);
CREATE INDEX IF NOT EXISTS idx_development_tasks_space ON development_tasks(space_id);
";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentSpace {
    pub id: String,
    pub project_id: Option<String>,
    pub project_name: String,
    pub project_path: String,
    pub repository_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommitLink {
    pub repository_id: String,
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub commit_time: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInput {
    pub repository_id: String,
    pub sha: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentLog {
    pub id: String,
    pub space_id: String,
    pub title: String,
    pub date: String,
    pub body: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    pub commits: Vec<CommitLink>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentTask {
    pub id: String,
    pub space_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub completed_at: Option<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    pub commits: Vec<CommitLink>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentRecords {
    pub space: DevelopmentSpace,
    pub logs: Vec<DevelopmentLog>,
    pub tasks: Vec<DevelopmentTask>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogInput {
    pub id: Option<String>,
    pub space_id: String,
    pub title: String,
    pub date: String,
    pub body: String,
    pub revision: Option<i64>,
    pub commits: Vec<CommitInput>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub id: Option<String>,
    pub space_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub revision: Option<i64>,
    pub commits: Vec<CommitInput>,
}

fn space_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DevelopmentSpace> {
    Ok(DevelopmentSpace {
        id: r.get(0)?,
        project_id: r.get(1)?,
        project_name: r.get(2)?,
        project_path: r.get(3)?,
        repository_id: r.get(4)?,
    })
}
fn space(conn: &Connection, id: &str) -> Result<DevelopmentSpace, String> {
    conn.query_row("SELECT id,project_id,project_name,project_path,repository_id FROM development_spaces WHERE id=?1",[id],space_row).map_err(|_|"开发空间不存在，请刷新".into())
}
fn links(conn: &Connection, id: &str, kind: &str) -> Result<Vec<CommitLink>, String> {
    let column = if kind == "logs" { "log_id" } else { "task_id" };
    let mut stmt=conn.prepare(&format!("SELECT repository_id,sha,subject,author,commit_time FROM development_commit_links WHERE {column}=?1 ORDER BY rowid")).map_err(error)?;
    let result = stmt
        .query_map([id], |r| {
            Ok(CommitLink {
                repository_id: r.get(0)?,
                sha: r.get(1)?,
                subject: r.get(2)?,
                author: r.get(3)?,
                commit_time: r.get(4)?,
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error);
    result
}
fn records(conn: &Connection, id: &str) -> Result<DevelopmentRecords, String> {
    let space = space(conn, id)?;
    let mut stmt=conn.prepare("SELECT id,space_id,title,date,body,revision,created_at,updated_at FROM development_logs WHERE space_id=?1 ORDER BY date DESC,created_at DESC").map_err(error)?;
    let mut logs = stmt
        .query_map([id], |r| {
            Ok(DevelopmentLog {
                id: r.get(0)?,
                space_id: r.get(1)?,
                title: r.get(2)?,
                date: r.get(3)?,
                body: r.get(4)?,
                revision: r.get(5)?,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
                commits: vec![],
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    for log in &mut logs {
        log.commits = links(conn, &log.id, "logs")?;
    }
    let mut stmt=conn.prepare("SELECT id,space_id,title,description,status,completed_at,revision,created_at,updated_at FROM development_tasks WHERE space_id=?1 ORDER BY created_at DESC").map_err(error)?;
    let mut tasks = stmt
        .query_map([id], |r| {
            Ok(DevelopmentTask {
                id: r.get(0)?,
                space_id: r.get(1)?,
                title: r.get(2)?,
                description: r.get(3)?,
                status: r.get(4)?,
                completed_at: r.get(5)?,
                revision: r.get(6)?,
                created_at: r.get(7)?,
                updated_at: r.get(8)?,
                commits: vec![],
            })
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    for task in &mut tasks {
        task.commits = links(conn, &task.id, "tasks")?;
    }
    Ok(DevelopmentRecords { space, logs, tasks })
}
fn validate_text(title: &str, body: &str, max: usize) -> Result<(), String> {
    if title.trim().is_empty()
        || title.chars().count() > 200
        || body.chars().count() > max
        || title.contains('\0')
        || body.contains('\0')
    {
        Err(format!("标题须为 1–200 字，内容不能超过 {max} 字"))
    } else {
        Ok(())
    }
}
fn valid_sha(sha: &str) -> bool {
    [40, 64].contains(&sha.len())
        && sha
            .bytes()
            .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
}
fn valid_time(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value).is_ok()
}
const CONFLICT: &str = "记录已被其他操作修改或删除，请刷新后重试";

impl Database {
    pub fn development_spaces(&self) -> Result<Vec<DevelopmentSpace>, String> {
        let conn = self.connect()?;
        let mut stmt=conn.prepare("SELECT s.id,s.project_id,COALESCE(p.name,s.project_name),COALESCE(p.path,s.project_path),s.repository_id FROM development_spaces s LEFT JOIN projects p ON p.id=s.project_id ORDER BY s.project_name,s.id").map_err(error)?;
        let result = stmt
            .query_map([], space_row)
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error);
        result
    }
    pub fn development_space(&self, project_id: &str) -> Result<DevelopmentSpace, String> {
        let epoch = self.development_request_epoch();
        let project = self.get(project_id)?;
        // 慢速 Git 查询不持有 SQLite 写锁，离线项目仍然可写日志和任务。
        let repo = crate::services::git::status(Path::new(&project.input.path))
            .ok()
            .and_then(|v| v.repository_id);
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(epoch)?;
        let current = tx
            .query_row(
                "SELECT name,path FROM projects WHERE id=?1",
                [project_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(error)?;
        if current.1 != project.input.path {
            return Err("项目位置已变化，请刷新后重试".into());
        }
        tx.execute("INSERT INTO development_spaces(id,project_id,project_name,project_path,repository_id) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(project_id) DO UPDATE SET project_name=excluded.project_name,project_path=excluded.project_path,repository_id=excluded.repository_id",params![uuid::Uuid::new_v4().to_string(),project_id,current.0,current.1,repo]).map_err(error)?;
        let result=tx.query_row("SELECT id,project_id,project_name,project_path,repository_id FROM development_spaces WHERE project_id=?1",[project_id],space_row).map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn development_records(&self, id: &str) -> Result<DevelopmentRecords, String> {
        let mut conn = self.connect()?;
        let tx = conn.transaction().map_err(error)?;
        let result = records(&tx, id)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn development_rebind(
        &self,
        id: &str,
        project_id: &str,
    ) -> Result<DevelopmentSpace, String> {
        let epoch = self.development_request_epoch();
        let project = self.get(project_id)?;
        let repo = crate::services::git::status(Path::new(&project.input.path))
            .ok()
            .and_then(|v| v.repository_id);
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(epoch)?;
        let original = space(&tx, id)?;
        if original.project_id.is_some() {
            return Err("只有已移除项目的开发空间可以重新关联".into());
        }
        let current = tx
            .query_row(
                "SELECT name,path FROM projects WHERE id=?1",
                [project_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(error)?;
        if current.1 != project.input.path {
            return Err("项目位置已变化，请刷新后重试".into());
        }
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM development_spaces WHERE project_id=?1",
                [project_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        if let Some(existing) = existing {
            let count:i64=tx.query_row("SELECT (SELECT count(*) FROM development_logs WHERE space_id=?1)+(SELECT count(*) FROM development_tasks WHERE space_id=?1)",[&existing],|r|r.get(0)).map_err(error)?;
            if count > 0 {
                return Err("目标项目已有开发记录，不能自动合并".into());
            }
            tx.execute("DELETE FROM development_spaces WHERE id=?1", [existing])
                .map_err(error)?;
        }
        tx.execute("UPDATE development_spaces SET project_id=?2,project_name=?3,project_path=?4,repository_id=?5 WHERE id=?1",params![id,project_id,current.0,current.1,repo]).map_err(error)?;
        let result = space(&tx, id)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    fn prepare_links(
        &self,
        space_id: &str,
        id: Option<&str>,
        kind: &str,
        input: &[CommitInput],
    ) -> Result<(DevelopmentSpace, Vec<CommitLink>), String> {
        if input.len() > 100 {
            return Err("单条记录最多关联 100 条提交".into());
        }
        let conn = self.connect()?;
        let original = space(&conn, space_id)?;
        let old = if let Some(id) = id {
            let exists:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM development_{kind} WHERE id=?1 AND space_id=?2)"),params![id,space_id],|r|r.get(0)).map_err(error)?;
            if !exists {
                return Err(CONFLICT.into());
            }
            links(&conn, id, kind)?
        } else {
            vec![]
        };
        let path = original
            .project_id
            .as_ref()
            .map(|id| self.get(id))
            .transpose()?
            .map(|p| p.input.path);
        let mut seen = HashSet::new();
        let mut prepared = vec![];
        for item in input {
            if !valid_sha(&item.sha) || item.repository_id.is_empty() {
                return Err("提交标识无效".into());
            }
            if !seen.insert((&item.repository_id, &item.sha)) {
                continue;
            }
            if let Some(link) = old
                .iter()
                .find(|v| v.repository_id == item.repository_id && v.sha == item.sha)
            {
                prepared.push(link.clone());
                continue;
            }
            let path = path
                .as_deref()
                .ok_or("已移除项目不能新增提交关联，请先手动重新关联项目")?;
            let (repository_id, commit) =
                crate::services::git::verify_commit(Path::new(path), &item.sha)?;
            if repository_id != item.repository_id {
                return Err("提交不属于当前项目仓库".into());
            }
            prepared.push(CommitLink {
                repository_id,
                sha: commit.sha,
                subject: commit.subject,
                author: commit.author,
                commit_time: commit.commit_time,
            });
        }
        // 用路径与绑定快照防止慢速 Git 校验期间项目重新定位或解除绑定。
        let mut expected = original;
        if let Some(path) = path {
            expected.project_path = path;
        }
        Ok((expected, prepared))
    }
    fn check_space(tx: &Connection, expected: &DevelopmentSpace) -> Result<(), String> {
        let current = space(tx, &expected.id)?;
        if current.project_id != expected.project_id {
            return Err("项目绑定已变化，请刷新后重试".into());
        }
        if let Some(id) = &current.project_id {
            let path: String = tx
                .query_row("SELECT path FROM projects WHERE id=?1", [id], |r| r.get(0))
                .map_err(error)?;
            if path != expected.project_path {
                return Err("项目位置已变化，请刷新后重试".into());
            }
        }
        Ok(())
    }
    pub fn development_save_log(&self, input: LogInput) -> Result<DevelopmentRecords, String> {
        let epoch = self.development_request_epoch();
        validate_text(&input.title, &input.body, 16000)?;
        if NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
            .ok()
            .map_or(true, |v| v.format("%Y-%m-%d").to_string() != input.date)
        {
            return Err("日志日期无效".into());
        }
        let (expected, links) =
            self.prepare_links(&input.space_id, input.id.as_deref(), "logs", &input.commits)?;
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(epoch)?;
        Self::check_space(&tx, &expected)?;
        let now = Utc::now().to_rfc3339();
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if input.id.is_some() {
            if tx.execute("UPDATE development_logs SET title=?3,date=?4,body=?5,revision=revision+1,updated_at=?6 WHERE id=?1 AND space_id=?2 AND revision=?7",params![id,input.space_id,input.title,input.date,input.body,now,input.revision]).map_err(error)?!=1 {return Err(CONFLICT.into());}
        } else {
            if input.revision.is_some() {
                return Err("新记录不能指定版本".into());
            }
            tx.execute(
                "INSERT INTO development_logs VALUES(?1,?2,?3,?4,?5,1,?6,?6)",
                params![id, input.space_id, input.title, input.date, input.body, now],
            )
            .map_err(error)?;
        }
        replace_links(&tx, &id, "logs", &links)?;
        let result = records(&tx, &input.space_id)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn development_save_task(&self, input: TaskInput) -> Result<DevelopmentRecords, String> {
        let epoch = self.development_request_epoch();
        validate_text(&input.title, &input.description, 8000)?;
        if !["todo", "inProgress", "done"].contains(&input.status.as_str()) {
            return Err("任务状态无效".into());
        }
        let (expected, links) = self.prepare_links(
            &input.space_id,
            input.id.as_deref(),
            "tasks",
            &input.commits,
        )?;
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(epoch)?;
        Self::check_space(&tx, &expected)?;
        let now = Utc::now().to_rfc3339();
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if input.id.is_some() {
            if tx.execute("UPDATE development_tasks SET title=?3,description=?4,status=?5,completed_at=CASE WHEN ?5='done' THEN COALESCE(completed_at,?6) ELSE NULL END,revision=revision+1,updated_at=?6 WHERE id=?1 AND space_id=?2 AND revision=?7",params![id,input.space_id,input.title,input.description,input.status,now,input.revision]).map_err(error)?!=1 {return Err(CONFLICT.into());}
        } else {
            if input.revision.is_some() {
                return Err("新记录不能指定版本".into());
            }
            let completed = if input.status == "done" {
                Some(&now)
            } else {
                None
            };
            tx.execute(
                "INSERT INTO development_tasks VALUES(?1,?2,?3,?4,?5,?6,1,?7,?7)",
                params![
                    id,
                    input.space_id,
                    input.title,
                    input.description,
                    input.status,
                    completed,
                    now
                ],
            )
            .map_err(error)?;
        }
        replace_links(&tx, &id, "tasks", &links)?;
        let result = records(&tx, &input.space_id)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn development_delete_record(
        &self,
        space_id: &str,
        id: &str,
        kind: &str,
        revision: i64,
    ) -> Result<DevelopmentRecords, String> {
        let epoch = self.development_request_epoch();
        if !["logs", "tasks"].contains(&kind) {
            return Err("记录类型无效".into());
        }
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(epoch)?;
        if tx
            .execute(
                &format!(
                    "DELETE FROM development_{kind} WHERE id=?1 AND space_id=?2 AND revision=?3"
                ),
                params![id, space_id, revision],
            )
            .map_err(error)?
            != 1
        {
            return Err(CONFLICT.into());
        }
        let result = records(&tx, space_id)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
}
fn replace_links(
    conn: &Connection,
    id: &str,
    kind: &str,
    links: &[CommitLink],
) -> Result<(), String> {
    let column = if kind == "logs" { "log_id" } else { "task_id" };
    conn.execute(
        &format!("DELETE FROM development_commit_links WHERE {column}=?1"),
        [id],
    )
    .map_err(error)?;
    for link in links {
        conn.execute(&format!("INSERT INTO development_commit_links(id,{column},repository_id,sha,subject,author,commit_time) VALUES(?1,?2,?3,?4,?5,?6,?7)"),params![uuid::Uuid::new_v4().to_string(),id,link.repository_id,link.sha,link.subject,link.author,link.commit_time]).map_err(error)?;
    }
    Ok(())
}

pub(super) fn archive_project(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("UPDATE development_spaces SET project_name=(SELECT name FROM projects WHERE id=?1),project_path=(SELECT path FROM projects WHERE id=?1) WHERE project_id=?1",[id]).map_err(error)?;
    Ok(())
}

// 只接受明确读取并验证的字段，外来索引、约束、触发器均不会进入当前数据库。
pub(super) fn read_backup(
    conn: &Connection,
    projects: &HashSet<String>,
) -> Result<Vec<DevelopmentRecords>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id,project_id,project_name,project_path,repository_id FROM development_spaces",
        )
        .map_err(error)?;
    let spaces = stmt
        .query_map([], space_row)
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    let mut space_ids = HashSet::new();
    let mut bound = HashSet::new();
    let mut log_ids = HashSet::new();
    let mut task_ids = HashSet::new();
    let mut result = vec![];
    let mut link_count = 0;
    for space in spaces {
        if uuid::Uuid::parse_str(&space.id).is_err()
            || !space_ids.insert(space.id.clone())
            || space.project_name.trim().is_empty()
            || space.project_name.chars().count() > 200
            || !Path::new(&space.project_path).is_absolute()
            || space.project_path.contains('\0')
            || space
                .repository_id
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 8192 || v.contains('\0'))
        {
            return Err("备份开发空间字段无效".into());
        }
        if let Some(id) = &space.project_id {
            if !projects.contains(id) || !bound.insert(id.clone()) {
                return Err("备份开发空间的项目绑定无效".into());
            }
        }
        let records = records(conn, &space.id)?;
        for log in &records.logs {
            validate_text(&log.title, &log.body, 16000)?;
            if uuid::Uuid::parse_str(&log.id).is_err()
                || !log_ids.insert(log.id.clone())
                || log.revision < 1
                || log.revision == i64::MAX
                || !valid_time(&log.created_at)
                || !valid_time(&log.updated_at)
                || NaiveDate::parse_from_str(&log.date, "%Y-%m-%d")
                    .ok()
                    .map_or(true, |v| v.format("%Y-%m-%d").to_string() != log.date)
            {
                return Err("备份日志字段无效".into());
            }
            validate_links(&log.commits)?;
            link_count += log.commits.len();
        }
        for task in &records.tasks {
            validate_text(&task.title, &task.description, 8000)?;
            if uuid::Uuid::parse_str(&task.id).is_err()
                || !task_ids.insert(task.id.clone())
                || task.revision < 1
                || task.revision == i64::MAX
                || !valid_time(&task.created_at)
                || !valid_time(&task.updated_at)
                || !["todo", "inProgress", "done"].contains(&task.status.as_str())
                || (task.status == "done") != task.completed_at.is_some()
                || task.completed_at.as_ref().is_some_and(|v| !valid_time(v))
            {
                return Err("备份任务字段无效".into());
            }
            validate_links(&task.commits)?;
            link_count += task.commits.len();
        }
        result.push(records);
    }
    for (table, count) in [
        ("development_logs", log_ids.len()),
        ("development_tasks", task_ids.len()),
        ("development_commit_links", link_count),
    ] {
        let actual: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .map_err(error)?;
        if actual != count as i64 {
            return Err("备份包含孤立或无效的开发记录关联".into());
        }
    }
    let invalid:i64=conn.query_row("SELECT count(*) FROM development_commit_links WHERE (log_id IS NULL AND task_id IS NULL) OR (log_id IS NOT NULL AND task_id IS NOT NULL)",[],|r|r.get(0)).map_err(error)?;
    if invalid > 0 {
        return Err("备份提交关联必须且只能属于一条日志或任务".into());
    }
    Ok(result)
}
fn validate_links(links: &[CommitLink]) -> Result<(), String> {
    if links.len() > 100 {
        return Err("备份单条记录关联过多".into());
    }
    let mut seen = HashSet::new();
    for link in links {
        if link.repository_id.is_empty()
            || link.repository_id.len() > 8192
            || link.repository_id.contains('\0')
            || !valid_sha(&link.sha)
            || !valid_time(&link.commit_time)
            || link.subject.len() > 1024 * 1024
            || link.author.len() > 65536
            || !seen.insert((&link.repository_id, &link.sha))
        {
            return Err("备份提交关联字段或重复关系无效".into());
        }
    }
    Ok(())
}
pub(super) fn write_backup(
    conn: &Connection,
    records: &[DevelopmentRecords],
) -> Result<(), String> {
    for records in records {
        let s = &records.space;
        conn.execute(
            "INSERT INTO development_spaces VALUES(?1,?2,?3,?4,?5)",
            params![
                s.id,
                s.project_id,
                s.project_name,
                s.project_path,
                s.repository_id
            ],
        )
        .map_err(error)?;
        for log in &records.logs {
            conn.execute(
                "INSERT INTO development_logs VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    log.id,
                    log.space_id,
                    log.title,
                    log.date,
                    log.body,
                    log.revision,
                    log.created_at,
                    log.updated_at
                ],
            )
            .map_err(error)?;
            replace_links(conn, &log.id, "logs", &log.commits)?;
        }
        for t in &records.tasks {
            conn.execute(
                "INSERT INTO development_tasks VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    t.id,
                    t.space_id,
                    t.title,
                    t.description,
                    t.status,
                    t.completed_at,
                    t.revision,
                    t.created_at,
                    t.updated_at
                ],
            )
            .map_err(error)?;
            replace_links(conn, &t.id, "tasks", &t.commits)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProjectInput;
    use std::process::Command;
    fn project(path: &Path) -> ProjectInput {
        ProjectInput {
            name: "开发项目".into(),
            path: path.to_string_lossy().into_owned(),
            description: "资料保持原样".into(),
            category: "other".into(),
            tags: vec![],
            tech_stack: vec![],
            favorite: false,
            notes: "".into(),
            default_ide: "system".into(),
        }
    }
    fn setup() -> (
        tempfile::TempDir,
        tempfile::TempDir,
        Database,
        crate::models::Project,
        DevelopmentSpace,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("test.sqlite")).unwrap();
        let p = db.create(project(repo.path())).unwrap();
        let s = db.development_space(&p.id).unwrap();
        (dir, repo, db, p, s)
    }
    fn log(space: &str) -> LogInput {
        LogInput {
            id: None,
            space_id: space.into(),
            title: "开发日志".into(),
            date: "2026-10-16".into(),
            body: "内容".into(),
            revision: None,
            commits: vec![],
        }
    }
    fn task(space: &str) -> TaskInput {
        TaskInput {
            id: None,
            space_id: space.into(),
            title: "开发任务".into(),
            description: "描述".into(),
            status: "todo".into(),
            revision: None,
            commits: vec![],
        }
    }
    fn git(path: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(path)
            .args([
                "-c",
                "user.name=开发测试",
                "-c",
                "user.email=development@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().into()
    }
    fn commit(path: &Path, contents: &str) -> String {
        std::fs::write(path.join("code.txt"), contents).unwrap();
        git(path, &["add", "--", "code.txt"]);
        git(path, &["commit", "-m", contents]);
        git(path, &["rev-parse", "HEAD"])
    }
    #[test]
    fn development_revision_conflicts_completion_and_project_time() {
        let (_dir, _repo, db, p, s) = setup();
        let result = db.development_save_log(log(&s.id)).unwrap();
        let old = &result.logs[0];
        let mut edit = log(&s.id);
        edit.id = Some(old.id.clone());
        edit.revision = Some(1);
        edit.title = "已修改".into();
        let result = db.development_save_log(edit.clone()).unwrap();
        assert_eq!(result.logs[0].revision, 2);
        assert!(db.development_save_log(edit).unwrap_err().contains("修改"));
        assert!(db
            .development_delete_record(&s.id, &old.id, "logs", 1)
            .is_err());
        let result = db.development_save_task(task(&s.id)).unwrap();
        let original = &result.tasks[0];
        assert!(original.completed_at.is_none());
        let mut t = task(&s.id);
        t.id = Some(original.id.clone());
        t.revision = Some(1);
        t.status = "done".into();
        let result = db.development_save_task(t.clone()).unwrap();
        let done = result.tasks[0].completed_at.clone().unwrap();
        t.revision = Some(2);
        t.title = "只改标题".into();
        let result = db.development_save_task(t.clone()).unwrap();
        assert_eq!(result.tasks[0].completed_at.as_deref(), Some(done.as_str()));
        t.revision = Some(3);
        t.status = "inProgress".into();
        let result = db.development_save_task(t.clone()).unwrap();
        assert!(result.tasks[0].completed_at.is_none());
        t.revision = Some(4);
        t.status = "done".into();
        assert!(db.development_save_task(t).unwrap().tasks[0]
            .completed_at
            .is_some());
        assert_eq!(db.get(&p.id).unwrap().updated_at, p.updated_at);
        assert_eq!(
            Database::new(db.path.clone())
                .unwrap()
                .development_records(&s.id)
                .unwrap()
                .tasks[0]
                .revision,
            5
        );
    }
    #[test]
    fn development_archive_rebind_and_cross_space_guards() {
        let (_dir, repo, db, p, s) = setup();
        let result = db.development_save_log(log(&s.id)).unwrap();
        let old = &result.logs[0];
        let other = tempfile::tempdir().unwrap();
        let p2 = db.create(project(other.path())).unwrap();
        let s2 = db.development_space(&p2.id).unwrap();
        let mut edit = log(&s2.id);
        edit.id = Some(old.id.clone());
        edit.revision = Some(1);
        assert!(db.development_save_log(edit).is_err());
        assert!(db
            .development_delete_record(&s2.id, &old.id, "logs", 1)
            .is_err());
        let mut input = p.input.clone();
        input.name = "归档名称".into();
        db.update(&p.id, input).unwrap();
        db.remove(&p.id).unwrap();
        let archived = db.development_records(&s.id).unwrap();
        assert!(archived.space.project_id.is_none());
        assert_eq!(archived.space.project_name, "归档名称");
        assert_eq!(archived.logs.len(), 1);
        let replacement = db.create(project(repo.path())).unwrap();
        let empty = db.development_space(&replacement.id).unwrap();
        assert_ne!(empty.id, s.id);
        assert_eq!(
            db.development_rebind(&s.id, &replacement.id).unwrap().id,
            s.id
        );
        assert!(db.development_records(&empty.id).is_err());
        db.batch_remove(vec![replacement.id.clone(), "missing".into()])
            .unwrap_err();
        assert_eq!(
            db.development_records(&s.id).unwrap().space.project_id,
            Some(replacement.id.clone())
        );
        db.batch_remove(vec![replacement.id]).unwrap();
        assert!(db
            .development_records(&s.id)
            .unwrap()
            .space
            .project_id
            .is_none());
    }
    #[test]
    fn development_offline_non_git_and_field_limits() {
        let (_dir, repo, db, p, s) = setup();
        repo.close().unwrap();
        assert!(db.development_space(&p.id).is_ok());
        db.development_save_log(log(&s.id)).unwrap();
        db.development_save_task(task(&s.id)).unwrap();
        let mut invalid = log(&s.id);
        invalid.title = " ".into();
        assert!(db.development_save_log(invalid).is_err());
        let mut invalid = log(&s.id);
        invalid.date = "2026-02-30".into();
        assert!(db.development_save_log(invalid).is_err());
        let mut invalid = log(&s.id);
        invalid.body = "文".repeat(16001);
        assert!(db.development_save_log(invalid).is_err());
        let mut invalid = task(&s.id);
        invalid.status = "finished".into();
        assert!(db.development_save_task(invalid).is_err());
        let mut invalid = task(&s.id);
        invalid.description = "文".repeat(8001);
        assert!(db.development_save_task(invalid).is_err());
    }
    #[test]
    fn development_multiple_real_commits_deduplicate_preserve_old_repository() {
        let (_dir, repo, db, p, s) = setup();
        git(repo.path(), &["init"]);
        let a = commit(repo.path(), "第一条");
        let b = commit(repo.path(), "第二条");
        let repo_id = crate::services::git::status(repo.path())
            .unwrap()
            .repository_id
            .unwrap();
        let mut input = log(&s.id);
        input.commits = vec![
            CommitInput {
                repository_id: repo_id.clone(),
                sha: a.clone(),
            },
            CommitInput {
                repository_id: repo_id.clone(),
                sha: b.clone(),
            },
            CommitInput {
                repository_id: repo_id.clone(),
                sha: a.clone(),
            },
        ];
        let result = db.development_save_log(input.clone()).unwrap();
        let old = &result.logs[0];
        assert_eq!(old.commits.len(), 2);
        let other = tempfile::tempdir().unwrap();
        git(other.path(), &["init"]);
        let other_sha = commit(other.path(), "另一个仓库");
        let other_repo = crate::services::git::status(other.path())
            .unwrap()
            .repository_id
            .unwrap();
        let mut forged = log(&s.id);
        forged.commits = vec![CommitInput {
            repository_id: other_repo,
            sha: other_sha,
        }];
        assert!(db.development_save_log(forged).is_err());
        let tree = git(repo.path(), &["rev-parse", "HEAD^{tree}"]);
        let mut forged = log(&s.id);
        forged.commits = vec![CommitInput {
            repository_id: repo_id.clone(),
            sha: tree,
        }];
        assert!(db.development_save_log(forged).is_err());
        let mut changed = p.input.clone();
        changed.path = other.path().to_string_lossy().into_owned();
        db.update(&p.id, changed).unwrap();
        assert_ne!(
            db.development_space(&p.id)
                .unwrap()
                .repository_id
                .as_deref(),
            Some(repo_id.as_str())
        );
        input.id = Some(old.id.clone());
        input.revision = Some(old.revision);
        input.title = "保留旧关联".into();
        assert_eq!(
            db.development_save_log(input).unwrap().logs[0].commits,
            old.commits
        );
        let mut forged = log(&s.id);
        forged.commits = vec![CommitInput {
            repository_id: repo_id,
            sha: a,
        }];
        assert!(db.development_save_log(forged).is_err());
        db.development_delete_record(&s.id, &old.id, "logs", 2)
            .unwrap();
        assert_eq!(
            db.connect()
                .unwrap()
                .query_row("SELECT count(*) FROM development_commit_links", [], |r| r
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            0
        );
        assert!(crate::services::git::verify_commit(repo.path(), &b).is_ok());
    }
    #[test]
    fn development_backup_v2_roundtrip_rejects_orphans_and_ignores_triggers() {
        let (dir, repo, db, p, s) = setup();
        git(repo.path(), &["init"]);
        let sha = commit(repo.path(), "备份提交");
        let id = crate::services::git::status(repo.path())
            .unwrap()
            .repository_id
            .unwrap();
        let mut input = log(&s.id);
        input.commits = vec![CommitInput {
            repository_id: id,
            sha,
        }];
        let original = db.development_save_log(input).unwrap();
        let mut input = task(&s.id);
        input.status = "done".into();
        db.development_save_task(input).unwrap();
        let backup = dir.path().join("v2.sqlite");
        db.export(&backup).unwrap();
        let external = Connection::open(&backup).unwrap();
        external.execute_batch("CREATE TRIGGER foreign_trigger AFTER INSERT ON development_logs BEGIN DELETE FROM projects; END;").unwrap();
        drop(external);
        db.remove(&p.id).unwrap();
        let prior_epoch = db.development_epoch();
        let old_request = db.for_development_request();
        db.import(&backup).unwrap();
        assert!(
            old_request.check_development_epoch(prior_epoch).is_err(),
            "恢复前的异步保存不能污染恢复结果"
        );
        assert!(
            old_request
                .development_save_log(log(&s.id))
                .unwrap_err()
                .contains("项目库已恢复"),
            "不能在导入后重新认领新代次"
        );
        let restored = db.development_records(&s.id).unwrap();
        assert_eq!(restored.logs[0].commits, original.logs[0].commits);
        assert_eq!(restored.tasks[0].status, "done");
        assert!(restored.tasks[0].completed_at.is_some());
        assert_eq!(db.get(&p.id).unwrap().updated_at, p.updated_at);
        assert_eq!(
            db.connect()
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='trigger'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        let external = Connection::open(&backup).unwrap();
        external.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
        external
            .execute("UPDATE development_commit_links SET log_id='missing'", [])
            .unwrap();
        drop(external);
        assert!(db.import(&backup).is_err());
        assert_eq!(
            db.development_records(&s.id).unwrap().logs[0].commits,
            original.logs[0].commits
        );
        assert!(db.get(&p.id).is_ok());
    }
    #[test]
    fn development_v1_migration_and_import_clear_records_but_auto_backup_preserves_them() {
        let (dir, _repo, db, p, s) = setup();
        db.development_save_log(log(&s.id)).unwrap();
        let backup = dir.path().join("old.sqlite");
        db.export(&backup).unwrap();
        let external = Connection::open(&backup).unwrap();
        external.execute_batch("DROP TABLE development_commit_links; DROP TABLE development_logs; DROP TABLE development_tasks; DROP TABLE development_spaces; PRAGMA user_version=1;").unwrap();
        drop(external);
        let migrated = Database::new(backup.clone()).unwrap();
        assert!(migrated.get(&p.id).is_ok());
        assert!(migrated.development_spaces().unwrap().is_empty());
        let external = Connection::open(&backup).unwrap();
        external.execute_batch("DROP TABLE development_commit_links; DROP TABLE development_logs; DROP TABLE development_tasks; DROP TABLE development_spaces; PRAGMA user_version=1;").unwrap();
        drop(external);
        let message = db.import(&backup).unwrap();
        assert!(message.contains("旧版备份不含日志和任务"));
        assert!(db.development_spaces().unwrap().is_empty());
        assert_eq!(db.get(&p.id).unwrap().updated_at, p.updated_at);
        let auto = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("projecthub-before-import-")
            })
            .unwrap();
        let previous = Database::new(auto.path()).unwrap();
        assert_eq!(previous.development_records(&s.id).unwrap().logs.len(), 1);
    }
    #[test]
    fn development_missing_objects_keep_existing_links_but_cannot_forge_new_record() {
        let (_dir, repo, db, p, s) = setup();
        git(repo.path(), &["init"]);
        let sha = commit(repo.path(), "将离线的提交");
        let repository_id = crate::services::git::status(repo.path())
            .unwrap()
            .repository_id
            .unwrap();
        let mut input = task(&s.id);
        input.commits = vec![CommitInput { repository_id, sha }];
        let saved = db.development_save_task(input.clone()).unwrap();
        let original = &saved.tasks[0];
        repo.close().unwrap();
        assert!(db.development_space(&p.id).unwrap().repository_id.is_none());
        input.id = Some(original.id.clone());
        input.revision = Some(original.revision);
        input.status = "done".into();
        let edited = db.development_save_task(input.clone()).unwrap();
        assert_eq!(edited.tasks[0].commits, original.commits);
        input.id = None;
        input.revision = None;
        assert!(db.development_save_task(input).is_err());
    }
}
