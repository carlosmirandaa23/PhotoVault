use serde::Serialize;
use std::{collections::BTreeMap, fs, path::PathBuf};

mod duplicates;
mod hash_cache;

#[derive(Serialize)]
struct ScanSummary {
    root: String,
    files: u64,
    bytes: u64,
    errors: u64,
    by_extension: BTreeMap<String, u64>,
}

const MEDIA: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "heic", "heif", "avif", "dng", "cr2", "nef", "arw", "raf", "orf", "rw2", "mp4", "mov", "avi", "mkv", "mts", "3gp"];

#[tauri::command]
fn scan_folder(root: String) -> Result<ScanSummary, String> {
    let path = PathBuf::from(&root);
    if !path.is_dir() { return Err("La carpeta seleccionada no existe o no es accesible".into()); }
    let mut result = ScanSummary { root, files: 0, bytes: 0, errors: 0, by_extension: BTreeMap::new() };
    let mut stack = vec![path];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => { result.errors += 1; continue; }
        };
        for entry in entries {
            let entry = match entry { Ok(entry) => entry, Err(_) => { result.errors += 1; continue; } };
            let kind = match entry.file_type() { Ok(kind) => kind, Err(_) => { result.errors += 1; continue; } };
            if kind.is_symlink() { continue; }
            if kind.is_dir() { stack.push(entry.path()); continue; }
            if !kind.is_file() { continue; }
            let ext = entry.path().extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            if !MEDIA.contains(&ext.as_str()) { continue; }
            match entry.metadata() {
                Ok(meta) => {
                    result.files += 1;
                    result.bytes = result.bytes.saturating_add(meta.len());
                    *result.by_extension.entry(ext).or_default() += 1;
                }
                Err(_) => result.errors += 1,
            }
        }
    }
    Ok(result)
}

#[tauri::command]
async fn find_exact_duplicates(root: String) -> Result<duplicates::DuplicateReport, String> {
    tauri::async_runtime::spawn_blocking(move || duplicates::scan(&PathBuf::from(root)))
        .await
        .map_err(|e| format!("Error en el análisis: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder, find_exact_duplicates])
        .run(tauri::generate_context!())
        .expect("No se pudo iniciar PhotoVault");
}
