// 开发记录以 SQLite 短事务串行写入；提交校验不占用全应用写任务锁。
async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|_| "开发记录后台任务失败")?
}
use crate::db::{
    development::{DevelopmentRecords, DevelopmentSpace, LogInput, TaskInput},
    Database,
};
use tauri::State;

#[tauri::command]
pub async fn development_spaces(db: State<'_, Database>) -> Result<Vec<DevelopmentSpace>, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_spaces()
    })
    .await
}
#[tauri::command]
pub async fn development_space(
    db: State<'_, Database>,
    project_id: String,
) -> Result<DevelopmentSpace, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_space(&project_id)
    })
    .await
}
#[tauri::command]
pub async fn development_records(
    db: State<'_, Database>,
    space_id: String,
) -> Result<DevelopmentRecords, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_records(&space_id)
    })
    .await
}
#[tauri::command]
pub async fn development_save_log(
    db: State<'_, Database>,
    input: LogInput,
) -> Result<DevelopmentRecords, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_save_log(input)
    })
    .await
}
#[tauri::command]
pub async fn development_save_task(
    db: State<'_, Database>,
    input: TaskInput,
) -> Result<DevelopmentRecords, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_save_task(input)
    })
    .await
}
#[tauri::command]
pub async fn development_delete_record(
    db: State<'_, Database>,
    space_id: String,
    id: String,
    kind: String,
    revision: i64,
) -> Result<DevelopmentRecords, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_delete_record(&space_id, &id, &kind, revision)
    })
    .await
}
#[tauri::command]
pub async fn development_rebind(
    db: State<'_, Database>,
    space_id: String,
    project_id: String,
) -> Result<DevelopmentSpace, String> {
    let db = db.inner().for_development_request();
    let expected = db.development_request_epoch();
    blocking(move || {
        if db.development_epoch() != expected {
            return Err("项目库已恢复，请重新载入".into());
        }
        db.development_rebind(&space_id, &project_id)
    })
    .await
}
