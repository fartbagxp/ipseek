use crate::db::{DbFile, StoredEntry};
use crate::resolve::{resolve, Resolution};
use axum::{extract::Extension, extract::Path, response::Json};
use ipnet::IpNet;
use std::net::IpAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub v4_list: Vec<(ipnet::Ipv4Net, StoredEntry)>,
    pub v6_list: Vec<(ipnet::Ipv6Net, StoredEntry)>,
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

/// Collect every range that contains `ip`, paired with its prefix length.
fn collect_matches(state: &AppState, ip: IpAddr) -> Vec<(u8, StoredEntry)> {
    let mut out = Vec::new();
    match ip {
        IpAddr::V4(a) => {
            for (net, entry) in &state.v4_list {
                if net.contains(&a) {
                    out.push((net.prefix_len(), entry.clone()));
                }
            }
        }
        IpAddr::V6(a) => {
            for (net, entry) in &state.v6_list {
                if net.contains(&a) {
                    out.push((net.prefix_len(), entry.clone()));
                }
            }
        }
    }
    out
}

fn resolution_json(ip: &str, res: &Resolution) -> serde_json::Value {
    let primary = res.primary();
    serde_json::json!({
        "error": "",
        "data": {
            "ip": ip,
            "provider": primary.provider,
            "prefix": primary.prefix,
            "region": primary.region,
            "service": primary.service,
            "metadata": primary.metadata,
            "kind": primary.kind,
            "summary": res.summary,
            "layers": res.layers,
        }
    })
}

pub async fn check_ip(
    Extension(state): Extension<Arc<AppState>>,
    Path(ip_str): Path<String>,
) -> Json<serde_json::Value> {
    let ip: IpAddr = match ip_str.parse() {
        Ok(i) => i,
        Err(_) => return Json(serde_json::json!({"error":"Invalid IP address","data":null})),
    };

    if is_private_ip(&ip) {
        return Json(serde_json::json!({
            "error":"",
            "data":{
                "ip": ip_str,
                "provider":"private",
                "prefix":null,
                "region":null,
                "service":null,
                "metadata":{},
                "kind": null,
                "summary":"private",
                "layers": []
            }
        }));
    }

    match resolve(collect_matches(&state, ip)) {
        Some(res) => Json(resolution_json(&ip_str, &res)),
        None => Json(serde_json::json!({
            "error":"",
            "data":{
                "ip": ip_str,
                "provider":"unknown",
                "prefix":null,
                "region":null,
                "service":null,
                "metadata":{},
                "kind": null,
                "summary":"unknown",
                "layers": []
            }
        })),
    }
}

pub fn build_state_from_db(db: &DbFile) -> AppState {
    let mut v4_list: Vec<(ipnet::Ipv4Net, StoredEntry)> = Vec::new();
    let mut v6_list: Vec<(ipnet::Ipv6Net, StoredEntry)> = Vec::new();
    for e in db.entries.iter() {
        if let Ok(net) = e.prefix.parse::<IpNet>() {
            match net {
                IpNet::V4(v4) => v4_list.push((v4, e.clone())),
                IpNet::V6(v6) => v6_list.push((v6, e.clone())),
            }
        }
    }
    AppState { v4_list, v6_list }
}
