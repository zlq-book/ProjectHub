pub mod development_records;
pub mod development_summary;
pub mod development_git;
pub mod weekly_report;
use crate::{db::Database, models::{Detection, ImportResult, Project, ProjectInput, ScanResult, Settings}, services};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn get_ai_settings(store: State<'_, services::ai::Store>) -> Result<services::ai::config::PublicConfig, String> { store.public() }
#[tauri::command]
pub async fn save_ai_settings(store: State<'_, services::ai::Store>, assistant: State<'_, services::assistant::Engine>, input: services::ai::config::ConfigInput) -> Result<services::ai::config::PublicConfig, String> { assistant.mutate(|| { let result=store.save(input)?; services::development_summary::invalidate(); services::weekly_report::generation::invalidate(); Ok(result) }) }

#[tauri::command]
pub async fn assistant_run(db: State<'_, Database>, store: State<'_, services::ai::Store>, assistant: State<'_, services::assistant::Engine>, input: services::assistant::protocol::Request) -> Result<services::assistant::protocol::ResultData, String> {
    let ticket = assistant.prepare(&input)?;
    let db = db.inner().clone(); let store = store.inner().clone(); let assistant = assistant.inner().clone();
    tauri::async_runtime::spawn_blocking(move || assistant.run_prepared(&db, &store, input, ticket)).await.map_err(|_| "助手任务执行失败，请重试".to_string())?
}
#[tauri::command]
pub async fn apply_assistant_proposal(db: State<'_, Database>, assistant: State<'_, services::assistant::Engine>, input: services::assistant::protocol::ApplyRequest) -> Result<Vec<Project>, String> {
    let db = db.inner().clone(); let assistant = assistant.inner().clone(); blocking(move || assistant.apply(&db, input)).await
}
#[tauri::command]
pub fn cancel_assistant(assistant: State<'_, services::assistant::Engine>, session_id: String, request_id: String) -> Result<(), String> { assistant.cancel(&session_id, &request_id) }
#[tauri::command]
pub fn clear_assistant(assistant: State<'_, services::assistant::Engine>, session_id: String) -> Result<(), String> { assistant.clear(&session_id) }
#[tauri::command]
pub async fn list_ai_models(store: State<'_, services::ai::Store>, input: services::ai::config::ConfigInput) -> Result<Vec<String>, String> {
    let config = store.preview(input)?;
    tauri::async_runtime::spawn_blocking(move || services::ai::connection::models(config)).await.map_err(|_| "获取模型列表失败".to_string())?
}
#[tauri::command]
pub async fn test_ai_connection(store: State<'_, services::ai::Store>, input: services::ai::config::ConfigInput) -> Result<services::ai::connection::TestResult, String> {
    let config = store.preview(input)?;
    tauri::async_runtime::spawn_blocking(move || services::ai::connection::test(config)).await.map_err(|_| "连接测试失败".to_string())?
}
#[tauri::command]
pub async fn analyze_project(db: State<'_, Database>, store: State<'_, services::ai::Store>, path: String, id: Option<String>) -> Result<services::ai::Analysis, String> {
    let db = db.inner().clone(); let config = store.load()?;
    // 网络等待不占用数据库本地操作锁，避免阻塞其他项目操作。
    tauri::async_runtime::spawn_blocking(move || {
        let path = db.validate_path(&path, id.as_deref())?;
        let (directory, _) = services::paths::validate(&path)?;
        services::ai::analyze(&directory, config)
    }).await.map_err(|_| "AI 解析任务失败，请重试".to_string())?
}

#[tauri::command]
pub async fn change_project_tag(db: State<'_, Database>, ids: Vec<String>, source: String, target: Option<String>) -> Result<Vec<Project>, String> {
    let db = db.inner().clone(); blocking(move || db.change_tag(ids, source, target)).await
}

async fn blocking<T: Send + 'static>(task: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    // 同一应用内串行化本地任务，尤其避免导入校验/自动备份/恢复期间插入写操作。
    static OPERATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = OPERATIONS.lock().map_err(|_| "本地任务锁异常，请重新启动软件".to_string())?;
        task()
    }).await.map_err(|e| format!("本地任务执行失败：{}", e))?
}
async fn choose_file<T: Send + 'static>(task: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    // 等待用户选择时不占用数据库操作锁。
    tauri::async_runtime::spawn_blocking(task).await.map_err(|e| format!("文件选择失败：{}", e))?
}
#[tauri::command]
pub async fn validate_project_path(db: State<'_, Database>, path: String) -> Result<String, String> { let db = db.inner().clone(); blocking(move || db.validate_path(&path, None)).await }
#[tauri::command]
pub async fn detect_project(db: State<'_, Database>, path: String, id: Option<String>) -> Result<Detection, String> {
    let db = db.inner().clone(); blocking(move || { let path = db.validate_path(&path, id.as_deref())?; services::detection::detect(&path) }).await
}
#[tauri::command]
pub async fn create_project(db: State<'_, Database>, input: ProjectInput) -> Result<Project, String> { let db = db.inner().clone(); blocking(move || db.create(input)).await }
#[tauri::command]
pub async fn update_project(db: State<'_, Database>, id: String, input: ProjectInput) -> Result<Project, String> { let db = db.inner().clone(); blocking(move || db.update(&id, input)).await }
#[tauri::command]
pub async fn list_projects(db: State<'_, Database>) -> Result<Vec<Project>, String> { let db = db.inner().clone(); blocking(move || db.list()).await }
#[tauri::command]
pub async fn get_project(db: State<'_, Database>, id: String) -> Result<Project, String> { let db = db.inner().clone(); blocking(move || db.get(&id)).await }
#[tauri::command]
pub async fn get_project_git(db: State<'_, Database>, id: String) -> Result<services::git::GitInfo, String> {
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = db.get(&id)?;
        services::git::inspect(std::path::Path::new(&project.input.path))
    }).await.map_err(|e| format!("Git 信息读取失败：{}", e))?
}
#[tauri::command]
pub async fn remove_project(db: State<'_, Database>, id: String) -> Result<(), String> { let db = db.inner().clone(); blocking(move || db.remove(&id)).await }
#[tauri::command]
pub async fn batch_update_projects(db: State<'_, Database>, ids: Vec<String>, favorite: Option<bool>, category: Option<String>) -> Result<Vec<Project>, String> {
    let db = db.inner().clone(); blocking(move || db.batch_update(ids, favorite, category)).await
}
#[tauri::command]
pub async fn batch_remove_projects(db: State<'_, Database>, ids: Vec<String>) -> Result<(), String> {
    let db = db.inner().clone(); blocking(move || db.batch_remove(ids)).await
}
#[tauri::command]
pub async fn import_projects(db: State<'_, Database>, assistant: State<'_, services::assistant::Engine>, inputs: Vec<ProjectInput>) -> Result<ImportResult, String> {
    let db = db.inner().clone(); let assistant = assistant.inner().clone(); blocking(move || assistant.mutate(|| db.import_projects(inputs))).await
}
#[tauri::command]
pub async fn scan_projects(db: State<'_, Database>, scanner: State<'_, services::discovery::Scanner>, roots: Vec<String>, depth: usize, request_id: String) -> Result<ScanResult, String> {
    let db = db.inner().clone(); let scanner = scanner.inner().clone();
    let lease = scanner.lease(&request_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease; // 完整持有 RAII 令牌，工作结束或异常时释放扫描状态。
        let known = db.known_paths()?;
        services::discovery::scan(roots, depth, &_lease.token, &known)
    }).await.map_err(|e| format!("扫描任务失败：{}", e))?
}
#[tauri::command]
pub fn cancel_scan(scanner: State<'_, services::discovery::Scanner>, request_id: String) -> Result<(), String> { scanner.cancel(&request_id) }
#[tauri::command]
pub async fn open_project(db: State<'_, Database>, id: String, ide: String) -> Result<(), String> { let db = db.inner().clone(); blocking(move || services::launcher::open(&db, &id, &ide)).await }
#[tauri::command]
pub async fn open_in_explorer(db: State<'_, Database>, id: String) -> Result<(), String> { let db = db.inner().clone(); blocking(move || services::launcher::open(&db, &id, "explorer")).await }
#[tauri::command]
pub async fn open_terminal(db: State<'_, Database>, id: String) -> Result<(), String> { let db = db.inner().clone(); blocking(move || services::launcher::open(&db, &id, "terminal")).await }
#[tauri::command]
pub async fn open_readme(db: State<'_, Database>, id: String) -> Result<(), String> { let db = db.inner().clone(); blocking(move || services::launcher::open(&db, &id, "readme")).await }
#[tauri::command]
pub async fn choose_project_folder(app: AppHandle) -> Result<Option<String>, String> {
    choose_file(move || app.dialog().file().set_title("选择本地项目文件夹").blocking_pick_folder()
        .map(|file| file.into_path().map(|p| p.to_string_lossy().into_owned()).map_err(|e| e.to_string())).transpose()).await
}
#[tauri::command]
pub async fn get_settings(db: State<'_, Database>) -> Result<Settings, String> { let db = db.inner().clone(); blocking(move || db.settings()).await }
#[tauri::command]
pub async fn save_settings(db: State<'_, Database>, settings: Settings) -> Result<(), String> { let db = db.inner().clone(); blocking(move || db.save_settings(settings)).await }
#[tauri::command]
pub fn database_path(db: State<'_, Database>) -> String { db.path.to_string_lossy().into_owned() }
#[tauri::command]
pub async fn export_backup(app: AppHandle, db: State<'_, Database>) -> Result<Option<String>, String> {
    let db = db.inner().clone();
    let file = choose_file(move || Ok(app.dialog().file().set_title("导出项目库备份").set_file_name("projecthub-backup.sqlite").add_filter("SQLite 数据库", &["sqlite"]).blocking_save_file())).await?;
    if let Some(file) = file {
        let path = file.into_path().map_err(|e| e.to_string())?;
        blocking(move || { db.export(&path)?; Ok(Some(format!("备份已导出至 {}", path.display()))) }).await
    } else { Ok(None) }
}
#[tauri::command]
pub async fn import_backup(app: AppHandle, db: State<'_, Database>, assistant: State<'_, services::assistant::Engine>) -> Result<Option<String>, String> {
    let db = db.inner().clone(); let assistant = assistant.inner().clone();
    let file = choose_file(move || Ok(app.dialog().file().set_title("导入项目库备份").add_filter("SQLite 数据库", &["sqlite", "db"]).blocking_pick_file())).await?;
    if let Some(file) = file {
        let path = file.into_path().map_err(|e| e.to_string())?;
        blocking(move || assistant.mutate(|| { let result=db.import(&path)?; services::git::invalidate_cache(); services::development_summary::invalidate(); services::weekly_report::generation::invalidate(); Ok(Some(result)) })).await
    } else { Ok(None) }
}

