use axum::{routing::get, Router, extract::Path, response::Json};
use log::debug;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::RwLock;
use iptrie::Ipv4Prefix;
use iptrie::Ipv4RTrieSet;
use clap::{Parser, Subcommand};

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use tempfile;
use flate2::{Compression, write::GzEncoder, read::GzDecoder};
use bincode;

mod parser;
use crate::parser::aws::{CidrBlock, AwsCidrRanges};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredEntry {
    prefix: String,
    provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DbFile {
    entries: Vec<StoredEntry>,
    source_commit: Option<String>,
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

pub async fn check_ip(Path(ip_str): Path<String>) -> Json<serde_json::Value> {
    debug!("Requested IP: {}", ip_str);
    let response = if let Ok(ip) = ip_str.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            serde_json::json!({
                "error": "",
                "data": "Private IP"
            })
        } else {
            serde_json::json!({
                "error": "",
                "data": "Public IP"
            })
        }
    } else {
        serde_json::json!({
            "error": "Invalid IP address",
            "data": ""
        })
    };

    Json(response)
}

// Function to fetch AWS CIDR ranges
async fn fetch_aws_cidr_ranges() -> Result<AwsCidrRanges, Box<dyn std::error::Error>> {
    let url = "https://raw.githubusercontent.com/fartbagxp/aas-cidr-ranges/master/data/raw/aws.json";
    let response = reqwest::get(url).await?.text().await?;
    let aws_cidr_ranges: AwsCidrRanges = serde_json::from_str(&response)?;
    Ok(aws_cidr_ranges)
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

    for e in entries.into_iter().filter(|e| e.kind == "file" && e.name.ends_with(".json")) {
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

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                let provider = e.name.trim_end_matches(".json").to_string();

                if let Some(arr) = val.get("prefixes").and_then(|v| v.as_array()) {
                    for obj in arr {
                        if let Some(pref) = obj.get("ip_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                        } else if let Some(pref) = obj.get("ipv4_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                        }
                    }
                }

                if let Some(arr6) = val.get("ipv6_prefixes").and_then(|v| v.as_array()) {
                    for obj in arr6 {
                        if let Some(pref) = obj.get("ipv6_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                        }
                    }
                }

                if let Some(arr) = val.as_array() {
                    for obj in arr {
                        if let Some(pref) = obj.get("ip_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                        }
                        if let Some(pref) = obj.get("ipv6_prefix").and_then(|v| v.as_str()) {
                            stored_entries.push(StoredEntry { prefix: pref.to_string(), provider: provider.clone() });
                        }
                    }
                }
            } else {
                log::warn!("failed to parse json from {}", url);
            }
        }
    }

    // dedupe and persist (same as previous logic)
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::with_capacity(stored_entries.len());
    for e in stored_entries {
        map.entry(e.prefix).or_insert(e.provider);
    }

    let deduped: Vec<StoredEntry> = map.into_iter().map(|(p, prov)| StoredEntry { prefix: p, provider: prov }).collect();

    let db = DbFile {
        entries: deduped,
        source_commit: None,
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

#[tokio::main]
async fn main() {
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
            if let Err(e) = run_update(&cli.db).await {
                eprintln!("run_update failed: {:#?}", e);
                std::process::exit(1);
            }
            println!("run_update completed. DB written to {}", cli.db);
            return;
        }
        Commands::Check { ip } => {
            match lookup_provider(&cli.db, &ip) {
                Ok(p) => println!("{} -> {}", ip, p),
                Err(e) => {
                    eprintln!("lookup failed: {:#?}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Commands::Serve => {
            // fallthrough to start server
        }
    }

    let prefixes = [
        "1.1.0.0/24",
        "1.1.1.0/24",
        "1.1.0.0/20",
        "1.1.2.0/24"
    ];

    let iter = prefixes.iter().map(|x| x.parse::<Ipv4Prefix>().unwrap());

    // a set based on Patricia trie
    let trie = Ipv4RTrieSet::from_iter(iter);

    // lpm lookup for Ipv4 address
    assert_eq!(trie.lookup(&"1.1.1.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.1.0/24");
    assert_eq!(trie.lookup(&"1.1.2.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.2.0/24");

    // lpm lookup for Ipv4 prefix also works
    assert_eq!(trie.lookup(&"1.1.0.0/25".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/24");
    assert_eq!(trie.lookup(&"1.1.0.0/21".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/20");

    // now, compute the set based on LC-trie
    let lctrie = trie.compress();

    // lpm lookup for Ipv4 address
    assert_eq!(lctrie.lookup(&"1.1.1.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.1.0/24");
    assert_eq!(lctrie.lookup(&"1.1.2.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.2.0/24");

    // lpm lookup for Ipv4 prefix also works
    assert_eq!(lctrie.lookup(&"1.1.0.0/25".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/24");
    assert_eq!(lctrie.lookup(&"1.1.0.0/21".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/20");

    // Start the server
    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/{ip}", get(check_ip));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Synchronous lookup helper used by CLI `check` command.
fn lookup_provider(db_path: &str, ip_str: &str) -> Result<String> {
    let db = load_db(db_path)?;
    let ip: IpAddr = ip_str.parse().context("parse ip")?;

    // build IPv4 trie
    let mut prefix_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut v4_prefixes = Vec::new();
    let mut v6_list: Vec<(IpNet, String)> = Vec::new();

    for e in db.entries.into_iter() {
        if let Ok(net) = e.prefix.parse::<IpNet>() {
            match net {
                IpNet::V4(v4) => {
                    // prefer parsing the textual prefix, fallback to constructing from addr/prefix_len
                    let p = v4.to_string().parse::<Ipv4Prefix>().unwrap_or_else(|_| Ipv4Prefix::new(u32::from(v4.addr()).into(), v4.prefix_len()).unwrap());
                    v4_prefixes.push(p);
                    prefix_map.insert(p.to_string(), e.provider);
                }
                IpNet::V6(v6) => {
                    v6_list.push((IpNet::V6(v6), e.provider));
                }
            }
        }
    }

    if let IpAddr::V4(a) = ip {
        let trie = Ipv4RTrieSet::from_iter(v4_prefixes.into_iter());
        let found = trie.lookup(&a);
        if let Some(provider) = prefix_map.get(&found.to_string()) {
            return Ok(provider.clone());
        }
        return Ok("unknown".to_string());
    } else {
        // IPv6 linear search
        for (net, prov) in v6_list {
            if net.contains(&ip) {
                return Ok(prov);
            }
        }
        Ok("unknown".to_string())
    }
}