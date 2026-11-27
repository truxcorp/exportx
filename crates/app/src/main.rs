// No console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window and the Tauri commands the screens (ui/app.js) call. All logic is in
//! `exportpro_app::commands` and the core; this file only adapts them to Tauri.

use std::path::PathBuf;

use exportpro_app::commands::{
    self, ApplyOutcome, LogOutcome, PlanRequest, PlanResult, PlannedFile, ProgressLine,
};
use exportpro_core::{Lang, Settings};
use tauri::ipc::Channel;

/// The settings file the CLI uses too.
fn settings_path() -> Result<PathBuf, String> {
    exportpro_core::settings::path().ok_or_else(|| "no config folder".to_owned())
}

#[tauri::command]
fn settings() -> Settings {
    settings_path()
        .map(|p| commands::settings(&p))
        .unwrap_or_default()
}

/// "fr" or "en": the language switched to, else the system's.
#[tauri::command]
fn lang() -> String {
    let system = sys_locale::get_locale();
    commands::lang(&settings(), system.as_deref())
        .code()
        .to_owned()
}

#[tauri::command]
fn set_lang(lang: String) -> Result<(), String> {
    commands::set_lang(&settings_path()?, Lang::parse(&lang))
}

#[tauri::command]
async fn plan(req: PlanRequest, lang: String) -> Result<PlanResult, String> {
    tauri::async_runtime::spawn_blocking(move || commands::plan(&req, Lang::parse(&lang)))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn apply(
    req: PlanRequest,
    expected: Vec<PlannedFile>,
    lang: String,
    progress: Channel<ProgressLine>,
) -> Result<ApplyOutcome, String> {
    let settings = settings_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        commands::apply(&req, &expected, Lang::parse(&lang), &settings, |line| {
            let _ = progress.send(line);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn erase(shoots: Vec<PathBuf>, lang: String) -> Result<LogOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || commands::erase_in(&shoots, Lang::parse(&lang)))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn undo(shoots: Vec<PathBuf>, lang: String) -> Result<LogOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || commands::undo_in(&shoots, Lang::parse(&lang)))
        .await
        .map_err(|e| e.to_string())
}

/// A folder the user chose, checked before the screen moves on (see `commands::check_folder`).
#[tauri::command]
fn check_folder(path: String, source: Option<PathBuf>, lang: String) -> Result<PathBuf, String> {
    commands::check_folder(&path, source.as_deref(), Lang::parse(&lang))
}

/// Show a shoot folder in the file manager.
#[tauri::command]
fn open_folder(path: PathBuf) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            settings,
            lang,
            set_lang,
            plan,
            apply,
            erase,
            undo,
            open_folder,
            check_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running ExportPro");
}
