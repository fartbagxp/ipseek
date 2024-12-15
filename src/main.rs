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

// async fn check_ip(
//     Path(ip_str): Path<String>,
//     state: Arc<AppState>,
// ) -> Json<serde_json::Value> {
//     debug!("Requested IP: {}", ip_str);

//     let response = if let Ok(ip) = ip_str.parse::<IpAddr>() {
//         let trie = state.ip_trie.read().await;
//         if let Some(region) = trie.longest_match(ip) {
//             serde_json::json!({
//                 "error": "",
//                 "data": format!("IP belongs to AWS range in region: {}", region)
//             })
//         } else {
//             serde_json::json!({
//                 "error": "",
//                 "data": "IP does not belong to any AWS range"
//             })
//         }
//     } else {
//         serde_json::json!({
//             "error": "Invalid IP address",
//             "data": ""
//         })
//     };

//     Json(response)
// }
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

// async fn populate_ip_trie() -> Result<IpTrie<IpAddr, String>, Box<dyn std::error::Error>> {
//     let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
//     let mut trie = IpTrie::new();

//     for cidr in aws_cidr_ranges.prefixes {
//         // Parse the IP prefix and insert it into the trie
//         if let Ok(ip_network) = cidr.ip_prefix.parse::<ipnet::IpNet>() {
//             trie.insert(ip_network.addr(), cidr.region.clone());
//         }
//     }

//     Ok(trie)
// }

// async fn check_ip_in_aws_ranges(
//     ip: &IpAddr,
//     trie: &IpTrie<IpAddr, String>,
// ) -> Option<String> {
//     trie.longest_match(*ip).map(|(_, region)| region.clone())
// }

// pub struct AppState {
//     ip_trie: Arc<RwLock<IpTrie<IpAddr, String>>>,
// }

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let aws_cidr_ranges = fetch_aws_cidr_ranges().await?;
    println!("{:#?}", aws_cidr_ranges);

    let ipv4_prefixes: Vec<Ipv4Prefix> = aws_cidr_ranges.prefixes
        .iter()
        .filter_map(|block| block.ip_prefix.parse::<Ipv4Prefix>().ok())
        .collect();

    println!("IPv4 Prefixes: {:#?}", ipv4_prefixes);

    let iter = ipv4_prefixes.iter();
    let trie = Ipv4RTrieSet::from_iter(iter.cloned());

    let ip = "51.0.28.5".parse::<Ipv4Addr>().unwrap();
    if let Some(matching_cidr) = find_matching_cidr(&trie, ip, &aws_cidr_ranges) {
        println!("Matching CIDR: {:#?}", matching_cidr);
    } else {
        println!("No matching CIDR found for IP: {}", ip);
    }

    // Start the server
    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}