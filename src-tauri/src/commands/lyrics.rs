// Lyrics LRC file commands
use std::fs;
use std::path::PathBuf;

/// Get LRC file path for a music file
fn get_lrc_path(music_path: &str) -> PathBuf {
    let path = PathBuf::from(music_path);
    path.with_extension("lrc")
}

/// Save LRC file alongside music file
#[tauri::command]
pub fn save_lrc_file(music_path: String, lrc_content: String) -> Result<(), String> {
    let lrc_path = get_lrc_path(&music_path);

    fs::write(&lrc_path, lrc_content).map_err(|e| format!("Failed to save LRC file: {}", e))?;

    Ok(())
}

/// Load LRC file if it exists
#[tauri::command]
pub fn load_lrc_file(music_path: String) -> Result<Option<String>, String> {
    let lrc_path = get_lrc_path(&music_path);

    if !lrc_path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&lrc_path).map_err(|e| format!("Failed to read LRC file: {}", e))?;

    Ok(Some(content))
}

/// Delete LRC file for a music file
#[tauri::command]
pub fn delete_lrc_file(music_path: String) -> Result<bool, String> {
    let lrc_path = get_lrc_path(&music_path);

    if !lrc_path.exists() {
        return Ok(false);
    }

    fs::remove_file(&lrc_path).map_err(|e| format!("Failed to delete LRC file: {}", e))?;
    Ok(true)
}

/// Proxy request to Musixmatch API to avoid CORS issues
#[tauri::command]
pub async fn musixmatch_request(action: String, params: Vec<(String, String)>) -> Result<String, String> {
    // Build a client with cookie store and proper redirect policy
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;
    
    let url = format!("https://apic-desktop.musixmatch.com/ws/1.1/{}", action);
    
    // Build query string
    let mut query_params: Vec<(String, String)> = params;
    query_params.push(("app_id".to_string(), "web-desktop-app-v1.0".to_string()));
    query_params.push(("t".to_string(), std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()));
    
    let response = client
        .get(&url)
        .query(&query_params)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Origin", "https://www.musixmatch.com")
        .header("Referer", "https://www.musixmatch.com/")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    Ok(text)
}
