//! Persistent *advisory* hash cache. It never authorizes deletion or a duplicate verdict.
//! A cache hit is still subjected to full byte comparison before being reported.
use rusqlite::{params, Connection, OptionalExtension};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub struct HashCache { db: Connection }

fn modified_ns(value: Option<SystemTime>) -> Option<String> {
    value?.duration_since(UNIX_EPOCH).ok().map(|v| v.as_nanos().to_string())
}

impl HashCache {
    pub fn open(root: &Path) -> Result<Self, String> {
        let canonical = root.canonicalize().map_err(|e| e.to_string())?;
        let base = dirs::cache_dir().ok_or("No se encontró el directorio de caché del usuario")?.join("PhotoVault");
        fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let key = blake3::hash(canonical.to_string_lossy().as_bytes()).to_hex().to_string();
        Self::open_at(&base.join(format!("{key}.sqlite")))
    }

    fn open_at(path: &Path) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e| e.to_string())?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS hashes (
            path TEXT PRIMARY KEY NOT NULL,
            size TEXT NOT NULL,
            modified_ns TEXT NOT NULL,
            blake3 TEXT NOT NULL
        );").map_err(|e| e.to_string())?;
        Ok(Self { db })
    }

    pub fn get(&self, path: &Path, size: u64, modified: Option<SystemTime>) -> Result<Option<String>, String> {
        let Some(timestamp) = modified_ns(modified) else { return Ok(None); };
        let key = path.to_string_lossy();
        let result: Option<String> = self.db.query_row(
            "SELECT blake3 FROM hashes WHERE path = ?1 AND size = ?2 AND modified_ns = ?3",
            params![key.as_ref(), size.to_string(), timestamp], |row| row.get(0),
        ).optional().map_err(|e| e.to_string())?;
        Ok(result.filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())))
    }

    pub fn put(&self, path: &Path, size: u64, modified: Option<SystemTime>, hash: &str) -> Result<(), String> {
        let Some(timestamp) = modified_ns(modified) else { return Ok(()); };
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) { return Err("Hash inválido".into()); }
        self.db.execute(
            "INSERT INTO hashes(path,size,modified_ns,blake3) VALUES (?1,?2,?3,?4)
             ON CONFLICT(path) DO UPDATE SET size=excluded.size, modified_ns=excluded.modified_ns, blake3=excluded.blake3",
            params![path.to_string_lossy().as_ref(), size.to_string(), timestamp, hash],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_invalidation_by_size_and_mtime() {
        let path = std::env::temp_dir().join(format!("photovault-cache-test-{}.sqlite", std::process::id()));
        let _ = fs::remove_file(&path);
        let db = HashCache::open_at(&path).unwrap();
        let image = PathBuf::from("sample.jpg");
        let now = SystemTime::now();
        let hash = "a".repeat(64);
        db.put(&image, 10, Some(now), &hash).unwrap();
        assert_eq!(db.get(&image, 10, Some(now)).unwrap(), Some(hash));
        assert_eq!(db.get(&image, 11, Some(now)).unwrap(), None);
        assert_eq!(db.get(&image, 10, Some(now + std::time::Duration::from_secs(1))).unwrap(), None);
        drop(db);
        fs::remove_file(path).unwrap();
    }
}
