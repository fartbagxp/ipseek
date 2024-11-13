use axum::{routing::get, Router, extract::Path, response::Json};
use std::net::IpAddr;
use log::debug;

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private()
        }
        IpAddr::V6(ipv6) => {
            ipv6.segments()[0] & 0xfe00 == 0xfc00
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

#[tokio::main]
async fn main() {
    env_logger::init();

    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}