// Tauri backend commands for plugin management
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub manifest_url: Option<String>,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub entry: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub ui_slots: Option<Vec<String>>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub license: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginState {
    pub name: String,
    pub enabled: bool,
    pub granted_permissions: Vec<String>,
    pub version: String,
    pub plugin_type: String,
    pub installed_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginInfo {
    pub name: String,
    pub enabled: bool,
    pub manifest: PluginManifest,
    pub granted_permissions: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct PluginStateStore {
    plugins: HashMap<String, PluginState>,
}

fn get_state_file_path(plugin_dir: &str) -> PathBuf {
    PathBuf::from(plugin_dir).join("plugin_state.json")
}

fn load_plugin_states(plugin_dir: &str) -> PluginStateStore {
    let state_path = get_state_file_path(plugin_dir);
    if let Ok(content) = fs::read_to_string(&state_path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        PluginStateStore::default()
    }
}

fn save_plugin_states(plugin_dir: &str, store: &PluginStateStore) -> Result<(), String> {
    let state_path = get_state_file_path(plugin_dir);
    let content = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(&state_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn read_plugin_manifest(plugin_path: &PathBuf) -> Option<PluginManifest> {
    let manifest_path = plugin_path.join("plugin.json");
    if let Ok(manifest_str) = fs::read_to_string(&manifest_path) {
        serde_json::from_str(&manifest_str).ok()
    } else {
        None
    }
}

#[tauri::command]
pub fn list_plugins(plugin_dir: String) -> Vec<PluginInfo> {
    let mut plugins = Vec::new();
    let dir = PathBuf::from(&plugin_dir);
    let states = load_plugin_states(&plugin_dir);

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(manifest) = read_plugin_manifest(&path) {
                    let name = manifest.name.clone();
                    let state = states.plugins.get(&name);

                    plugins.push(PluginInfo {
                        name: name.clone(),
                        enabled: state.map(|s| s.enabled).unwrap_or(false),
                        manifest,
                        granted_permissions: state
                            .map(|s| s.granted_permissions.clone())
                            .unwrap_or_default(),
                    });
                }
            }
        }
    }
    plugins
}

#[tauri::command]
pub fn enable_plugin(name: String, plugin_dir: String) -> Result<bool, String> {
    let mut states = load_plugin_states(&plugin_dir);

    if let Some(state) = states.plugins.get_mut(&name) {
        state.enabled = true;
        save_plugin_states(&plugin_dir, &states)?;
        Ok(true)
    } else {
        // Plugin not in state yet, need to add it
        // Use safe folder name (matching install logic)
        let safe_name = name.replace(" ", "-").to_lowercase();
        let plugin_path = PathBuf::from(&plugin_dir).join(&safe_name);
        if let Some(manifest) = read_plugin_manifest(&plugin_path) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            states.plugins.insert(
                name.clone(),
                PluginState {
                    name: name.clone(),
                    enabled: true,
                    granted_permissions: vec![],
                    version: manifest.version,
                    plugin_type: manifest.plugin_type,
                    installed_at: now,
                },
            );
            save_plugin_states(&plugin_dir, &states)?;
            Ok(true)
        } else {
            Err(format!("Plugin not found: {}", name))
        }
    }
}

#[tauri::command]
pub fn disable_plugin(name: String, plugin_dir: String) -> Result<bool, String> {
    let mut states = load_plugin_states(&plugin_dir);

    if let Some(state) = states.plugins.get_mut(&name) {
        state.enabled = false;
        save_plugin_states(&plugin_dir, &states)?;
        Ok(true)
    } else {
        Err(format!("Plugin not tracked: {}", name))
    }
}

#[tauri::command]
pub async fn install_plugin(repo_url: String, plugin_dir: String) -> Result<PluginInfo, String> {
    // Parse GitHub URL to get owner/repo
    let parts: Vec<&str> = repo_url.trim_end_matches('/').split('/').collect();

    if parts.len() < 2 {
        return Err("Invalid repository URL".to_string());
    }

    let owner = parts[parts.len() - 2];
    let repo = parts[parts.len() - 1];

    let client = reqwest::Client::new();

    // First, get repo info to find default branch
    let repo_api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    let repo_response = client
        .get(&repo_api_url)
        .header("User-Agent", "Audion-Plugin-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch repo info: {}", e))?;

    let default_branch = if repo_response.status().is_success() {
        let repo_info: serde_json::Value = repo_response
            .json()
            .await
            .map_err(|e| format!("Failed to parse repo info: {}", e))?;
        repo_info["default_branch"]
            .as_str()
            .unwrap_or("main")
            .to_string()
    } else {
        "main".to_string()
    };

    // Fetch plugin.json from raw content
    let manifest_url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/plugin.json",
        owner, repo, default_branch
    );

    let manifest_response = client
        .get(&manifest_url)
        .header("User-Agent", "Audion-Plugin-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch plugin.json: {}", e))?;

    if !manifest_response.status().is_success() {
        return Err(format!(
            "Failed to fetch plugin.json: HTTP {}",
            manifest_response.status()
        ));
    }

    let manifest: PluginManifest = manifest_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse plugin.json: {}", e))?;

    // Create plugin directory
    let plugin_name = manifest.name.clone();
    let safe_name = plugin_name.replace(" ", "-").to_lowercase();
    let plugin_path = PathBuf::from(&plugin_dir).join(&safe_name);
    fs::create_dir_all(&plugin_path).map_err(|e| format!("Failed to create plugin dir: {}", e))?;

    // Save plugin.json
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(plugin_path.join("plugin.json"), &manifest_json)
        .map_err(|e| format!("Failed to save plugin.json: {}", e))?;

    // Fetch the entry file (index.js or plugin.wasm)
    let entry_url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        owner, repo, default_branch, manifest.entry
    );

    let entry_response = client
        .get(&entry_url)
        .header("User-Agent", "Audion-Plugin-Manager")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch entry file: {}", e))?;

    if !entry_response.status().is_success() {
        return Err(format!(
            "Failed to fetch {}: HTTP {}",
            manifest.entry,
            entry_response.status()
        ));
    }

    let entry_bytes = entry_response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read entry file: {}", e))?;

    fs::write(plugin_path.join(&manifest.entry), &entry_bytes)
        .map_err(|e| format!("Failed to save entry file: {}", e))?;

    // Add to state
    let mut states = load_plugin_states(&plugin_dir);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    states.plugins.insert(
        manifest.name.clone(),
        PluginState {
            name: manifest.name.clone(),
            enabled: false,
            granted_permissions: vec![],
            version: manifest.version.clone(),
            plugin_type: manifest.plugin_type.clone(),
            installed_at: now,
        },
    );
    save_plugin_states(&plugin_dir, &states)?;

    Ok(PluginInfo {
        name: manifest.name.clone(),
        enabled: false,
        manifest,
        granted_permissions: vec![],
    })
}

#[tauri::command]
pub fn uninstall_plugin(name: String, plugin_dir: String) -> Result<bool, String> {
    // Convert to safe folder name (matching install logic)
    let safe_name = name.replace(" ", "-").to_lowercase();
    let plugin_path = PathBuf::from(&plugin_dir).join(&safe_name);

    if !plugin_path.exists() {
        return Err(format!("Plugin not found: {}", name));
    }

    // Remove plugin directory
    fs::remove_dir_all(&plugin_path).map_err(|e| format!("Failed to remove plugin: {}", e))?;

    // Remove from state (using original name as key)
    let mut states = load_plugin_states(&plugin_dir);
    states.plugins.remove(&name);
    save_plugin_states(&plugin_dir, &states)?;

    Ok(true)
}

#[tauri::command]
pub fn get_plugin_permissions(name: String, plugin_dir: String) -> Option<Vec<String>> {
    let plugin_path = PathBuf::from(plugin_dir).join(&name);
    read_plugin_manifest(&plugin_path).map(|m| m.permissions)
}

#[tauri::command]
pub fn grant_permissions(
    name: String,
    plugin_dir: String,
    permissions: Vec<String>,
) -> Result<bool, String> {
    let mut states = load_plugin_states(&plugin_dir);

    if let Some(state) = states.plugins.get_mut(&name) {
        // Merge new permissions with existing ones
        for perm in permissions {
            if !state.granted_permissions.contains(&perm) {
                state.granted_permissions.push(perm);
            }
        }
        save_plugin_states(&plugin_dir, &states)?;
        Ok(true)
    } else {
        Err(format!("Plugin not tracked: {}", name))
    }
}

#[tauri::command]
pub fn revoke_permissions(
    name: String,
    plugin_dir: String,
    permissions: Vec<String>,
) -> Result<bool, String> {
    let mut states = load_plugin_states(&plugin_dir);

    if let Some(state) = states.plugins.get_mut(&name) {
        state
            .granted_permissions
            .retain(|p| !permissions.contains(p));
        save_plugin_states(&plugin_dir, &states)?;
        Ok(true)
    } else {
        Err(format!("Plugin not tracked: {}", name))
    }
}

#[tauri::command]
pub fn get_plugin_dir(app_handle: tauri::AppHandle) -> Result<String, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let plugin_dir = app_dir.join("plugins");
    fs::create_dir_all(&plugin_dir).map_err(|e| e.to_string())?;
    Ok(plugin_dir.to_string_lossy().to_string())
}
