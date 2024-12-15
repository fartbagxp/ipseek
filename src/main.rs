mod aws;

use aws::{fetch_aws_cidr_ranges, find_matching_cidr};
use axum::{routing::get, Router, extract::Path, response::Json};
use log::debug;
use reqwest::Error;
use std::net::{IpAddr, Ipv4Addr};
use iptrie::Ipv4Prefix;
use iptrie::Ipv4RTrieSet;


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

pub async fn check_ip(Path(ip_str): Path<String>) -> Json<serde_json::Value> {
    debug!("Requested IP: {}", ip_str);
    let response = if let Ok(ip) = ip_str.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            serde_json::json!({
                "error": "",
                "data": "Private IP"
            })
        } else {
            serde_json::json!({
                "error": "",
                "data": "Public IP"
            })
        }
    } else {
        serde_json::json!({
            "error": "Invalid IP address",
            "data": ""
        })
    };

    Json(response)
}

async fn build_trie() -> Result<Ipv4RTrieSet, Error> {
    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    println!("{:#?}", aws_cidr_ranges);

    let ipv4_prefixes: Vec<Ipv4Prefix> = aws_cidr_ranges.prefixes
        .iter()
        .filter_map(|block| block.ip_prefix.parse::<Ipv4Prefix>().ok())
        .collect();

    println!("IPv4 Prefixes: {:#?}", ipv4_prefixes);

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
    let ip = "51.0.28.5".parse::<Ipv4Addr>().unwrap();
    find_matching_cidr_for_ip(&trie, ip).await?;

    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}