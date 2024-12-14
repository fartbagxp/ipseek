mod aws;

use axum::{routing::get, Router, extract::Path, response::Json};
use log::debug;
use std::net::{IpAddr, Ipv4Addr};
use iptrie::Ipv4Prefix;
use iptrie::Ipv4RTrieSet;

use crate::aws::AwsCidrRanges;

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

// Function to fetch AWS CIDR ranges
async fn fetch_aws_cidr_ranges() -> Result<AwsCidrRanges, Box<dyn std::error::Error>> {
    let url = "https://raw.githubusercontent.com/fartbagxp/aas-cidr-ranges/master/data/raw/aws.json";
    let response = reqwest::get(url).await?.text().await?;
    let aws_cidr_ranges: AwsCidrRanges = serde_json::from_str(&response)?;
    Ok(aws_cidr_ranges)
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
async fn main() {
    env_logger::init();

    let prefixes = [
        "1.1.0.0/24",
        "1.1.1.0/24",
        "1.1.0.0/20",
        "1.1.2.0/24"
    ];

    let iter = prefixes.iter().map(|x| x.parse::<Ipv4Prefix>().unwrap());

    // a set based on Patricia trie
    let trie = Ipv4RTrieSet::from_iter(iter);

    // lpm lookup for Ipv4 address
    assert_eq!(trie.lookup(&"1.1.1.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.1.0/24");
    assert_eq!(trie.lookup(&"1.1.2.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.2.0/24");

    // lpm lookup for Ipv4 prefix also works
    assert_eq!(trie.lookup(&"1.1.0.0/25".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/24");
    assert_eq!(trie.lookup(&"1.1.0.0/21".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/20");

    // now, compute the set based on LC-trie
    let lctrie = trie.compress();

    // lpm lookup for Ipv4 address
    assert_eq!(lctrie.lookup(&"1.1.1.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.1.0/24");
    assert_eq!(lctrie.lookup(&"1.1.2.2".parse::<Ipv4Addr>().unwrap()).to_string(), "1.1.2.0/24");

    // lpm lookup for Ipv4 prefix also works
    assert_eq!(lctrie.lookup(&"1.1.0.0/25".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/24");
    assert_eq!(lctrie.lookup(&"1.1.0.0/21".parse::<Ipv4Prefix>().unwrap()).to_string(), "1.1.0.0/20");

    // Start the server
    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}