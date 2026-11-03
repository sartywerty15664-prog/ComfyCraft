#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": "ComfyCraft",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "linux_only": true
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running ComfyCraft");
}
