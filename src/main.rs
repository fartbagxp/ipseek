mod aws;

use aws::{fetch_aws_cidr_ranges, find_matching_cidr};
use axum::{routing::get, Router, extract::Path, response::Json, Extension};
use log::debug;
use serde_json::json;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::RwLock;
use iptrie::Ipv4Prefix;
use iptrie::Ipv4RTrieSet;

struct AppState {
    trie: Arc<RwLock<Ipv4RTrieSet>>,
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

pub async fn check_ip(
    Path(ip_str): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Json<serde_json::Value> {
    debug!("Requested IP: {}", ip_str);
    let response = if let Ok(ip) = ip_str.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            json!({
                "error": "",
                "data": "Private IP"
            })
        } else {
            // Check AWS CIDR for public IP
            let ip_v4 = match ip {
                IpAddr::V4(ip_v4) => ip_v4,
                IpAddr::V6(_) => return Json(json!({
                    "error": "IPv6 not supported",
                    "data": ""
                })),
            };

            let trie = state.trie.read().await;
            let aws_cidr_ranges = fetch_aws_cidr_ranges().await.unwrap(); // Fetch the ranges here or pass them in the state

            if let Some(matching_cidr) = find_matching_cidr(&trie, ip_v4, &aws_cidr_ranges) {
                json!({
                    "error": "",
                    "data": {
                        "ip": ip_str,
                        "ip_prefix": matching_cidr.ip_prefix,
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
    } else {
        json!({
            "error": "Invalid IP address",
            "data": ""
        })
    };

    Json(response)
}

async fn build_trie() -> Result<Ipv4RTrieSet, Error> {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    // println!("{:#?}", aws_cidr_ranges);

    let ipv4_prefixes: Vec<Ipv4Prefix> = aws_cidr_ranges.prefixes
        .iter()
        .filter_map(|block| block.ip_prefix.parse::<Ipv4Prefix>().ok())
        .collect();

    // println!("IPv4 Prefixes: {:#?}", ipv4_prefixes);

    let iter = ipv4_prefixes.iter();
    let trie = Ipv4RTrieSet::from_iter(iter.cloned());

    Ok(trie)
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

    let trie = build_trie().await?;
    let state = Arc::new(AppState {
        trie: Arc::new(RwLock::new(trie)),
    });

    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip))
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}