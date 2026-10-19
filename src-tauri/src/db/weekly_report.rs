use super::{error, Database};
use crate::services::weekly_report::protocol::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
pub(super) const SCHEMA:&str="
CREATE TABLE IF NOT EXISTS weekly_report_preferences(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS weekly_reports(id TEXT PRIMARY KEY,adopted_version_id TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS weekly_report_versions(id TEXT PRIMARY KEY,report_id TEXT NOT NULL REFERENCES weekly_reports(id) ON DELETE CASCADE,revision INTEGER NOT NULL CHECK(revision>=1),json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS weekly_report_sources(version_id TEXT NOT NULL REFERENCES weekly_report_versions(id) ON DELETE CASCADE,source_id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(version_id,source_id));
";
#[derive(Default)]
pub(super) struct Backup {
    prefs: Preferences,
    groups: Vec<ReportGroup>,
}
fn encode<T: serde::Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string(v).map_err(|_| "周报序列化失败".into())
}
fn decode<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, String> {
    if s.len() > 16 * 1024 * 1024 {
        return Err("周报记录过大".into());
    }
    serde_json::from_str(s).map_err(|_| "周报记录格式无效".into())
}
fn preferences(c: &Connection) -> Result<Preferences, String> {
    let text: Option<String> = c
        .query_row(
            "SELECT json FROM weekly_report_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(error)?;
    let p: Preferences = match text {
        Some(t) => decode(&t)?,
        None => Default::default(),
    };
    p.validate()?;
    Ok(p)
}
fn get(c: &Connection, id: &str) -> Result<ReportGroup, String> {
    let (adopted, created): (String, String) = c
        .query_row(
            "SELECT adopted_version_id,created_at FROM weekly_reports WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(error)?;
    let mut stmt=c.prepare("SELECT id,report_id,revision,json FROM weekly_report_versions WHERE report_id=?1 ORDER BY rowid").map_err(error)?;
    let rows = stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    if rows.len() > 1000 {
        return Err("单份周报版本过多".into());
    }
    let mut versions = vec![];
    for (vid, rid, revision, text) in rows {
        let mut v: ReportVersion = decode(&text)?;
        if v.id != vid || v.report_id != rid || v.revision != revision || !v.sources.is_empty() {
            return Err("周报版本关系无效".into());
        }
        let mut sources=c.prepare("SELECT source_id,json FROM weekly_report_sources WHERE version_id=?1 ORDER BY rowid").map_err(error)?;
        for row in sources
            .query_map([&vid], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(error)?
        {
            let (sid, text) = row.map_err(error)?;
            let s: ReportSource = decode(&text)?;
            if s.id != sid {
                return Err("来源标识无效".into());
            }
            v.sources.push(s);
        }
        validate_version(&v)?;
        versions.push(v);
    }
    let g = ReportGroup {
        id: id.into(),
        adopted_version_id: adopted,
        versions,
        created_at: created,
    };
    validate_group(&g)?;
    Ok(g)
}
pub fn validate_version(v: &ReportVersion) -> Result<(), String> {
    v.content.validate(false)?;
    Preferences {
        authors: v.context.authors.clone(),
        ..Default::default()
    }
    .validate()?;
    if v.context.projects.len() > 20
        || v.context.diffs.len() > 100
        || v.context.model.len() > 200
        || v.context.base_url.len() > 4096
        || v.context.warnings.len() > 100
        || v.context.diffs.iter().any(|d| {
            !v.sources
                .iter()
                .any(|s| s.id == d.source_id && s.files.contains(&d.path))
        })
    {
        return Err("周报生成上下文无效".into());
    }
    if v.id.is_empty()
        || v.id.len() > 100
        || v.report_id.len() > 100
        || v.revision < 1
        || v.sources.len() > 10200
        || v.hints.difficulties.chars().count() > 8000
        || v.hints.plans.chars().count() > 8000
        || v.hints.reflection.chars().count() > 8000
        || chrono::DateTime::parse_from_rfc3339(&v.created_at).is_err()
        || chrono::DateTime::parse_from_rfc3339(&v.updated_at).is_err()
    {
        return Err("周报版本或时间无效".into());
    }
    let mut ids = HashSet::new();
    for s in &v.sources {
        if s.id.is_empty()
            || s.id.len() > 300
            || !ids.insert(&s.id)
            || !matches!(s.kind.as_str(), "commit" | "log")
            || s.body.len() > 128 * 1024
            || s.title.len() > 16000
            || s.files.len() > 4000
            || s.renames.len() > 2000
            || s.log_commits.len() > 100
            || s.project_ids.len() > 20
            || s.files.iter().any(|p| p.len() > 4096)
            || s.author_email.len() > 320
        {
            return Err("周报来源无效或过大".into());
        }
        if s.kind == "commit" {
            let sha = s.sha.as_deref().unwrap_or("");
            if !matches!(sha.len(), 40 | 64)
                || !sha.bytes().all(|b| b.is_ascii_hexdigit())
                || s.repository_id.as_deref().unwrap_or("").is_empty()
            {
                return Err("提交来源无效".into());
            }
        } else if s.log_id.as_deref().unwrap_or("").is_empty()
            || s.revision.unwrap_or(0) < 1
            || chrono::NaiveDate::parse_from_str(&s.log_date, "%Y-%m-%d").is_err()
        {
            return Err("日志来源无效".into());
        }
    }
    for o in &v.content.outcomes {
        if o.source_ids.iter().any(|id| !ids.contains(id))
            || (o.origin == "ai" && o.source_ids.is_empty())
        {
            return Err("成果引用不存在的材料".into());
        }
    }
    Ok(())
}
fn validate_group(g: &ReportGroup) -> Result<(), String> {
    if g.id.is_empty()
        || g.versions.is_empty()
        || !g.versions.iter().any(|v| v.id == g.adopted_version_id)
        || chrono::DateTime::parse_from_rfc3339(&g.created_at).is_err()
    {
        return Err("周报采用版本无效".into());
    }
    let mut seen = HashSet::new();
    for v in &g.versions {
        if v.report_id != g.id
            || !seen.insert(&v.id)
            || v.parent_version_id
                .as_ref()
                .is_some_and(|p| !seen.contains(p) || p == &v.id)
        {
            return Err("周报版本引用无效".into());
        }
        validate_version(v)?;
    }
    Ok(())
}
fn write_version(c: &Connection, v: &ReportVersion) -> Result<(), String> {
    let mut record = v.clone();
    record.sources.clear();
    c.execute("INSERT INTO weekly_report_versions(id,report_id,revision,json) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,json=excluded.json",params![v.id,v.report_id,v.revision,encode(&record)?]).map_err(error)?;
    for s in &v.sources {
        c.execute(
            "INSERT INTO weekly_report_sources VALUES(?1,?2,?3)",
            params![v.id, s.id, encode(s)?],
        )
        .map_err(error)?;
    }
    Ok(())
}
impl Database {
    pub fn weekly_preferences(&self) -> Result<Preferences, String> {
        preferences(&self.connect()?)
    }
    pub fn weekly_save_preferences(&self, p: Preferences) -> Result<(), String> {
        p.validate()?;
        let mut c = self.connect()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(self.development_request_epoch())?;
        tx.execute("INSERT INTO weekly_report_preferences VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",[encode(&p)?]).map_err(error)?;
        tx.commit().map_err(error)
    }
    pub fn weekly_get(&self, id: &str) -> Result<ReportGroup, String> {
        get(&self.connect()?, id)
    }
    pub fn weekly_list(&self) -> Result<Vec<ReportGroup>, String> {
        let c = self.connect()?;
        let mut stmt = c
            .prepare("SELECT id FROM weekly_reports ORDER BY rowid DESC")
            .map_err(error)?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)?;
        ids.into_iter().map(|id| get(&c, &id)).collect()
    }
    pub fn weekly_save(&self, i: SaveInput) -> Result<ReportGroup, String> {
        i.content.validate(false)?;
        let mut c = self.connect()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(self.development_request_epoch())?;
        let now = chrono::Utc::now().to_rfc3339();
        let rid = i
            .report_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let existing = if i.report_id.is_some() {
            Some(get(&tx, &rid)?)
        } else {
            None
        };
        if existing.as_ref().is_some_and(|g| g.versions.len() >= 1000) && i.version_id.is_none() {
            return Err("周报版本达到上限".into());
        }
        let old = i.version_id.as_ref().and_then(|id| {
            existing
                .as_ref()
                .and_then(|g| g.versions.iter().find(|v| &v.id == id))
        });
        if i.version_id.is_some() && (old.is_none() || i.revision != old.map(|v| v.revision)) {
            return Err("周报已被其他操作修改，请刷新后重试".into());
        }
        if i.parent_version_id.as_ref().is_some_and(|id| {
            !existing
                .as_ref()
                .is_some_and(|g| g.versions.iter().any(|v| &v.id == id))
        }) {
            return Err("父版本不属于此周报".into());
        }
        let v = ReportVersion {
            id: old
                .map(|v| v.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            report_id: rid.clone(),
            parent_version_id: old
                .and_then(|v| v.parent_version_id.clone())
                .or(i.parent_version_id),
            revision: old.map(|v| v.revision + 1).unwrap_or(1),
            content: i.content,
            sources: i.sources,
            hints: i.hints,
            context: i.context,
            created_at: old.map(|v| v.created_at.clone()).unwrap_or(now.clone()),
            updated_at: now.clone(),
        };
        validate_version(&v)?;
        if existing.is_none() {
            tx.execute(
                "INSERT INTO weekly_reports VALUES(?1,?2,?3)",
                params![rid, v.id, now],
            )
            .map_err(error)?;
        }
        if old.is_some() {
            tx.execute(
                "DELETE FROM weekly_report_sources WHERE version_id=?1",
                [&v.id],
            )
            .map_err(error)?;
        }
        write_version(&tx, &v)?;
        let result = get(&tx, &rid)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn weekly_adopt(&self, report_id: &str, version_id: &str) -> Result<ReportGroup, String> {
        let mut c = self.connect()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        self.check_development_epoch(self.development_request_epoch())?;
        let group = get(&tx, report_id)?;
        if !group.versions.iter().any(|v| v.id == version_id) {
            return Err("采用版本不属于此周报".into());
        }
        tx.execute(
            "UPDATE weekly_reports SET adopted_version_id=?1 WHERE id=?2",
            params![version_id, report_id],
        )
        .map_err(error)?;
        let result = get(&tx, report_id)?;
        tx.commit().map_err(error)?;
        Ok(result)
    }
    pub fn weekly_saved_version(&self, id: &str, revision: i64) -> Result<ReportVersion, String> {
        let c = self.connect()?;
        let rid: String = c
            .query_row(
                "SELECT report_id FROM weekly_report_versions WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(error)?;
        let group = get(&c, &rid)?;
        let v = group
            .versions
            .into_iter()
            .find(|v| v.id == id && v.revision == revision)
            .ok_or("周报版本已变化，请保存后重新导出")?;
        v.content.validate(true)?;
        Ok(v)
    }
}
pub(super) fn read_backup(c: &Connection) -> Result<Backup, String> {
    let prefs = preferences(c)?;
    let mut stmt = c
        .prepare("SELECT id FROM weekly_reports ORDER BY rowid")
        .map_err(error)?;
    let ids = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(error)?;
    if ids.len() > 5000 {
        return Err("备份周报数量过多".into());
    }
    let groups = ids
        .into_iter()
        .map(|id| get(c, &id))
        .collect::<Result<Vec<_>, _>>()?;
    let stored: i64 = c
        .query_row("SELECT count(*) FROM weekly_report_versions", [], |r| {
            r.get(0)
        })
        .map_err(error)?;
    let sources: i64 = c
        .query_row("SELECT count(*) FROM weekly_report_sources", [], |r| {
            r.get(0)
        })
        .map_err(error)?;
    if stored != groups.iter().map(|g| g.versions.len() as i64).sum::<i64>()
        || sources
            != groups
                .iter()
                .flat_map(|g| &g.versions)
                .map(|v| v.sources.len() as i64)
                .sum::<i64>()
    {
        return Err("备份存在孤立周报记录".into());
    }
    Ok(Backup { prefs, groups })
}
pub(super) fn write_backup(c: &Connection, b: &Backup) -> Result<(), String> {
    c.execute(
        "INSERT INTO weekly_report_preferences VALUES(1,?1)",
        [encode(&b.prefs)?],
    )
    .map_err(error)?;
    for g in &b.groups {
        c.execute(
            "INSERT INTO weekly_reports VALUES(?1,?2,?3)",
            params![g.id, g.adopted_version_id, g.created_at],
        )
        .map_err(error)?;
        for v in &g.versions {
            write_version(c, v)?;
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "weekly_report_tests.rs"]
mod tests;
