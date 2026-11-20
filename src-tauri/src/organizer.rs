//! Read-only photo organization proposal. No file operations except metadata/decoding.
use image::ImageReader;
use serde::Serialize;
use std::{fs, path::{Path,PathBuf}};

const PHOTOS: &[&str] = &["jpg","jpeg","png","webp","bmp","gif","tif","tiff"];
const VIDEOS: &[&str] = &["mp4","mov","avi","mkv","mts","3gp","m4v","wmv","m2ts","webm"];
const MAX_FILES: usize = 20_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum Category { Photo, Snapshot, PossibleTrash, Undated, Video }

#[derive(Debug, Serialize)]
pub struct PlannedFile {
    pub path: String,
    pub category: Category,
    pub suggested_folder: String,
    pub reason: String,
    pub date_source: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
#[derive(Debug, Serialize)]
pub struct OrganizationPlan {
    pub analyzed: usize,
    pub skipped: usize,
    pub errors: usize,
    pub files: Vec<PlannedFile>,
    pub read_only: bool,
}

fn parse_date(name: &str) -> Option<String> {
    let bytes=name.as_bytes();
    for i in 0..bytes.len().saturating_sub(7) {
        let s=&bytes[i..];
        if s.len()<8 || !s[..8].iter().all(u8::is_ascii_digit) { continue; }
        let year=std::str::from_utf8(&s[..4]).ok()?.parse::<u32>().ok()?;
        let month=std::str::from_utf8(&s[4..6]).ok()?.parse::<u32>().ok()?;
        let day=std::str::from_utf8(&s[6..8]).ok()?.parse::<u32>().ok()?;
        if (1970..=2100).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day) {
            return Some(format!("{year:04}-{month:02}-{day:02}"));
        }
    }
    // ISO YYYY-MM-DD, only if separator positions and digits match.
    for part in name.as_bytes().windows(10) {
        if part[4]==b'-' && part[7]==b'-'
            && [0,1,2,3,5,6,8,9].iter().all(|&j|part[j].is_ascii_digit()) {
            let s=std::str::from_utf8(part).ok()?;
            let year=s[0..4].parse::<u32>().ok()?;
            let month=s[5..7].parse::<u32>().ok()?;
            let day=s[8..10].parse::<u32>().ok()?;
            if (1970..=2100).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day) {
                return Some(s.to_owned());
            }
        }
    }
    None
}

fn screen_ratio(w:u32,h:u32)->bool {
    if h==0 { return false; }
    let ratio=w as f64/h as f64;
    [16./9.,19./9.,18./9.,19.5/9.,20./9.,4./3.,3./2.,16./10.,
     9./16.,9./19.,9./19.5,9./20.,3./4.,2./3.,10./16.]
     .iter().any(|&r| ((ratio-r)/r).abs()<0.015)
}
fn snapshot_name(name:&str)->bool {
    let n=name.to_ascii_lowercase();
    ["screenshot","captura","screen","img-wa","whatsapp","signal","telegram","snap"]
        .iter().any(|p|n.contains(p))
}
fn camera_name(name:&str)->bool {
    let n=name.to_ascii_lowercase();
    ["img_","dsc","dscn","dscf","dcim","photo","foto","cam","mvimg_","pxl_"]
        .iter().any(|p|n.starts_with(p))
}
fn classify(name:&str, dimensions:Option<(u32,u32)>, video:bool, date:Option<&str>)->(Category,String) {
    if video { return (Category::Video,"Video: fecha del nombre o pendiente de verificar".into()); }
    let Some((w,h))=dimensions else { return (Category::Undated,"No se pudieron leer las dimensiones; revisar formato o archivo".into()); };
    if snapshot_name(name) {
        return (Category::Snapshot,"Nombre compatible con captura o imagen de mensajería; sugerencia".into());
    }
    if camera_name(name) {
        return (Category::Photo,"Nombre compatible con cámara; falta verificar EXIF".into());
    }
    if u64::from(w)*u64::from(h)<1_000_000 {
        return (Category::PossibleTrash,"Menos de 1 MP y sin indicios en el nombre de cámara; NO implica que sea basura".into());
    }
    if screen_ratio(w,h) {
        return (Category::Snapshot,"Proporción de pantalla; también puede ser una fotografía real".into());
    }
    if date.is_none() {
        return (Category::Undated,"Sin fecha reconocible en el nombre; no se ha verificado EXIF".into());
    }
    (Category::Photo,"Imagen de resolución suficiente; clasificación provisional sin EXIF".into())
}

fn inspect(path:&Path)->Result<PlannedFile,String> {
    let name=path.file_name().and_then(|x|x.to_str()).unwrap_or("");
    let ext=path.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase();
    let video=VIDEOS.contains(&ext.as_str());
    let date=parse_date(name);
    // File modification time is deliberately not used as a capture date.
    let dimensions=if video { None } else {
        ImageReader::open(path).map_err(|e|e.to_string())?
            .with_guessed_format().map_err(|e|e.to_string())?
            .into_dimensions().ok()
    };
    let (category,reason)=classify(name,dimensions,video,date.as_deref());
    let year=date.as_deref().map(|d|&d[..4]);
    let suggested_folder=match (&category,year) {
        (Category::Photo,Some(y))=>format!("Fotos/{y}/{}", &date.as_ref().unwrap()[..7]),
        (Category::Video,Some(y))=>format!("Fotos/{y}/{}", &date.as_ref().unwrap()[..7]),
        (Category::Snapshot,Some(y))=>format!("Snapshots/{y}"),
        (Category::PossibleTrash,_)=>"Basura Probable (revisión manual)".into(),
        _=>"_Sin_Fecha (revisión manual)".into()
    };
    Ok(PlannedFile {
        path:path.to_string_lossy().into_owned(), category, suggested_folder, reason,
        date_source:if date.is_some(){"nombre".into()}else{"no_verificada".into()},
        width:dimensions.map(|d|d.0),height:dimensions.map(|d|d.1),
    })
}

pub fn plan(root:&Path)->Result<OrganizationPlan,String> {
    if !root.is_dir() {return Err("La carpeta no existe o no es accesible".into());}
    let mut stack=vec![root.to_path_buf()];
    let mut paths:Vec<PathBuf>=Vec::new();
    let mut errors=0;
    while let Some(dir)=stack.pop() {
        let entries=match fs::read_dir(dir){Ok(x)=>x,Err(_)=>{errors+=1;continue}};
        for entry in entries {
            let entry=match entry{Ok(x)=>x,Err(_)=>{errors+=1;continue}};
            let kind=match entry.file_type(){Ok(x)=>x,Err(_)=>{errors+=1;continue}};
            if kind.is_symlink(){continue;}
            if kind.is_dir(){stack.push(entry.path());continue;}
            if !kind.is_file(){continue;}
            let ext=entry.path().extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase();
            if PHOTOS.contains(&ext.as_str())||VIDEOS.contains(&ext.as_str()){paths.push(entry.path());}
        }
    }
    paths.sort();
    let skipped=paths.len().saturating_sub(MAX_FILES);
    paths.truncate(MAX_FILES);
    let mut files=Vec::with_capacity(paths.len());
    for path in paths {
        match inspect(&path){
            Ok(mut item)=>{
                if let Ok(Some(saved))=crate::decisions::get(root,&path) {
                    if saved.content_changed {
                        item.reason=format!("Decisión anterior: {}. Archivo modificado: requiere nueva revisión. {}",saved.decision,item.reason);
                    } else {
                        match saved.decision.as_str(){
                            "keep"|"photo"=>{
                                item.category=Category::Photo;
                                item.suggested_folder="_Conservar (decisión del usuario)".into();
                            }
                            "snapshot"=>{
                                item.category=Category::Snapshot;
                                item.suggested_folder="Snapshots (decisión del usuario)".into();
                            }
                            _=>{}
                        }
                        item.reason=format!("Decisión del usuario: {}. {}",saved.decision,item.reason);
                    }
                }
                files.push(item);
            }
            Err(_)=>errors+=1
        }
    }
    Ok(OrganizationPlan{analyzed:files.len(),skipped,errors,files,read_only:true})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn low_resolution_is_only_a_suggestion() {
        let (cat,reason)=classify("unknown.png",Some((320,240)),false,None);
        assert_eq!(cat,Category::PossibleTrash);
        assert!(reason.contains("NO implica"));
    }
    #[test]
    fn camera_name_protects_small_photos() {
        assert_eq!(classify("IMG_20050101_123456.jpg",Some((320,240)),false,Some("2005-01-01")).0,Category::Photo);
    }
    #[test]
    fn screenshot_name_overrides_resolution() {
        assert_eq!(classify("Screenshot_20240101.png",Some((1440,3040)),false,Some("2024-01-01")).0,Category::Snapshot);
    }
    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("IMG_20240715_143022.jpg").as_deref(),Some("2024-07-15"));
        assert_eq!(parse_date("foto-2024-07-15.jpg").as_deref(),Some("2024-07-15"));
        assert_eq!(parse_date("20241399.jpg"),None);
    }
    #[test]
    fn never_guess_video_capture_date_from_mtime() {
        let (cat,_)=classify("video.mp4",None,true,None);
        assert_eq!(cat,Category::Video);
    }
}
