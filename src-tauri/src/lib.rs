use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const USER_AGENT: &str = "ComfyCraft/0.1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Instance {
    id: String,
    name: String,
    game_version: String,
    loader: String,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    version: String,
    libraries_downloaded: usize,
    assets_downloaded: usize,
    already_present: usize,
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Debug, Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    sha1: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameVersion {
    id: String,
    version_type: String,
}

#[derive(Clone)]
struct DownloadTask {
    url: String,
    path: PathBuf,
    sha1: Option<String>,
    size: Option<u64>,
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

fn read_instance(dir: &Path, id: &str) -> Result<Instance, String> {
    if !valid_id(id) { return Err("Invalid instance identifier".into()); }
    let path = dir.join(id).join("instance.json");
    let data = fs::read_to_string(path).map_err(|e| format!("Cannot read instance: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Invalid instance metadata: {e}"))
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty() || path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(format!("Unsafe download path: {value}"));
    }
    Ok(path.to_path_buf())
}

fn file_matches(path: &Path, expected_sha1: Option<&str>, expected_size: Option<u64>) -> bool {
    let Ok(metadata) = fs::metadata(path) else { return false; };
    if !metadata.is_file() { return false; }
    if let Some(size) = expected_size {
        if metadata.len() != size { return false; }
    }
    if let Some(hash) = expected_sha1 {
        let Ok(bytes) = fs::read(path) else { return false; };
        let actual = format!("{:x}", Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(hash) { return false; }
    }
    true
}

async fn download_verified(client: &reqwest::Client, task: &DownloadTask) -> Result<bool, String> {
    if !task.url.starts_with("https://") {
        return Err(format!("Refusing non-HTTPS download: {}", task.url));
    }
    if file_matches(&task.path, task.sha1.as_deref(), task.size) {
        return Ok(false);
    }
    let response = client.get(&task.url).send().await
        .map_err(|e| format!("Download request failed: {e}"))?
        .error_for_status().map_err(|e| format!("Download returned an error: {e}"))?;
    let bytes = response.bytes().await.map_err(|e| format!("Cannot read downloaded file: {e}"))?;
    if let Some(size) = task.size {
        if bytes.len() as u64 != size {
            return Err(format!("Downloaded file has an unexpected size: {}", task.path.display()));
        }
    }
    if let Some(hash) = task.sha1.as_deref() {
        let actual = format!("{:x}", Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(hash) {
            return Err(format!("SHA-1 verification failed: {}", task.path.display()));
        }
    }
    let parent = task.path.parent().ok_or("Download path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| format!("Cannot create download directory: {e}"))?;
    let temp = task.path.with_extension("comfycraft-part");
    fs::write(&temp, &bytes).map_err(|e| format!("Cannot write downloaded file: {e}"))?;
    fs::rename(&temp, &task.path).map_err(|e| format!("Cannot finalize downloaded file: {e}"))?;
    Ok(true)
}

async fn download_many(client: &reqwest::Client, tasks: Vec<DownloadTask>) -> Result<(usize, usize), String> {
    use futures_util::{stream, StreamExt, TryStreamExt};
    let total = tasks.len();
    let pending = tasks.into_iter().map(|task| {
        let client = client.clone();
        async move { download_verified(&client, &task).await }
    });
    let downloaded = stream::iter(pending)
        .buffer_unordered(10)
        .try_collect::<Vec<bool>>()
        .await
        .map_err(|e| e)?;
    let fresh = downloaded.iter().filter(|value| **value).count();
    Ok((fresh, total.saturating_sub(fresh)))
}

fn linux_arch_matches(requested: &str) -> bool {
    match requested {
        "x86_64" => std::env::consts::ARCH == "x86_64",
        "x86" => std::env::consts::ARCH == "x86",
        "aarch64" => std::env::consts::ARCH == "aarch64",
        "arm" => std::env::consts::ARCH.starts_with("arm"),
        other => other == std::env::consts::ARCH,
    }
}

fn rule_allows_with_features(library: &Value, features_enabled: &Value) -> bool {
    let Some(rules) = library.get("rules").and_then(Value::as_array) else { return true; };
    let mut allowed = false;
    for rule in rules {
        let os = rule.get("os");
        let name_matches = os.and_then(|v| v.get("name")).and_then(Value::as_str)
            .map(|name| name == std::env::consts::OS).unwrap_or(true);
        let arch_matches = os.and_then(|v| v.get("arch")).and_then(Value::as_str)
            .map(linux_arch_matches).unwrap_or(true);
        let features_match = rule.get("features").and_then(Value::as_object).map(|required| {
            required.iter().all(|(key, expected)| {
                features_enabled.get(key).and_then(Value::as_bool) == expected.as_bool()
            })
        }).unwrap_or(true);
        if name_matches && arch_matches && features_match {
            match rule.get("action").and_then(Value::as_str) {
                Some("allow") => allowed = true,
                Some("disallow") => allowed = false,
                _ => {}
            }
        }
    }
    allowed
}

fn rule_allows(library: &Value) -> bool {
    // Interactive launcher-only flags remain disabled until their UI and argument values exist.
    rule_allows_with_features(library, &serde_json::json!({
        "has_custom_resolution": false,
        "is_demo_user": false,
        "has_quick_plays_support": false,
        "is_quick_play_singleplayer": false,
        "is_quick_play_multiplayer": false,
        "is_quick_play_realms": false
    }))
}

fn artifact_task(base: &Path, artifact: &Value) -> Result<DownloadTask, String> {
    let relative = artifact.get("path").and_then(Value::as_str).ok_or("Artifact is missing its path")?;
    let path = base.join("libraries").join(safe_relative_path(relative)?);
    let url = artifact.get("url").and_then(Value::as_str).ok_or("Artifact is missing its URL")?.to_string();
    let sha1 = artifact.get("sha1").and_then(Value::as_str).map(str::to_string);
    let size = artifact.get("size").and_then(Value::as_u64);
    Ok(DownloadTask { url, path, sha1, size })
}

fn extract_native_archive(archive_path: &Path, destination: &Path) -> Result<usize, String> {
    use std::io::Read;
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("Cannot open native archive {}: {e}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid native archive {}: {e}", archive_path.display()))?;
    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_dir() { continue; }
        let Some(relative) = entry.enclosed_name() else { continue; };
        if relative.components().any(|part| matches!(part, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
            continue;
        }
        if relative.components().next().and_then(|p| p.as_os_str().to_str()) == Some("META-INF") {
            continue;
        }
        let output = destination.join(relative);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Cannot create native directory: {e}"))?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| format!("Cannot read native entry: {e}"))?;
        fs::write(&output, bytes).map_err(|e| format!("Cannot extract native file: {e}"))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn java_runtime() -> Value {
    use std::process::Command;
    let output = Command::new("java").arg("-version").output();
    let Ok(output) = output else {
        return serde_json::json!({"available": false, "executable": "java", "version": null});
    };
    let text = format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout));
    let quoted = text.split('"').nth(1).unwrap_or("");
    let first = if quoted.is_empty() {
        text.split_whitespace().find(|part| part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)).unwrap_or("")
    } else { quoted };
    let major_text = first.split('.').next().unwrap_or("");
    let major = major_text.parse::<u32>().ok().map(|n| if n == 1 {
        first.split('.').nth(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(n)
    } else { n });
    serde_json::json!({
        "available": output.status.success(),
        "executable": "java",
        "version": first,
        "major": major
    })
}

#[tauri::command]
fn get_app_info() -> Value {
    serde_json::json!({
        "name": "ComfyCraft",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "linux_only": true
    })
}

#[tauri::command]
async fn list_game_versions() -> Result<Vec<GameVersion>, String> {
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build().map_err(|e| e.to_string())?;
    let manifest: VersionManifest = client.get(MANIFEST_URL).send().await
        .map_err(|e| format!("Cannot download Minecraft version manifest: {e}"))?
        .error_for_status().map_err(|e| format!("Minecraft version manifest request failed: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse Minecraft version manifest: {e}"))?;
    Ok(manifest.versions.into_iter()
        .filter(|version| version.version_type == "release")
        .map(|version| GameVersion { id: version.id, version_type: version.version_type })
        .collect())
}

#[tauri::command]
async fn install_game(app: AppHandle, instance_id: String) -> Result<InstallResult, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    let game_dir = root.join(&instance_id).join("game");
    fs::create_dir_all(&game_dir).map_err(|e| format!("Cannot create game directory: {e}"))?;
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build().map_err(|e| e.to_string())?;

    let manifest: VersionManifest = client.get(MANIFEST_URL).send().await
        .map_err(|e| format!("Cannot download Minecraft version manifest: {e}"))?
        .error_for_status().map_err(|e| format!("Minecraft version manifest request failed: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse Minecraft version manifest: {e}"))?;
    let version_entry = manifest.versions.iter().find(|v| v.id == instance.game_version)
        .ok_or_else(|| format!("Minecraft version {} is no longer present in the official manifest", instance.game_version))?;
    let metadata_response = client.get(&version_entry.url).send().await
        .map_err(|e| format!("Cannot download version metadata: {e}"))?
        .error_for_status().map_err(|e| format!("Version metadata request failed: {e}"))?;
    let metadata_bytes = metadata_response.bytes().await.map_err(|e| format!("Cannot read version metadata: {e}"))?;
    let metadata_hash = format!("{:x}", Sha1::digest(&metadata_bytes));
    if !metadata_hash.eq_ignore_ascii_case(&version_entry.sha1) {
        return Err("Minecraft version metadata failed SHA-1 verification".into());
    }
    let metadata: Value = serde_json::from_slice(&metadata_bytes).map_err(|e| format!("Invalid version metadata: {e}"))?;
    let versions_dir = game_dir.join("versions").join(&instance.game_version);
    fs::create_dir_all(&versions_dir).map_err(|e| e.to_string())?;
    fs::write(versions_dir.join(format!("{}.json", instance.game_version)), &metadata_bytes)
        .map_err(|e| format!("Cannot save version metadata: {e}"))?;

    let client_download = metadata.get("downloads").and_then(|v| v.get("client"))
        .ok_or("Official version metadata does not contain a client download")?;
    let client_task = DownloadTask {
        url: client_download.get("url").and_then(Value::as_str).ok_or("Client download URL is missing")?.to_string(),
        path: versions_dir.join(format!("{}.jar", instance.game_version)),
        sha1: client_download.get("sha1").and_then(Value::as_str).map(str::to_string),
        size: client_download.get("size").and_then(Value::as_u64),
    };
    download_verified(&client, &client_task).await?;

    let mut library_tasks = Vec::new();
    if let Some(libraries) = metadata.get("libraries").and_then(Value::as_array) {
        for library in libraries {
            if !rule_allows(library) { continue; }
            if let Some(artifact) = library.get("downloads").and_then(|v| v.get("artifact")) {
                library_tasks.push(artifact_task(&game_dir, artifact)?);
            }
            if let (Some(natives), Some(classifiers)) = (
                library.get("natives").and_then(|v| v.get("linux")).and_then(Value::as_str),
                library.get("downloads").and_then(|v| v.get("classifiers")).and_then(Value::as_object),
            ) {
                let classifier_key = natives.replace("${arch}", if std::env::consts::ARCH == "x86_64" { "64" } else { "32" });
                if let Some(classifier) = classifiers.get(&classifier_key) {
                    library_tasks.push(artifact_task(&game_dir, classifier)?);
                }
            }
        }
    }
    let (libraries_downloaded, libraries_existing) = download_many(&client, library_tasks).await?;

    let natives_dir = game_dir.join("natives");
    fs::create_dir_all(&natives_dir).map_err(|e| format!("Cannot create native directory: {e}"))?;
    if let Some(libraries) = metadata.get("libraries").and_then(Value::as_array) {
        for library in libraries {
            if !rule_allows(library) { continue; }
            let Some(native_template) = library.get("natives").and_then(|v| v.get("linux")).and_then(Value::as_str) else { continue; };
            let Some(classifiers) = library.get("downloads").and_then(|v| v.get("classifiers")).and_then(Value::as_object) else { continue; };
            let classifier_key = native_template.replace("${arch}", if std::env::consts::ARCH == "x86_64" { "64" } else { "32" });
            if let Some(classifier) = classifiers.get(&classifier_key) {
                let relative = classifier.get("path").and_then(Value::as_str).ok_or("Native archive path is missing")?;
                let archive_path = game_dir.join("libraries").join(safe_relative_path(relative)?);
                if archive_path.is_file() {
                    extract_native_archive(&archive_path, &natives_dir)?;
                }
            }
        }
    }

    let mut assets_downloaded = 0usize;
    let mut assets_existing = 0usize;
    if let Some(asset_index) = metadata.get("assetIndex") {
        let index_id = asset_index.get("id").and_then(Value::as_str).ok_or("Asset index ID is missing")?;
        let index_url = asset_index.get("url").and_then(Value::as_str).ok_or("Asset index URL is missing")?;
        let index_path = game_dir.join("assets").join("indexes").join(format!("{index_id}.json"));
        let index_task = DownloadTask {
            url: index_url.to_string(),
            path: index_path.clone(),
            sha1: asset_index.get("sha1").and_then(Value::as_str).map(str::to_string),
            size: asset_index.get("size").and_then(Value::as_u64),
        };
        download_verified(&client, &index_task).await?;
        let index: Value = serde_json::from_slice(&fs::read(&index_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Cannot parse asset index: {e}"))?;
        let mut asset_tasks = Vec::new();
        if let Some(objects) = index.get("objects").and_then(Value::as_object) {
            for object in objects.values() {
                let hash = object.get("hash").and_then(Value::as_str).ok_or("Asset object hash is missing")?;
                if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Asset index contains an invalid SHA-1 hash".into());
                }
                let size = object.get("size").and_then(Value::as_u64);
                let relative = format!("{}/{}", &hash[..2], hash);
                let path = game_dir.join("assets").join("objects").join(safe_relative_path(&relative)?);
                asset_tasks.push(DownloadTask {
                    url: format!("https://resources.download.minecraft.net/{relative}"),
                    path,
                    sha1: Some(hash.to_string()),
                    size,
                });
            }
        }
        let (fresh, existing) = download_many(&client, asset_tasks).await?;
        assets_downloaded = fresh;
        assets_existing = existing;
    }

    if let Some(logging) = metadata.get("logging").and_then(|v| v.get("client")).and_then(|v| v.get("file")) {
        if let (Some(url), Some(id)) = (logging.get("url").and_then(Value::as_str), logging.get("id").and_then(Value::as_str)) {
            let task = DownloadTask {
                url: url.to_string(),
                path: game_dir.join("assets").join("log_configs").join(safe_relative_path(id)?),
                sha1: logging.get("sha1").and_then(Value::as_str).map(str::to_string),
                size: logging.get("size").and_then(Value::as_u64),
            };
            download_verified(&client, &task).await?;
        }
    }

    let marker = serde_json::json!({
        "version": instance.game_version,
        "installedAt": SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs(),
        "verified": true
    });
    fs::write(root.join(&instance_id).join("installation.json"), serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot save installation marker: {e}"))?;

    Ok(InstallResult {
        version: instance.game_version,
        libraries_downloaded,
        assets_downloaded,
        already_present: libraries_existing + assets_existing,
    })
}

#[tauri::command]
fn get_runtime_info(app: AppHandle, instance_id: String) -> Result<Value, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    let game_dir = root.join(&instance_id).join("game");
    let metadata_path = game_dir.join("versions").join(&instance.game_version)
        .join(format!("{}.json", instance.game_version));
    let metadata: Value = serde_json::from_slice(&fs::read(&metadata_path)
        .map_err(|e| format!("Game metadata is not installed: {e}"))?)
        .map_err(|e| format!("Cannot parse installed game metadata: {e}"))?;
    let mut classpath = Vec::<String>::new();
    if let Some(libraries) = metadata.get("libraries").and_then(Value::as_array) {
        for library in libraries {
            if !rule_allows(library) { continue; }
            if let Some(artifact) = library.get("downloads").and_then(|v| v.get("artifact")) {
                if let Some(relative) = artifact.get("path").and_then(Value::as_str) {
                    let path = game_dir.join("libraries").join(safe_relative_path(relative)?);
                    if path.is_file() { classpath.push(path.to_string_lossy().into_owned()); }
                }
            }
        }
    }
    let client_jar = game_dir.join("versions").join(&instance.game_version)
        .join(format!("{}.jar", instance.game_version));
    if !client_jar.is_file() {
        return Err("Minecraft client JAR is missing; repair the installation first".into());
    }
    classpath.push(client_jar.to_string_lossy().into_owned());
    Ok(serde_json::json!({
        "version": instance.game_version,
        "java": java_runtime(),
        "classpath": classpath.join(":"),
        "classpathEntries": classpath.len(),
        "nativesDirectory": game_dir.join("natives"),
        "assetsDirectory": game_dir.join("assets"),
        "assetIndex": metadata.get("assetIndex").and_then(|v| v.get("id")).and_then(Value::as_str),
        "gameDirectory": game_dir
    }))
}


fn collect_argument_values(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => output.push(text.clone()),
        Value::Array(items) => {
            for item in items { collect_argument_values(item, output); }
        }
        Value::Object(object) => {
            if !rule_allows(value) { return; }
            if let Some(inner) = object.get("value") {
                collect_argument_values(inner, output);
            }
        }
        _ => {}
    }
}

fn expand_argument_placeholders(value: &str, game_dir: &Path, assets_dir: &Path,
    natives_dir: &Path, classpath: &str, asset_index: &str,
    version_name: &str, version_type: &str) -> (String, Vec<String>) {
    let replacements = [
        ("${game_directory}", game_dir.to_string_lossy().into_owned()),
        ("${assets_root}", assets_dir.to_string_lossy().into_owned()),
        ("${assets_directory}", assets_dir.to_string_lossy().into_owned()),
        ("${natives_directory}", natives_dir.to_string_lossy().into_owned()),
        ("${classpath}", classpath.to_string()),
        ("${asset_index}", asset_index.to_string()),
        ("${library_directory}", game_dir.join("libraries").to_string_lossy().into_owned()),
        ("${launcher_name}", "ComfyCraft".to_string()),
        ("${launcher_version}", env!("CARGO_PKG_VERSION").to_string()),
        ("${version_name}", version_name.to_string()),
        ("${version_type}", version_type.to_string()),
    ];
    let mut expanded = value.to_string();
    for (key, replacement) in replacements { expanded = expanded.replace(key, &replacement); }
    let mut unresolved = Vec::new();
    let mut rest = expanded.as_str();
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else { break; };
        let token = &after[..end];
        if !unresolved.iter().any(|item| item == token) { unresolved.push(token.to_string()); }
        rest = &after[end + 1..];
    }
    (expanded, unresolved)
}

#[tauri::command]
fn inspect_launch_arguments(app: AppHandle, instance_id: String) -> Result<Value, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    let game_dir = root.join(&instance_id).join("game");
    let metadata_path = game_dir.join("versions").join(&instance.game_version)
        .join(format!("{}.json", instance.game_version));
    let metadata: Value = serde_json::from_slice(&fs::read(&metadata_path)
        .map_err(|e| format!("Game metadata is not installed: {e}"))?)
        .map_err(|e| format!("Cannot parse game metadata: {e}"))?;
    let runtime = get_runtime_info(app.clone(), instance_id.clone())?;
    let classpath = runtime.get("classpath").and_then(Value::as_str).unwrap_or("");
    let assets_dir = game_dir.join("assets");
    let natives_dir = game_dir.join("natives");
    let asset_index = metadata.get("assetIndex").and_then(|v| v.get("id"))
        .and_then(Value::as_str).unwrap_or("");
    let mut jvm = Vec::new();
    let mut game = Vec::new();
    if let Some(arguments) = metadata.get("arguments") {
        if let Some(values) = arguments.get("jvm") { collect_argument_values(values, &mut jvm); }
        if let Some(values) = arguments.get("game") { collect_argument_values(values, &mut game); }
    } else {
        return Err("This Minecraft version uses legacy arguments; compatibility handling is not implemented yet".into());
    }
    let mut unresolved = Vec::new();
    let expand = |items: Vec<String>, unresolved: &mut Vec<String>| -> Vec<String> {
        items.into_iter().map(|item| {
            let (value, missing) = expand_argument_placeholders(
                &item, &game_dir, &assets_dir, &natives_dir, classpath, asset_index,
                &instance.game_version,
                metadata.get("type").and_then(Value::as_str).unwrap_or("release")
            );
            for token in missing {
                if !unresolved.contains(&token) { unresolved.push(token); }
            }
            value
        }).collect()
    };
    let jvm = expand(jvm, &mut unresolved);
    let game = expand(game, &mut unresolved);
    let required_java_major = metadata.get("javaVersion").and_then(|v| v.get("majorVersion"))
        .and_then(Value::as_u64).unwrap_or(25);
    let java = runtime.get("java").cloned().unwrap_or(Value::Null);
    Ok(serde_json::json!({
        "version": instance.game_version,
        "requiredJavaMajor": required_java_major,
        "javaRuntime": java,
        "jvmArgumentCount": jvm.len(),
        "gameArgumentCount": game.len(),
        "jvmArguments": jvm,
        "gameArguments": game,
        "unresolvedPlaceholders": unresolved,
        "authenticationRequired": true,
        "launchEnabled": false,
        "note": "Resolved argument vectors are available for validation. Authentication and process supervision are not implemented; do not launch yet."
    }))
}

#[tauri::command]
fn check_launch_readiness(app: AppHandle, instance_id: String) -> Result<Value, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    let game_dir = root.join(&instance_id).join("game");
    let version_dir = game_dir.join("versions").join(&instance.game_version);
    let metadata_path = version_dir.join(format!("{}.json", instance.game_version));
    let metadata: Value = serde_json::from_slice(&fs::read(&metadata_path)
        .map_err(|e| format!("Minecraft metadata is missing: {e}"))?)
        .map_err(|e| format!("Minecraft metadata is invalid: {e}"))?;
    let java = java_runtime();
    let required_java = metadata.get("javaVersion").and_then(|v| v.get("majorVersion"))
        .and_then(Value::as_u64).unwrap_or(25);
    let java_major = java.get("major").and_then(Value::as_u64);
    let java_ok = java.get("available").and_then(Value::as_bool) == Some(true)
        && java_major.map(|major| major >= required_java).unwrap_or(false);
    let client_ok = version_dir.join(format!("{}.jar", instance.game_version)).is_file();
    let natives_ok = game_dir.join("natives").is_dir();
    let assets_ok = game_dir.join("assets").join("indexes").is_dir()
        && metadata.get("assetIndex").and_then(|v| v.get("id")).and_then(Value::as_str)
            .map(|id| game_dir.join("assets").join("indexes").join(format!("{id}.json")).is_file())
            .unwrap_or(false);
    let classpath_count = metadata.get("libraries").and_then(Value::as_array).map(|libraries| {
        libraries.iter().filter(|library| rule_allows(library)).filter(|library| {
            library.get("downloads").and_then(|v| v.get("artifact"))
                .and_then(|v| v.get("path")).and_then(Value::as_str)
                .and_then(|path| safe_relative_path(path).ok())
                .map(|path| game_dir.join("libraries").join(path).is_file()).unwrap_or(false)
        }).count()
    }).unwrap_or(0);
    let checks = vec![
        serde_json::json!({"id":"java","label":format!("Java {required_java} or newer"),"ok":java_ok,"detail":java.get("version").cloned().unwrap_or(Value::Null)}),
        serde_json::json!({"id":"client","label":"Minecraft client","ok":client_ok}),
        serde_json::json!({"id":"natives","label":"Linux native libraries","ok":natives_ok}),
        serde_json::json!({"id":"assets","label":"Game assets","ok":assets_ok}),
        serde_json::json!({"id":"libraries","label":"Runtime libraries","ok":classpath_count > 0,"count":classpath_count})
    ];
    let ready = checks.iter().all(|check| check.get("ok").and_then(Value::as_bool) == Some(true));
    Ok(serde_json::json!({"ready":ready,"checks":checks,"authentication":"Ely.by sign-in is not implemented yet"}))
}

#[tauri::command]
fn is_game_installed(app: AppHandle, instance_id: String) -> Result<bool, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    let marker_path = root.join(&instance_id).join("installation.json");
    let Ok(data) = fs::read_to_string(marker_path) else { return Ok(false); };
    let Ok(marker) = serde_json::from_str::<Value>(&data) else { return Ok(false); };
    Ok(marker.get("version").and_then(Value::as_str) == Some(instance.game_version.as_str())
        && marker.get("verified").and_then(Value::as_bool) == Some(true))
}

#[tauri::command]
fn list_instances(app: AppHandle) -> Result<Vec<Instance>, String> {
    let dir = instances_dir(&app)?;
    let mut instances = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("Cannot list instances: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(instance) = read_instance(&dir, &id) { instances.push(instance); }
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
        .invoke_handler(tauri::generate_handler![
            get_app_info, list_game_versions, install_game, get_runtime_info, inspect_launch_arguments, check_launch_readiness, is_game_installed,
            list_instances, create_instance, rename_instance, delete_instance
        ])
        .run(tauri::generate_context!())
        .expect("error while running ComfyCraft");
}


#[cfg(test)]
mod launch_rule_tests {
    use super::*;

    #[test]
    fn libraries_without_rules_are_allowed() {
        assert!(rule_allows(&serde_json::json!({"name":"example"})));
    }

    #[test]
    fn rules_apply_in_order() {
        let library = serde_json::json!({"rules":[
            {"action":"allow"},
            {"action":"disallow","os":{"name":"linux"}}
        ]});
        assert!(!rule_allows(&library));
    }

    #[test]
    fn feature_requirements_use_explicit_launcher_values() {
        let library = serde_json::json!({"rules":[
            {"action":"allow","features":{"has_custom_resolution":false}},
            {"action":"disallow","features":{"is_demo_user":true}}
        ]});
        assert!(rule_allows(&library));
    }

    #[test]
    fn unsupported_feature_keys_do_not_accidentally_match() {
        let library = serde_json::json!({"rules":[
            {"action":"allow","features":{"future_launcher_feature":true}}
        ]});
        assert!(!rule_allows(&library));
    }
}
