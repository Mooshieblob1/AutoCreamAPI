use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SteamGame {
    pub app_id: u32,
    pub name: String,
    pub install_dir: String,
    pub has_x86: bool,
    pub has_x64: bool,
    pub cream_api_applied: bool,
}

/// Find Steam install path from the Windows registry
pub fn find_steam_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey("Software\\Valve\\Steam") {
            if let Ok(path) = key.get_value::<String, _>("SteamPath") {
                let p = PathBuf::from(path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        // Fallback: check default install location
        let default = PathBuf::from("C:\\Program Files (x86)\\Steam");
        if default.exists() {
            return Some(default);
        }
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").ok()?;
        let paths = [
            format!("{}/.steam/steam", home),
            format!("{}/.local/share/Steam", home),
        ];
        for p in &paths {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }
}

/// Parse Steam's libraryfolders.vdf to find all library directories
pub fn find_library_folders(steam_path: &Path) -> Vec<PathBuf> {
    let mut folders = Vec::new();

    // Always include the main Steam directory
    let main_steamapps = steam_path.join("steamapps");
    if main_steamapps.exists() {
        folders.push(main_steamapps);
    }

    // Try both known locations for libraryfolders.vdf
    let vdf_paths = [
        steam_path.join("steamapps").join("libraryfolders.vdf"),
        steam_path.join("config").join("libraryfolders.vdf"),
    ];

    for vdf_path in &vdf_paths {
        if let Ok(content) = fs::read_to_string(vdf_path) {
            // Simple VDF parser — libraryfolders.vdf has entries like:
            // "0" { "path" "C:\\SteamLibrary" ... }
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"path\"") {
                    if let Some(path_str) = extract_vdf_value(trimmed) {
                        let lib_path = PathBuf::from(&path_str).join("steamapps");
                        if lib_path.exists() && !folders.contains(&lib_path) {
                            folders.push(lib_path);
                        }
                    }
                }
            }
        }
    }

    folders
}

/// Parse all .acf manifest files in a steamapps directory
pub fn scan_library(steamapps_dir: &Path) -> Vec<SteamGame> {
    let mut games = Vec::new();

    let entries = match fs::read_dir(steamapps_dir) {
        Ok(e) => e,
        Err(_) => return games,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("acf") {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(game) = parse_acf(&content, steamapps_dir) {
                games.push(game);
            }
        }
    }

    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games
}

/// Parse a single .acf manifest file
fn parse_acf(content: &str, steamapps_dir: &Path) -> Option<SteamGame> {
    let mut fields: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // ACF files have key-value pairs like: "appid"		"570"
        let parts: Vec<&str> = trimmed.splitn(2, '\t').collect();
        if parts.len() == 2 {
            let key = parts[0].trim().trim_matches('"');
            let val = parts[1].trim().trim_matches('"');
            fields.insert(key.to_lowercase(), val.to_string());
        }
        // Also handle space-separated (some ACF files use spaces)
        if parts.len() == 1 && trimmed.contains("\"") {
            let cleaned = trimmed.replace('\t', " ");
            let segments: Vec<&str> = cleaned.split('"').collect();
            if segments.len() >= 4 {
                let key = segments[1].to_lowercase();
                let val = segments[3].to_string();
                fields.insert(key, val);
            }
        }
    }

    let app_id: u32 = fields.get("appid")?.parse().ok()?;
    let name = fields.get("name")?.clone();
    let installdir = fields.get("installdir")?;

    // Skip tools, SDKs, etc.
    if name.is_empty() {
        return None;
    }

    let game_dir = steamapps_dir.join("common").join(installdir);
    if !game_dir.exists() {
        return None;
    }

    let install_dir = game_dir.to_string_lossy().to_string();
    let has_x86 = game_dir.join("steam_api.dll").exists();
    let has_x64 = game_dir.join("steam_api64.dll").exists();

    // Check if CreamAPI is already applied: look for backup originals
    let cream_api_applied = game_dir.join("steam_api_o.dll").exists()
        || game_dir.join("steam_api64_o.dll").exists();

    // Only show games that have Steam API DLLs
    if !has_x86 && !has_x64 {
        // Recursively search subdirectories for steam_api DLLs
        let (sub_x86, sub_x64, sub_applied) = scan_subdirs_for_steam_api(&game_dir, 0);
        if !sub_x86 && !sub_x64 {
            return None;
        }
        return Some(SteamGame {
            app_id,
            name,
            install_dir,
            has_x86: sub_x86,
            has_x64: sub_x64,
            cream_api_applied: sub_applied,
        });
    }

    Some(SteamGame {
        app_id,
        name,
        install_dir,
        has_x86,
        has_x64,
        cream_api_applied,
    })
}

/// Maximum depth to search subdirectories for steam_api DLLs
const MAX_SEARCH_DEPTH: u32 = 4;

/// Recursively search subdirectories for steam_api DLLs
fn scan_subdirs_for_steam_api(dir: &Path, depth: u32) -> (bool, bool, bool) {
    if depth >= MAX_SEARCH_DEPTH {
        return (false, false, false);
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let sub = entry.path();
                let x86 = sub.join("steam_api.dll").exists();
                let x64 = sub.join("steam_api64.dll").exists();
                let applied =
                    sub.join("steam_api_o.dll").exists() || sub.join("steam_api64_o.dll").exists();
                if x86 || x64 {
                    return (x86, x64, applied);
                }
                // Recurse deeper
                let (rx86, rx64, rapplied) = scan_subdirs_for_steam_api(&sub, depth + 1);
                if rx86 || rx64 {
                    return (rx86, rx64, rapplied);
                }
            }
        }
    }
    (false, false, false)
}

/// Extract a value from a VDF line like: "key" "value"
fn extract_vdf_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 4 {
        Some(parts[3].replace("\\\\", "\\"))
    } else {
        None
    }
}

/// Get all installed Steam games across all libraries
pub fn get_all_games() -> Vec<SteamGame> {
    let steam_path = match find_steam_path() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let library_folders = find_library_folders(&steam_path);
    let mut all_games = Vec::new();

    for folder in library_folders {
        let mut games = scan_library(&folder);
        all_games.append(&mut games);
    }

    // Deduplicate by app_id
    all_games.sort_by_key(|g| g.app_id);
    all_games.dedup_by_key(|g| g.app_id);
    all_games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    all_games
}
