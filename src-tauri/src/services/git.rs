use serde::Serialize;
use std::path::Path;
mod development;
mod runner;
pub mod weekly_report;
pub use development::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub repository: bool,
    pub branch: Option<String>,
    pub remote: Option<String>,
}

fn redact_remote(remote: String) -> String {
    if let Some((scheme, rest)) = remote.split_once("://") {
        let end = rest.find('/').unwrap_or(rest.len());
        let authority = rest[..end].rsplit('@').next().unwrap_or("");
        let path = rest[end..].split(['?', '#']).next().unwrap_or("");
        format!("{}://{}{}", scheme, authority, path)
    } else {
        remote
    }
}

// 原有项目详情保持兼容，仓库定位、离线保护和进程限制共享开发工作区的实现。
pub fn inspect(directory: &Path) -> Result<GitInfo, String> {
    let Some(repo) = development::Repository::open(directory, 5)? else {
        return Ok(GitInfo {
            repository: false,
            branch: None,
            remote: None,
        });
    };
    let remote = runner::run(
        &repo.exe,
        &repo.root,
        &["config", "--get", "remote.origin.url"],
        repo.deadline,
        64 * 1024,
    )?;
    if remote.timed_out || remote.truncated {
        return Err("Git 远程信息读取超时或超限".into());
    }
    let remote = if remote.success {
        Some(
            String::from_utf8(remote.bytes)
                .map_err(|_| "Git 远程地址不是 UTF-8")?
                .trim()
                .to_string(),
        )
        .filter(|value| !value.is_empty())
        .map(redact_remote)
    } else {
        None
    };
    Ok(GitInfo {
        repository: true,
        branch: repo.branch,
        remote,
    })
}

#[cfg(test)]
fn executable() -> Result<std::path::PathBuf, String> {
    runner::executable()
}
#[cfg(test)]
fn query(exe: &Path, directory: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let value = runner::run(
        exe,
        directory,
        args,
        std::time::Instant::now() + std::time::Duration::from_secs(5),
        65536,
    )?;
    if value.timed_out || value.truncated {
        return Err("Git 测试读取超时或超限".into());
    }
    if value.success {
        Ok(Some(
            String::from_utf8(value.bytes)
                .map_err(|_| "Git 测试输出无效")?
                .trim()
                .into(),
        ))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_real_branch_remote_and_plain_directory() {
        let directory = tempfile::tempdir().unwrap();
        assert!(!inspect(directory.path()).unwrap().repository);
        let exe = executable().unwrap();
        assert!(query(
            &exe,
            directory.path(),
            &["init", "--initial-branch=projecthub-test"]
        )
        .unwrap()
        .is_some());
        assert!(query(
            &exe,
            directory.path(),
            &[
                "config",
                "remote.origin.url",
                "https://user:secret@example.com/team/project.git?token=secret"
            ]
        )
        .unwrap()
        .is_some());
        let info = inspect(directory.path()).unwrap();
        assert!(info.repository);
        assert_eq!(info.branch.as_deref(), Some("projecthub-test"));
        assert_eq!(
            info.remote.as_deref(),
            Some("https://example.com/team/project.git")
        );
        assert!(query(
            &exe,
            directory.path(),
            &["config", "--unset", "remote.origin.url"]
        )
        .unwrap()
        .is_some());
        assert!(inspect(directory.path()).unwrap().remote.is_none());
    }
    #[test]
    fn missing_directory_and_remote_redaction() {
        let directory = tempfile::tempdir().unwrap();
        assert!(inspect(&directory.path().join("missing")).is_err());
        assert_eq!(
            redact_remote("git@example.com:team/project.git".into()),
            "git@example.com:team/project.git"
        );
        assert_eq!(
            redact_remote("ssh://git:secret@example.com/team/project.git".into()),
            "ssh://example.com/team/project.git"
        );
    }
}
