use axum::{routing::get, Router, extract::Path, response::Json, extract::Extension};
use log::debug;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use clap::{Parser, Subcommand};

use anyhow::{bail, Context, Result};
use tempfile;
use bincode;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredEntry {
    prefix: String,
    provider: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredFile {
    name: String,
    content: String,
    prefixes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DbFile {
    entries: Vec<StoredEntry>,
    source_commit: Option<String>,
    /// Raw downloaded files (filename -> content)
    files: Vec<StoredFile>,
}

/// Normalize a provider label (usually derived from a filename) to a canonical short name.
fn canonical_provider(name: &str) -> String {
    let mut s = name.to_lowercase();
    // strip common extensions if present
    if s.ends_with(".json") {
        s = s.trim_end_matches(".json").to_string();
    }
    if s.ends_with(".txt") {
        s = s.trim_end_matches(".txt").to_string();
    }

    // split on non-alphanumeric boundaries
    let mut tokens: Vec<&str> = s.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).collect();

    // stopwords which commonly appear as suffixes in filenames
    let stopwords = [
        "ips", "ipv4", "ipv6", "v4", "v6", "ip", "iprange", "ip-range", "range", "ranges",
        "public", "hosted", "cdn", "api", "webhook", "prefixes", "prefix",
    ];

    while tokens.len() > 1 {
        let last = *tokens.last().unwrap();
        if stopwords.contains(&last) || last.starts_with("ipv") || last.starts_with("ips") {
            tokens.pop();
        } else {
            break;
        }
    }

    tokens.join("-")
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            // Check if the first 7 bits of the address match `fc00::/7`
            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

#[derive(Clone)]
struct AppState {
    v4_list: Vec<(ipnet::Ipv4Net, String)>,
    v6_list: Vec<(IpNet, String)>,
    files: Vec<StoredFile>,
}

pub async fn check_ip(Extension(state): Extension<Arc<AppState>>, Path(ip_str): Path<String>) -> Json<serde_json::Value> {
    debug!("Requested IP: {}", ip_str);
    let response = match ip_str.parse::<IpAddr>() {
        Ok(ip) => {
            if is_private_ip(&ip) {
                serde_json::json!({"error":"","data":"Private IP"})
            } else {
                // perform lookup using cached state
                let provider = match ip {
                    IpAddr::V4(a) => {
                        // LPM
                        let mut best_len: i32 = -1;
                        let mut best_provider: Option<String> = None;
                        for (net, prov) in &state.v4_list {
                            if net.contains(&a) {
                                let len = net.prefix_len() as i32;
                                if len > best_len {
                                    // prefer provider coming from file prefixes if present
                                    let s = net.to_string();
                                    let mut candidate = prov.clone();
                                    for f in &state.files {
                                        if f.prefixes.iter().any(|p| p == &s) {
                                            candidate = canonical_provider(&f.name);
                                            break;
                                        }
                                    }
                                    best_len = len;
                                    best_provider = Some(candidate);
                                }
                            }
                        }
                        canonical_provider(&best_provider.unwrap_or_else(|| "unknown".to_string()))
                    }
                    IpAddr::V6(_) => {
                        let mut prov = "unknown".to_string();
                        for (net, p) in &state.v6_list {
                            if net.contains(&ip) {
                                prov = p.clone();
                                break;
                            }
                        }
                        canonical_provider(&prov)
                    }
                };

                serde_json::json!({"error":"","data":provider})
            }
        }
        Err(_) => serde_json::json!({"error":"Invalid IP address","data":""}),
    };

    Json(response)
}

async fn run_update(db_path: &str) -> Result<()> {
    log::info!("Fetching dataset file list from GitHub API...");
    let client = reqwest::Client::new();
    let api_url = "https://api.github.com/repos/fartbagxp/aas-cidr-ranges/contents/data/raw";
    let mut req = client.get(api_url)
        .header("User-Agent", "ip-ranger/1.0");

    // optional token to avoid strict rate limits
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.bearer_auth(token);
    }

    let list_resp = req.send().await.context("listing files from GitHub API")?;
    if !list_resp.status().is_success() {
        bail!("GitHub API list failed: {}", list_resp.status());
    }

    #[derive(serde::Deserialize)]
    struct GhEntry {
        name: String,
        download_url: Option<String>,
        #[serde(rename = "type")]
        kind: String,
    }

    let entries: Vec<GhEntry> = list_resp.json().await.context("parsing GitHub list JSON")?;

    let mut stored_entries: Vec<StoredEntry> = Vec::new();
    let mut stored_files: Vec<StoredFile> = Vec::new();

    for e in entries.into_iter().filter(|e| e.kind == "file" && (e.name.ends_with(".json") || e.name.ends_with(".txt"))) {
        if let Some(url) = e.download_url {
            log::info!("Fetching {}", url);
            let text = client.get(&url)
                .header("User-Agent", "ip-ranger/1.0")
                .send()
                .await
                .with_context(|| format!("fetching {}", url))?
                .text()
                .await
                .with_context(|| format!("reading {}", url))?;

            // Parse file and collect prefixes for this file (support JSON or simple TXT lists)
            let name = e.name.clone();
            // Print the filename immediately so `cargo run -- update` shows progress in plain output
            println!("downloaded: {}", name);

            let mut this_file_prefixes: Vec<String> = Vec::new();

            // First try JSON parsing (expected for many provider files)
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                let provider = name.trim_end_matches(".json").trim_end_matches(".txt").to_string();

                if let Some(arr) = val.get("prefixes").and_then(|v| v.as_array()) {
                    for obj in arr {
                        if let Some(pref) = obj.get("ip_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                            this_file_prefixes.push(pref.to_string());
                        } else if let Some(pref) = obj.get("ipv4_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                            this_file_prefixes.push(pref.to_string());
                        }
                    }
                }

                if let Some(arr6) = val.get("ipv6_prefixes").and_then(|v| v.as_array()) {
                    for obj in arr6 {
                        if let Some(pref) = obj.get("ipv6_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                            this_file_prefixes.push(pref.to_string());
                        }
                    }
                }

                if let Some(arr) = val.as_array() {
                    // Some files are plain arrays of prefix objects
                    for obj in arr {
                        if let Some(pref) = obj.get("ip_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                            this_file_prefixes.push(pref.to_string());
                        }
                        if let Some(pref) = obj.get("ipv6_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                            this_file_prefixes.push(pref.to_string());
                        }
                    }
                }
            } else {
                // Fallback: treat the file as a single-column list of CIDRs (TXT)
                let provider = name.trim_end_matches(".json").trim_end_matches(".txt").to_string();
                for line in text.lines() {
                    let l = line.trim();
                    if l.is_empty() { continue; }
                    stored_entries.push(StoredEntry { prefix: l.to_string(), provider: provider.clone() });
                    this_file_prefixes.push(l.to_string());
                }
            }

            stored_files.push(StoredFile { name: name.clone(), content: text.clone(), prefixes: this_file_prefixes });
        }
    }

    // No special-case fetching; repository files may include .txt lists (e.g. cloudflare) which we now parse

    // dedupe and persist (same as previous logic)
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::with_capacity(stored_entries.len());
    for e in stored_entries {
        map.entry(e.prefix).or_insert(e.provider);
    }

    let deduped: Vec<StoredEntry> = map.into_iter().map(|(p, prov)| StoredEntry { prefix: p, provider: prov }).collect();

    // Report which files will be stored in the DB for visibility
    let file_names: Vec<String> = stored_files.iter().map(|f| f.name.clone()).collect();
    println!("storing {} files into DB: {:?}", file_names.len(), file_names);

    let db = DbFile {
        entries: deduped,
        source_commit: None,
        files: stored_files,
    };

    // serialize + gzip
    let tmpdir = tempfile::tempdir().context("create tmpdir")?;
    let tmp_out = tmpdir.path().join("db.bin.gz");
    let f = std::fs::File::create(&tmp_out)?;
    let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
    let bytes = bincode::serde::encode_to_vec(&db, bincode::config::standard())?;
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

    println!("DB written to {}", db_path.to_string_lossy());
    Ok(())
}

fn load_db(db_path: &str) -> Result<DbFile> {
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

/// Synchronous lookup helper used by CLI `check` command.
fn lookup_provider(db_path: &str, ip_str: &str) -> Result<String> {
    let db = load_db(db_path)?;
    let ip: IpAddr = ip_str.parse().context("parse ip")?;

    // Build plain IpNet lists for robust longest-prefix matching
    let mut v4_list: Vec<(ipnet::Ipv4Net, String)> = Vec::new();
    let mut v6_list: Vec<(IpNet, String)> = Vec::new();

    for e in db.entries.into_iter() {
        if let Ok(net) = e.prefix.parse::<IpNet>() {
            match net {
                IpNet::V4(v4) => v4_list.push((v4, e.provider)),
                IpNet::V6(v6) => v6_list.push((IpNet::V6(v6), e.provider)),
            }
        }
    }

    if let IpAddr::V4(a) = ip {
        // Longest-prefix match among v4_list. When multiple providers exist for the same prefix,
        // prefer a provider derived from the filename if that file contains the prefix text.
        let mut best_len: i32 = -1;
        let mut best_provider: Option<String> = None;
        for (net, prov) in &v4_list {
            if net.contains(&a) {
                let len = net.prefix_len() as i32;
                if len > best_len {
                    // prefer provider coming from file contents if present
                    let s = net.to_string();
                    let mut candidate = prov.clone();
                    for f in &db.files {
                        if f.content.contains(&s) {
                            candidate = canonical_provider(&f.name);
                            break;
                        }
                    }
                    best_len = len;
                    best_provider = Some(candidate);
                }
            }
        }
        if let Some(p) = best_provider {
            return Ok(canonical_provider(&p));
        }

        // Fallback: scan per-file prefix sets and prefer the filename-derived provider
        for f in db.files.into_iter() {
            for pref in &f.prefixes {
                if let Ok(net) = pref.parse::<ipnet::Ipv4Net>() {
                    if net.contains(&a) {
                        let prov = canonical_provider(&f.name);
                        return Ok(prov);
                    }
                }
            }
        }

        return Ok("unknown".to_string());
    } else {
        // IPv6 linear search
        for (net, prov) in v6_list {
            if net.contains(&ip) {
                return Ok(canonical_provider(&prov));
            }
        }
        Ok("unknown".to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    // CLI
    #[derive(Parser)]
    #[command(author, version, about, long_about = None)]
    struct Cli {
        /// Path to local DB file
        #[arg(long, default_value = "./db.bin")]
        db: String,
        #[command(subcommand)]
        command: Commands,
    }

    #[derive(Subcommand)]
    enum Commands {
        /// Fetch datasets and rebuild local DB
        Update,
        /// Check a single IP address against the local DB
        Check { ip: String },
        /// Run HTTP server
        Serve,
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Update => {
            println!("Running run_update -> {}", cli.db);
            run_update(&cli.db).await?;
            println!("run_update completed. DB written to {}", cli.db);
            return Ok(());
        }
        Commands::Check { ip } => {
            let p = lookup_provider(&cli.db, &ip)?;
            println!("{} -> {}", ip, p);
            return Ok(());
        }
        Commands::Serve => {
            // Build cached app state from DB and pass into the server
            let db = match load_db(&cli.db) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("failed to load db {}: {:#?}", cli.db, e);
                    std::process::exit(1);
                }
            };

            let mut v4_list: Vec<(ipnet::Ipv4Net, String)> = Vec::new();
            let mut v6_list: Vec<(IpNet, String)> = Vec::new();
            for e in db.entries.iter() {
                if let Ok(net) = e.prefix.parse::<IpNet>() {
                    match net {
                        IpNet::V4(v4) => v4_list.push((v4, e.provider.clone())),
                        IpNet::V6(v6) => v6_list.push((IpNet::V6(v6), e.provider.clone())),
                    }
                }
            }

            // normalize providers in the cached state for consistent HTTP responses
            let v4_list = v4_list.into_iter().map(|(n, p)| (n, canonical_provider(&p))).collect();
            let v6_list = v6_list.into_iter().map(|(n, p)| (n, canonical_provider(&p))).collect();

            let state = Arc::new(AppState { v4_list, v6_list, files: db.files.clone() });

            // start server with state extension
            let app = Router::new()
                .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
                .route("/{ip}", get(check_ip))
                .layer(Extension(state));

            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
            axum::serve(listener, app).await.unwrap();
            return Ok(());
        }
    }

    // If we reach here without Serve command, fall through (this should be unreachable)
    Ok(())
}
