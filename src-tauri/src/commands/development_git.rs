use crate::{db::Database, services::git};
use std::path::Path;
use tauri::State;

async fn read<T: Send + 'static>(
    db: &Database,
    project_id: String,
    task: impl FnOnce(&Path, &str) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let db = db.clone();
    // 仅短暂打开数据库读取项目路径，Git 子进程不占用应用的本地写任务锁。
    tauri::async_runtime::spawn_blocking(move || {
        let project = db.get(&project_id)?;
        task(Path::new(&project.input.path), &project_id)
    })
    .await
    .map_err(|error| format!("Git 后台读取失败：{error}"))?
}
#[tauri::command]
pub async fn development_git_status(
    db: State<'_, Database>,
    project_id: String, refresh:Option<bool>,
) -> Result<git::StatusSnapshot, String> {
    read(db.inner(), project_id, move |path, _| git::status_cached(path,refresh.unwrap_or(false))).await
}
#[tauri::command]
pub async fn development_git_history(
    db: State<'_, Database>,
    project_id: String,
    cursor: Option<String>,
) -> Result<git::History, String> {
    read(db.inner(), project_id, move |path, id| {
        git::history(path, id, cursor.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn development_git_files(
    db: State<'_, Database>,
    project_id: String,
    sha: String,
) -> Result<git::CommitFiles, String> {
    read(db.inner(), project_id, move |path, _| {
        git::commit_files(path, &sha)
    })
    .await
}
#[tauri::command]
pub async fn development_git_diff(
    db: State<'_, Database>,
    project_id: String,
    sha: Option<String>,
    stage: String,
    file_path: String,
) -> Result<git::Diff, String> {
    read(db.inner(), project_id, move |path, _| {
        git::diff(path, sha.as_deref(), &stage, &file_path)
    })
    .await
}
#[tauri::command]
pub async fn development_git_activity(
    db: State<'_, Database>,
    project_id: String,
) -> Result<git::Activity, String> {
    read(db.inner(), project_id, |path, _| git::activity(path)).await
}
