use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Instance {
    id: String,
    name: String,
    game_version: String,
    loader: String,
    created_at: u64,
}

fn instances_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|e| format!("Cannot resolve app data directory: {e}"))?;
    dir.push("instances");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create instances directory: {e}"))?;
    Ok(dir)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn read_instance(dir: &PathBuf, id: &str) -> Result<Instance, String> {
    if !valid_id(id) { return Err("Invalid instance identifier".into()); }
    let path = dir.join(id).join("instance.json");
    let data = fs::read_to_string(path).map_err(|e| format!("Cannot read instance: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Invalid instance metadata: {e}"))
}

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "ComfyCraft",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "linux_only": true
    })
}

#[tauri::command]
fn list_instances(app: AppHandle) -> Result<Vec<Instance>, String> {
    let dir = instances_dir(&app)?;
    let mut instances = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("Cannot list instances: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(instance) = read_instance(&instances_dir(&app)?, &id) { instances.push(instance); }
    }
    instances.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(instances)
}

#[tauri::command]
fn create_instance(app: AppHandle, name: String, game_version: String) -> Result<Instance, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 { return Err("Name must contain 1–40 characters".into()); }
    if game_version.trim().is_empty() || game_version.len() > 32 { return Err("Invalid Minecraft version".into()); }
    let dir = instances_dir(&app)?;
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?;
    let now = elapsed.as_secs();
    let id = format!("instance-{}-{}", elapsed.as_nanos(), std::process::id());
    let instance_dir = dir.join(&id);
    fs::create_dir(&instance_dir).map_err(|e| format!("Cannot create instance: {e}"))?;
    for child in ["mods", "resourcepacks", "shaderpacks", "saves", "logs"] {
        if let Err(error) = fs::create_dir(instance_dir.join(child)) {
            let _ = fs::remove_dir_all(&instance_dir);
            return Err(format!("Cannot initialize instance folders: {error}"));
        }
    }
    let instance = Instance { id, name: name.to_string(), game_version: game_version.trim().to_string(), loader: "Vanilla".into(), created_at: now };
    let json = serde_json::to_vec_pretty(&instance).map_err(|e| e.to_string())?;
    if let Err(error) = fs::write(instance_dir.join("instance.json.tmp"), json)
        .and_then(|_| fs::rename(instance_dir.join("instance.json.tmp"), instance_dir.join("instance.json"))) {
        let _ = fs::remove_dir_all(&instance_dir);
        return Err(format!("Cannot save instance metadata: {error}"));
    }
    Ok(instance)
}

#[tauri::command]
fn rename_instance(app: AppHandle, id: String, name: String) -> Result<Instance, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 { return Err("Name must contain 1–40 characters".into()); }
    let dir = instances_dir(&app)?;
    let mut instance = read_instance(&dir, &id)?;
    instance.name = name.to_string();
    let path = dir.join(&id).join("instance.json");
    let temp = dir.join(&id).join("instance.json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(&instance).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())?;
    Ok(instance)
}

#[tauri::command]
fn delete_instance(app: AppHandle, id: String) -> Result<(), String> {
    let dir = instances_dir(&app)?;
    let _ = read_instance(&dir, &id)?;
    fs::remove_dir_all(dir.join(id)).map_err(|e| format!("Cannot remove instance: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_info, list_instances, create_instance, rename_instance, delete_instance])
        .run(tauri::generate_context!())
        .expect("error while running ComfyCraft");
}
