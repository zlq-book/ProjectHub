use super::*;
use std::{fs, process::Command};

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new(super::super::executable().unwrap())
        .current_dir(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "测试作者")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "测试作者")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().into()
}
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "--initial-branch=main"]);
    fs::write(dir.path().join("中文 空格.txt"), "first\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &["-c", "commit.gpgsign=false", "commit", "-m", "根提交"],
    );
    dir
}

#[test]
fn v14_filters_are_not_executed_and_text_literals_are_not_binary() {
    let dir = repo();
    fs::write(
        dir.path().join("literal.txt"),
        "Binary files are text\nGIT binary patch\n",
    )
    .unwrap();
    let preview = diff(dir.path(), None, "untracked", "literal.txt").unwrap();
    assert!(!preview.binary);
    assert!(preview.text.contains("GIT binary patch"));
    git(
        dir.path(),
        &[
            "config",
            "filter.danger.clean",
            "projecthub-must-not-execute",
        ],
    );
    assert!(
        status(dir.path()).unwrap().complete,
        "未应用的全局过滤器不影响普通仓库"
    );
    fs::write(dir.path().join(".gitattributes"), "*.txt filter=danger\n").unwrap();
    let snapshot = status(dir.path()).unwrap();
    assert!(!snapshot.complete);
    assert!(snapshot.warning.unwrap().contains("过滤器"));
    assert!(snapshot.latest_commit_at.is_some());
    assert!(history(dir.path(), "filter", None).is_ok());
    assert!(diff(dir.path(), None, "worktree", "中文 空格.txt")
        .unwrap_err()
        .contains("过滤器"));
}

#[test]
fn v14_shallow_boundary_preserves_real_parent_and_merge_uses_first_parent() {
    let dir = repo();
    let root = git(dir.path(), &["rev-parse", "HEAD"]);
    git(dir.path(), &["switch", "-c", "feature"]);
    fs::write(dir.path().join("feature.txt"), "feature\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "feature"]);
    git(dir.path(), &["switch", "main"]);
    fs::write(dir.path().join("main.txt"), "main\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "main"]);
    let first = git(dir.path(), &["rev-parse", "HEAD"]);
    git(dir.path(), &["merge", "--no-ff", "feature", "-m", "merge"]);
    let sha = git(dir.path(), &["rev-parse", "HEAD"]);
    let files = commit_files(dir.path(), &sha).unwrap();
    assert_eq!(files.first_parent.as_deref(), Some(first.as_str()));
    assert_eq!(files.files.len(), 1);
    assert_eq!(files.files[0].path, "feature.txt");
    fs::write(dir.path().join(".git/shallow"), format!("{sha}\n")).unwrap();
    let files = commit_files(dir.path(), &sha).unwrap();
    assert_eq!(files.first_parent.as_deref(), Some(first.as_str()));
    assert_eq!(files.files.len(), 1);
    assert_ne!(files.first_parent.as_deref(), Some(root.as_str()));
}

#[test]
fn v14_real_status_retains_mm_and_literal_paths() {
    let dir = repo();
    fs::write(dir.path().join("中文 空格.txt"), "staged\n").unwrap();
    git(dir.path(), &["add", "."]);
    fs::write(dir.path().join("中文 空格.txt"), "worktree\n").unwrap();
    fs::write(dir.path().join("[literal].txt"), "new\n").unwrap();
    let value = status(dir.path()).unwrap();
    assert!(value.repository && value.complete);
    assert_eq!(value.files.len(), 2);
    let file = value
        .files
        .iter()
        .find(|file| file.path == "中文 空格.txt")
        .unwrap();
    assert_eq!((&*file.index_status, &*file.worktree_status), ("M", "M"));
    assert!(value
        .files
        .iter()
        .any(|file| file.path == "[literal].txt" && file.untracked));
}
#[test]
fn v14_root_commit_files_diff_and_activity() {
    let dir = repo();
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    let log = history(dir.path(), "project", None).unwrap();
    assert_eq!(log.head.as_deref(), Some(head.as_str()));
    assert_eq!(log.commits.len(), 1);
    let files = commit_files(dir.path(), &head).unwrap();
    assert!(files.first_parent.is_none());
    assert_eq!(files.files[0].additions, Some(1));
    let patch = diff(dir.path(), Some(&head), "worktree", "中文 空格.txt").unwrap();
    assert!(patch.text.contains("+first"));
    let value = activity(dir.path()).unwrap();
    assert!(value.complete);
    assert_eq!(value.timestamps.len(), 1);
}
#[test]
fn v14_empty_repository_and_plain_directory_are_distinct() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!status(dir.path()).unwrap().repository);
    git(dir.path(), &["init", "--initial-branch=empty"]);
    assert!(status(dir.path()).unwrap().head.is_none());
    assert!(history(dir.path(), "empty", None)
        .unwrap()
        .commits
        .is_empty());
}

#[test]
fn v14_pagination_rejects_cross_project_and_moved_head() {
    let dir = repo();
    for i in 0..31 {
        git(
            dir.path(),
            &[
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                &format!("page {i}"),
            ],
        );
    }
    let first = history(dir.path(), "p1", None).unwrap();
    assert_eq!(first.commits.len(), 30);
    let cursor = first.next_cursor.unwrap();
    assert!(history(dir.path(), "p2", Some(&cursor)).is_err());
    assert_eq!(
        history(dir.path(), "p1", Some(&cursor))
            .unwrap()
            .commits
            .len(),
        2
    );
    git(dir.path(), &["switch", "-c", "same-head-other-branch"]);
    assert!(
        history(dir.path(), "p1", Some(&cursor)).is_err(),
        "切分支即使 HEAD 相同也应使旧游标失效"
    );
    git(dir.path(), &["switch", "main"]);
    git(
        dir.path(),
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "moved",
        ],
    );
    assert!(history(dir.path(), "p1", Some(&cursor)).is_err());
}

#[test]
fn v14_rename_binary_and_untracked_preview() {
    let dir = repo();
    git(dir.path(), &["mv", "中文 空格.txt", "renamed.txt"]);
    let state = status(dir.path()).unwrap();
    assert_eq!(state.files[0].old_path.as_deref(), Some("中文 空格.txt"));
    fs::write(dir.path().join("binary.dat"), [0_u8, 1, 2, 3]).unwrap();
    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "rename and binary",
        ],
    );
    let sha = git(dir.path(), &["rev-parse", "HEAD"]);
    let files = commit_files(dir.path(), &sha).unwrap();
    assert!(files
        .files
        .iter()
        .any(|f| f.status == "R" && f.old_path.as_deref() == Some("中文 空格.txt")));
    let binary = files.files.iter().find(|f| f.path == "binary.dat").unwrap();
    assert!(binary.binary);
    assert!(binary.additions.is_none());
    assert!(
        diff(dir.path(), Some(&sha), "worktree", "binary.dat")
            .unwrap()
            .binary
    );
    fs::write(dir.path().join("[literal].txt"), "preview\n").unwrap();
    assert_eq!(
        diff(dir.path(), None, "untracked", "[literal].txt")
            .unwrap()
            .text,
        "preview\n"
    );
    assert!(diff(dir.path(), None, "untracked", "../outside").is_err());
    assert!(commit_files(dir.path(), "HEAD~1").is_err());
}

#[test]
fn v14_partial_clone_is_refused_before_object_reads() {
    let dir = repo();
    git(dir.path(), &["config", "remote.origin.promisor", "true"]);
    let error = history(dir.path(), "partial", None).unwrap_err();
    assert!(error.contains("部分克隆"), "{error}");
}

#[test]
fn v14_runner_caps_output_and_rejects_expired_deadline() {
    let dir = repo();
    let exe = super::super::executable().unwrap();
    let output = runner::run(
        &exe,
        dir.path(),
        &["log", "--format=%B", "HEAD"],
        Instant::now() + Duration::from_secs(5),
        4,
    )
    .unwrap();
    assert!(output.truncated);
    assert!(output.bytes.len() <= 4);
    assert!(runner::run(
        &exe,
        dir.path(),
        &["status"],
        Instant::now() - Duration::from_millis(1),
        1024
    )
    .is_err());
}

#[test]
fn v14_utc_transport_uses_commit_time_and_all_authors() {
    let dir = repo();
    let output = Command::new(super::super::executable().unwrap())
        .current_dir(dir.path())
        .args([
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "time",
        ])
        .env("GIT_AUTHOR_NAME", "另一作者")
        .env("GIT_AUTHOR_EMAIL", "other@example.invalid")
        .env("GIT_COMMITTER_NAME", "另一作者")
        .env("GIT_COMMITTER_EMAIL", "other@example.invalid")
        .env("GIT_AUTHOR_DATE", "2025-01-01T00:00:00-08:00")
        .env("GIT_COMMITTER_DATE", "2026-10-16T01:02:03+08:00")
        .output()
        .unwrap();
    assert!(output.status.success());
    let log = history(dir.path(), "time", None).unwrap();
    assert_eq!(log.commits[0].commit_time, "2026-10-15T17:02:03+00:00");
    assert_eq!(log.commits[0].author_time, "2025-01-01T08:00:00+00:00");
    let activity = activity(dir.path()).unwrap();
    assert_eq!(activity.timestamps.len(), 2);
    assert!(activity
        .timestamps
        .iter()
        .any(|time| time == "2026-10-15T17:02:03+00:00"));
}

#[test]
fn v14_diff_disables_external_drivers_and_truncates_preview_lines() {
    let dir = repo();
    git(
        dir.path(),
        &[
            "config",
            "diff.external",
            "projecthub-command-must-not-execute",
        ],
    );
    git(
        dir.path(),
        &[
            "config",
            "diff.test.textconv",
            "projecthub-command-must-not-execute",
        ],
    );
    fs::write(dir.path().join(".gitattributes"), "*.txt diff=test\n").unwrap();
    fs::write(dir.path().join("中文 空格.txt"), "safe\n").unwrap();
    assert!(diff(dir.path(), None, "worktree", "中文 空格.txt")
        .unwrap()
        .text
        .contains("+safe"));
    fs::write(dir.path().join("large.txt"), "line\n".repeat(6000)).unwrap();
    let value = diff(dir.path(), None, "untracked", "large.txt").unwrap();
    assert!(value.truncated);
    assert_eq!(value.text.lines().count(), 5000);
}
