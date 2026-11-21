//! User decisions persist outside original media. No EXIF writes.
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{fs,path::{Path,PathBuf},time::UNIX_EPOCH};

#[derive(Debug,Clone,Serialize)]
pub struct Decision {
    pub decision:String,
    pub reviewed_at:String,
    pub content_changed:bool,
}
fn database(root:&Path)->Result<Connection,String>{
    let canonical=root.canonicalize().map_err(|e|e.to_string())?;
    if !canonical.is_dir(){return Err("La biblioteca no es una carpeta".into());}
    let dir=dirs::data_local_dir().ok_or("No se encontró el directorio de datos local")?.join("PhotoVault");
    fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    let key=blake3::hash(canonical.to_string_lossy().as_bytes()).to_hex();
    let db=Connection::open(dir.join(format!("decisions-{key}.sqlite"))).map_err(|e|e.to_string())?;
    db.busy_timeout(std::time::Duration::from_secs(3)).map_err(|e|e.to_string())?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS decisions(
        path TEXT PRIMARY KEY NOT NULL,
        decision TEXT NOT NULL CHECK(decision IN ('keep','snapshot','photo','review_later','clear')),
        size TEXT NOT NULL,
        modified_ns TEXT NOT NULL,
        reviewed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );").map_err(|e|e.to_string())?;
    Ok(db)
}
fn checked_path(root:&Path,path:&Path)->Result<PathBuf,String>{
    let base=root.canonicalize().map_err(|e|e.to_string())?;
    let actual=path.canonicalize().map_err(|e|e.to_string())?;
    if !actual.starts_with(base) || !actual.is_file(){return Err("Archivo fuera de la biblioteca".into());}
    Ok(actual)
}
fn fingerprint(path:&Path)->Result<(String,String),String>{
    let m=fs::metadata(path).map_err(|e|e.to_string())?;
    let stamp=m.modified().map_err(|e|e.to_string())?
        .duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos().to_string();
    Ok((m.len().to_string(),stamp))
}
pub fn save(root:&Path,path:&Path,decision:&str)->Result<(),String>{
    if !["keep","snapshot","photo","review_later","clear"].contains(&decision){
        return Err("Decisión no admitida".into());
    }
    let actual=checked_path(root,path)?;
    let db=database(root)?;
    if decision=="clear" {
        db.execute("DELETE FROM decisions WHERE path=?1",params![actual.to_string_lossy().as_ref()])
            .map_err(|e|e.to_string())?;
    } else {
        let (size,stamp)=fingerprint(&actual)?;
        db.execute("INSERT INTO decisions(path,decision,size,modified_ns) VALUES(?1,?2,?3,?4)
            ON CONFLICT(path) DO UPDATE SET decision=excluded.decision,size=excluded.size,
            modified_ns=excluded.modified_ns,reviewed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            params![actual.to_string_lossy().as_ref(),decision,size,stamp])
            .map_err(|e|e.to_string())?;
    }
    Ok(())
}
pub fn get(root:&Path,path:&Path)->Result<Option<Decision>,String>{
    let actual=checked_path(root,path)?;
    let db=database(root)?;
    let stored:Option<(String,String,String,String)>=db.query_row(
        "SELECT decision,size,modified_ns,reviewed_at FROM decisions WHERE path=?1",
        params![actual.to_string_lossy().as_ref()],
        |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))
    ).optional().map_err(|e|e.to_string())?;
    match stored {
        None=>Ok(None),
        Some((decision,size,stamp,reviewed_at))=>{
            let (current_size,current_stamp)=fingerprint(&actual)?;
            Ok(Some(Decision{decision,reviewed_at,content_changed:size!=current_size||stamp!=current_stamp}))
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rejects_invalid_decisions(){
        let root=std::env::temp_dir();
        assert!(save(&root,&root.join("nonexistent.jpg"),"delete").is_err());
    }
}
