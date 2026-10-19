mod models;
mod services;
mod db;
mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            let db = db::Database::new(directory.join("projecthub.sqlite"))
                .map_err(std::io::Error::other)?;
            app.manage(db);
            app.manage(services::discovery::Scanner::default());
            app.manage(services::ai::Store::new(directory.join("ai-settings.json")));
            app.manage(services::assistant::Engine::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::validate_project_path, commands::detect_project,
            commands::create_project, commands::update_project, commands::list_projects,
            commands::get_project, commands::remove_project, commands::open_project,
            commands::get_project_git,
            commands::development_records::development_spaces, commands::development_records::development_space,
            commands::development_records::development_records, commands::development_records::development_save_log,
            commands::development_records::development_save_task, commands::development_records::development_delete_record,
            commands::development_records::development_rebind,
            commands::development_summary::development_summary_cancel, commands::development_summary::development_summary_preview, commands::development_summary::development_summary_generate,
            commands::development_git::development_git_status, commands::development_git::development_git_history,
            commands::development_git::development_git_files, commands::development_git::development_git_diff,
            commands::development_git::development_git_activity,
            commands::weekly_report::weekly_report_authors, commands::weekly_report::weekly_report_preferences, commands::weekly_report::weekly_report_save_preferences,
            commands::weekly_report::weekly_report_collect, commands::weekly_report::weekly_report_cancel_collection,
            commands::weekly_report::weekly_report_preview, commands::weekly_report::weekly_report_generate, commands::weekly_report::weekly_report_cancel,
            commands::weekly_report::weekly_report_list, commands::weekly_report::weekly_report_get, commands::weekly_report::weekly_report_save_version,
            commands::weekly_report::weekly_report_adopt_version, commands::weekly_report::weekly_report_export,
            commands::get_ai_settings, commands::save_ai_settings, commands::analyze_project,
            commands::list_ai_models, commands::test_ai_connection,
            commands::assistant_run, commands::apply_assistant_proposal,
            commands::cancel_assistant, commands::clear_assistant,
            commands::open_in_explorer, commands::open_terminal, commands::open_readme,
            commands::choose_project_folder, commands::get_settings, commands::save_settings,
            commands::database_path, commands::export_backup, commands::import_backup,
            commands::scan_projects, commands::cancel_scan, commands::import_projects,
            commands::batch_update_projects, commands::batch_remove_projects,
            commands::change_project_tag,
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 ProjectHub，请检查应用数据目录权限");
}
