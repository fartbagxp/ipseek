use anyhow::{Context, Result};
use serde_json::Value;
use crate::db::{DbFile, StoredEntry, StoredFile, Metadata};
use std::collections::HashMap;

fn normalize_ip_prefix(prefix: &str) -> String {
    if prefix.contains('/') {
        prefix.to_string()
    } else {
        // Check if it's a valid IP and add appropriate CIDR suffix
        if let Ok(ip) = prefix.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(_) => format!("{}/32", prefix),
                std::net::IpAddr::V6(_) => format!("{}/128", prefix),
            }
        } else {
            prefix.to_string()
        }
    }
}

fn create_entry(prefix: String, provider: String, region: Option<String>, service: Option<String>, metadata: HashMap<String, String>) -> StoredEntry {
    StoredEntry {
        prefix,
        provider,
        region,
        service,
        metadata: Metadata { fields: metadata },
    }
}

pub fn run_update_from_local(data_dir: &str) -> Result<DbFile> {
    let path = std::path::Path::new(data_dir);
    if !path.exists() {
        anyhow::bail!("Directory does not exist: {}", data_dir);
    }

    let mut stored_entries: Vec<StoredEntry> = Vec::new();
    let mut stored_files: Vec<StoredFile> = Vec::new();

    // Read all .json and .txt files from the directory
    for entry in std::fs::read_dir(path).context("reading data directory")? {
        let entry = entry?;
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        let name = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if !name.ends_with(".json") && !name.ends_with(".txt") {
            continue;
        }

        log::info!("Processing local file: {}", name);
        println!("processing: {}", name);

        let text = std::fs::read_to_string(&file_path)
            .with_context(|| format!("reading file {}", name))?;

        let mut this_file_prefixes: Vec<String> = Vec::new();

        // First try JSON parsing (expected for many provider files)
        if let Ok(val) = serde_json::from_str::<Value>(&text) {
            let provider = name.trim_end_matches(".json").trim_end_matches(".txt").to_string();

            // Azure format: { "values": [{ "properties": { "addressPrefixes": [...] } }] }
            if let Some(values) = val.get("values").and_then(|v| v.as_array()) {
                for value in values {
                    if let Some(props) = value.get("properties") {
                        let region = props.get("region").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let service = props.get("systemService").and_then(|v| v.as_str()).map(|s| s.to_string());

                        let mut metadata = HashMap::new();
                        if let Some(platform) = props.get("platform").and_then(|v| v.as_str()) {
                            metadata.insert("platform".to_string(), platform.to_string());
                        }
                        if let Some(cloud) = val.get("cloud").and_then(|v| v.as_str()) {
                            metadata.insert("cloud".to_string(), cloud.to_string());
                        }

                        if let Some(addr_prefixes) = props.get("addressPrefixes").and_then(|v| v.as_array()) {
                            for prefix in addr_prefixes {
                                if let Some(pref_str) = prefix.as_str() {
                                    let normalized = normalize_ip_prefix(pref_str);
                                    stored_entries.push(create_entry(
                                        normalized.clone(),
                                        provider.clone(),
                                        region.clone(),
                                        service.clone(),
                                        metadata.clone()
                                    ));
                                    this_file_prefixes.push(normalized);
                                }
                            }
                        }
                    }
                }
            }

            // AWS format: { "prefixes": [{ "ip_prefix": "...", "region": "...", "service": "..." }] }
            if let Some(arr) = val.get("prefixes").and_then(|v| v.as_array()) {
                for obj in arr {
                    let pref_opt = obj.get("ip_prefix").and_then(|v| v.as_str())
                        .or_else(|| obj.get("ipv4_prefix").and_then(|v| v.as_str()));

                    if let Some(pref) = pref_opt {
                        let region = obj.get("region").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let service = obj.get("service").and_then(|v| v.as_str()).map(|s| s.to_string());

                        let mut metadata = HashMap::new();
                        if let Some(nbg) = obj.get("network_border_group").and_then(|v| v.as_str()) {
                            metadata.insert("network_border_group".to_string(), nbg.to_string());
                        }

                        stored_entries.push(create_entry(
                            pref.to_string(),
                            provider.clone(),
                            region,
                            service,
                            metadata
                        ));
                        this_file_prefixes.push(pref.to_string());
                    }
                }
            }

            if let Some(arr6) = val.get("ipv6_prefixes").and_then(|v| v.as_array()) {
                for obj in arr6 {
                    if let Some(pref) = obj.get("ipv6_prefix").and_then(|v| v.as_str()) {
                        let region = obj.get("region").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let service = obj.get("service").and_then(|v| v.as_str()).map(|s| s.to_string());

                        let mut metadata = HashMap::new();
                        if let Some(nbg) = obj.get("network_border_group").and_then(|v| v.as_str()) {
                            metadata.insert("network_border_group".to_string(), nbg.to_string());
                        }

                        stored_entries.push(create_entry(
                            pref.to_string(),
                            provider.clone(),
                            region,
                            service,
                            metadata
                        ));
                        this_file_prefixes.push(pref.to_string());
                    }
                }
            }

            // GitHub format: { "hooks": [...], "web": [...], ... }
            // Check for service-specific arrays
            let service_keys = ["hooks", "web", "api", "git", "packages", "pages", "importer", "actions", "dependabot"];
            for service_key in &service_keys {
                if let Some(arr) = val.get(service_key).and_then(|v| v.as_array()) {
                    for item in arr {
                        if let Some(pref_str) = item.as_str() {
                            let normalized = normalize_ip_prefix(pref_str);
                            stored_entries.push(create_entry(
                                normalized.clone(),
                                provider.clone(),
                                None,
                                Some(service_key.to_string()),
                                HashMap::new()
                            ));
                            this_file_prefixes.push(normalized);
                        }
                    }
                }
            }

            // Fallback: Generic array of objects with ip_prefix/ipv6_prefix
            if let Some(arr) = val.as_array() {
                for obj in arr {
                    if let Some(pref) = obj.get("ip_prefix").and_then(|v| v.as_str()) {
                        stored_entries.push(create_entry(
                            pref.to_string(),
                            provider.clone(),
                            None,
                            None,
                            HashMap::new()
                        ));
                        this_file_prefixes.push(pref.to_string());
                    }
                    if let Some(pref) = obj.get("ipv6_prefix").and_then(|v| v.as_str()) {
                        stored_entries.push(create_entry(
                            pref.to_string(),
                            provider.clone(),
                            None,
                            None,
                            HashMap::new()
                        ));
                        this_file_prefixes.push(pref.to_string());
                    }
                }
            }
        } else {
            // Plain text format
            let provider = name.trim_end_matches(".json").trim_end_matches(".txt").to_string();
            for line in text.lines() {
                let l = line.trim();
                if l.is_empty() { continue; }

                let prefix = normalize_ip_prefix(l);
                stored_entries.push(create_entry(
                    prefix.clone(),
                    provider.clone(),
                    None,
                    None,
                    HashMap::new()
                ));
                this_file_prefixes.push(prefix);
            }
        }

        stored_files.push(StoredFile { name: name.clone(), content: text.clone(), prefixes: this_file_prefixes });
    }

    // dedupe - keep the first occurrence with most complete metadata
    let mut map: std::collections::HashMap<String, StoredEntry> = std::collections::HashMap::with_capacity(stored_entries.len());
    for e in stored_entries {
        map.entry(e.prefix.clone()).or_insert(e);
    }

    let deduped: Vec<StoredEntry> = map.into_values().collect();

    println!("Processed {} files, {} unique prefixes", stored_files.len(), deduped.len());

    let db = DbFile {
        entries: deduped,
        source_commit: None,
        files: stored_files,
    };

    Ok(db)
}

