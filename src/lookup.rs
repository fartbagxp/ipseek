use anyhow::Result;
use std::net::IpAddr;
use crate::db::{DbFile, StoredFile};
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
        // LPM
        let mut best_len: i32 = -1;
        let mut best_provider: Option<String> = None;
        for (net, prov) in &v4_list {
            if net.contains(&a) {
                let len = net.prefix_len() as i32;
                if len > best_len {
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
        if let Some(p) = best_provider { return Ok(canonical_provider(&p)); }

        for f in db.files.into_iter() {
            for pref in &f.prefixes {
                if let Ok(net) = pref.parse::<ipnet::Ipv4Net>() {
                    if net.contains(&a) {
                        return Ok(canonical_provider(&f.name));
                    }
                }
            }
        }

        return Ok("unknown".to_string());
    }

    for (net, prov) in v6_list {
        if net.contains(&ip) { return Ok(canonical_provider(&prov)); }
    }
    Ok("unknown".to_string())
}
