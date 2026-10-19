use crate::{
    db::Database,
    services::{
        ai::Store,
        weekly_report::{collection, export, generation, protocol::*},
    },
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
async fn run<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|_| "周报后台任务失败")?
}
#[tauri::command]
pub async fn weekly_report_authors(
    db: State<'_, Database>,
    project_ids: Vec<String>,
) -> Result<Vec<AuthorRule>, String> {
    let db = db.inner().clone();
    run(move || {
        if project_ids.is_empty() || project_ids.len() > 20 {
            return Err("请选择 1–20 个项目".into());
        }
        let mut result: Vec<AuthorRule> = vec![];
        for id in project_ids {
            let p = db.get(&id)?;
            for a in
                crate::services::git::weekly_report::authors(std::path::Path::new(&p.input.path))?
            {
                if !result
                    .iter()
                    .any(|v| v.kind == a.kind && v.value == a.value)
                {
                    result.push(a);
                }
            }
        }
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn weekly_report_preferences(db: State<'_, Database>) -> Result<Preferences, String> {
    let db = db.inner().clone();
    run(move || db.weekly_preferences()).await
}
#[tauri::command]
pub async fn weekly_report_save_preferences(
    db: State<'_, Database>,
    input: Preferences,
) -> Result<(), String> {
    let db = db.for_development_request();
    run(move || db.weekly_save_preferences(input)).await
}
#[tauri::command]
pub async fn weekly_report_collect(
    db: State<'_, Database>,
    input: CollectInput,
) -> Result<Collection, String> {
    let db = db.for_development_request();
    run(move || collection::collect(&db, input)).await
}
#[tauri::command]
pub fn weekly_report_cancel_collection(id: String) -> Result<(), String> {
    collection::cancel(&id)
}
#[tauri::command]
pub async fn weekly_report_preview(
    db: State<'_, Database>,
    store: State<'_, Store>,
    input: PreviewInput,
) -> Result<ReportPreview, String> {
    let db = db.inner().clone();
    let store = store.inner().clone();
    run(move || generation::preview(&db, &store, input)).await
}
#[tauri::command]
pub async fn weekly_report_generate(
    db: State<'_, Database>,
    store: State<'_, Store>,
    token: String,
) -> Result<GeneratedDraft, String> {
    let db = db.inner().clone();
    let store = store.inner().clone();
    run(move || generation::generate(&db, &store, &token)).await
}
#[tauri::command]
pub fn weekly_report_cancel(token: String) -> Result<(), String> {
    generation::cancel(&token)
}
#[tauri::command]
pub async fn weekly_report_list(db: State<'_, Database>) -> Result<Vec<ReportGroup>, String> {
    let db = db.inner().clone();
    run(move || db.weekly_list()).await
}
#[tauri::command]
pub async fn weekly_report_get(db: State<'_, Database>, id: String) -> Result<ReportGroup, String> {
    let db = db.inner().clone();
    run(move || db.weekly_get(&id)).await
}
#[tauri::command]
pub async fn weekly_report_save_version(
    db: State<'_, Database>,
    input: SaveInput,
) -> Result<ReportGroup, String> {
    let db = db.for_development_request();
    run(move || db.weekly_save(input)).await
}
#[tauri::command]
pub async fn weekly_report_adopt_version(
    db: State<'_, Database>,
    report_id: String,
    version_id: String,
) -> Result<ReportGroup, String> {
    let db = db.for_development_request();
    run(move || db.weekly_adopt(&report_id, &version_id)).await
}
#[tauri::command]
pub async fn weekly_report_export(
    app: AppHandle,
    db: State<'_, Database>,
    id: String,
    revision: i64,
    format: String,
) -> Result<serde_json::Value, String> {
    if !matches!(format.as_str(), "xlsx" | "md") {
        return Err("不支持的周报导出格式".into());
    }
    let db = db.inner().clone();
    let db2 = db.clone();
    let id2 = id.clone();
    let v = run(move || db2.weekly_saved_version(&id2, revision)).await?;
    let epoch = db.development_epoch();
    let filename = export::filename(&v.content, &format);
    let ext = format.clone();
    let file = run(move || {
        Ok(app
            .dialog()
            .file()
            .set_title("导出工作周报")
            .set_file_name(&filename)
            .add_filter("工作周报", &[&ext])
            .blocking_save_file())
    })
    .await?;
    let Some(file) = file else {
        return Ok(serde_json::json!({"status":"cancelled"}));
    };
    let path = file.into_path().map_err(|_| "导出路径无效")?;
    run(move || {
        if db.development_epoch() != epoch {
            return Err("项目库已变化，请重新打开周报".into());
        }
        let v = db.weekly_saved_version(&id, revision)?;
        let bytes = if format == "xlsx" {
            export::xlsx(&v.content)?
        } else {
            export::markdown(&v.content)?.into_bytes()
        };
        let parent = path.parent().ok_or("导出目录无效")?;
        let temporary = parent.join(format!(".projecthub-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| "无法写入导出目录")?;
            f.write_all(&bytes).map_err(|_| "周报写入失败")?;
            f.sync_all().map_err(|_| "周报写入失败")?;
            drop(f);
            std::fs::rename(&temporary, &path)
                .map_err(|_| "无法替换导出文件，请关闭占用它的 Excel／编辑器后重试")?;
            Ok(serde_json::json!({"status":"saved","path":path.to_string_lossy()}))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    })
    .await
}
