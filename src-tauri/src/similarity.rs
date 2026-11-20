//! Read-only perceptual similarity scan. Results are suggestions, never deletion instructions.
use image::{imageops::FilterType, ImageReader};
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}, sync::atomic::{AtomicBool, Ordering}};

const EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff"];
const MAX_FILES: usize = 3000;
const THRESHOLD: u32 = 8;

#[derive(Debug, Serialize)]
pub struct SimilarPair {
    pub first: String,
    pub second: String,
    pub distance: u32,
}

#[derive(Debug, Serialize)]
pub struct SimilarityReport {
    pub analyzed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub pairs: Vec<SimilarPair>,
    pub threshold: u32,
}

fn phash(path: &Path) -> Result<u64, String> {
    let reader = ImageReader::open(path).map_err(|e| e.to_string())?;
    let img = reader.decode().map_err(|e| e.to_string())?
        .resize_exact(32, 32, FilterType::Triangle).to_luma8();
    let mut coefficients = [0f64; 64];
    for u in 0..8 {
        for v in 0..8 {
            let mut sum = 0f64;
            for y in 0..32 {
                for x in 0..32 {
                    let pixel = img.get_pixel(x, y)[0] as f64;
                    sum += pixel * (std::f64::consts::PI * (2*x+1) as f64 * u as f64 / 64.0).cos()
                        * (std::f64::consts::PI * (2*y+1) as f64 * v as f64 / 64.0).cos();
                }
            }
            coefficients[(u*8+v) as usize] = sum;
        }
    }
    // Exclude DC from the median to reduce global brightness sensitivity.
    let mut values = coefficients[1..].to_vec();
    values.sort_by(|a,b| a.total_cmp(b));
    let median = values[values.len()/2];
    let mut hash = 0u64;
    for (i, coefficient) in coefficients.iter().enumerate() {
        if i > 0 && *coefficient > median { hash |= 1u64 << i; }
    }
    Ok(hash)
}

pub fn scan(root: &Path, cancel: &AtomicBool, mut progress: impl FnMut(u64,u64)) -> Result<SimilarityReport,String> {
    if !root.is_dir() { return Err("La carpeta no existe o no es accesible".into()); }
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut errors = 0;
    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) { return Err("Análisis cancelado".into()); }
        let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => { errors += 1; continue; } };
        for entry in entries {
            if cancel.load(Ordering::Relaxed) { return Err("Análisis cancelado".into()); }
            let entry = match entry { Ok(e) => e, Err(_) => { errors += 1; continue; } };
            let kind = match entry.file_type() { Ok(t) => t, Err(_) => { errors += 1; continue; } };
            if kind.is_symlink() { continue; }
            if kind.is_dir() { stack.push(entry.path()); continue; }
            if !kind.is_file() { continue; }
            let ext = entry.path().extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            if EXTENSIONS.contains(&ext.as_str()) { paths.push(entry.path()); }
        }
    }
    paths.sort();
    let skipped = paths.len().saturating_sub(MAX_FILES);
    paths.truncate(MAX_FILES);
    let total = paths.len() as u64;
    let mut fingerprints: Vec<(PathBuf,u64)> = Vec::new();
    progress(0,total);
    for (i,path) in paths.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) { return Err("Análisis cancelado".into()); }
        match phash(&path) {
            Ok(hash) => fingerprints.push((path,hash)),
            Err(_) => errors += 1,
        }
        progress(i as u64+1,total);
    }
    let mut pairs = Vec::new();
    for i in 0..fingerprints.len() {
        if cancel.load(Ordering::Relaxed) { return Err("Análisis cancelado".into()); }
        for j in (i+1)..fingerprints.len() {
            let distance = (fingerprints[i].1 ^ fingerprints[j].1).count_ones();
            if distance <= THRESHOLD {
                pairs.push(SimilarPair {
                    first: fingerprints[i].0.to_string_lossy().into_owned(),
                    second: fingerprints[j].0.to_string_lossy().into_owned(),
                    distance,
                });
            }
        }
    }
    pairs.sort_by(|a,b| a.distance.cmp(&b.distance).then(a.first.cmp(&b.first)).then(a.second.cmp(&b.second)));
    Ok(SimilarityReport { analyzed: fingerprints.len(), skipped, errors, pairs, threshold: THRESHOLD })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_images_share_phash() {
        let path = std::env::temp_dir().join(format!("photovault-phash-{}.png", std::process::id()));
        image::GrayImage::from_fn(64,64,|x,y| image::Luma([((x*3+y*7)%256) as u8])).save(&path).unwrap();
        assert_eq!(phash(&path).unwrap(), phash(&path).unwrap());
        fs::remove_file(path).unwrap();
    }
}
