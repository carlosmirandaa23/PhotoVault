use serde::Serialize;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};
use tauri::{Emitter, State};

mod duplicates;
mod hash_cache;

#[derive(Default)]
struct ScanState(Mutex<Option<Arc<AtomicBool>>>);

#[derive(Clone, serde::Serialize)]
struct ScanProgress { phase: String, processed: u64, total: u64 }

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
async fn find_exact_duplicates(
    root: String,
    app: tauri::AppHandle,
    state: State<'_, ScanState>,
) -> Result<duplicates::DuplicateReport, String> {
    let token = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.0.lock().map_err(|_| "Estado de análisis no disponible")?;
        if slot.is_some() { return Err("Ya hay un análisis en curso".into()); }
        *slot = Some(token.clone());
    }
    let token_for_work = token.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        duplicates::scan_with_progress(&PathBuf::from(root), &token_for_work, |phase, processed, total| {
            let _ = app.emit("photovault:scan-progress", ScanProgress {
                phase: phase.to_owned(), processed, total
            });
        })
    }).await.map_err(|e| format!("Error en el análisis: {e}"));
    if let Ok(mut slot) = state.0.lock() { *slot = None; }
    result?
}

#[tauri::command]
fn cancel_duplicate_scan(state: State<'_, ScanState>) -> Result<(), String> {
    let slot = state.0.lock().map_err(|_| "Estado de análisis no disponible")?;
    if let Some(token) = slot.as_ref() { token.store(true, Ordering::Relaxed); }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ScanState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder, find_exact_duplicates, cancel_duplicate_scan])
        .run(tauri::generate_context!())
        .expect("No se pudo iniciar PhotoVault");
}
