use anyhow::{Context, Result};
use serde_json::Value;
use crate::db::{DbFile, StoredEntry, StoredFile};

pub async fn run_update_from_url(api_url: &str) -> Result<DbFile> {
    let client = reqwest::Client::new();
    let mut req = client.get(api_url).header("User-Agent", "ip-ranger/1.0");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.bearer_auth(token);
    }

    let list_resp = req.send().await.context("listing files from GitHub API")?;
    if !list_resp.status().is_success() {
        anyhow::bail!("GitHub API list failed: {}", list_resp.status());
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
            println!("downloaded: {}", name);

            let mut this_file_prefixes: Vec<String> = Vec::new();

            // First try JSON parsing (expected for many provider files)
            if let Ok(val) = serde_json::from_str::<Value>(&text) {
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

    // dedupe
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::with_capacity(stored_entries.len());
    for e in stored_entries {
        map.entry(e.prefix).or_insert(e.provider);
    }

    let deduped: Vec<StoredEntry> = map.into_iter().map(|(p, prov)| StoredEntry { prefix: p, provider: prov }).collect();

    let db = DbFile {
        entries: deduped,
        source_commit: None,
        files: stored_files,
    };

    Ok(db)
}
