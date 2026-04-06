use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlcEntry {
    pub app_id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub success: bool,
    pub message: String,
}

/// Read a CreamAPI DLL from the resource directory
fn read_cream_dll(resource_dir: &Path, filename: &str) -> Result<Vec<u8>, String> {
    let dll_path = resource_dir.join(filename);
    if !dll_path.exists() {
        return Err(format!(
            "CreamAPI DLL not found: {}\nExpected at: {}",
            filename,
            dll_path.display()
        ));
    }
    fs::read(&dll_path).map_err(|e| format!("Failed to read {}: {}", filename, e))
}

/// Apply CreamAPI to a game directory
pub fn apply(
    resource_dir: &Path,
    install_dir: &str,
    has_x86: bool,
    has_x64: bool,
    app_id: u32,
    dlcs: &[DlcEntry],
    offline: bool,
    extra_protection: bool,
) -> ApplyResult {
    let game_dir = Path::new(install_dir);

    if !game_dir.exists() {
        return ApplyResult {
            success: false,
            message: format!("Game directory not found: {}", install_dir),
        };
    }

    // Find which subdirectory contains the steam_api DLLs
    let target_dir = find_steam_api_dir(game_dir, has_x86, has_x64);

    // Apply x86 DLL
    if has_x86 {
        let cream_bytes = match read_cream_dll(resource_dir, "steam_api.dll") {
            Ok(b) => b,
            Err(e) => return ApplyResult { success: false, message: e },
        };
        if let Err(e) = apply_dll(&target_dir, "steam_api.dll", "steam_api_o.dll", &cream_bytes) {
            return ApplyResult {
                success: false,
                message: format!("Failed to apply x86 DLL: {}", e),
            };
        }
    }

    // Apply x64 DLL
    if has_x64 {
        let cream_bytes = match read_cream_dll(resource_dir, "steam_api64.dll") {
            Ok(b) => b,
            Err(e) => return ApplyResult { success: false, message: e },
        };
        if let Err(e) =
            apply_dll(&target_dir, "steam_api64.dll", "steam_api64_o.dll", &cream_bytes)
        {
            return ApplyResult {
                success: false,
                message: format!("Failed to apply x64 DLL: {}", e),
            };
        }
    }

    // Generate cream_api.ini
    if let Err(e) = write_config(&target_dir, app_id, dlcs, offline, extra_protection) {
        return ApplyResult {
            success: false,
            message: format!("Failed to write config: {}", e),
        };
    }

    ApplyResult {
        success: true,
        message: format!(
            "CreamAPI applied successfully with {} DLCs",
            dlcs.len()
        ),
    }
}

/// Remove CreamAPI from a game directory (restore original DLLs)
pub fn remove(install_dir: &str) -> ApplyResult {
    let game_dir = Path::new(install_dir);

    let dirs_to_check = get_dirs_to_check(game_dir);

    let mut restored = false;
    for dir in &dirs_to_check {
        restored |= restore_dll(dir, "steam_api.dll", "steam_api_o.dll");
        restored |= restore_dll(dir, "steam_api64.dll", "steam_api64_o.dll");

        // Remove config file
        let config_path = dir.join("cream_api.ini");
        if config_path.exists() {
            let _ = fs::remove_file(&config_path);
        }

        // Remove backup files
        let backup86 = dir.join("steam_api.dll.backup");
        let backup64 = dir.join("steam_api64.dll.backup");
        if backup86.exists() {
            let _ = fs::remove_file(&backup86);
        }
        if backup64.exists() {
            let _ = fs::remove_file(&backup64);
        }
    }

    if restored {
        ApplyResult {
            success: true,
            message: "CreamAPI removed, original DLLs restored".to_string(),
        }
    } else {
        ApplyResult {
            success: false,
            message: "No CreamAPI installation found to remove".to_string(),
        }
    }
}

/// Find the directory containing steam_api DLLs (root or one level deep)
fn find_steam_api_dir(game_dir: &Path, has_x86: bool, has_x64: bool) -> std::path::PathBuf {
    if has_x86 && game_dir.join("steam_api.dll").exists() {
        return game_dir.to_path_buf();
    }
    if has_x64 && game_dir.join("steam_api64.dll").exists() {
        return game_dir.to_path_buf();
    }

    // Search subdirectories
    if let Ok(entries) = fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let sub = entry.path();
                if (has_x86 && sub.join("steam_api.dll").exists())
                    || (has_x64 && sub.join("steam_api64.dll").exists())
                {
                    return sub;
                }
            }
        }
    }

    game_dir.to_path_buf()
}

/// Get all directories to check for removal (root + one level deep)
fn get_dirs_to_check(game_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut dirs = vec![game_dir.to_path_buf()];
    if let Ok(entries) = fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let sub = entry.path();
                if sub.join("steam_api_o.dll").exists() || sub.join("steam_api64_o.dll").exists() {
                    dirs.push(sub);
                }
            }
        }
    }
    dirs
}

/// Apply a single CreamAPI DLL: backup original, write replacement
fn apply_dll(
    dir: &Path,
    api_name: &str,
    orig_name: &str,
    cream_bytes: &[u8],
) -> Result<(), String> {
    let api_path = dir.join(api_name);
    let orig_path = dir.join(orig_name);
    let backup_path = dir.join(format!("{}.backup", api_name));

    if !api_path.exists() {
        return Err(format!("{} not found in {}", api_name, dir.display()));
    }

    // Create backup
    fs::copy(&api_path, &backup_path)
        .map_err(|e| format!("Failed to create backup: {}", e))?;

    // Rename original to *_o.dll (only if not already done)
    if !orig_path.exists() {
        fs::rename(&api_path, &orig_path)
            .map_err(|e| format!("Failed to rename original: {}", e))?;
    }

    // Write CreamAPI DLL
    fs::write(&api_path, cream_bytes)
        .map_err(|e| format!("Failed to write CreamAPI DLL: {}", e))?;

    Ok(())
}

/// Restore the original DLL from the *_o.dll backup
fn restore_dll(dir: &Path, api_name: &str, orig_name: &str) -> bool {
    let api_path = dir.join(api_name);
    let orig_path = dir.join(orig_name);

    if orig_path.exists() {
        // Remove the CreamAPI DLL and restore original
        let _ = fs::remove_file(&api_path);
        if fs::rename(&orig_path, &api_path).is_ok() {
            return true;
        }
    }
    false
}

/// Write cream_api.ini configuration file
fn write_config(
    dir: &Path,
    app_id: u32,
    dlcs: &[DlcEntry],
    offline: bool,
    extra_protection: bool,
) -> Result<(), String> {
    let config_path = dir.join("cream_api.ini");
    let mut file =
        fs::File::create(&config_path).map_err(|e| format!("Failed to create config: {}", e))?;

    let mut config = String::new();

    config.push_str("[steam]\n");
    config.push_str(&format!("appid = {}\n", app_id));
    config.push_str(&format!(
        "unlockall = {}\n",
        if dlcs.is_empty() { "true" } else { "false" }
    ));
    config.push_str(&format!(
        "orgapi = {}\n",
        if dir.join("steam_api_o.dll").exists() {
            "steam_api_o.dll"
        } else {
            "steam_api_o.dll"
        }
    ));
    config.push_str(&format!(
        "orgapi64 = {}\n",
        if dir.join("steam_api64_o.dll").exists() {
            "steam_api64_o.dll"
        } else {
            "steam_api64_o.dll"
        }
    ));
    config.push_str(&format!(
        "extraprotection = {}\n",
        if extra_protection { "true" } else { "false" }
    ));

    config.push_str("\n[steam_misc]\n");
    config.push_str(&format!(
        "forceoffline = {}\n",
        if offline { "true" } else { "false" }
    ));

    if !dlcs.is_empty() {
        config.push_str("\n[dlc]\n");
        for dlc in dlcs {
            config.push_str(&format!("{} = {}\n", dlc.app_id, dlc.name));
        }
    }

    file.write_all(config.as_bytes())
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct DllStatus {
    pub dll_dir: String,
    pub has_x86: bool,
    pub has_x64: bool,
}

/// Check whether the CreamAPI DLLs are present in the resource directory
pub fn get_dll_status(resource_dir: &Path) -> DllStatus {
    DllStatus {
        has_x86: resource_dir.join("steam_api.dll").exists(),
        has_x64: resource_dir.join("steam_api64.dll").exists(),
        dll_dir: resource_dir.display().to_string(),
    }
}
