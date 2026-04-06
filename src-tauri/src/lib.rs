mod creamapi;
mod dlc;
mod steam;

use creamapi::{ApplyResult, DlcEntry, DllStatus};
use dlc::DlcInfo;
use steam::SteamGame;
use tauri::Manager;
use std::path::PathBuf;

fn get_resource_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resource_dir()
        .map_err(|e| format!("Failed to resolve resource dir: {}", e))
}

#[tauri::command]
async fn get_installed_games() -> Result<Vec<SteamGame>, String> {
    Ok(steam::get_all_games())
}

#[tauri::command]
async fn get_dlc_list(app_id: u32) -> Result<Vec<DlcInfo>, String> {
    dlc::fetch_dlc_list(app_id).await
}

#[tauri::command]
async fn apply_cream_api(
    app: tauri::AppHandle,
    install_dir: String,
    has_x86: bool,
    has_x64: bool,
    app_id: u32,
    dlcs: Vec<DlcEntry>,
    offline: bool,
    extra_protection: bool,
) -> Result<ApplyResult, String> {
    let resource_dir = get_resource_dir(&app)?;
    Ok(creamapi::apply(
        &resource_dir,
        &install_dir,
        has_x86,
        has_x64,
        app_id,
        &dlcs,
        offline,
        extra_protection,
    ))
}

#[tauri::command]
async fn remove_cream_api(install_dir: String) -> Result<ApplyResult, String> {
    Ok(creamapi::remove(&install_dir))
}

#[tauri::command]
async fn check_dll_status(app: tauri::AppHandle) -> Result<DllStatus, String> {
    let resource_dir = get_resource_dir(&app)?;
    Ok(creamapi::get_dll_status(&resource_dir))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_installed_games,
            get_dlc_list,
            apply_cream_api,
            remove_cream_api,
            check_dll_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
