use super::{development::Repository, runner};
use crate::services::weekly_report::protocol::{AuthorRule, ReportRange, ReportSource};
use std::{collections::HashSet, path::Path};

pub fn matches(authors: &[AuthorRule], name: &str, email: &str) -> bool {
    authors.iter().any(|a| match a.kind.as_str() {
        "email" => a.value.trim().eq_ignore_ascii_case(email.trim()),
        "name" => a.value.trim() == name.trim(),
        _ => false,
    })
}
pub fn authors(path: &Path) -> Result<Vec<AuthorRule>, String> {
    let Some(repo) = Repository::open(path, 15)? else {
        return Ok(vec![]);
    };
    let Some(head) = repo.head.as_deref() else {
        return Ok(vec![]);
    };
    let out = runner::run(
        &repo.exe,
        &repo.root,
        &[
            "log",
            "--max-count=20000",
            "--format=%an%x00%ae%x00",
            head,
            "--",
        ],
        repo.deadline,
        2 * 1024 * 1024,
    )?;
    if !out.success || out.timed_out || out.truncated {
        return Err("作者候选读取失败或超限，请手动填写".into());
    }
    let text = String::from_utf8(out.bytes).map_err(|_| "作者候选不是 UTF-8")?;
    let parts: Vec<_> = text.split('\0').collect();
    let mut seen = HashSet::new();
    let mut result = vec![];
    for pair in parts.chunks_exact(2) {
        for (kind, value) in [("email", pair[1].trim()), ("name", pair[0].trim())] {
            if !value.is_empty() && seen.insert((kind, value.to_lowercase())) {
                result.push(AuthorRule {
                    kind: kind.into(),
                    value: value.into(),
                });
            }
        }
        if result.len() > 1000 {
            return Err("作者候选超过 1000 个，请手动填写".into());
        }
    }
    Ok(result)
}
pub fn commits(
    path: &Path,
    project_id: &str,
    project_name: &str,
    range: &ReportRange,
    authors: &[AuthorRule],
    expected_head: Option<&str>,
    cancel: impl Fn() -> Result<(), String>,
) -> Result<(Vec<ReportSource>, bool), String> {
    let Some(repo) = Repository::open(path, 30)? else {
        return Ok((vec![], true));
    };
    if expected_head.is_some() && repo.head.as_deref()!=expected_head {return Err("汇总期间仓库 HEAD 发生变化，请重新汇总".into())}
    let Some(head) = repo.head.as_deref() else {
        return Ok((vec![], true));
    };
    let out = runner::run(
        &repo.exe,
        &repo.root,
        &[
            "log",
            "--max-count=20001",
            "--format=%H%x00%an%x00%ae%x00%aI%x00%cI%x00%s%x00%B%x00",
            head,
            "--",
        ],
        repo.deadline,
        8 * 1024 * 1024,
    )?;
    if out.timed_out || out.truncated || !out.success {
        return Err("Git 历史读取失败、超时或超过容量，请缩小范围".into());
    }
    let text = String::from_utf8(out.bytes).map_err(|_| "Git 历史不是 UTF-8")?;
    let parts: Vec<&str> = text.split('\0').collect();
    let mut result = vec![];
    let mut seen = HashSet::new();
    let mut scanned = 0;
    for item in parts.chunks_exact(7) {
        cancel()?;
        scanned += 1;
        if scanned > 20000 {
            return Ok((result, false));
        }
        let sha = item[0].trim();
        if !matches(authors, item[1], item[2]) || !range.contains(item[4])? {
            continue;
        }
        if result.len() >= 500 {
            return Ok((result, false));
        }
        if !seen.insert(sha.to_string()) {
            continue;
        }
        let files = super::commit_files(path, sha)?;
        if !files.complete || files.warning.is_some() {
            return Err("提交文件列表不完整，请缩小范围后重试".into());
        }
        result.push(ReportSource {
            id: format!("{}:{sha}", repo.id),
            kind: "commit".into(),
            project_ids: vec![project_id.into()],
            project_name: project_name.into(),
            title: item[5].into(),
            body: item[6].into(),
            repository_id: Some(repo.id.clone()),
            sha: Some(sha.into()),
            author: item[1].into(),
            author_email: item[2].into(),
            author_time: item[3].into(),
            commit_time: item[4].into(),
            log_id: None,
            log_date: String::new(),
            log_commits: vec![],
            revision: None,
            renames: files
                .files
                .iter()
                .filter_map(|f| f.old_path.as_ref().map(|old| (old.clone(), f.path.clone())))
                .collect(),
            files: files.files.into_iter().map(|f| f.path).collect(),
        });
    }
    Ok((result, !repo.shallow))
}
