use crate::db::{DbFile, Metadata, StoredEntry, StoredFile};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

fn normalize_ip_prefix(prefix: &str) -> Option<String> {
    let p = prefix.trim();
    if p.is_empty() {
        return None;
    }
    if p.contains('/') {
        // Validate it parses as a CIDR.
        if p.parse::<ipnet::IpNet>().is_ok() {
            return Some(p.to_string());
        }
        return None;
    }
    match p.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(_)) => Some(format!("{}/32", p)),
        Ok(std::net::IpAddr::V6(_)) => Some(format!("{}/128", p)),
        Err(_) => None,
    }
}

/// Accumulates parsed entries for a single source file.
struct Sink<'a> {
    provider: &'a str,
    entries: &'a mut Vec<StoredEntry>,
    prefixes: Vec<String>,
    count: usize,
}

impl<'a> Sink<'a> {
    fn push(
        &mut self,
        raw_prefix: &str,
        region: Option<String>,
        service: Option<String>,
        metadata: HashMap<String, String>,
    ) {
        if let Some(prefix) = normalize_ip_prefix(raw_prefix) {
            self.entries.push(StoredEntry {
                prefix: prefix.clone(),
                provider: self.provider.to_string(),
                region: region.filter(|s| !s.is_empty()),
                service: service.filter(|s| !s.is_empty()),
                metadata: Metadata { fields: metadata },
            });
            self.prefixes.push(prefix);
            self.count += 1;
        }
    }

    fn simple(&mut self, raw_prefix: &str, service: Option<String>) {
        self.push(raw_prefix, None, service, HashMap::new());
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

/// Try every known structured shape against a parsed JSON value.
fn parse_json(val: &Value, sink: &mut Sink) {
    // Azure: { "values": [{ "properties": { "addressPrefixes": [...] } }] }
    if let Some(values) = val.get("values").and_then(|v| v.as_array()) {
        let cloud = str_of(val, "cloud");
        for value in values {
            if let Some(props) = value.get("properties") {
                let region = str_of(props, "region");
                let service = str_of(props, "systemService");
                let mut meta = HashMap::new();
                if let Some(p) = str_of(props, "platform") {
                    meta.insert("platform".into(), p);
                }
                if let Some(c) = &cloud {
                    meta.insert("cloud".into(), c.clone());
                }
                if let Some(arr) = props.get("addressPrefixes").and_then(|v| v.as_array()) {
                    for p in arr.iter().filter_map(|x| x.as_str()) {
                        sink.push(p, region.clone(), service.clone(), meta.clone());
                    }
                }
            }
        }
    }

    // AWS / GCP: { "prefixes": [{ ip_prefix|ipv4_prefix|ipv4Prefix|ipv6Prefix, ... }] }
    if let Some(arr) = val.get("prefixes").and_then(|v| v.as_array()) {
        for obj in arr {
            let pref = [
                "ip_prefix",
                "ipv4_prefix",
                "ipv4Prefix",
                "ipv6_prefix",
                "ipv6Prefix",
            ]
            .iter()
            .find_map(|k| str_of(obj, k));
            if let Some(pref) = pref {
                let region = str_of(obj, "region").or_else(|| str_of(obj, "scope"));
                let service = str_of(obj, "service");
                let mut meta = HashMap::new();
                if let Some(nbg) = str_of(obj, "network_border_group") {
                    meta.insert("network_border_group".into(), nbg);
                }
                sink.push(&pref, region, service, meta);
            }
        }
    }

    // AWS IPv6: { "ipv6_prefixes": [{ "ipv6_prefix": "..." }] }
    if let Some(arr) = val.get("ipv6_prefixes").and_then(|v| v.as_array()) {
        for obj in arr {
            if let Some(pref) = str_of(obj, "ipv6_prefix") {
                let region = str_of(obj, "region");
                let service = str_of(obj, "service");
                let mut meta = HashMap::new();
                if let Some(nbg) = str_of(obj, "network_border_group") {
                    meta.insert("network_border_group".into(), nbg);
                }
                sink.push(&pref, region, service, meta);
            }
        }
    }

    // Fastly: { "addresses": [...], "ipv6_addresses": [...] }
    for key in ["addresses", "ipv6_addresses"] {
        if let Some(arr) = val.get(key).and_then(|v| v.as_array()) {
            for p in arr.iter().filter_map(|x| x.as_str()) {
                sink.simple(p, None);
            }
        }
    }

    // Atlassian: { "items": [{ "cidr": "...", "product": [...], "region": [...] }] }
    if let Some(items) = val.get("items").and_then(|v| v.as_array()) {
        for it in items {
            if let Some(cidr) = str_of(it, "cidr") {
                let region = it
                    .get("region")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let service = it
                    .get("product")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                sink.push(&cidr, region, service, HashMap::new());
            }
        }
    }

    // Oracle: { "regions": [{ "region": "...", "cidrs": [{ "cidr": "..." }] }] }
    if let Some(regions) = val.get("regions").and_then(|v| v.as_array()) {
        for r in regions {
            let region = str_of(r, "region");
            if let Some(cidrs) = r.get("cidrs").and_then(|v| v.as_array()) {
                for c in cidrs {
                    if let Some(cidr) = str_of(c, "cidr") {
                        sink.push(&cidr, region.clone(), None, HashMap::new());
                    }
                }
            }
        }
    }

    // Zscaler / ZDX hub: { "hubPrefixes": ["...", ...] }
    if let Some(arr) = val.get("hubPrefixes").and_then(|v| v.as_array()) {
        for p in arr.iter().filter_map(|x| x.as_str()) {
            sink.simple(p, None);
        }
    }

    // M365: { "endpoints": [{ "serviceArea": "Exchange", "ips": [...] }] }
    if let Some(endpoints) = val.get("endpoints").and_then(|v| v.as_array()) {
        for ep in endpoints {
            if let Some(ips) = ep.get("ips").and_then(|v| v.as_array()) {
                let service = ep
                    .get("serviceArea")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                for p in ips.iter().filter_map(|x| x.as_str()) {
                    sink.push(p, None, service.clone(), HashMap::new());
                }
            }
        }
    }

    // Vultr: { "subnets": [{ "ip_prefix": "...", "alpha2code": "JP", "region": "JP-13" }] }
    if let Some(subnets) = val.get("subnets").and_then(|v| v.as_array()) {
        for s in subnets {
            if let Some(pref) = s.get("ip_prefix").and_then(|v| v.as_str()) {
                let region = s
                    .get("alpha2code")
                    .and_then(|v| v.as_str())
                    .or_else(|| s.get("region").and_then(|v| v.as_str()))
                    .map(String::from);
                sink.push(pref, region, None, HashMap::new());
            }
        }
    }

    // GitHub: top-level service arrays of CIDR strings.
    // Datadog: top-level objects { "prefixes_ipv4": [...], "prefixes_ipv6": [...] }.
    // Seqera: { "egress": [...], "ingress": [...] }.
    // Okta: { "<cell>": { "ip_ranges": [...] } }.
    // Grafana synthetic / generic: { "<name>": [ip, ...] }.
    if let Some(obj) = val.as_object() {
        for (key, sub) in obj {
            match sub {
                // Direct array of address strings -> service/region = key.
                Value::Array(arr) if arr.iter().all(|x| x.is_string()) => {
                    for p in arr.iter().filter_map(|x| x.as_str()) {
                        if normalize_ip_prefix(p).is_some() {
                            sink.simple(p, Some(key.clone()));
                        }
                    }
                }
                // Nested object carrying its own address lists.
                Value::Object(inner) => {
                    let mut matched = false;
                    for (subkey, list) in inner {
                        // Datadog: prefixes_ipv4 / prefixes_ipv6 ; Okta: ip_ranges.
                        let is_addr_list = subkey.contains("prefix")
                            || subkey.contains("ip_range")
                            || subkey.contains("addresses");
                        if is_addr_list {
                            if let Some(arr) = list.as_array() {
                                for p in arr.iter().filter_map(|x| x.as_str()) {
                                    sink.push(p, None, Some(key.clone()), HashMap::new());
                                }
                                matched = true;
                            }
                        }
                    }
                    // Deeply nested shapes (e.g. Zscaler cenr:
                    // continent -> city -> [{ "range": ... }]).
                    if !matched {
                        collect_ranges_deep(sub, key, sink);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Recursively pull `range`/`cidr`/`ip_prefix` style fields out of arbitrarily
/// nested structures (used for Zscaler `cenr` continent/city trees).
fn collect_ranges_deep(val: &Value, region: &str, sink: &mut Sink) {
    match val {
        Value::Array(arr) => {
            for v in arr {
                collect_ranges_deep(v, region, sink);
            }
        }
        Value::Object(map) => {
            for k in ["range", "cidr", "ip_prefix", "ipv4_prefix", "ipv6_prefix"] {
                if let Some(p) = map.get(k).and_then(|v| v.as_str()) {
                    sink.push(p, Some(region.to_string()), None, HashMap::new());
                }
            }
            for v in map.values() {
                if v.is_array() || v.is_object() {
                    collect_ranges_deep(v, region, sink);
                }
            }
        }
        _ => {}
    }
}

/// Last-resort: walk any JSON and record every string that is a valid IP/CIDR,
/// so no provider file is ever silently dropped.
fn collect_any_cidr(val: &Value, sink: &mut Sink) {
    match val {
        Value::String(s) => {
            if normalize_ip_prefix(s).is_some() {
                sink.simple(s, None);
            }
        }
        Value::Array(arr) => arr.iter().for_each(|v| collect_any_cidr(v, sink)),
        Value::Object(map) => map.values().for_each(|v| collect_any_cidr(v, sink)),
        _ => {}
    }
}

fn gov_asn_category(filename: &str) -> &str {
    match filename {
        "fed-gov-prefixes.csv" => "US Federal Government",
        "state-gov-prefixes.csv" => "US State Government",
        "city-gov-prefixes.csv" => "US City Government",
        "health-it-prefixes.csv" => "US Health IT Vendors",
        "hospitals-prefixes.csv" => "US Hospital Systems",
        "insurance-prefixes.csv" => "US Health Insurance",
        "pbm-prefixes.csv" => "US Pharmacy Benefit Managers",
        "academic-prefixes.csv" => "US Academic Institutions",
        _ => filename,
    }
}

fn ingest_gov_asn_dir(
    asn_dir: &str,
    stored_entries: &mut Vec<StoredEntry>,
    stored_files: &mut Vec<StoredFile>,
) -> Result<()> {
    let path = std::path::Path::new(asn_dir);
    if !path.exists() {
        anyhow::bail!("gov-asn directory does not exist: {}", asn_dir);
    }

    for entry in std::fs::read_dir(path).context("reading gov-asn directory")? {
        let entry = entry?;
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }
        let name = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.ends_with(".csv") => n.to_string(),
            _ => continue,
        };

        let category = gov_asn_category(&name).to_string();
        log::info!("Processing gov-asn file: {} ({})", name, category);

        let text = std::fs::read_to_string(&file_path)
            .with_context(|| format!("reading file {}", name))?;

        let mut prefixes: Vec<String> = Vec::new();
        let mut count = 0usize;

        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            // Schema: abbreviation,agency,asn,prefix,ip_version,...
            let cols: Vec<&str> = l.splitn(6, ',').collect();
            if cols.len() < 4 {
                continue;
            }
            let abbreviation = cols[0].trim();
            let agency = cols[1].trim();
            let asn = cols[2].trim();
            let prefix_raw = cols[3].trim();

            if let Some(prefix) = normalize_ip_prefix(prefix_raw) {
                let mut metadata = HashMap::new();
                if !abbreviation.is_empty() {
                    metadata.insert("abbreviation".to_string(), abbreviation.to_string());
                }
                if !asn.is_empty() {
                    metadata.insert("asn".to_string(), asn.to_string());
                }
                stored_entries.push(StoredEntry {
                    prefix: prefix.clone(),
                    provider: category.clone(),
                    region: None,
                    service: if agency.is_empty() {
                        None
                    } else {
                        Some(agency.to_string())
                    },
                    metadata: Metadata { fields: metadata },
                });
                prefixes.push(prefix);
                count += 1;
            }
        }

        if count == 0 {
            log::warn!("No prefixes parsed from gov-asn file {}", name);
        } else {
            println!("  {} prefixes from {} ({})", count, name, category);
        }

        stored_files.push(StoredFile {
            name: name.clone(),
            content: text,
            prefixes,
        });
    }

    Ok(())
}

pub fn run_update_from_local(data_dir: &str, gov_asn_path: Option<&str>) -> Result<DbFile> {
    let path = std::path::Path::new(data_dir);
    if !path.exists() {
        anyhow::bail!("Directory does not exist: {}", data_dir);
    }

    let mut stored_entries: Vec<StoredEntry> = Vec::new();
    let mut stored_files: Vec<StoredFile> = Vec::new();

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
        // IANA registry CSVs map /8s to RIRs, not to providers — skip them so
        // results stay focused on actual cloud providers and services.
        if !name.ends_with(".json") && !name.ends_with(".txt") && !name.ends_with(".csv") {
            continue;
        }
        if name == "iana-ipv4.csv" || name == "iana-ipv6.csv" {
            continue;
        }

        log::info!("Processing local file: {}", name);
        let text = std::fs::read_to_string(&file_path)
            .with_context(|| format!("reading file {}", name))?;

        let provider = name
            .trim_end_matches(".json")
            .trim_end_matches(".txt")
            .trim_end_matches(".csv")
            .to_string();

        let mut sink = Sink {
            provider: &provider,
            entries: &mut stored_entries,
            prefixes: Vec::new(),
            count: 0,
        };

        if name.ends_with(".json") {
            if let Ok(val) = serde_json::from_str::<Value>(&text) {
                parse_json(&val, &mut sink);
                // Nothing matched a known shape -> sweep for any IP/CIDR strings.
                if sink.count == 0 {
                    collect_any_cidr(&val, &mut sink);
                }
            }
        } else {
            // Plain text / CSV: one prefix per line, first comma-field for CSV.
            // Header rows (non-IP first field) are silently ignored by normalize_ip_prefix.
            for line in text.lines() {
                let l = line.trim();
                if l.is_empty() || l.starts_with('#') {
                    continue;
                }
                let candidate = l.split(',').next().unwrap_or(l).trim();
                sink.simple(candidate, None);
            }
        }

        let prefixes = std::mem::take(&mut sink.prefixes);
        if sink.count == 0 {
            log::warn!("No prefixes parsed from {}", name);
        }
        stored_files.push(StoredFile {
            name: name.clone(),
            content: text,
            prefixes,
        });
    }

    if let Some(asn_dir) = gov_asn_path {
        println!("Ingesting gov-asn prefixes from {}...", asn_dir);
        ingest_gov_asn_dir(asn_dir, &mut stored_entries, &mut stored_files)?;
    }

    // Dedupe by (prefix, provider) so cross-provider overlaps are preserved,
    // keeping the first (most complete) entry per provider/prefix.
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut deduped: Vec<StoredEntry> = Vec::with_capacity(stored_entries.len());
    for e in stored_entries {
        if seen.insert((e.prefix.clone(), e.provider.clone())) {
            deduped.push(e);
        }
    }

    println!(
        "Processed {} files, {} unique (prefix, provider) entries",
        stored_files.len(),
        deduped.len()
    );

    Ok(DbFile {
        entries: deduped,
        source_commit: None,
        files: stored_files,
    })
}
