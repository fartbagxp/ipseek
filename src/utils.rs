use crate::aws::{fetch_aws_cidr_ranges, find_matching_cidr_ipv4, find_matching_cidr_ipv6};
use iptrie::{Ipv4Prefix, Ipv4RTrieSet, Ipv6Prefix, Ipv6RTrieSet};
use reqwest::Error;
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub tries: Arc<RwLock<HashMap<String, Ipv4RTrieSet>>>,
    pub tries_v6: Arc<RwLock<HashMap<String, Ipv6RTrieSet>>>,
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

pub async fn lookup_aws_cidr(
    ip: IpAddr,
    trie: &Ipv4RTrieSet,
    trie_v6: &Ipv6RTrieSet,
) -> serde_json::Value {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await.unwrap();

    match ip {
        IpAddr::V4(ip_v4) => {
            if let Some(matching_cidr) = find_matching_cidr_ipv4(trie, ip_v4, &aws_cidr_ranges) {
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
        IpAddr::V6(ip_v6) => json!(if let Some(matching_cidr) =
            find_matching_cidr_ipv6(trie_v6, ip_v6, &aws_cidr_ranges)
        {
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
        }),
    }
}

pub async fn find_matching_cidr_for_ip(trie: &Ipv4RTrieSet, ip: Ipv4Addr) -> Result<(), Error> {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    if let Some(matching_cidr) = find_matching_cidr_ipv4(trie, ip, &aws_cidr_ranges) {
        println!("Matching CIDR: {:#?}", matching_cidr);
    } else {
        println!("No matching CIDR found for IP: {}", ip);
    }
    Ok(())
}

pub async fn build_tries(
) -> Result<(HashMap<String, Ipv4RTrieSet>, HashMap<String, Ipv6RTrieSet>), Error> {
    let mut tries = HashMap::new();
    let mut tries_v6 = HashMap::new();

    // Build AWS IPv4 trie
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    let aws_ipv4_prefixes: Vec<Ipv4Prefix> = aws_cidr_ranges
        .prefixes
        .iter()
        .filter_map(|block| block.ip_prefix.parse::<Ipv4Prefix>().ok())
        .collect();
    let aws_trie = Ipv4RTrieSet::from_iter(aws_ipv4_prefixes.iter().cloned());
    tries.insert("aws".to_string(), aws_trie);

    // Build AWS IPv6 trie
    let aws_ipv6_prefixes: Vec<Ipv6Prefix> = aws_cidr_ranges
        .ipv6_prefixes
        .iter()
        .filter_map(|block| block.ipv6_prefix.parse::<Ipv6Prefix>().ok())
        .collect();
    let aws_trie_v6 = Ipv6RTrieSet::from_iter(aws_ipv6_prefixes.iter().cloned());
    tries_v6.insert("aws".to_string(), aws_trie_v6);

    Ok((tries, tries_v6))
}
