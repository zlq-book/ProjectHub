use super::protocol::*;
use crate::{db::Database, services::git};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Snapshot {
    pub value: Collection,
    pub epoch: u64,
    pub created: Instant,
}
fn items() -> &'static Mutex<HashMap<String, Snapshot>> {
    static ITEMS: OnceLock<Mutex<HashMap<String, Snapshot>>> = OnceLock::new();
    ITEMS.get_or_init(Mutex::default)
}
fn cancelled() -> &'static Mutex<HashMap<String, Instant>> {
    static ITEMS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    ITEMS.get_or_init(Mutex::default)
}
pub fn cancel(id: &str) -> Result<(), String> {
    let mut c = cancelled().lock().map_err(|_| "取消锁异常")?;
    c.retain(|_, t| t.elapsed() < Duration::from_secs(600));
    if c.len() >= 256 {
        return Err("取消请求过多，请稍后重试".into());
    }
    c.insert(id.into(), Instant::now());
    Ok(())
}
pub fn check_cancel(id: &str) -> Result<(), String> {
    if cancelled()
        .lock()
        .map_err(|_| "取消锁异常")?
        .contains_key(id)
    {
        Err("本次操作已取消".into())
    } else {
        Ok(())
    }
}
pub fn get(id: &str) -> Result<Snapshot, String> {
    let item = items()
        .lock()
        .map_err(|_| "材料锁异常")?
        .get(id)
        .cloned()
        .ok_or("材料已失效，请重新汇总")?;
    if item.created.elapsed() > Duration::from_secs(600) {
        return Err("材料已过期，请重新汇总".into());
    }
    check_cancel(id)?;
    Ok(item)
}
pub fn verify(db: &Database, s: &Snapshot) -> Result<(), String> {
    if s.epoch != db.development_epoch() {
        return Err("项目库已恢复，请重新汇总".into());
    }
    for p in &s.value.projects {
        let current = db.get(&p.project_id)?;
        if current.input.path != p.path {
            return Err("项目路径已变化，请重新汇总".into());
        }
        if p.repository_id.is_some() {
            let status = git::status(Path::new(&p.path))?;
            if status.head != p.head || status.repository_id != p.repository_id {
                return Err("仓库 HEAD 已变化，请重新汇总".into());
            }
        }
    }
    let spaces = db.development_spaces()?;
    for source in s.value.sources.iter().filter(|s| s.kind == "log") {
        let space = spaces
            .iter()
            .find(|sp| {
                sp.project_id
                    .as_ref()
                    .is_some_and(|id| source.project_ids.contains(id))
            })
            .ok_or("日志所属项目已变化")?;
        let records = db.development_records(&space.id)?;
        let log = records
            .logs
            .iter()
            .find(|l| Some(&l.id) == source.log_id.as_ref())
            .ok_or("日志已删除，请重新汇总")?;
        if Some(log.revision) != source.revision || log.body != source.body {
            return Err("开发日志已变化，请重新汇总".into());
        }
    }
    Ok(())
}
pub fn collect(db: &Database, input: CollectInput) -> Result<Collection, String> {
    input.range.validate()?;
    Preferences {
        authors: input.authors.clone(),
        ..Default::default()
    }
    .validate()?;
    if input.project_ids.is_empty()
        || input.project_ids.len() > 20
        || input.request_id.len() > 100
        || input.request_id.is_empty()
    {
        return Err("请选择 1–20 个项目".into());
    }
    let epoch = db.development_epoch();
    let mut ids = HashSet::new();
    if input.project_ids.iter().any(|id| !ids.insert(id)) {
        return Err("项目选择重复".into());
    }
    let mut value = Collection {
        id: input.request_id.clone(),
        range: input.range.clone(),
        authors: input.authors.clone(),
        projects: vec![],
        sources: vec![],
        warnings: vec![],
    };
    let spaces = db.development_spaces()?;
    for id in &input.project_ids {
        check_cancel(&input.request_id)?;
        let project = db.get(id)?;
        let mut evidence = ProjectEvidence {
            project_id: id.clone(),
            project_name: project.input.name.clone(),
            path: project.input.path.clone(),
            repository_id: None,
            head: None,
            branch: None,
            complete: true,
            warning: None,
        };
        let path = Path::new(&project.input.path);
        match git::status(path) {
            Ok(status) => {
                evidence.repository_id = status.repository_id;
                evidence.head = status.head;
                evidence.branch = status.branch;
                if status.repository {
                    if input.authors.is_empty() {
                        evidence.complete = false;
                        evidence.warning = Some("请配置并确认本人 Git 姓名／邮箱".into())
                    } else {
                        match git::weekly_report::commits(
                            path,
                            id,
                            &project.input.name,
                            &input.range,
                            &input.authors,
                            evidence.head.as_deref(),
                            || check_cancel(&input.request_id),
                        ) {
                            Ok((sources, complete)) => {
                                evidence.complete = complete;
                                if !complete {
                                    evidence.warning=Some("浅仓库或提交达到上限，范围不完整，请准备完整仓库或缩小范围".into())
                                }
                                for mut s in sources {
                                    if let Some(existing) =
                                        value.sources.iter_mut().find(|v| v.id == s.id)
                                    {
                                        existing.project_ids.append(&mut s.project_ids)
                                    } else {
                                        value.sources.push(s)
                                    }
                                }
                            }
                            Err(e) => {
                                evidence.complete = false;
                                evidence.warning = Some(e)
                            }
                        }
                    }
                }
            }
            Err(e) => {
                evidence.warning = Some(format!("Git 不可用：{e}；已有本地日志仍可选择"));
            }
        }
        if let Some(space) = spaces.iter().find(|s| s.project_id.as_ref() == Some(id)) {
            for log in db.development_records(&space.id)?.logs {
                if log.date >= input.range.start_date && log.date <= input.range.end_date {
                    if value.sources.iter().filter(|s| s.kind == "log").count() >= 200 {
                        return Err("日志超过 200 条，请缩小范围".into());
                    }
                    value.sources.push(ReportSource {
                        id: format!("log:{}", log.id),
                        kind: "log".into(),
                        project_ids: vec![id.clone()],
                        project_name: project.input.name.clone(),
                        title: log.title,
                        body: log.body,
                        log_id: Some(log.id),
                        log_date: log.date,
                        log_commits: log.commits,
                        revision: Some(log.revision),
                        ..Default::default()
                    });
                }
            }
        }
        if let Some(w) = &evidence.warning {
            value
                .warnings
                .push(format!("{}：{w}", evidence.project_name))
        }
        value.projects.push(evidence);
    }
    check_cancel(&input.request_id)?;
    if epoch != db.development_epoch() {
        return Err("项目库已变化，请重新汇总".into());
    }
    let mut cache = items().lock().map_err(|_| "材料锁异常")?;
    cache.retain(|_, s| s.created.elapsed() < Duration::from_secs(600));
    if cache.len() >= 16 {
        return Err("准备中的周报过多，请稍后重试".into());
    }
    cache.insert(
        value.id.clone(),
        Snapshot {
            value: value.clone(),
            epoch,
            created: Instant::now(),
        },
    );
    Ok(value)
}
