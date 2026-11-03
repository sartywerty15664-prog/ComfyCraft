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
    #[serde(skip)]
    active: bool,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JavaRuntime {
    executable: String,
    version: String,
    major: u32,
    vendor: Option<String>,
    selected: bool,
}

fn parse_java_version(output: &str) -> Option<(String, u32)> {
    let version = output.split('"').nth(1).or_else(|| {
        output.split_whitespace().find(|part| part.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
    })?;
    let major_part = version.split('.').next()?;
    let major = if major_part == "1" {
        version.split('.').nth(1)?.split(['.', '_', '+']).next()?.parse::<u32>().ok()?
    } else {
        major_part.split(['.', '_', '+']).next()?.parse::<u32>().ok()?
    };
    Some((version.to_string(), major))
}

fn java_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        paths.push(PathBuf::from(home).join("bin/java"));
    }
    if let Some(path_value) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_value) {
            paths.push(directory.join("java"));
        }
    }
    for base in [Path::new("/usr/lib/jvm"), Path::new("/usr/java"), Path::new("/opt/java")] {
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("bin/java");
                paths.push(candidate);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    paths.into_iter().filter_map(|path| {
        if !path.is_file() { return None; }
        let canonical = fs::canonicalize(&path).unwrap_or(path);
        if seen.insert(canonical.clone()) { Some(canonical) } else { None }
    }).collect()
}

fn detect_java_runtimes() -> Vec<JavaRuntime> {
    use std::process::Command;
    let mut runtimes = Vec::new();
    for executable in java_candidate_paths() {
        let Ok(output) = Command::new(&executable).arg("-version").output() else { continue; };
        if !output.status.success() { continue; }
        let text = format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout));
        let Some((version, major)) = parse_java_version(&text) else { continue; };
        let vendor = text.lines().next().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string);
        runtimes.push(JavaRuntime {
            executable: executable.to_string_lossy().into_owned(),
            version,
            major,
            vendor,
            selected: false,
        });
    }
    runtimes.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.executable.cmp(&b.executable)));
    runtimes
}

fn selected_java_file(app: &AppHandle) -> Result<PathBuf, String> {
    let mut path = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    path.push("selected-java.json");
    Ok(path)
}

fn selected_java_path(app: &AppHandle) -> Option<PathBuf> {
    let path = selected_java_file(app).ok()?;
    let content = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;
    value.get("executable").and_then(Value::as_str).map(PathBuf::from)
}

fn java_runtimes_for_app(app: &AppHandle) -> Vec<JavaRuntime> {
    let selected = selected_java_path(app).and_then(|path| fs::canonicalize(path).ok());
    let mut runtimes = detect_java_runtimes();
    for runtime in &mut runtimes {
        runtime.selected = selected.as_ref().map(|path| path == Path::new(&runtime.executable)).unwrap_or(false);
    }
    if !runtimes.iter().any(|runtime| runtime.selected) {
        if let Some(first) = runtimes.first_mut() { first.selected = true; }
    }
    runtimes
}

fn java_runtime(app: &AppHandle) -> Value {
    let runtimes = java_runtimes_for_app(app);
    match runtimes.iter().find(|runtime| runtime.selected) {
        Some(runtime) => serde_json::to_value(runtime).map(|mut value| {
            if let Some(object) = value.as_object_mut() { object.insert("available".into(), Value::Bool(true)); }
            value
        }).unwrap_or_else(|_| serde_json::json!({"available": false, "version": null})),
        None => serde_json::json!({"available": false, "executable": null, "version": null, "major": null}),
    }
}

#[tauri::command]
fn list_java_runtimes(app: AppHandle) -> Vec<JavaRuntime> {
    java_runtimes_for_app(&app)
}

#[tauri::command]
fn select_java_runtime(app: AppHandle, executable: String) -> Result<Vec<JavaRuntime>, String> {
    let candidate = fs::canonicalize(&executable).map_err(|e| format!("Cannot resolve Java executable: {e}"))?;
    let valid = detect_java_runtimes().iter().any(|runtime| Path::new(&runtime.executable) == candidate);
    if !valid { return Err("Selected Java executable is not a detected, valid runtime".into()); }
    let path = selected_java_file(&app)?;
    let temporary = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(&serde_json::json!({"executable": candidate}))
        .map_err(|e| e.to_string())?;
    fs::write(&temporary, data).map_err(|e| format!("Cannot save Java selection: {e}"))?;
    fs::rename(&temporary, &path).map_err(|e| format!("Cannot finalize Java selection: {e}"))?;
    Ok(java_runtimes_for_app(&app))
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


fn maven_artifact_task(game_dir: &Path, library: &Value) -> Result<DownloadTask, String> {
    if let Some(artifact) = library.get("downloads").and_then(|v| v.get("artifact")) {
        return artifact_task(game_dir, artifact);
    }
    let coordinate = library.get("name").and_then(Value::as_str).ok_or("Fabric library is missing its Maven coordinate")?;
    let (coordinate, extension) = coordinate.split_once('@').unwrap_or((coordinate, "jar"));
    let parts: Vec<&str> = coordinate.split(':').collect();
    if parts.len() < 3 || parts.len() > 4 || !["jar", "zip"].contains(&extension) {
        return Err(format!("Unsupported Fabric library coordinate: {coordinate}"));
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3).map(|v| format!("-{v}")).unwrap_or_default();
    if [parts[0], artifact, version].iter().any(|part| part.is_empty() || part.contains('/') || part.contains('\\')) {
        return Err(format!("Unsafe Fabric library coordinate: {coordinate}"));
    }
    let relative = format!("{group}/{artifact}/{version}/{artifact}-{version}{classifier}.{extension}");
    let path = game_dir.join("libraries").join(safe_relative_path(&relative)?);
    let base = library.get("url").and_then(Value::as_str).unwrap_or("https://maven.fabricmc.net/");
    if !base.starts_with("https://") { return Err("Fabric library repository must use HTTPS".into()); }
    let url = format!("{}{}", base.trim_end_matches('/'), format!("/{relative}"));
    Ok(DownloadTask { url, path, sha1: None, size: None })
}

#[tauri::command]
async fn install_fabric(app: AppHandle, instance_id: String) -> Result<Value, String> {
    let root = instances_dir(&app)?;
    let mut instance = read_instance(&root, &instance_id)?;
    let game_dir = root.join(&instance_id).join("game");
    let version_dir = game_dir.join("versions").join(&instance.game_version);
    let metadata_path = version_dir.join(format!("{}.json", instance.game_version));
    if !metadata_path.is_file() {
        return Err("Install Minecraft first, then add Fabric.".into());
    }
    let marker_path = root.join(&instance_id).join("installation.json");
    if !marker_path.is_file() { return Err("Minecraft installation marker is missing; repair the game first.".into()); }

    let client = reqwest::Client::builder().user_agent(USER_AGENT).build().map_err(|e| e.to_string())?;
    let versions_url = format!("https://meta.fabricmc.net/v2/versions/loader/{}", instance.game_version);
    let available: Vec<Value> = client.get(&versions_url).send().await
        .map_err(|e| format!("Cannot query Fabric versions: {e}"))?
        .error_for_status().map_err(|e| format!("Fabric version query failed: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse Fabric versions: {e}"))?;
    let selected = available.iter().find(|item| item.get("loader").and_then(|v| v.get("stable")).and_then(Value::as_bool) == Some(true))
        .or_else(|| available.first()).ok_or_else(|| format!("Fabric does not currently support Minecraft {}", instance.game_version))?;
    let loader_version = selected.get("loader").and_then(|v| v.get("version")).and_then(Value::as_str)
        .ok_or("Fabric response did not include a loader version")?;
    let profile_url = format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json", instance.game_version, loader_version);
    let profile: Value = client.get(&profile_url).send().await
        .map_err(|e| format!("Cannot download Fabric profile: {e}"))?
        .error_for_status().map_err(|e| format!("Fabric profile request failed: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse Fabric profile: {e}"))?;

    let mut metadata: Value = serde_json::from_slice(&fs::read(&metadata_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot parse installed Minecraft metadata: {e}"))?;
    let main_class = profile.get("mainClass").and_then(Value::as_str)
        .ok_or("Fabric profile is missing its client main class")?;
    metadata["mainClass"] = Value::String(main_class.to_string());

    let mut libraries = metadata.get("libraries").and_then(Value::as_array).cloned().unwrap_or_default();
    let fabric_libraries = profile.get("libraries").and_then(Value::as_array)
        .ok_or("Fabric profile does not contain a libraries list")?;
    let mut tasks = Vec::new();
    for library in fabric_libraries {
        let name = library.get("name").and_then(Value::as_str).ok_or("Fabric library has no name")?;
        if !libraries.iter().any(|existing| existing.get("name").and_then(Value::as_str) == Some(name)) {
            libraries.push(library.clone());
        }
        if rule_allows(library) { tasks.push(maven_artifact_task(&game_dir, library)?); }
    }
    let (downloaded, existing) = download_many(&client, tasks).await?;
    metadata["libraries"] = Value::Array(libraries);

    if let Some(profile_args) = profile.get("arguments").and_then(Value::as_object) {
        for key in ["jvm", "game"] {
            if let Some(extra) = profile_args.get(key).and_then(Value::as_array) {
                let target = metadata["arguments"][key].as_array_mut();
                if let Some(target) = target { target.extend(extra.iter().cloned()); }
                else { metadata["arguments"][key] = Value::Array(extra.clone()); }
            }
        }
    }
    let serialized = serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?;
    let temporary = metadata_path.with_extension("json.tmp");
    fs::write(&temporary, serialized).map_err(|e| format!("Cannot stage Fabric metadata: {e}"))?;
    fs::rename(&temporary, &metadata_path).map_err(|e| format!("Cannot activate Fabric metadata: {e}"))?;

    instance.loader = format!("Fabric {loader_version}");
    let instance_path = root.join(&instance_id).join("instance.json");
    let instance_temp = instance_path.with_extension("json.tmp");
    fs::write(&instance_temp, serde_json::to_vec_pretty(&instance).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot save Fabric instance metadata: {e}"))?;
    fs::rename(&instance_temp, &instance_path).map_err(|e| format!("Cannot finalize Fabric instance metadata: {e}"))?;

    let mut marker: Value = serde_json::from_slice(&fs::read(&marker_path).map_err(|e| e.to_string())?)
        .unwrap_or_else(|_| serde_json::json!({}));
    marker["loader"] = Value::String(instance.loader.clone());
    fs::write(&marker_path, serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot update installation marker: {e}"))?;

    Ok(serde_json::json!({
        "minecraftVersion": instance.game_version,
        "loader": instance.loader,
        "librariesDownloaded": downloaded,
        "librariesAlreadyPresent": existing,
        "status": "installed"
    }))
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
        "java": java_runtime(&app),
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
    let java = java_runtime(&app);
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


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModrinthMod {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    icon_url: Option<String>,
    downloads: u64,
    author: String,
    latest_version: Option<String>,
}

fn modrinth_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent("ComfyCraft/0.1.0 (https://github.com/sartywerty15664-prog/ComfyCraft)")
        .connect_timeout(std::time::Duration::from_secs(12))
        .timeout(std::time::Duration::from_secs(90))
        .build().map_err(|e| format!("Cannot initialize Modrinth client: {e}"))
}

fn valid_modrinth_project_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[tauri::command]
async fn search_modrinth_mods(app: AppHandle, instance_id: String, query: String) -> Result<Vec<ModrinthMod>, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    if !instance.loader.starts_with("Fabric ") {
        return Err("Install Fabric for this instance before browsing Fabric mods.".into());
    }
    let query = query.trim();
    if query.len() > 100 { return Err("Search query is too long.".into()); }
    let client = modrinth_client()?;
    let facets = serde_json::json!([
        ["project_type:mod"],
        [format!("versions:{}", instance.game_version)],
        ["categories:fabric"]
    ]).to_string();
    let response: Value = client.get("https://api.modrinth.com/v2/search")
        .query(&[("query", query), ("facets", facets.as_str()), ("limit", "30")])
        .send().await.map_err(|e| format!("Modrinth search failed: {e}"))?
        .error_for_status().map_err(|e| format!("Modrinth search returned an error: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse Modrinth search results: {e}"))?;
    let hits = response.get("hits").and_then(Value::as_array).ok_or("Modrinth returned no results list")?;
    Ok(hits.iter().filter_map(|hit| Some(ModrinthMod {
        project_id: hit.get("project_id")?.as_str()?.to_string(),
        slug: hit.get("slug").and_then(Value::as_str).unwrap_or_default().to_string(),
        title: hit.get("title").and_then(Value::as_str).unwrap_or("Untitled mod").to_string(),
        description: hit.get("description").and_then(Value::as_str).unwrap_or_default().to_string(),
        icon_url: hit.get("icon_url").and_then(Value::as_str).map(str::to_string),
        downloads: hit.get("downloads").and_then(Value::as_u64).unwrap_or(0),
        author: hit.get("author").and_then(Value::as_str).unwrap_or("Unknown").to_string(),
        latest_version: hit.get("latest_version").and_then(Value::as_str).map(str::to_string),
    })).collect())
}

fn safe_mod_filename(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 180 || name == "." || name == ".."
        || name.contains('/') || name.contains('\\') || name.chars().any(char::is_control) {
        return Err("Modrinth returned an unsafe file name.".into());
    }
    if !name.to_ascii_lowercase().ends_with(".jar") {
        return Err("Selected Modrinth file is not a JAR.".into());
    }
    Ok(name.to_string())
}

fn sha1_matches(bytes: &[u8], expected: &str) -> bool {
    format!("{:x}", Sha1::digest(bytes)).eq_ignore_ascii_case(expected)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledModEntry {
    project_id: String,
    version_id: String,
    name: String,
    filename: String,
    sha1: String,
}

fn read_installed_mods(instance_dir: &Path) -> Result<Vec<InstalledModEntry>, String> {
    let path = instance_dir.join(".comfycraft-mods.json");
    if !path.exists() { return Ok(Vec::new()); }
    let data = fs::read(path).map_err(|e| format!("Cannot read mod manifest: {e}"))?;
    serde_json::from_slice(&data).map_err(|e| format!("Invalid mod manifest: {e}"))
}

fn write_installed_mods(instance_dir: &Path, entries: &[InstalledModEntry]) -> Result<(), String> {
    let path = instance_dir.join(".comfycraft-mods.json");
    let temp = instance_dir.join(".comfycraft-mods.json.tmp");
    let data = serde_json::to_vec_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(&temp, data).map_err(|e| format!("Cannot stage mod manifest: {e}"))?;
    fs::rename(temp, path).map_err(|e| format!("Cannot save mod manifest: {e}"))
}

#[tauri::command]
fn list_installed_mods(app: AppHandle, instance_id: String) -> Result<Vec<InstalledModEntry>, String> {
    let root = instances_dir(&app)?;
    let _instance = read_instance(&root, &instance_id)?;
    read_installed_mods(&root.join(instance_id))
}

async fn modrinth_compatible_version(client: &reqwest::Client, project_id: &str, game_version: &str) -> Result<Value, String> {
    let url = format!("https://api.modrinth.com/v2/project/{project_id}/version");
    let versions: Vec<Value> = client.get(url).query(&[
        ("game_versions", format!("[\"{game_version}\"]")),
        ("loaders", "[\"fabric\"]".to_string()),
    ]).send().await.map_err(|e| format!("Cannot resolve required mod dependency: {e}"))?
        .error_for_status().map_err(|e| format!("Mod dependency lookup failed: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse dependency versions: {e}"))?;
    versions.into_iter().next().ok_or_else(|| format!("No Fabric version compatible with Minecraft {game_version} was found for dependency {project_id}"))
}

#[tauri::command]
async fn install_modrinth_mod(app: AppHandle, instance_id: String, project_id: String) -> Result<Vec<InstalledModEntry>, String> {
    if !valid_modrinth_project_id(&project_id) { return Err("Invalid Modrinth project identifier.".into()); }
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    if !instance.loader.starts_with("Fabric ") {
        return Err("This instance needs Fabric before mods can be installed.".into());
    }
    let instance_dir = root.join(&instance_id);
    let mods_dir = instance_dir.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("Cannot create mods directory: {e}"))?;
    let client = modrinth_client()?;
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(project_id);
    let mut visited = std::collections::HashSet::new();
    let mut pending: Vec<(String, String, String, String, Vec<u8>)> = Vec::new();

    while let Some(project) = queue.pop_front() {
        if !visited.insert(project.clone()) { continue; }
        if visited.len() > 40 { return Err("Dependency tree is too large; installation stopped.".into()); }
        let version = modrinth_compatible_version(&client, &project, &instance.game_version).await?;
        if let Some(dependencies) = version.get("dependencies").and_then(Value::as_array) {
            for dependency in dependencies {
                if dependency.get("dependency_type").and_then(Value::as_str) != Some("required") { continue; }
                if let Some(version_id) = dependency.get("version_id").and_then(Value::as_str) {
                    let dep_url = format!("https://api.modrinth.com/v2/version/{version_id}");
                    let dep_version: Value = client.get(dep_url).send().await
                        .map_err(|e| format!("Cannot inspect required dependency: {e}"))?
                        .error_for_status().map_err(|e| format!("Required dependency lookup failed: {e}"))?
                        .json().await.map_err(|e| format!("Cannot parse required dependency: {e}"))?;
                    let dep_project = dep_version.get("project_id").and_then(Value::as_str)
                        .ok_or("Dependency version has no project ID")?.to_string();
                    if !visited.contains(&dep_project) { queue.push_back(dep_project); }
                } else if let Some(dep_project) = dependency.get("project_id").and_then(Value::as_str) {
                    if !visited.contains(dep_project) { queue.push_back(dep_project.to_string()); }
                }
            }
        }
        let version_id = version.get("id").and_then(Value::as_str).ok_or("Modrinth version has no ID")?.to_string();
        let files = version.get("files").and_then(Value::as_array).ok_or("Modrinth version has no files")?;
        let file = files.iter().find(|file| file.get("primary").and_then(Value::as_bool) == Some(true))
            .or_else(|| files.first()).ok_or("Modrinth version has no downloadable file")?;
        let filename = safe_mod_filename(file.get("filename").and_then(Value::as_str).ok_or("Modrinth file has no name")?)?;
        let url = file.get("url").and_then(Value::as_str).ok_or("Modrinth file has no URL")?;
        if !url.starts_with("https://") { return Err("Refusing non-HTTPS Modrinth download.".into()); }
        let hashes = file.get("hashes").ok_or("Modrinth file has no hashes")?;
        let expected_sha1 = hashes.get("sha1").and_then(Value::as_str).ok_or("Modrinth file has no SHA-1 hash")?.to_string();
        let bytes = client.get(url).send().await.map_err(|e| format!("Mod download failed: {e}"))?
            .error_for_status().map_err(|e| format!("Mod download returned an error: {e}"))?
            .bytes().await.map_err(|e| format!("Cannot read mod download: {e}"))?.to_vec();
        if !sha1_matches(&bytes, &expected_sha1) {
            return Err(format!("SHA-1 verification failed for {filename}"));
        }
        let title = version.get("name").and_then(Value::as_str).unwrap_or(&project).to_string();
        pending.push((project, version_id, title, filename, bytes));
    }

    let mut entries = read_installed_mods(&instance_dir)?;
    let mut staged = Vec::new();
    for (project, version, name, filename, bytes) in pending {
        let target = mods_dir.join(&filename);
        if target.exists() && !entries.iter().any(|entry| entry.filename == filename && entry.project_id == project) {
            return Err(format!("A file named {filename} already exists and is not managed by ComfyCraft; it was left untouched."));
        }
        let temp = mods_dir.join(format!(".{filename}.comfycraft-part"));
        fs::write(&temp, &bytes).map_err(|e| format!("Cannot stage {filename}: {e}"))?;
        staged.push((temp, target, InstalledModEntry {
            project_id: project, version_id: version, name, filename,
            sha1: format!("{:x}", Sha1::digest(&bytes)),
            active: false,
        }));
    }
    for (temp, target, entry) in staged {
        if let Some(existing) = entries.iter().find(|item| item.project_id == entry.project_id) {
            let old_path = mods_dir.join(&existing.filename);
            if old_path != target { let _ = fs::remove_file(old_path); }
        }
        fs::rename(temp, &target).map_err(|e| format!("Cannot activate mod file: {e}"))?;
        entries.retain(|item| item.project_id != entry.project_id);
        entries.push(entry);
    }
    write_installed_mods(&instance_dir, &entries)?;
    Ok(entries)
}

#[tauri::command]
fn uninstall_modrinth_mod(app: AppHandle, instance_id: String, project_id: String) -> Result<Vec<InstalledModEntry>, String> {
    let root = instances_dir(&app)?;
    let _instance = read_instance(&root, &instance_id)?;
    let instance_dir = root.join(&instance_id);
    let mods_dir = instance_dir.join("mods");
    let mut entries = read_installed_mods(&instance_dir)?;
    let Some(entry) = entries.iter().find(|entry| entry.project_id == project_id).cloned() else {
        return Err("This mod is not managed by ComfyCraft.".into());
    };
    let path = mods_dir.join(safe_mod_filename(&entry.filename)?);
    if path.exists() {
        let bytes = fs::read(&path).map_err(|e| format!("Cannot verify installed mod before removal: {e}"))?;
        if !sha1_matches(&bytes, &entry.sha1) {
            return Err("The installed file changed outside ComfyCraft; it was left untouched.".into());
        }
        fs::remove_file(path).map_err(|e| format!("Cannot remove mod file: {e}"))?;
    }
    entries.retain(|item| item.project_id != project_id);
    write_installed_mods(&instance_dir, &entries)?;
    Ok(entries)
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModUpdate {
    project_id: String,
    current_version_id: String,
    latest_version_id: String,
    latest_name: String,
}

#[tauri::command]
async fn check_mod_updates(app: AppHandle, instance_id: String) -> Result<Vec<ModUpdate>, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    if !instance.loader.starts_with("Fabric ") { return Ok(Vec::new()); }
    let entries = read_installed_mods(&root.join(&instance_id))?;
    let client = modrinth_client()?;
    let mut updates = Vec::new();
    for entry in entries {
        let url = format!("https://api.modrinth.com/v2/project/{}/version", entry.project_id);
        let versions: Vec<Value> = client.get(url).query(&[
            ("game_versions", format!("[\"{}\"]", instance.game_version)),
            ("loaders", "[\"fabric\"]".to_string()),
        ]).send().await.map_err(|e| format!("Cannot check updates for {}: {e}", entry.name))?
            .error_for_status().map_err(|e| format!("Modrinth update check failed: {e}"))?
            .json().await.map_err(|e| format!("Cannot parse update information: {e}"))?;
        if let Some(latest) = versions.first() {
            let latest_id = latest.get("id").and_then(Value::as_str).unwrap_or_default();
            if !latest_id.is_empty() && latest_id != entry.version_id {
                updates.push(ModUpdate {
                    project_id: entry.project_id,
                    current_version_id: entry.version_id,
                    latest_version_id: latest_id.to_string(),
                    latest_name: latest.get("name").and_then(Value::as_str).unwrap_or("New version").to_string(),
                });
            }
        }
    }
    Ok(updates)
}

fn content_manifest_name(content_type: &str) -> Result<(&'static str, &'static str), String> {
    match content_type {
        "resourcepack" => Ok(("resourcepacks", ".comfycraft-resourcepacks.json")),
        "shader" => Ok(("shaderpacks", ".comfycraft-shaders.json")),
        _ => Err("Unsupported content type.".into()),
    }
}

fn safe_content_filename(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 180 || name == "." || name == ".."
        || name.contains('/') || name.contains('\\') || name.chars().any(char::is_control) {
        return Err("Modrinth returned an unsafe file name.".into());
    }
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".zip") {
        return Err("Resource packs and shader packs must be ZIP files.".into());
    }
    Ok(name.to_string())
}

fn read_content_manifest(instance_dir: &Path, filename: &str) -> Result<Vec<InstalledModEntry>, String> {
    let path = instance_dir.join(filename);
    if !path.exists() { return Ok(Vec::new()); }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read content manifest: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid content manifest: {e}"))
}

fn write_content_manifest(instance_dir: &Path, filename: &str, entries: &[InstalledModEntry]) -> Result<(), String> {
    let path = instance_dir.join(filename);
    let temp = instance_dir.join(format!("{filename}.tmp"));
    let bytes = serde_json::to_vec_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(&temp, bytes).map_err(|e| format!("Cannot stage content manifest: {e}"))?;
    fs::rename(temp, path).map_err(|e| format!("Cannot save content manifest: {e}"))
}

#[tauri::command]
async fn search_modrinth_content(app: AppHandle, instance_id: String, query: String, content_type: String) -> Result<Vec<ModrinthMod>, String> {
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    if !matches!(content_type.as_str(), "resourcepack" | "shader") {
        return Err("Unsupported content type.".into());
    }
    if query.trim().len() > 100 { return Err("Search query is too long.".into()); }
    let client = modrinth_client()?;
    let facets = serde_json::json!([
        [format!("project_type:{content_type}")],
        [format!("versions:{}", instance.game_version)]
    ]).to_string();
    let response: Value = client.get("https://api.modrinth.com/v2/search")
        .query(&[("query", query.trim()), ("facets", facets.as_str()), ("limit", "30")])
        .send().await.map_err(|e| format!("Modrinth search failed: {e}"))?
        .error_for_status().map_err(|e| format!("Modrinth search returned an error: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse Modrinth results: {e}"))?;
    let hits = response.get("hits").and_then(Value::as_array).ok_or("Modrinth returned no results list")?;
    Ok(hits.iter().filter_map(|hit| Some(ModrinthMod {
        project_id: hit.get("project_id")?.as_str()?.to_string(),
        slug: hit.get("slug").and_then(Value::as_str).unwrap_or_default().to_string(),
        title: hit.get("title").and_then(Value::as_str).unwrap_or("Untitled project").to_string(),
        description: hit.get("description").and_then(Value::as_str).unwrap_or_default().to_string(),
        icon_url: hit.get("icon_url").and_then(Value::as_str).map(str::to_string),
        downloads: hit.get("downloads").and_then(Value::as_u64).unwrap_or(0),
        author: hit.get("author").and_then(Value::as_str).unwrap_or("Unknown").to_string(),
        latest_version: hit.get("latest_version").and_then(Value::as_str).map(str::to_string),
    })).collect())
}

#[tauri::command]
fn list_installed_content(app: AppHandle, instance_id: String, content_type: String) -> Result<Vec<InstalledModEntry>, String> {
    let root = instances_dir(&app)?;
    let _instance = read_instance(&root, &instance_id)?;
    let (folder, manifest) = content_manifest_name(&content_type)?;
    let instance_dir = root.join(instance_id);
    let options = fs::read_to_string(instance_dir.join("options.txt")).unwrap_or_default();
    let mut entries = read_content_manifest(&instance_dir, manifest)?;
    let active_packs: Vec<String> = options.lines().find_map(|line| line.strip_prefix("resourcePacks:"))
        .and_then(|value| serde_json::from_str(value).ok()).unwrap_or_default();
    let active_shader = options.lines().find_map(|line| line.strip_prefix("shaderPack=")).unwrap_or("");
    for entry in &mut entries {
        let _ = folder;
        entry.active = if content_type == "resourcepack" {
            active_packs.contains(&format!("file/{}", entry.filename))
        } else {
            active_shader == entry.filename
        };
    }
    Ok(entries)
}

fn write_options_values(instance_dir: &Path, updates: &[(&str, String)]) -> Result<(), String> {
    let path = instance_dir.join("options.txt");
    let current = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = current.lines().map(str::to_string).collect();
    for (key, value) in updates {
        let prefix = format!("{key}:");
        let equals = format!("{key}=");
        let replacement = if key == &"resourcePacks" { format!("{prefix}{value}") } else { format!("{equals}{value}") };
        if let Some(index) = lines.iter().position(|line| line.starts_with(&prefix) || line.starts_with(&equals)) {
            lines[index] = replacement;
        } else {
            lines.push(replacement);
        }
    }
    let temp = instance_dir.join("options.txt.comfycraft.tmp");
    let mut data = lines.join("\n");
    data.push('\n');
    fs::write(&temp, data).map_err(|e| format!("Cannot stage Minecraft options: {e}"))?;
    fs::rename(temp, path).map_err(|e| format!("Cannot save Minecraft options: {e}"))
}

#[tauri::command]
fn set_modrinth_content_enabled(app: AppHandle, instance_id: String, project_id: String, content_type: String, enabled: bool) -> Result<Vec<InstalledModEntry>, String> {
    if !valid_modrinth_project_id(&project_id) { return Err("Invalid Modrinth project identifier.".into()); }
    let root = instances_dir(&app)?;
    let _instance = read_instance(&root, &instance_id)?;
    let (folder, manifest) = content_manifest_name(&content_type)?;
    let instance_dir = root.join(&instance_id);
    let mut entries = read_content_manifest(&instance_dir, manifest)?;
    let entry = entries.iter().find(|item| item.project_id == project_id).cloned()
        .ok_or("This content is not managed by ComfyCraft.")?;
    let file_path = instance_dir.join(folder).join(safe_content_filename(&entry.filename)?);
    let bytes = fs::read(&file_path).map_err(|e| format!("Cannot verify content file: {e}"))?;
    if !sha1_matches(&bytes, &entry.sha1) { return Err("Content file integrity check failed; activation was not changed.".into()); }
    let options = fs::read_to_string(instance_dir.join("options.txt")).unwrap_or_default();
    if content_type == "resourcepack" {
        let mut packs: Vec<String> = options.lines().find_map(|line| line.strip_prefix("resourcePacks:"))
            .and_then(|value| serde_json::from_str(value).ok()).unwrap_or_else(|| vec!["vanilla".into()]);
        let pack_id = format!("file/{}", entry.filename);
        packs.retain(|item| item != &pack_id);
        if enabled { packs.push(pack_id); }
        let encoded = serde_json::to_string(&packs).map_err(|e| e.to_string())?;
        write_options_values(&instance_dir, &[("resourcePacks", encoded)])?;
    } else {
        if enabled {
            let installed_mods = read_installed_mods(&instance_dir)?;
            if !installed_mods.iter().any(|item| item.project_id == "YL57xq9U") {
                return Err("Install Iris for this Fabric instance before enabling shader packs.".into());
            }
        }
        let current = options.lines().find_map(|line| line.strip_prefix("shaderPack=")).unwrap_or("");
        let next = if enabled { entry.filename.as_str() } else if current == entry.filename { "" } else { current };
        write_options_values(&instance_dir, &[("shaderPack", next.to_string())])?;
    }
    let mut updated = read_content_manifest(&instance_dir, manifest)?;
    let active_packs: Vec<String> = fs::read_to_string(instance_dir.join("options.txt")).unwrap_or_default().lines()
        .find_map(|line| line.strip_prefix("resourcePacks:")).and_then(|value| serde_json::from_str(value).ok()).unwrap_or_default();
    let active_shader = fs::read_to_string(instance_dir.join("options.txt")).unwrap_or_default().lines()
        .find_map(|line| line.strip_prefix("shaderPack=")).unwrap_or("").to_string();
    for item in &mut updated {
        item.active = if content_type == "resourcepack" { active_packs.contains(&format!("file/{}", item.filename)) }
            else { active_shader == item.filename };
    }
    Ok(updated)
}

#[tauri::command]
async fn install_modrinth_content(app: AppHandle, instance_id: String, project_id: String, content_type: String) -> Result<Vec<InstalledModEntry>, String> {
    if !valid_modrinth_project_id(&project_id) { return Err("Invalid Modrinth project identifier.".into()); }
    let root = instances_dir(&app)?;
    let instance = read_instance(&root, &instance_id)?;
    let (folder, manifest) = content_manifest_name(&content_type)?;
    let instance_dir = root.join(&instance_id);
    let target_dir = instance_dir.join(folder);
    fs::create_dir_all(&target_dir).map_err(|e| format!("Cannot create {folder} directory: {e}"))?;
    let client = modrinth_client()?;
    let url = format!("https://api.modrinth.com/v2/project/{project_id}/version");
    let versions: Vec<Value> = client.get(url).query(&[("game_versions", format!("[\"{}\"]", instance.game_version))])
        .send().await.map_err(|e| format!("Cannot resolve compatible content version: {e}"))?
        .error_for_status().map_err(|e| format!("Content version lookup failed: {e}"))?
        .json().await.map_err(|e| format!("Cannot parse content versions: {e}"))?;
    let version = versions.first().ok_or_else(|| format!("No {content_type} version supports Minecraft {}.", instance.game_version))?;
    let version_id = version.get("id").and_then(Value::as_str).ok_or("Modrinth version has no ID")?.to_string();
    let files = version.get("files").and_then(Value::as_array).ok_or("Modrinth version has no files")?;
    let file = files.iter().find(|f| f.get("primary").and_then(Value::as_bool) == Some(true))
        .or_else(|| files.first()).ok_or("Modrinth version has no downloadable file")?;
    let filename = safe_content_filename(file.get("filename").and_then(Value::as_str).ok_or("Modrinth file has no filename")?)?;
    let download_url = file.get("url").and_then(Value::as_str).ok_or("Modrinth file has no URL")?;
    if !download_url.starts_with("https://") { return Err("Refusing non-HTTPS Modrinth download.".into()); }
    let expected = file.get("hashes").and_then(|h| h.get("sha1")).and_then(Value::as_str).ok_or("Modrinth file has no SHA-1 hash")?;
    let bytes = client.get(download_url).send().await.map_err(|e| format!("Content download failed: {e}"))?
        .error_for_status().map_err(|e| format!("Content download returned an error: {e}"))?
        .bytes().await.map_err(|e| format!("Cannot read content download: {e}"))?.to_vec();
    if !sha1_matches(&bytes, expected) { return Err(format!("SHA-1 verification failed for {filename}")); }
    let mut entries = read_content_manifest(&instance_dir, manifest)?;
    let target = target_dir.join(&filename);
    if target.exists() && !entries.iter().any(|entry| entry.project_id == project_id && entry.filename == filename) {
        return Err(format!("{filename} already exists and is not managed by ComfyCraft; it was left untouched."));
    }
    let temp = target_dir.join(format!(".{filename}.comfycraft-part"));
    fs::write(&temp, &bytes).map_err(|e| format!("Cannot stage {filename}: {e}"))?;
    if let Some(old) = entries.iter().find(|entry| entry.project_id == project_id) {
        let old_path = target_dir.join(safe_content_filename(&old.filename)?);
        if old_path != target && old_path.exists() { fs::remove_file(old_path).map_err(|e| format!("Cannot replace old content file: {e}"))?; }
    }
    fs::rename(temp, &target).map_err(|e| format!("Cannot activate content file: {e}"))?;
    entries.retain(|entry| entry.project_id != project_id);
    entries.push(InstalledModEntry {
        project_id, version_id,
        name: version.get("name").and_then(Value::as_str).unwrap_or("Installed content").to_string(),
        filename, sha1: format!("{:x}", Sha1::digest(&bytes)), active: false,
    });
    write_content_manifest(&instance_dir, manifest, &entries)?;
    Ok(entries)
}

#[tauri::command]
fn uninstall_modrinth_content(app: AppHandle, instance_id: String, project_id: String, content_type: String) -> Result<Vec<InstalledModEntry>, String> {
    let root = instances_dir(&app)?;
    let _instance = read_instance(&root, &instance_id)?;
    let (folder, manifest) = content_manifest_name(&content_type)?;
    let instance_dir = root.join(&instance_id);
    let target_dir = instance_dir.join(folder);
    let mut entries = read_content_manifest(&instance_dir, manifest)?;
    let entry = entries.iter().find(|item| item.project_id == project_id).cloned()
        .ok_or("This content is not managed by ComfyCraft.")?;
    let path = target_dir.join(safe_content_filename(&entry.filename)?);
    if path.exists() {
        let bytes = fs::read(&path).map_err(|e| format!("Cannot verify content before removal: {e}"))?;
        if !sha1_matches(&bytes, &entry.sha1) { return Err("The installed file changed outside ComfyCraft; it was left untouched.".into()); }
        fs::remove_file(path).map_err(|e| format!("Cannot remove content file: {e}"))?;
    }
    entries.retain(|item| item.project_id != project_id);
    write_content_manifest(&instance_dir, manifest, &entries)?;
    Ok(entries)
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
            get_app_info, list_game_versions, list_java_runtimes, select_java_runtime, install_game, install_fabric, search_modrinth_mods, install_modrinth_mod, list_installed_mods, uninstall_modrinth_mod, check_mod_updates, search_modrinth_content, list_installed_content, install_modrinth_content, uninstall_modrinth_content, set_modrinth_content_enabled, get_runtime_info, inspect_launch_arguments, check_launch_readiness, is_game_installed,
            list_instances, create_instance, rename_instance, delete_instance
        ])
        .run(tauri::generate_context!())
        .expect("error while running ComfyCraft");
}


#[cfg(test)]
mod launch_rule_tests {
    use super::*;

    #[test]
    fn parses_modern_java_versions() {
        assert_eq!(parse_java_version("openjdk version \"25.0.1\" 2025-04-15"), Some(("25.0.1".into(), 25)));
        assert_eq!(parse_java_version("java version \"21\" 2023-09-19"), Some(("21".into(), 21)));
    }

    #[test]
    fn parses_legacy_java_versions() {
        assert_eq!(parse_java_version("java version \"1.8.0_402\""), Some(("1.8.0_402".into(), 8)));
    }

    #[test]
    fn rejects_unrecognized_java_version_output() {
        assert_eq!(parse_java_version("java launcher unavailable"), None);
    }

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

    #[test]
    fn modrinth_project_ids_are_path_safe() {
        assert!(valid_modrinth_project_id("AANobbMI"));
        assert!(!valid_modrinth_project_id("../AANobbMI"));
        assert!(!valid_modrinth_project_id("bad/id"));
        assert!(!valid_modrinth_project_id("")); 
    }

    #[test]
    fn content_types_are_allowlisted() {
        assert_eq!(content_manifest_name("resourcepack").unwrap().0, "resourcepacks");
        assert_eq!(content_manifest_name("shader").unwrap().0, "shaderpacks");
        assert!(content_manifest_name("mods").is_err());
    }

    #[test]
    fn content_filenames_reject_traversal_and_non_zip_files() {
        assert_eq!(safe_content_filename("pack.zip").unwrap(), "pack.zip");
        assert!(safe_content_filename("../pack.zip").is_err());
        assert!(safe_content_filename("pack.jar").is_err());
    }
}
