use crate::{
    db::Database,
    services::{ai::Store, development_summary as summary},
};
use tauri::State;
#[tauri::command]
pub async fn development_summary_preview(
    db: State<'_, Database>,
    store: State<'_, Store>,
    project_id: String,
    sha: Option<String>,
    stage: String,
    files: Vec<String>,
) -> Result<summary::Preview, String> {
    let db = db.inner().clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        summary::preview(&db, &store, project_id, sha, stage, files)
    })
    .await
    .map_err(|_| "摘要预览任务失败")?
}
#[tauri::command]
pub async fn development_summary_generate(
    db: State<'_, Database>,
    store: State<'_, Store>,
    token: String,
) -> Result<String, String> {
    let db = db.inner().clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || summary::generate(&db, &store, &token))
        .await
        .map_err(|_| "摘要生成任务失败")?
}

#[tauri::command]
pub fn development_summary_cancel(token: String) -> Result<(), String> {
    summary::cancel(&token)
}
