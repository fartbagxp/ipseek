use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredEntry {
    pub prefix: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredFile {
    pub name: String,
    pub content: String,
    pub prefixes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DbFile {
    pub entries: Vec<StoredEntry>,
    pub source_commit: Option<String>,
    /// Raw downloaded files (filename -> content)
    pub files: Vec<StoredFile>,
}

pub fn load_db(db_path: &str) -> Result<DbFile> {
    let mut f = std::fs::File::open(db_path).with_context(|| format!("open db {}", db_path))?;
    let mut buf = Vec::new();
    use std::io::Read;
    f.read_to_end(&mut buf)?;

    // Try gzipped JSON first
    let mut decoder = flate2::read::GzDecoder::new(&buf[..]);
    let mut decompressed = Vec::new();
    if decoder.read_to_end(&mut decompressed).is_ok() && !decompressed.is_empty() {
        if let Ok(val) = serde_json::from_slice::<DbFile>(&decompressed) {
            return Ok(val);
        }
    }

    // Fallback to plain JSON
    let val = serde_json::from_slice::<DbFile>(&buf).context("deserialize JSON")?;
    Ok(val)
}

pub fn save_db(db: &DbFile, db_path: &str) -> Result<()> {
    // serialize + gzip as JSON
    let tmpdir = tempfile::tempdir().context("create tmpdir")?;
    let tmp_out = tmpdir.path().join("db.bin.gz");
    let f = std::fs::File::create(&tmp_out)?;
    let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
    let json_bytes = serde_json::to_vec(db)?;
    use std::io::Write;
    enc.write_all(&json_bytes)?;
    enc.finish()?;

    let db_path = std::path::PathBuf::from(db_path);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(&tmp_out, &db_path) {
        Ok(_) => {}
        Err(e) => {
            log::warn!("rename failed, falling back to copy: {}", e);
            std::fs::copy(&tmp_out, &db_path).context("copying db file fallback")?;
        }
    }

    Ok(())
}
