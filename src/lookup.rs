use crate::db::{DbFile, StoredEntry};
use crate::resolve::resolve;
use anyhow::Result;
use ipnet::IpNet;
use std::net::IpAddr;

/// Collect every range in the DB that contains `ip`, with its prefix length.
fn collect_matches(db: &DbFile, ip: IpAddr) -> Vec<(u8, StoredEntry)> {
    let mut out = Vec::new();
    for e in &db.entries {
        let net: IpNet = match e.prefix.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let contained = match (ip, net) {
            (IpAddr::V4(a), IpNet::V4(n)) => n.contains(&a),
            (IpAddr::V6(a), IpNet::V6(n)) => n.contains(&a),
            _ => false,
        };
        if contained {
            out.push((net.prefix_len(), e.clone()));
        }
    }
    out
}

/// Resolve an IP string to a one-line, layered, human-readable description,
/// e.g. `stripe (service) on aws (cloud · us-east-1)`.
pub fn lookup_provider(db: DbFile, ip_str: &str) -> Result<String> {
    let ip: IpAddr = ip_str.parse()?;
    match resolve(collect_matches(&db, ip)) {
        Some(res) => Ok(res.summary),
        None => Ok("unknown".to_string()),
    }
}
