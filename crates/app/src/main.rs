// No console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The window and the Tauri commands the screens (ui/app.js) call. All logic is in
//! `exportpro_app::commands` and the core; this file only adapts them to Tauri.

use std::path::PathBuf;

use exportpro_app::commands::{
    self, ApplyOutcome, LogOutcome, PlanRequest, PlanResult, PlannedFile, ProgressLine,
};
use exportpro_core::apply::Mode;
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
    mode: Mode,
    lang: String,
    progress: Channel<ProgressLine>,
) -> Result<ApplyOutcome, String> {
    let settings = settings_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        let lang = Lang::parse(&lang);
        commands::apply_mode(&req, &expected, mode, lang, &settings, |line| {
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
fn set_check_updates(on: bool) -> Result<(), String> {
    commands::set_check_updates(&settings_path()?, on)
}

#[tauri::command]
fn info() -> commands::Info {
    commands::info()
}

#[tauri::command]
async fn undo_move(shoots: Vec<PathBuf>, run: String, lang: String) -> Result<LogOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        commands::undo_move(&shoots, &run, Lang::parse(&lang))
    })
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

/// How this copy was installed (ADR 0007). Debug builds take `EXPORTX_BUNDLE=appimage` so
/// the tests can show the install offer without packaging the app.
fn bundle() -> commands::Bundle {
    use tauri::utils::{config::BundleType, platform::bundle_type};
    if cfg!(debug_assertions) && std::env::var("EXPORTX_BUNDLE").as_deref() == Ok("appimage") {
        return commands::Bundle::AppImage;
    }
    match bundle_type() {
        Some(BundleType::AppImage) => commands::Bundle::AppImage,
        Some(BundleType::App) => commands::Bundle::MacApp,
        _ => commands::Bundle::Other,
    }
}

/// Is a newer release out, and what to offer for it (an install, or the release page).
#[tauri::command]
async fn check_update(lang: String) -> Result<Option<commands::UpdateOffer>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = exportpro_core::update::endpoint();
        commands::check_update(&url, env!("CARGO_PKG_VERSION"), bundle())
            .map_err(|why| Lang::parse(&lang).update_check_failed(&why))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Download the newer release, check it against the update key, install it and restart.
#[tauri::command]
async fn install_update(app: tauri::AppHandle, lang: String) -> Result<(), String> {
    let failed = |why: String| Lang::parse(&lang).update_install_failed(&why);
    download_and_install(&app).await.map_err(failed)?;
    app.restart()
}

async fn download_and_install(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let url = exportpro_core::update::endpoint()
        .parse::<tauri::Url>()
        .map_err(|e| e.to_string())?;
    let update = app
        .updater_builder()
        .endpoints(vec![url])
        .and_then(|b| {
            b.version_comparator(|current, remote| {
                exportpro_core::update::newer(&current.to_string(), &remote.version.to_string())
            })
            .build()
        })
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no newer version".to_owned())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())
}

/// Open the release page of `version` in the browser (installs the updater can't replace).
#[tauri::command]
fn open_release(version: String) -> Result<(), String> {
    let page = exportpro_core::update::release_page(&version);
    tauri_plugin_opener::open_url(page, None::<&str>).map_err(|e| e.to_string())
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            settings,
            lang,
            set_lang,
            plan,
            apply,
            erase,
            undo,
            undo_move,
            info,
            set_check_updates,
            open_folder,
            check_folder,
            check_update,
            install_update,
            open_release
        ])
        .run(tauri::generate_context!())
        .expect("error while running ExportX");
}
