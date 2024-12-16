use crate::aws::{fetch_aws_cidr_ranges, find_matching_cidr};
use iptrie::{Ipv4Prefix, Ipv4RTrieSet};
use reqwest::Error;
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub tries: Arc<RwLock<HashMap<String, Ipv4RTrieSet>>>,
}

pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            // Check if the first 7 bits of the address match `fc00::/7`
            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

pub fn parse_ip(ip_str: &str) -> Result<IpAddr, String> {
    ip_str
        .parse::<IpAddr>()
        .map_err(|_| "Invalid IP address".to_string())
}

pub fn check_ip_type(ip: &IpAddr) -> &'static str {
    if is_private_ip(ip) {
        "Private IP"
    } else {
        "Public IP"
    }
}

pub async fn lookup_aws_cidr(ip: IpAddr, trie: &Ipv4RTrieSet) -> serde_json::Value {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await.unwrap();

    match ip {
        IpAddr::V4(ip_v4) => {
            if let Some(matching_cidr) = find_matching_cidr(trie, ip_v4, &aws_cidr_ranges) {
                json!({
                    "error": "",
                    "data": {
                        "ip": ip.to_string(),
                        "region": matching_cidr.region,
                        "service": matching_cidr.service,
                        "network_border_group": matching_cidr.network_border_group,
                    }
                })
            } else {
                json!({
                    "error": "",
                    "data": "Public IP not in AWS CIDR"
                })
            }
        }
        IpAddr::V6(_) => json!({
            "error": "IPv6 not supported",
            "data": ""
        }),
    }
}

pub async fn find_matching_cidr_for_ip(trie: &Ipv4RTrieSet, ip: Ipv4Addr) -> Result<(), Error> {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    if let Some(matching_cidr) = find_matching_cidr(trie, ip, &aws_cidr_ranges) {
        println!("Matching CIDR: {:#?}", matching_cidr);
    } else {
        println!("No matching CIDR found for IP: {}", ip);
    }
    Ok(())
}

pub async fn build_tries() -> Result<HashMap<String, Ipv4RTrieSet>, Error> {
    let mut tries = HashMap::new();

    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    let aws_ipv4_prefixes: Vec<Ipv4Prefix> = aws_cidr_ranges
        .prefixes
        .iter()
        .filter_map(|block| block.ip_prefix.parse::<Ipv4Prefix>().ok())
        .collect();
    let aws_trie = Ipv4RTrieSet::from_iter(aws_ipv4_prefixes.iter().cloned());
    tries.insert("aws".to_string(), aws_trie);

    Ok(tries)
}
