//! Duplicate cleanup. The only file operation in PhotoVault that changes the
//! library, and it is always:
//! - explicitly requested by the user, group by group, naming the copy to keep;
//! - re-verified byte by byte against the kept copy immediately before acting;
//! - a move to the system Recycle Bin / Trash, never a permanent delete;
//! - recorded in an operations log outside the library, so it can be undone.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
pub struct CleanupGroup {
    pub keep: String,
    pub remove: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct CleanupResult {
    pub batch_id: String,
    pub moved: Vec<String>,
    pub failed: Vec<Failure>,
    pub bytes_freed: u64,
}

#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub batch_id: String,
    pub created_at: String,
    pub files: u64,
    pub bytes: u64,
    pub restored: u64,
}

#[derive(Debug, Serialize)]
pub struct RestoreResult {
    pub restored: Vec<String>,
    pub failed: Vec<Failure>,
}

fn data_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_local_dir().ok_or("No se encontró el directorio de datos local")?.join("PhotoVault");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.is_dir() { return Err("La biblioteca no es una carpeta".into()); }
    Ok(canonical)
}

fn log_db(root: &Path) -> Result<Connection, String> {
    let key = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
    let db = Connection::open(data_dir()?.join(format!("operations-{key}.sqlite"))).map_err(|e| e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(3)).map_err(|e| e.to_string())?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS operations(
            id INTEGER PRIMARY KEY,
            batch_id TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            path TEXT NOT NULL,
            kept_path TEXT NOT NULL,
            size INTEGER NOT NULL,
            blake3 TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('trashed','restored'))
        );
        CREATE INDEX IF NOT EXISTS operations_batch ON operations(batch_id);",
    ).map_err(|e| e.to_string())?;
    Ok(db)
}

/// The path must be a regular file (not a link) inside the library.
fn library_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    let original = PathBuf::from(path);
    let meta = fs::symlink_metadata(&original).map_err(|e| format!("No accesible: {e}"))?;
    if !meta.file_type().is_file() { return Err("No es un archivo normal".into()); }
    let actual = original.canonicalize().map_err(|e| e.to_string())?;
    if !actual.starts_with(root) { return Err("Archivo fuera de la biblioteca".into()); }
    Ok(actual)
}

fn file_hash(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn new_batch_id() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{nanos:x}-{:x}", std::process::id())
}

pub fn move_duplicates_to_trash(root: &Path, groups: Vec<CleanupGroup>) -> Result<CleanupResult, String> {
    let root = canonical_root(root)?;
    let db = log_db(&root)?;
    let mut result = CleanupResult { batch_id: new_batch_id(), moved: vec![], failed: vec![], bytes_freed: 0 };
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let fail = |result: &mut CleanupResult, path: &str, reason: String| {
        result.failed.push(Failure { path: path.to_owned(), reason });
    };
    for group in groups {
        let keep = match library_file(&root, &group.keep) {
            Ok(p) => p,
            Err(e) => {
                for path in &group.remove { fail(&mut result, path, format!("La copia a conservar no es válida: {e}")); }
                continue;
            }
        };
        seen.insert(keep.clone());
        for path in &group.remove {
            let target = match library_file(&root, path) { Ok(p) => p, Err(e) => { fail(&mut result, path, e); continue; } };
            // Never remove the kept copy, a file kept in another group, or the same file twice.
            if !seen.insert(target.clone()) {
                fail(&mut result, path, "El archivo es la copia conservada o ya se procesó".into());
                continue;
            }
            match crate::duplicates::identical_files(&keep, &target) {
                Ok(true) => {}
                Ok(false) => { fail(&mut result, path, "Ya no es idéntico a la copia conservada; vuelve a analizar".into()); continue; }
                Err(e) => { fail(&mut result, path, format!("No se pudo verificar: {e}")); continue; }
            }
            let (size, hash) = match (fs::metadata(&target), file_hash(&target)) {
                (Ok(m), Ok(h)) => (m.len(), h),
                (Err(e), _) | (_, Err(e)) => { fail(&mut result, path, format!("No se pudo leer: {e}")); continue; }
            };
            if let Err(e) = trash::delete(&target) {
                fail(&mut result, path, format!("No se pudo enviar a la Papelera: {e}"));
                continue;
            }
            let logged = db.execute(
                "INSERT INTO operations(batch_id,path,kept_path,size,blake3,status) VALUES(?1,?2,?3,?4,?5,'trashed')",
                params![result.batch_id, target.to_string_lossy(), keep.to_string_lossy(), size as i64, hash],
            );
            if let Err(e) = logged {
                // The file is safely in the Trash; the user can still restore it there.
                fail(&mut result, path, format!("Enviado a la Papelera, pero no se pudo registrar: {e}"));
            }
            result.bytes_freed = result.bytes_freed.saturating_add(size);
            result.moved.push(target.to_string_lossy().into_owned());
        }
    }
    Ok(result)
}

pub fn history(root: &Path) -> Result<Vec<BatchSummary>, String> {
    let root = canonical_root(root)?;
    let db = log_db(&root)?;
    let mut stmt = db.prepare(
        "SELECT batch_id, MIN(created_at), COUNT(*), SUM(size), SUM(status='restored')
         FROM operations GROUP BY batch_id ORDER BY MIN(id) DESC LIMIT 50",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(BatchSummary {
        batch_id: r.get(0)?, created_at: r.get(1)?,
        files: r.get::<_, i64>(2)? as u64, bytes: r.get::<_, i64>(3)? as u64, restored: r.get::<_, i64>(4)? as u64,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

#[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android"))))]
pub fn restore_batch(root: &Path, batch_id: &str) -> Result<RestoreResult, String> {
    let root = canonical_root(root)?;
    let db = log_db(&root)?;
    let pending: Vec<(i64, String, String)> = {
        let mut stmt = db.prepare("SELECT id, path, blake3 FROM operations WHERE batch_id=?1 AND status='trashed'")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![batch_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let mut items = trash::os_limited::list().map_err(|e| format!("No se pudo leer la Papelera: {e}"))?;
    // Most recently deleted first, so an older item with the same name is not picked.
    items.sort_by(|a, b| b.time_deleted.cmp(&a.time_deleted));
    let mut result = RestoreResult { restored: vec![], failed: vec![] };
    for (id, path, hash) in pending {
        let original = PathBuf::from(&path);
        let Some(index) = items.iter().position(|item| item.original_path() == original) else {
            result.failed.push(Failure { path, reason: "No está en la Papelera (¿se vació o ya se restauró?)".into() });
            continue;
        };
        let item = items.remove(index);
        if original.exists() {
            result.failed.push(Failure { path, reason: "Ya existe un archivo con ese nombre en la ubicación original".into() });
            continue;
        }
        if let Err(e) = trash::os_limited::restore_all([item]) {
            result.failed.push(Failure { path, reason: format!("No se pudo restaurar: {e}") });
            continue;
        }
        if file_hash(&original).ok().as_deref() != Some(hash.as_str()) {
            result.failed.push(Failure { path: path.clone(), reason: "Restaurado, pero el contenido no coincide con el registrado".into() });
        }
        db.execute("UPDATE operations SET status='restored' WHERE id=?1", params![id]).map_err(|e| e.to_string())?;
        result.restored.push(path);
    }
    Ok(result)
}

#[cfg(not(any(target_os = "windows", all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))))]
pub fn restore_batch(_root: &Path, _batch_id: &str) -> Result<RestoreResult, String> {
    Err("En este sistema restaura los archivos desde la Papelera manualmente".into())
}

#[cfg(all(test, unix, not(target_os = "macos")))]
mod tests {
    use super::*;
    use std::io::Write;

    // One test touches process-wide environment variables, so everything that
    // depends on them lives in this single test.
    #[test]
    fn trash_verify_log_and_restore() {
        let base = std::env::temp_dir().join(format!("photovault-cleanup-{}-{}", std::process::id(), new_batch_id()));
        let home = base.join("home");
        let library = home.join("library");
        fs::create_dir_all(library.join("sub")).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));

        let write = |p: &Path, bytes: &[u8]| File::create(p).unwrap().write_all(bytes).unwrap();
        let keep = library.join("a.jpg");
        let copy = library.join("sub/a (1).jpg");
        let different = library.join("b.jpg");
        write(&keep, b"same photo bytes");
        write(&copy, b"same photo bytes");
        write(&different, b"same photo byteX");
        let s = |p: &Path| p.to_string_lossy().into_owned();

        // Different content, the kept copy itself, and paths outside the library are refused.
        let outside = base.join("outside.jpg");
        write(&outside, b"same photo bytes");
        let refused = move_duplicates_to_trash(&library, vec![CleanupGroup {
            keep: s(&keep), remove: vec![s(&different), s(&keep), s(&outside)],
        }]).unwrap();
        assert!(refused.moved.is_empty());
        assert_eq!(refused.failed.len(), 3);
        assert!(different.exists() && keep.exists() && outside.exists());

        // An identical copy goes to the Trash and is logged.
        let done = move_duplicates_to_trash(&library, vec![CleanupGroup { keep: s(&keep), remove: vec![s(&copy)] }]).unwrap();
        assert_eq!(done.moved.len(), 1, "{:?}", done.failed);
        assert_eq!(done.bytes_freed, 16);
        assert!(!copy.exists() && keep.exists());
        let batches = history(&library).unwrap();
        assert_eq!(batches[0].batch_id, done.batch_id);
        assert_eq!((batches[0].files, batches[0].restored), (1, 0));

        // Undo puts the file back with the same content.
        let restored = restore_batch(&library, &done.batch_id).unwrap();
        assert_eq!(restored.restored.len(), 1, "{:?}", restored.failed);
        assert_eq!(fs::read(&copy).unwrap(), b"same photo bytes");
        assert_eq!(history(&library).unwrap()[0].restored, 1);

        fs::remove_dir_all(base).unwrap();
    }
}
