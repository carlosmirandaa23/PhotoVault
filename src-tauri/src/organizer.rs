//! Read-only photo organization proposal. No file operations except reading
//! metadata, image headers and EXIF.
//!
//! Capture date priority: EXIF DateTimeOriginal (or DateTimeDigitized/DateTime)
//! → date in the file name → none. File modification time is never used as a
//! capture date: it usually reflects a copy or a download.
use image::ImageReader;
use serde::Serialize;
use std::{fs::{self, File}, io::BufReader, path::{Path, PathBuf}};

const PHOTOS: &[&str] = &["jpg","jpeg","png","webp","bmp","gif","tif","tiff","heic","heif"];
const DECODABLE: &[&str] = &["jpg","jpeg","png","webp","bmp","gif","tif","tiff"];
const VIDEOS: &[&str] = &["mp4","mov","avi","mkv","mts","3gp","m4v","wmv","m2ts","webm"];
const MAX_FILES: usize = 20_000;

/// Grouping rules from the original organizer script.
pub struct Options {
    /// A new event starts when two consecutive dated files are further apart than this.
    pub event_gap_hours: i64,
    /// Groups with at least this many files get their own event folder; smaller ones go by month.
    pub event_min_files: usize,
}
impl Default for Options {
    fn default() -> Self { Options { event_gap_hours: 12, event_min_files: 20 } }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum Category { Photo, Snapshot, PossibleTrash, Undated, Video }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateSource { Exif, Name }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CaptureDate { year: i32, month: u32, day: u32, seconds: Option<u32>, source: DateSource }

impl CaptureDate {
    fn new(year: i32, month: u32, day: u32, time: Option<(u32,u32,u32)>, source: DateSource) -> Option<Self> {
        if !(1970..=2100).contains(&year) || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) { return None; }
        let seconds = match time {
            Some((h,m,s)) if h < 24 && m < 60 && s < 61 => Some(h*3600 + m*60 + s.min(59)),
            Some(_) => return None,
            None => None,
        };
        Some(CaptureDate { year, month, day, seconds, source })
    }
    /// Seconds since 1970 (local wall-clock time, no zone). Unknown time counts as midnight.
    fn timestamp(&self) -> i64 { days_from_civil(self.year, self.month, self.day) * 86_400 + i64::from(self.seconds.unwrap_or(0)) }
    fn ymd(&self) -> String { format!("{:04}-{:02}-{:02}", self.year, self.month, self.day) }
    fn display(&self) -> String {
        match self.seconds {
            Some(s) => format!("{} {:02}:{:02}:{:02}", self.ymd(), s/3600, s/60%60, s%60),
            None => self.ymd(),
        }
    }
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m { 1|3|5|7|8|10|12 => 31, 4|6|9|11 => 30, _ => if (y%4==0 && y%100!=0) || y%400==0 { 29 } else { 28 } }
}
// Howard Hinnant's days_from_civil.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    era * 146_097 + yoe * 365 + yoe/4 - yoe/100 + doy - 719_468
}

#[derive(Debug, Serialize)]
pub struct PlannedFile {
    pub path: String,
    pub category: Category,
    pub suggested_folder: String,
    pub reason: String,
    /// "exif", "nombre" or "no_verificada".
    pub date_source: String,
    pub date: Option<String>,
    pub camera: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct OrganizationPlan {
    pub analyzed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub events: usize,
    pub files: Vec<PlannedFile>,
    pub read_only: bool,
}

fn digits(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) { return None; }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

/// Time right after a date in a file name: `_143022`, `-143022`, ` 14.30.22`, `_14-30-22`.
fn parse_time_after(bytes: &[u8]) -> Option<(u32,u32,u32)> {
    let rest = bytes.strip_prefix(b"_").or_else(|| bytes.strip_prefix(b"-"))
        .or_else(|| bytes.strip_prefix(b" ")).or_else(|| bytes.strip_prefix(b"T"))?;
    if rest.len() >= 6 && rest[..6].iter().all(u8::is_ascii_digit) && rest.get(6).map_or(true, |b| !b.is_ascii_digit()) {
        return Some((digits(&rest[0..2])?, digits(&rest[2..4])?, digits(&rest[4..6])?));
    }
    if rest.len() >= 8 && rest[2] == rest[5] && matches!(rest[2], b'.' | b'-' | b':') {
        return Some((digits(&rest[0..2])?, digits(&rest[3..5])?, digits(&rest[6..8])?));
    }
    None
}

fn parse_name_date(name: &str) -> Option<CaptureDate> {
    let b = name.as_bytes();
    // YYYYMMDD, not part of a longer run of digits (avoids matching inside counters).
    for i in 0..b.len().saturating_sub(7) {
        if i > 0 && b[i-1].is_ascii_digit() { continue; }
        if !b[i..i+8].iter().all(u8::is_ascii_digit) || b.get(i+8).is_some_and(u8::is_ascii_digit) { continue; }
        let time = parse_time_after(&b[i+8..]);
        if let Some(date) = CaptureDate::new(digits(&b[i..i+4])? as i32, digits(&b[i+4..i+6])?, digits(&b[i+6..i+8])?, time, DateSource::Name) {
            return Some(date);
        }
    }
    // YYYY-MM-DD or YYYY_MM_DD.
    for i in 0..b.len().saturating_sub(9) {
        let p = &b[i..i+10];
        if !(p[4] == p[7] && matches!(p[4], b'-' | b'_')) { continue; }
        let (Some(y), Some(m), Some(d)) = (digits(&p[0..4]), digits(&p[5..7]), digits(&p[8..10])) else { continue };
        if let Some(date) = CaptureDate::new(y as i32, m, d, parse_time_after(&b[i+10..]), DateSource::Name) {
            return Some(date);
        }
    }
    None
}

/// EXIF date text "YYYY:MM:DD HH:MM:SS".
fn parse_exif_date(text: &str) -> Option<CaptureDate> {
    let b = text.trim().as_bytes();
    if b.len() < 19 { return None; }
    CaptureDate::new(digits(&b[0..4])? as i32, digits(&b[5..7])?, digits(&b[8..10])?,
        Some((digits(&b[11..13])?, digits(&b[14..16])?, digits(&b[17..19])?)), DateSource::Exif)
}

#[derive(Debug, Default)]
struct ExifInfo { date: Option<CaptureDate>, camera: Option<String> }

fn read_exif(path: &Path) -> ExifInfo {
    let Ok(file) = File::open(path) else { return ExifInfo::default() };
    let Ok(exif) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) else { return ExifInfo::default() };
    let text = |tag| exif.get_field(tag, exif::In::PRIMARY).and_then(|f| match &f.value {
        exif::Value::Ascii(parts) => parts.first().map(|v| String::from_utf8_lossy(v).trim().trim_end_matches('\0').to_owned()),
        _ => None,
    }).filter(|v| !v.is_empty());
    let date = [exif::Tag::DateTimeOriginal, exif::Tag::DateTimeDigitized, exif::Tag::DateTime]
        .into_iter().find_map(|tag| text(tag).as_deref().and_then(parse_exif_date));
    let camera = match (text(exif::Tag::Make), text(exif::Tag::Model)) {
        (Some(make), Some(model)) if model.to_lowercase().starts_with(&make.to_lowercase()) => Some(model),
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (one, other) => one.or(other),
    };
    ExifInfo { date, camera }
}

/// Only tall phone-screen proportions (18:9 to 21:9). 4:3, 3:2 and 16:9 are
/// normal camera formats and are not treated as evidence of a screenshot.
fn phone_screen_ratio(w: u32, h: u32) -> bool {
    let (long, short) = (w.max(h) as f64, w.min(h) as f64);
    short > 0.0 && (1.95..=2.4).contains(&(long / short))
}
fn snapshot_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["screenshot","captura","screen","img-wa","whatsapp","signal","telegram","snapchat"].iter().any(|p| n.contains(p))
}
fn camera_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if ["img_","dsc","dscn","dscf","dcim","photo","foto","cam","mvimg_","pxl_","p10","gopr","dji_"].iter().any(|p| n.starts_with(p)) {
        return true;
    }
    // Samsung and many Android cameras: 20240715_143022.jpg
    let b = n.as_bytes();
    b.len() >= 15 && b[..8].iter().all(u8::is_ascii_digit) && b[8] == b'_' && b[9..15].iter().all(u8::is_ascii_digit)
}

fn classify(name: &str, dimensions: Option<(u32,u32)>, video: bool, date: Option<&CaptureDate>, camera: Option<&str>) -> (Category, String) {
    if video {
        return (Category::Video, if date.is_some() { "Video con fecha en el nombre".into() } else { "Video sin fecha en el nombre; revisar".into() });
    }
    if let Some(model) = camera {
        if !snapshot_name(name) { return (Category::Photo, format!("EXIF de cámara: {model}")); }
    }
    let Some((w,h)) = dimensions else {
        return (Category::Undated, "No se pudieron leer las dimensiones; revisar formato o archivo".into());
    };
    if snapshot_name(name) {
        return (Category::Snapshot, "Nombre de captura o de app de mensajería; sugerencia".into());
    }
    if camera_name(name) {
        return (Category::Photo, "Nombre típico de cámara".into());
    }
    if u64::from(w) * u64::from(h) < 1_000_000 {
        return (Category::PossibleTrash, "Menos de 1 MP, sin EXIF de cámara ni nombre de cámara; NO implica que sea basura (puede ser una foto antigua o compartida)".into());
    }
    if phone_screen_ratio(w, h) {
        return (Category::Snapshot, "Proporción alargada de pantalla de teléfono y sin EXIF de cámara; también puede ser una foto recortada".into());
    }
    if date.is_none() {
        return (Category::Undated, "Sin EXIF ni fecha en el nombre".into());
    }
    (Category::Photo, "Resolución suficiente; sin EXIF de cámara".into())
}

fn inspect(path: &Path) -> Result<(PlannedFile, Option<CaptureDate>), String> {
    let name = path.file_name().and_then(|x| x.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    let video = VIDEOS.contains(&ext.as_str());
    let exif = if video { ExifInfo::default() } else { read_exif(path) };
    let date = exif.date.or_else(|| parse_name_date(name));
    let dimensions = if !DECODABLE.contains(&ext.as_str()) { None } else {
        ImageReader::open(path).map_err(|e| e.to_string())?
            .with_guessed_format().map_err(|e| e.to_string())?
            .into_dimensions().ok()
    };
    let (category, reason) = classify(name, dimensions, video, date.as_ref(), exif.camera.as_deref());
    let date_source = match date.map(|d| d.source) { Some(DateSource::Exif) => "exif", Some(DateSource::Name) => "nombre", None => "no_verificada" };
    Ok((PlannedFile {
        path: path.to_string_lossy().into_owned(), category, suggested_folder: String::new(), reason,
        date_source: date_source.into(), date: date.map(|d| d.display()), camera: exif.camera,
        width: dimensions.map(|d| d.0), height: dimensions.map(|d| d.1),
    }, date))
}

fn month_folder(d: &CaptureDate) -> String { format!("Fotos/{:04}/{:04}-{:02}", d.year, d.year, d.month) }

/// Assign folders. Dated photos and videos are sorted by time and split into
/// events wherever the gap exceeds `event_gap_hours`; large events get their
/// own folder, the rest go by month. Returns the number of event folders.
fn assign_folders(items: &mut [(PlannedFile, Option<CaptureDate>)], options: &Options) -> usize {
    let mut dated: Vec<usize> = items.iter().enumerate()
        .filter(|(_, (f, d))| matches!(f.category, Category::Photo | Category::Video) && d.is_some())
        .map(|(i, _)| i).collect();
    dated.sort_by_key(|&i| (items[i].1.unwrap().timestamp(), i));
    let mut events = 0;
    let mut start = 0;
    while start < dated.len() {
        let mut end = start + 1;
        while end < dated.len() {
            let gap = items[dated[end]].1.unwrap().timestamp() - items[dated[end-1]].1.unwrap().timestamp();
            if gap > options.event_gap_hours * 3600 { break; }
            end += 1;
        }
        let group = &dated[start..end];
        if group.len() >= options.event_min_files {
            events += 1;
            let first = items[group[0]].1.unwrap();
            let last = items[*group.last().unwrap()].1.unwrap();
            let span = if first.ymd() == last.ymd() { first.ymd() } else { format!("{} a {}", first.ymd(), last.ymd()) };
            for &i in group { items[i].0.suggested_folder = format!("Fotos/{:04}/{span} Evento", first.year); }
        } else {
            for &i in group { items[i].0.suggested_folder = month_folder(&items[i].1.unwrap()); }
        }
        start = end;
    }
    for (file, date) in items.iter_mut().filter(|(f, _)| f.suggested_folder.is_empty()) {
        file.suggested_folder = match (&file.category, date) {
            (Category::Snapshot, Some(d)) => format!("Snapshots/{:04}", d.year),
            (Category::Snapshot, None) => "Snapshots/_Sin_Fecha".into(),
            (Category::PossibleTrash, _) => "Basura Probable (revisión manual)".into(),
            _ => "_Sin_Fecha (revisión manual)".into(),
        };
    }
    events
}

pub fn plan(root: &Path) -> Result<OrganizationPlan, String> { plan_with(root, &Options::default()) }

pub fn plan_with(root: &Path, options: &Options) -> Result<OrganizationPlan, String> {
    if !root.is_dir() { return Err("La carpeta no existe o no es accesible".into()); }
    let mut stack = vec![root.to_path_buf()];
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut errors = 0;
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(dir) { Ok(x) => x, Err(_) => { errors += 1; continue } };
        for entry in entries {
            let entry = match entry { Ok(x) => x, Err(_) => { errors += 1; continue } };
            let kind = match entry.file_type() { Ok(x) => x, Err(_) => { errors += 1; continue } };
            if kind.is_symlink() { continue; }
            if kind.is_dir() { stack.push(entry.path()); continue; }
            if !kind.is_file() { continue; }
            let ext = entry.path().extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            if PHOTOS.contains(&ext.as_str()) || VIDEOS.contains(&ext.as_str()) { paths.push(entry.path()); }
        }
    }
    paths.sort();
    let skipped = paths.len().saturating_sub(MAX_FILES);
    paths.truncate(MAX_FILES);
    let decisions = crate::decisions::Store::open(root).ok();
    let mut items = Vec::with_capacity(paths.len());
    for path in paths {
        let (mut item, date) = match inspect(&path) { Ok(found) => found, Err(_) => { errors += 1; continue } };
        if let Some(saved) = decisions.as_ref().and_then(|db| db.get(&path).ok().flatten()) {
            if saved.content_changed {
                item.reason = format!("Decisión anterior: {}. Archivo modificado: requiere nueva revisión. {}", saved.decision, item.reason);
            } else {
                match saved.decision.as_str() {
                    "keep" | "photo" => item.category = Category::Photo,
                    "snapshot" => item.category = Category::Snapshot,
                    _ => {}
                }
                item.reason = format!("Decisión del usuario: {}. {}", saved.decision, item.reason);
            }
        }
        items.push((item, date));
    }
    let events = assign_folders(&mut items, options);
    let files: Vec<PlannedFile> = items.into_iter().map(|(f, _)| f).collect();
    Ok(OrganizationPlan { analyzed: files.len(), skipped, errors, events, files, read_only: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn low_resolution_is_only_a_suggestion() {
        let (cat, reason) = classify("unknown.png", Some((320,240)), false, None, None);
        assert_eq!(cat, Category::PossibleTrash);
        assert!(reason.contains("NO implica"));
    }
    #[test]
    fn camera_name_protects_small_photos() {
        assert_eq!(classify("IMG_20050101_123456.jpg", Some((320,240)), false, None, None).0, Category::Photo);
    }
    #[test]
    fn screenshot_name_overrides_resolution() {
        assert_eq!(classify("Screenshot_20240101.png", Some((1440,3040)), false, None, None).0, Category::Snapshot);
    }
    #[test]
    fn normal_camera_proportions_are_not_screenshots() {
        let date = parse_name_date("20240715_143022.jpg");
        // Samsung naming, 4:3.
        assert_eq!(classify("20240715_143022.jpg", Some((4000,3000)), false, date.as_ref(), None).0, Category::Photo);
        // Unknown name, 3:2 and 16:9 with a date: photo, not snapshot.
        let date = parse_name_date("viaje 2023-05-01.jpg");
        assert_eq!(classify("viaje 2023-05-01.jpg", Some((6000,4000)), false, date.as_ref(), None).0, Category::Photo);
        assert_eq!(classify("viaje 2023-05-01.jpg", Some((1920,1080)), false, date.as_ref(), None).0, Category::Photo);
        // Tall phone screen without camera EXIF: snapshot suggestion.
        assert_eq!(classify("algo.png", Some((1080,2400)), false, None, None).0, Category::Snapshot);
    }
    #[test]
    fn camera_exif_wins_over_heuristics() {
        let (cat, reason) = classify("P1010001.JPG", Some((640,480)), false, None, Some("Panasonic DMC-FZ8"));
        assert_eq!(cat, Category::Photo);
        assert!(reason.contains("Panasonic"));
    }
    #[test]
    fn date_parsing() {
        assert_eq!(parse_name_date("IMG_20240715_143022.jpg").map(|d| d.display()).as_deref(), Some("2024-07-15 14:30:22"));
        assert_eq!(parse_name_date("foto-2024-07-15.jpg").map(|d| d.display()).as_deref(), Some("2024-07-15"));
        assert_eq!(parse_name_date("WhatsApp Image 2024-07-15 at 14.30.22.jpeg").map(|d| d.ymd()).as_deref(), Some("2024-07-15"));
        assert_eq!(parse_name_date("20241399.jpg"), None);
        assert_eq!(parse_name_date("20230229.jpg"), None, "2023 no es bisiesto");
        assert!(parse_name_date("20240229.jpg").is_some());
        assert_eq!(parse_name_date("IMG_120240715999.jpg"), None, "no tomar fechas dentro de contadores largos");
        assert_eq!(parse_exif_date("2019:12:31 23:59:58").map(|d| d.display()).as_deref(), Some("2019-12-31 23:59:58"));
        assert_eq!(parse_exif_date("0000:00:00 00:00:00"), None);
    }
    #[test]
    fn never_guess_video_capture_date_from_mtime() {
        assert_eq!(classify("video.mp4", None, true, None, None).0, Category::Video);
    }

    fn dated(name: &str, ts: &str) -> (PlannedFile, Option<CaptureDate>) {
        let date = parse_exif_date(ts);
        (PlannedFile { path: name.into(), category: Category::Photo, suggested_folder: String::new(), reason: String::new(),
            date_source: "exif".into(), date: None, camera: None, width: None, height: None }, date)
    }
    #[test]
    fn events_split_by_gap_and_threshold() {
        let mut items = Vec::new();
        // 25 photos every 10 minutes on one afternoon: an event.
        for i in 0..25 { items.push(dated(&format!("e{i}"), &format!("2024:07:15 {:02}:{:02}:00", 12 + i*10/60, i*10%60))); }
        // 3 photos two days later: by month.
        for i in 0..3 { items.push(dated(&format!("m{i}"), &format!("2024:07:17 09:0{i}:00"))); }
        // One photo the next month.
        items.push(dated("aug", "2024:08:02 10:00:00"));
        let events = assign_folders(&mut items, &Options::default());
        assert_eq!(events, 1);
        assert_eq!(items[0].0.suggested_folder, "Fotos/2024/2024-07-15 Evento");
        assert_eq!(items[24].0.suggested_folder, "Fotos/2024/2024-07-15 Evento");
        assert_eq!(items[25].0.suggested_folder, "Fotos/2024/2024-07");
        assert_eq!(items[28].0.suggested_folder, "Fotos/2024/2024-08");
    }

    /// Minimal little-endian TIFF/EXIF block with Make, Model and DateTimeOriginal.
    fn exif_block(make: &str, model: &str, date: &str) -> Vec<u8> {
        let ascii = |s: &str| { let mut v = s.as_bytes().to_vec(); v.push(0); v };
        let (make, model, date) = (ascii(make), ascii(model), ascii(date));
        let ifd0_len = 2 + 3*12 + 4;
        let make_off = 8 + ifd0_len;
        let model_off = make_off + make.len();
        let exif_ifd_off = model_off + model.len();
        let date_off = exif_ifd_off + 2 + 12 + 4;
        let mut t = b"II*\0".to_vec();
        t.extend(8u32.to_le_bytes());
        let entry = |t: &mut Vec<u8>, tag: u16, typ: u16, count: usize, value: usize| {
            t.extend(tag.to_le_bytes()); t.extend(typ.to_le_bytes());
            t.extend((count as u32).to_le_bytes()); t.extend((value as u32).to_le_bytes());
        };
        t.extend(3u16.to_le_bytes());
        entry(&mut t, 0x010F, 2, make.len(), make_off);
        entry(&mut t, 0x0110, 2, model.len(), model_off);
        entry(&mut t, 0x8769, 4, 1, exif_ifd_off);
        t.extend(0u32.to_le_bytes());
        t.extend(&make); t.extend(&model);
        t.extend(1u16.to_le_bytes());
        entry(&mut t, 0x9003, 2, date.len(), date_off);
        t.extend(0u32.to_le_bytes());
        t.extend(&date);
        t
    }
    #[test]
    fn reads_exif_from_a_real_jpeg_and_prefers_it_over_the_name() {
        let dir = std::env::temp_dir().join(format!("photovault-exif-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::RgbImage::new(64, 48).write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
        let jpeg = jpeg.into_inner();
        let tiff = exif_block("Canon", "Canon EOS 5D", "2009:08:01 18:45:10");
        let mut app1 = vec![0xFF, 0xE1];
        app1.extend(((tiff.len() + 8) as u16).to_be_bytes());
        app1.extend(b"Exif\0\0");
        app1.extend(&tiff);
        // Name says 2020; EXIF says 2009 and must win. 64x48 would otherwise be "posible basura".
        let path = dir.join("copia 20200101.jpg");
        let mut file = File::create(&path).unwrap();
        file.write_all(&jpeg[..2]).unwrap();
        file.write_all(&app1).unwrap();
        file.write_all(&jpeg[2..]).unwrap();
        drop(file);
        let before = fs::read(&path).unwrap();
        let (item, _) = inspect(&path).unwrap();
        assert_eq!(item.date_source, "exif");
        assert_eq!(item.date.as_deref(), Some("2009-08-01 18:45:10"));
        assert_eq!(item.camera.as_deref(), Some("Canon EOS 5D"));
        assert_eq!(item.category, Category::Photo);
        assert_eq!(fs::read(&path).unwrap(), before, "el análisis no modifica el archivo");
        fs::remove_dir_all(dir).unwrap();
    }
}
