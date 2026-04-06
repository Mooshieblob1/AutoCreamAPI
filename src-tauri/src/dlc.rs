use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct DlcInfo {
    pub app_id: u32,
    pub name: String,
    pub from_store: bool,
}

// ── Steam Store API types ──

#[derive(Deserialize)]
struct StoreResponse {
    success: bool,
    data: Option<StoreAppData>,
}

#[derive(Deserialize)]
struct StoreAppData {
    #[serde(default)]
    dlc: Vec<u32>,
    #[serde(default)]
    r#type: String,
    name: Option<String>,
}

// ── SteamCMD Web API types ──

#[derive(Deserialize)]
struct SteamCmdResponse {
    success: bool,
    data: Option<HashMap<String, SteamCmdAppData>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SteamCmdAppData {
    common: Option<SteamCmdCommon>,
    extended: Option<SteamCmdExtended>,
    depots: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SteamCmdCommon {
    name: Option<String>,
    #[serde(rename = "type")]
    app_type: Option<String>,
}

#[derive(Deserialize)]
struct SteamCmdExtended {
    listofdlc: Option<String>,
}

/// Fetch DLC list by combining Steam Store API and SteamCMD Web API
pub async fn fetch_dlc_list(app_id: u32) -> Result<Vec<DlcInfo>, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    // Fetch from both sources concurrently
    let (store_result, cmd_result) = tokio::join!(
        fetch_from_store(&client, app_id),
        fetch_from_steamcmd(&client, app_id),
    );

    let mut dlc_map: HashMap<u32, DlcInfo> = HashMap::new();

    // Process Steam Store results (primary, trusted source)
    if let Ok(store_dlcs) = store_result {
        for dlc in store_dlcs {
            dlc_map.insert(dlc.app_id, dlc);
        }
    }

    // Process SteamCMD results (adds hidden DLCs)
    if let Ok(cmd_dlcs) = cmd_result {
        for dlc in cmd_dlcs {
            dlc_map.entry(dlc.app_id).or_insert(dlc);
        }
    }

    if dlc_map.is_empty() {
        return Err("No DLCs found from any source".to_string());
    }

    let mut dlc_list: Vec<DlcInfo> = dlc_map.into_values().collect();
    dlc_list.sort_by_key(|d| d.app_id);

    // Try to resolve names for DLCs that only have IDs
    resolve_dlc_names(&client, &mut dlc_list).await;

    Ok(dlc_list)
}

/// Fetch DLC IDs + names from Steam Store API
async fn fetch_from_store(client: &Client, app_id: u32) -> Result<Vec<DlcInfo>, String> {
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );

    let resp: HashMap<String, StoreResponse> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Store API request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Store API parse failed: {}", e))?;

    let store_resp = resp
        .get(&app_id.to_string())
        .ok_or("App not found in store response")?;

    if !store_resp.success {
        return Err("Store API returned failure".to_string());
    }

    let data = store_resp
        .data
        .as_ref()
        .ok_or("No data in store response")?;

    if data.r#type != "game" && data.r#type != "demo" {
        return Err(format!("App is not a game (type: {})", data.r#type));
    }

    let mut dlcs = Vec::new();
    for dlc_id in &data.dlc {
        dlcs.push(DlcInfo {
            app_id: *dlc_id,
            name: format!("DLC {}", dlc_id), // Will be resolved later
            from_store: true,
        });
    }

    Ok(dlcs)
}

/// Fetch DLC IDs from SteamCMD Web API (includes hidden DLCs)
async fn fetch_from_steamcmd(client: &Client, app_id: u32) -> Result<Vec<DlcInfo>, String> {
    let url = format!("https://api.steamcmd.net/v1/info/{}", app_id);

    let resp: SteamCmdResponse = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("SteamCMD API request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("SteamCMD API parse failed: {}", e))?;

    if !resp.success {
        return Err("SteamCMD API returned failure".to_string());
    }

    let data = resp.data.ok_or("No data in SteamCMD response")?;
    let app_data = data
        .get(&app_id.to_string())
        .ok_or("App not found in SteamCMD response")?;

    let mut dlc_ids: Vec<u32> = Vec::new();

    // Source 1: extended.listofdlc (comma-separated IDs)
    if let Some(extended) = &app_data.extended {
        if let Some(dlc_str) = &extended.listofdlc {
            for id_str in dlc_str.split(',') {
                if let Ok(id) = id_str.trim().parse::<u32>() {
                    if !dlc_ids.contains(&id) {
                        dlc_ids.push(id);
                    }
                }
            }
        }
    }

    // Source 2: depots with dlcappid field
    if let Some(depots) = &app_data.depots {
        for (_key, depot_val) in depots {
            if let Some(obj) = depot_val.as_object() {
                if let Some(dlc_appid) = obj.get("dlcappid") {
                    let id = if let Some(s) = dlc_appid.as_str() {
                        s.parse::<u32>().ok()
                    } else {
                        dlc_appid.as_u64().map(|n| n as u32)
                    };
                    if let Some(id) = id {
                        if !dlc_ids.contains(&id) {
                            dlc_ids.push(id);
                        }
                    }
                }
            }
        }
    }

    Ok(dlc_ids
        .into_iter()
        .map(|id| DlcInfo {
            app_id: id,
            name: format!("DLC {}", id),
            from_store: false,
        })
        .collect())
}

/// Resolve DLC names by fetching from Steam Store API (batched)
async fn resolve_dlc_names(client: &Client, dlcs: &mut Vec<DlcInfo>) {
    // Fetch names in small batches to avoid rate limiting
    for dlc in dlcs.iter_mut() {
        if dlc.name.starts_with("DLC ") {
            let url = format!(
                "https://store.steampowered.com/api/appdetails?appids={}&filters=basic",
                dlc.app_id
            );
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(data) = resp.json::<HashMap<String, StoreResponse>>().await {
                    if let Some(store) = data.get(&dlc.app_id.to_string()) {
                        if store.success {
                            if let Some(d) = &store.data {
                                if let Some(name) = &d.name {
                                    dlc.name = name.clone();
                                }
                            }
                        }
                    }
                }
            }
            // Small delay to be polite to the API
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }
}
