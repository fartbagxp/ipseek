use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredEntry {
    pub prefix: String,
    pub provider: String,
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
    // support gzipped bincode
    let mut maybe = flate2::read::GzDecoder::new(&buf[..]);
    let mut decompressed = Vec::new();
    if let Ok(_) = maybe.read_to_end(&mut decompressed) {
        if let Ok((val, _)) = bincode::serde::decode_from_slice::<DbFile, _>(&decompressed, bincode::config::standard()) {
            return Ok(val);
        }
    }
    // fallback: try decoding raw slice with bincode v2 serde
    let (val, _) = bincode::serde::decode_from_slice::<DbFile, _>(&buf, bincode::config::standard()).context("deserialize plain bincode")?;
    Ok(val)
}

pub fn save_db(db: &DbFile, db_path: &str) -> Result<()> {
    // serialize + gzip
    let tmpdir = tempfile::tempdir().context("create tmpdir")?;
    let tmp_out = tmpdir.path().join("db.bin.gz");
    let f = std::fs::File::create(&tmp_out)?;
    let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
    let bytes = bincode::serde::encode_to_vec(db, bincode::config::standard())?;
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
