use axum::{extract::Path, extract::Extension, response::Json};
use std::sync::Arc;
use crate::db::{DbFile, StoredEntry};
use ipnet::IpNet;

#[derive(Clone)]
pub struct AppState {
    pub v4_list: Vec<(ipnet::Ipv4Net, StoredEntry)>,
    pub v6_list: Vec<(IpNet, StoredEntry)>,
}

fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => ipv4.is_private(),
        std::net::IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

fn canonical_provider(name: &str) -> String {
    let mut s = name.to_lowercase();
    if s.ends_with(".json") { s = s.trim_end_matches(".json").to_string(); }
    if s.ends_with(".txt") { s = s.trim_end_matches(".txt").to_string(); }
    let mut tokens: Vec<&str> = s.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).collect();
    let stopwords = ["ips","ipv4","ipv6","v4","v6","ip","iprange","ip-range","range","ranges","public","hosted","cdn","api","webhook","prefixes","prefix"];
    while tokens.len() > 1 { let last = *tokens.last().unwrap(); if stopwords.contains(&last) || last.starts_with("ipv") || last.starts_with("ips") { tokens.pop(); } else { break; } }
    tokens.join("-")
}

pub async fn check_ip(Extension(state): Extension<Arc<AppState>>, Path(ip_str): Path<String>) -> Json<serde_json::Value> {
    let ip: std::net::IpAddr = match ip_str.parse() {
        Ok(i) => i,
        Err(_) => return Json(serde_json::json!({"error":"Invalid IP address","data":null})),
    };

    if is_private_ip(&ip) {
        return Json(serde_json::json!({
            "error":"",
            "data":{
                "provider":"private",
                "prefix":null,
                "region":null,
                "service":null,
                "metadata":{}
            }
        }));
    }

    let result = match ip {
        std::net::IpAddr::V4(a) => {
            let mut best_len: i32 = -1;
            let mut best_entry: Option<&StoredEntry> = None;
            for (net, entry) in &state.v4_list {
                if net.contains(&a) {
                    let len = net.prefix_len() as i32;
                    if len > best_len {
                        best_len = len;
                        best_entry = Some(entry);
                    }
                }
            }
            best_entry
        }
        std::net::IpAddr::V6(_) => {
            for (net, entry) in &state.v6_list {
                if net.contains(&ip) {
                    return Json(serde_json::json!({
                        "error":"",
                        "data":{
                            "provider": canonical_provider(&entry.provider),
                            "prefix": entry.prefix,
                            "region": entry.region,
                            "service": entry.service,
                            "metadata": entry.metadata
                        }
                    }));
                }
            }
            None
        }
    };

    if let Some(entry) = result {
        Json(serde_json::json!({
            "error":"",
            "data":{
                "provider": canonical_provider(&entry.provider),
                "prefix": entry.prefix,
                "region": entry.region,
                "service": entry.service,
                "metadata": entry.metadata.fields
            }
        }))
    } else {
        Json(serde_json::json!({
            "error":"",
            "data":{
                "provider":"unknown",
                "prefix":null,
                "region":null,
                "service":null,
                "metadata":{}
            }
        }))
    }
}

pub fn build_state_from_db(db: &DbFile) -> AppState {
    let mut v4_list: Vec<(ipnet::Ipv4Net, StoredEntry)> = Vec::new();
    let mut v6_list: Vec<(IpNet, StoredEntry)> = Vec::new();
    for e in db.entries.iter() {
        if let Ok(net) = e.prefix.parse::<IpNet>() {
            match net {
                IpNet::V4(v4) => v4_list.push((v4, e.clone())),
                IpNet::V6(v6) => v6_list.push((IpNet::V6(v6), e.clone())),
            }
        }
    }

    AppState { v4_list, v6_list }
}
