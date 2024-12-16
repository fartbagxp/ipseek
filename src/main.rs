mod aws;

use aws::{fetch_aws_cidr_ranges, find_matching_cidr, V4CidrBlock};
use axum::{routing::get, Router, extract::Path, response::Json, Extension};
use log::debug;
use ipnet::IpNet;
use reqwest::Error;
use serde_json::json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::RwLock;
use iptrie::Ipv4Prefix;
use iptrie::Ipv4RTrieSet;

struct AppState {
    tries: Arc<RwLock<HashMap<String, Ipv4RTrieSet>>>,
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            // Check if the first 7 bits of the address match `fc00::/7`
            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

fn parse_ip(ip_str: &str) -> Result<IpAddr, String> {
    ip_str.parse::<IpAddr>().map_err(|_| "Invalid IP address".to_string())
}

fn check_ip_type(ip: &IpAddr) -> &'static str {
    if is_private_ip(ip) {
        "Private IP"
    } else {
        "Public IP"
    }
}

async fn lookup_aws_cidr(ip: IpAddr, trie: &Ipv4RTrieSet) -> serde_json::Value {
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

pub async fn check_ip(
    Path(ip_str): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Json<serde_json::Value> {
    debug!("Requested IP: {}", ip_str);

    let response = match parse_ip(&ip_str) {
        Ok(ip) => match check_ip_type(&ip) {
            "Private IP" => json!({
                "error": "",
                "data": "Private IP"
            }),
            "Public IP" => {
                let tries = state.tries.read().await;
                if let Some(trie) = tries.get("aws") {
                    lookup_aws_cidr(ip, trie).await
                } else {
                    json!({
                        "error": "Trie not found",
                        "data": ""
                    })
                }
            },
            _ => json!({
                "error": "Unknown IP type",
                "data": ""
            }),
        },
        Err(err) => json!({
            "error": err,
            "data": ""
        }),
    };

    Json(response)
}

async fn build_tries() -> Result<HashMap<String, Ipv4RTrieSet>, Error> {
    let mut tries = HashMap::new();

    // Build AWS trie
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    let aws_ipv4_prefixes: Vec<Ipv4Prefix> = aws_cidr_ranges.prefixes
        .iter()
        .filter_map(|block| block.ip_prefix.parse::<Ipv4Prefix>().ok())
        .collect();
    let aws_trie = Ipv4RTrieSet::from_iter(aws_ipv4_prefixes.iter().cloned());
    tries.insert("aws".to_string(), aws_trie);

    // Add more tries here if needed
    // For example, you can build another trie for a different set of IP ranges

    Ok(tries)
}

async fn find_matching_cidr_for_ip(trie: &Ipv4RTrieSet, ip: Ipv4Addr) -> Result<(), Error> {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    if let Some(matching_cidr) = find_matching_cidr(trie, ip, &aws_cidr_ranges) {
        println!("Matching CIDR: {:#?}", matching_cidr);
    } else {
        println!("No matching CIDR found for IP: {}", ip);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let tries = build_tries().await?;
    let state = Arc::new(AppState {
        tries: Arc::new(RwLock::new(tries)),
    });

    // Start the server
    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip))
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}