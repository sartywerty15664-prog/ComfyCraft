use serde::Serialize;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetVerification {
    pub index_id: String,
    pub total: usize,
    pub verified: usize,
    pub missing: usize,
    pub corrupt: usize,
    pub bytes_verified: u64,
    pub workers_used: usize,
    pub elapsed_ms: u128,
    pub problem_samples: Vec<String>,
}

/// Verify the local Minecraft asset object cache using a bounded native Rust
/// worker pool. Each worker reads and hashes independent immutable files;
/// no Minecraft game state is touched from these threads.
#[tauri::command]
pub fn verify_asset_cache(app: AppHandle, instance_id: String) -> Result<AssetVerification, String> {
    let root = super::instances_dir(&app)?;
    let instance = super::read_instance(&root, &instance_id)?;
    let game_dir = root.join(&instance_id).join("game");
    let metadata_path = game_dir
        .join("versions")
        .join(&instance.game_version)
        .join(format!("{}.json", instance.game_version));
    let metadata: Value = serde_json::from_slice(
        &fs::read(&metadata_path).map_err(|e| format!("Cannot read game metadata: {e}"))?
    ).map_err(|e| format!("Cannot parse game metadata: {e}"))?;

    let index_id = metadata
        .get("assetIndex")
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        .ok_or("Game metadata does not declare an asset index")?
        .to_string();
    if index_id.is_empty() || index_id.len() > 32 || !index_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err("Asset index identifier is unsafe".into());
    }

    let index_path = game_dir.join("assets").join("indexes").join(format!("{index_id}.json"));
    let index: Value = serde_json::from_slice(
        &fs::read(&index_path).map_err(|e| format!("Cannot read asset index {index_id}: {e}"))?
    ).map_err(|e| format!("Asset index is invalid JSON: {e}"))?;
    let objects = index.get("objects").and_then(Value::as_object)
        .ok_or("Asset index does not contain an objects map")?;

    let mut entries = Vec::with_capacity(objects.len());
    for (logical_name, object) in objects {
        let hash = object.get("hash").and_then(Value::as_str)
            .ok_or_else(|| format!("Asset object {logical_name} has no hash"))?;
        if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Asset object {logical_name} has an invalid SHA-1 hash"));
        }
        let size = object.get("size").and_then(Value::as_u64)
            .ok_or_else(|| format!("Asset object {logical_name} has no valid size"))?;
        let path = game_dir.join("assets").join("objects").join(&hash[..2]).join(hash);
        entries.push((path, hash.to_ascii_lowercase(), size));
    }

    let total = entries.len();
    let entries = Arc::new(entries);
    let cursor = AtomicUsize::new(0);
    let verified = AtomicUsize::new(0);
    let missing = AtomicUsize::new(0);
    let corrupt = AtomicUsize::new(0);
    let bytes_verified = std::sync::atomic::AtomicU64::new(0);
    let problem_samples = Mutex::new(Vec::<String>::new());
    let available = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).max(1);
    let workers = available.min(16).min(total.max(1));
    let started = Instant::now();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let entries = Arc::clone(&entries);
            let cursor = &cursor;
            let verified = &verified;
            let missing = &missing;
            let corrupt = &corrupt;
            let bytes_verified = &bytes_verified;
            let problem_samples = &problem_samples;
            scope.spawn(move || loop {
                let index = cursor.fetch_add(1, Ordering::Relaxed);
                let Some((path, expected_hash, expected_size)) = entries.get(index) else { break; };
                match fs::metadata(path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        missing.fetch_add(1, Ordering::Relaxed);
                        let mut samples = problem_samples.lock().expect("sample mutex poisoned");
                        if samples.len() < 12 { samples.push(format!("missing: {}", path.display())); }
                    }
                    Err(error) => {
                        corrupt.fetch_add(1, Ordering::Relaxed);
                        let mut samples = problem_samples.lock().expect("sample mutex poisoned");
                        if samples.len() < 12 { samples.push(format!("unreadable: {} ({error})", path.display())); }
                    }
                    Ok(metadata) if !metadata.is_file() || metadata.len() != *expected_size => {
                        corrupt.fetch_add(1, Ordering::Relaxed);
                        let mut samples = problem_samples.lock().expect("sample mutex poisoned");
                        if samples.len() < 12 { samples.push(format!("size mismatch: {}", path.display())); }
                    }
                    Ok(_) => match fs::read(path) {
                        Ok(bytes) => {
                            let actual = format!("{:x}", Sha1::digest(&bytes));
                            if actual == expected_hash.as_str() {
                                verified.fetch_add(1, Ordering::Relaxed);
                                bytes_verified.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                            } else {
                                corrupt.fetch_add(1, Ordering::Relaxed);
                                let mut samples = problem_samples.lock().expect("sample mutex poisoned");
                                if samples.len() < 12 { samples.push(format!("SHA-1 mismatch: {}", path.display())); }
                            }
                        }
                        Err(error) => {
                            corrupt.fetch_add(1, Ordering::Relaxed);
                            let mut samples = problem_samples.lock().expect("sample mutex poisoned");
                            if samples.len() < 12 { samples.push(format!("read failed: {} ({error})", path.display())); }
                        }
                    }
                }
            });
        }
    });

    Ok(AssetVerification {
        index_id,
        total,
        verified: verified.load(Ordering::Relaxed),
        missing: missing.load(Ordering::Relaxed),
        corrupt: corrupt.load(Ordering::Relaxed),
        bytes_verified: bytes_verified.load(Ordering::Relaxed),
        workers_used: workers,
        elapsed_ms: started.elapsed().as_millis(),
        problem_samples: problem_samples.into_inner().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_format_is_lowercase_hex() {
        let hash = format!("{:x}", Sha1::digest(b"comfycraft"));
        assert_eq!(hash.len(), 40);
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    }

    #[test]
    fn rejects_path_like_asset_index_ids() {
        for value in ["../34", "34/evil", ""] {
            assert!(value.is_empty() || value.len() > 32 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        }
        let oversized = "a".repeat(33);
        assert!(oversized.len() > 32);
    }
}
