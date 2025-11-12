use anyhow::Result;
use std::net::IpAddr;
use crate::db::{DbFile, StoredEntry};
use ipnet::IpNet;

fn canonical_provider(name: &str) -> String {
    let mut s = name.to_lowercase();
    if s.ends_with(".json") { s = s.trim_end_matches(".json").to_string(); }
    if s.ends_with(".txt") { s = s.trim_end_matches(".txt").to_string(); }
    let mut tokens: Vec<&str> = s.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).collect();
    let stopwords = ["ips","ipv4","ipv6","v4","v6","ip","iprange","ip-range","range","ranges","public","hosted","cdn","api","webhook","prefixes","prefix"];
    while tokens.len() > 1 {
        let last = *tokens.last().unwrap();
        if stopwords.contains(&last) || last.starts_with("ipv") || last.starts_with("ips") { tokens.pop(); } else { break; }
    }
    tokens.join("-")
}

pub fn lookup_provider(db: DbFile, ip_str: &str) -> Result<String> {
    let ip: IpAddr = ip_str.parse()?;

    let mut v4_list: Vec<(ipnet::Ipv4Net, StoredEntry)> = Vec::new();
    let mut v6_list: Vec<(IpNet, StoredEntry)> = Vec::new();

    for e in db.entries.into_iter() {
        if let Ok(net) = e.prefix.parse::<IpNet>() {
            match net {
                IpNet::V4(v4) => v4_list.push((v4, e.clone())),
                IpNet::V6(v6) => v6_list.push((IpNet::V6(v6), e.clone())),
            }
        }
    }

    if let IpAddr::V4(a) = ip {
        // LPM
        let mut best_len: i32 = -1;
        let mut best_entry: Option<&StoredEntry> = None;
        for (net, entry) in &v4_list {
            if net.contains(&a) {
                let len = net.prefix_len() as i32;
                if len > best_len {
                    best_len = len;
                    best_entry = Some(entry);
                }
            }
        }
        if let Some(entry) = best_entry {
            let provider = canonical_provider(&entry.provider);
            let mut parts = vec![provider];
            if let Some(ref region) = entry.region {
                if !region.is_empty() {
                    parts.push(format!("region={}", region));
                }
            }
            if let Some(ref service) = entry.service {
                if !service.is_empty() {
                    parts.push(format!("service={}", service));
                }
            }
            return Ok(parts.join(" "));
        }

        return Ok("unknown".to_string());
    }

    for (net, entry) in v6_list {
        if net.contains(&ip) {
            let provider = canonical_provider(&entry.provider);
            let mut parts = vec![provider];
            if let Some(ref region) = entry.region {
                if !region.is_empty() {
                    parts.push(format!("region={}", region));
                }
            }
            if let Some(ref service) = entry.service {
                if !service.is_empty() {
                    parts.push(format!("service={}", service));
                }
            }
            return Ok(parts.join(" "));
        }
    }
    Ok("unknown".to_string())
}
