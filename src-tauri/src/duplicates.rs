//! Read-only exact duplicate discovery. A hash is a candidate filter, not proof.
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub size_bytes: u64,
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DuplicateReport {
    pub scanned_files: u64,
    pub errors: u64,
    pub groups: Vec<DuplicateGroup>,
    pub redundant_bytes: u64,
    pub cache_hits: u64,
    pub hashes_computed: u64,
    pub cache_available: bool,
}

#[derive(Clone)]
struct Candidate {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

fn snapshot(path: &Path) -> io::Result<Candidate> {
    let m = fs::symlink_metadata(path)?;
    if !m.file_type().is_file() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Not a regular file")); }
    Ok(Candidate { path: path.to_path_buf(), size: m.len(), modified: m.modified().ok() })
}

fn stable(file: &Candidate) -> io::Result<bool> {
    let after = snapshot(&file.path)?;
    Ok(after.size == file.size && after.modified == file.modified)
}

fn digest(file: &Candidate) -> io::Result<Option<blake3::Hash>> {
    if !stable(file)? { return Ok(None); }
    let mut reader = BufReader::new(File::open(&file.path)?);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    if !stable(file)? { return Ok(None); }
    Ok(Some(hasher.finalize()))
}

/// Full byte comparison is mandatory before grouping. Both inputs are checked
/// before and after the comparison. No deletion is performed.
fn equal_bytes(a: &Candidate, b: &Candidate) -> io::Result<bool> {
    if a.size != b.size || !stable(a)? || !stable(b)? { return Ok(false); }
    let mut left = BufReader::new(File::open(&a.path)?);
    let mut right = BufReader::new(File::open(&b.path)?);
    let mut x = [0u8; 65536];
    let mut y = [0u8; 65536];
    loop {
        let nx = left.read(&mut x)?;
        let ny = right.read(&mut y)?;
        if nx != ny || x[..nx] != y[..ny] { return Ok(false); }
        if nx == 0 { break; }
    }
    Ok(stable(a)? && stable(b)?)
}

fn is_media(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "jpg"|"jpeg"|"png"|"webp"|"gif"|"bmp"|"tif"|"tiff"|"heic"|"heif"|"avif"|"dng"|"cr2"|"nef"|"arw"|"raf"|"orf"|"rw2"|"mp4"|"mov"|"avi"|"mkv"|"mts"|"3gp")
}

pub fn scan(root: &Path) -> Result<DuplicateReport, String> {
    if !root.is_dir() { return Err("La carpeta no existe o no es accesible".into()); }
    let mut report = DuplicateReport { scanned_files: 0, errors: 0, groups: vec![], redundant_bytes: 0, cache_hits: 0, hashes_computed: 0, cache_available: false };
    let cache = crate::hash_cache::HashCache::open(root).ok();
    report.cache_available = cache.is_some();
    let mut by_size: BTreeMap<u64, Vec<Candidate>> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) { Ok(v) => v, Err(_) => { report.errors += 1; continue; } };
        for entry in entries {
            let entry = match entry { Ok(v) => v, Err(_) => { report.errors += 1; continue; } };
            let kind = match entry.file_type() { Ok(v) => v, Err(_) => { report.errors += 1; continue; } };
            if kind.is_symlink() { continue; }
            if kind.is_dir() { stack.push(entry.path()); continue; }
            if !kind.is_file() || !is_media(&entry.path()) { continue; }
            report.scanned_files += 1;
            match snapshot(&entry.path()) {
                Ok(candidate) => by_size.entry(candidate.size).or_default().push(candidate),
                Err(_) => report.errors += 1,
            }
        }
    }
    for (_, candidates) in by_size.into_iter().filter(|(_, v)| v.len() > 1) {
        let mut by_hash: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
        for candidate in candidates {
            let cached = cache.as_ref().and_then(|db| db.get(&candidate.path, candidate.size, candidate.modified).ok().flatten());
            let hash = if let Some(hex) = cached {
                if stable(&candidate).unwrap_or(false) {
                    report.cache_hits += 1;
                    Some(hex)
                } else { report.errors += 1; None }
            } else {
                match digest(&candidate) {
                    Ok(Some(hash)) => {
                        report.hashes_computed += 1;
                        let hex = hash.to_hex().to_string();
                        if let Some(db) = &cache { if db.put(&candidate.path, candidate.size, candidate.modified, &hex).is_err() { report.errors += 1; } }
                        Some(hex)
                    },
                    Ok(None) | Err(_) => { report.errors += 1; None },
                }
            };
            if let Some(hex) = hash { by_hash.entry(hex).or_default().push(candidate); }
        }
        for (_, same_hash) in by_hash.into_iter().filter(|(_, v)| v.len() > 1) {
            let mut verified: Vec<Vec<Candidate>> = vec![];
            for candidate in same_hash {
                let mut placed = false;
                for group in &mut verified {
                    match equal_bytes(&group[0], &candidate) {
                        Ok(true) => { group.push(candidate.clone()); placed = true; break; }
                        Ok(false) => {}
                        Err(_) => { report.errors += 1; }
                    }
                }
                if !placed { verified.push(vec![candidate]); }
            }
            for group in verified.into_iter().filter(|v| v.len() > 1) {
                // Revalidate all group members before showing a finding.
                let all_stable = group.iter().all(|c| stable(c).unwrap_or(false));
                if !all_stable { report.errors += 1; continue; }
                let size = group[0].size;
                report.redundant_bytes = report.redundant_bytes.saturating_add(size.saturating_mul((group.len() - 1) as u64));
                let mut paths: Vec<String> = group.into_iter().map(|c| c.path.to_string_lossy().into_owned()).collect();
                paths.sort();
                report.groups.push(DuplicateGroup { size_bytes: size, paths });
            }
        }
    }
    report.groups.sort_by(|a,b| a.paths[0].cmp(&b.paths[0]));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn exact_duplicates_only_and_no_mutation() {
        let dir = std::env::temp_dir().join(format!("photovault-test-{}-{}", std::process::id(), std::thread::current().name().unwrap_or("unit")));
        fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.jpg");
        let b = dir.join("b.jpg");
        let c = dir.join("c.jpg");
        let other = dir.join("skip.txt");
        File::create(&a).unwrap().write_all(b"same image bytes").unwrap();
        fs::copy(&a, &b).unwrap();
        File::create(&c).unwrap().write_all(b"same image byteX").unwrap();
        File::create(&other).unwrap().write_all(b"same image bytes").unwrap();
        let report = scan(&dir).unwrap();
        assert_eq!(report.scanned_files, 3);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].paths.len(), 2);
        assert_eq!(report.redundant_bytes, 16);
        assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
        assert!(c.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_directory_has_no_duplicates() {
        let dir = std::env::temp_dir().join(format!("photovault-empty-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let report = scan(&dir).unwrap();
        assert_eq!(report.groups.len(), 0);
        fs::remove_dir_all(dir).unwrap();
    }
}
