//! Read-only perceptual similarity scan. Results are suggestions, never deletion instructions.
use image::{imageops::FilterType, ImageReader};
use serde::Serialize;
use rayon::prelude::*;
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
    // Process images in bounded parallel batches; emit progress on the calling thread.
    // Preserve deterministic path order and check cancellation between batches.
    let mut fingerprints: Vec<(PathBuf,u64)> = Vec::new();
    let mut processed = 0u64;
    progress(0,total);
    for batch in paths.chunks(32) {
        if cancel.load(Ordering::Relaxed) { return Err("Análisis cancelado".into()); }
        let outcomes: Vec<_> = batch.par_iter().map(|path| {
            if cancel.load(Ordering::Relaxed) { return None; }
            Some((path.clone(), phash(path)))
        }).collect();
        for outcome in outcomes {
            match outcome {
                Some((path, Ok(hash))) => fingerprints.push((path,hash)),
                Some((_, Err(_))) => errors += 1,
                None => {}
            }
        }
        processed += batch.len() as u64;
        progress(processed,total);
    }
    if cancel.load(Ordering::Relaxed) { return Err("Análisis cancelado".into()); }
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
    /// Benchmark the 65-image fixture. Not a pass/fail quality gate yet:
    /// report precision/recall and false positives before changing algorithms.
    #[test]
    #[ignore = "requires PHOTOVAULT_TEST_DIR pointing to the 65-image fixture"]
    fn benchmark_labeled_photo_families() {
        use std::collections::BTreeMap;
        let dir = PathBuf::from(std::env::var("PHOTOVAULT_TEST_DIR")
            .expect("Define PHOTOVAULT_TEST_DIR con la ruta de las 65 fotos"));
        let mut images = Vec::new();
        let mut stack = vec![dir.clone()];
        let mut inspected_files = 0usize;
        let mut unmatched_examples: Vec<String> = Vec::new();
        while let Some(folder) = stack.pop() {
            for entry in fs::read_dir(&folder).unwrap_or_else(|e| panic!("No se puede abrir {}: {e}", folder.display())) {
                let path = entry.expect("Entrada inválida").path();
                if path.is_dir() { stack.push(path); continue; }
                if !path.is_file() { continue; }
                inspected_files += 1;
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let mut parts = stem.splitn(3, '_');
                let family = parts.next().unwrap_or("");
                let variant = parts.next().unwrap_or("");
                if !matches!(family, "F01" | "F02" | "F03" | "F04" | "F05")
                    || variant.parse::<u8>().is_err()
                    || !matches!(path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("jpg" | "jpeg" | "png" | "webp"))
                {
                    if unmatched_examples.len() < 8 { unmatched_examples.push(path.display().to_string()); }
                    continue;
                }
                images.push((family.to_string(), variant.to_string(), path));
            }
        }
        images.sort_by(|a,b| a.2.cmp(&b.2));
        assert_eq!(images.len(), 65,
            "Se esperaban 65 fotografías etiquetadas. Carpeta: {}. Archivos encontrados: {}. Coincidencias F01_00...F05_12: {}. Ejemplos no reconocidos: {:?}. Verifica PHOTOVAULT_TEST_DIR.",
            dir.display(), inspected_files, images.len(), unmatched_examples);
        let hashes: Vec<u64> = images.iter().map(|(_,_,p)| phash(p).expect("No se pudo calcular pHash")).collect();
        println!("\\nBENCHMARK PHOTOVAULT — {} fotografías", images.len());
        println!("Verdad de referencia: misma familia Fxx = misma captura editada; familias diferentes = tomas distintas.");
        println!("Umbral | TP | FP | FN | TN | Precisión | Recall");
        for threshold in [0u32, 2, 4, 6, 8, 10, 12] {
            let (mut tp,mut fp,mut fn_,mut tn) = (0u32,0u32,0u32,0u32);
            let mut false_positives = Vec::new();
            for i in 0..images.len() {
                for j in i+1..images.len() {
                    let same_capture = images[i].0 == images[j].0;
                    let predicted = (hashes[i] ^ hashes[j]).count_ones() <= threshold;
                    match (same_capture,predicted) {
                        (true,true) => tp += 1,
                        (false,true) => {
                            fp += 1;
                            if threshold == 8 && false_positives.len() < 15 {
                                false_positives.push(format!("{} <> {}",
                                    images[i].2.file_name().unwrap().to_string_lossy(),
                                    images[j].2.file_name().unwrap().to_string_lossy()));
                            }
                        }
                        (true,false) => fn_ += 1,
                        (false,false) => tn += 1,
                    }
                }
            }
            let precision = if tp+fp == 0 { 1.0 } else { tp as f64 / (tp+fp) as f64 };
            let recall = if tp+fn_ == 0 { 1.0 } else { tp as f64 / (tp+fn_) as f64 };
            println!("{threshold:>6} | {tp:>3} | {fp:>3} | {fn_:>3} | {tn:>4} | {precision:>8.2}% | {recall:>6.2}%",
                precision=precision*100.0, recall=recall*100.0);
            if threshold == 8 {
                println!("Ejemplos de falsos positivos (umbral 8):");
                for example in false_positives { println!("  {example}"); }
            }
        }
        println!("Distancia al ORIGINAL por variante:");
        let mut by_variant: BTreeMap<String,Vec<u32>> = BTreeMap::new();
        for i in 0..images.len() {
            if images[i].1 != "00" { continue; }
            for j in 0..images.len() {
                if images[j].0 == images[i].0 && j != i {
                    by_variant.entry(images[j].1.clone()).or_default()
                        .push((hashes[i]^hashes[j]).count_ones());
                }
            }
        }
        for (variant, distances) in by_variant {
            println!("  Variante {variant}: {:?}", distances);
        }
    }

}
