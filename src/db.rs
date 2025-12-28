use anyhow::{Context, Result};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Archive, RkyvSerialize, RkyvDeserialize)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct StoredFile {
    pub name: String,
    pub content: String,
    pub prefixes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
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

    // Try gzipped data first
    let mut decoder = flate2::read::GzDecoder::new(&buf[..]);
    let mut decompressed = Vec::new();
    if decoder.read_to_end(&mut decompressed).is_ok() && !decompressed.is_empty() {
        // Try rkyv first (new format - rkyv 0.8 API)
        if let Ok(db) = rkyv::from_bytes::<DbFile, rkyv::rancor::Error>(&decompressed) {
            return Ok(db);
        }

        // Fallback to JSON (old format for backward compatibility)
        if let Ok(val) = serde_json::from_slice::<DbFile>(&decompressed) {
            log::info!("Loaded database from legacy JSON format");
            return Ok(val);
        }
    }

    // Fallback to plain JSON (oldest format)
    let val = serde_json::from_slice::<DbFile>(&buf).context("deserialize JSON")?;
    log::info!("Loaded database from legacy uncompressed JSON format");
    Ok(val)
}

pub fn save_db(db: &DbFile, db_path: &str) -> Result<()> {
    // Serialize using rkyv (faster and more compact than JSON)
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(db)
        .map_err(|e| anyhow::anyhow!("serialize with rkyv: {}", e))?;

    // Compress with gzip
    let tmpdir = tempfile::tempdir().context("create tmpdir")?;
    let tmp_out = tmpdir.path().join("db.bin.gz");
    let f = std::fs::File::create(&tmp_out)?;
    let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
    use std::io::Write;
    enc.write_all(&bytes)?;
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
