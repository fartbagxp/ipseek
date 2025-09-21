use axum::{body::Body, http::{Request, StatusCode}, Router, routing::get};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tower::ServiceExt; // for `oneshot` method
use axum::body::to_bytes;
use ip_checker::check_ip;

#[tokio::test]
async fn test_check_ip_private() {
    let app = Router::new().route("/:ip", get(check_ip));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/192.168.1.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"], "Private IP");
}

#[tokio::test]
async fn test_check_ip_public() {
    let app = Router::new().route("/:ip", get(check_ip));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/8.8.8.8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"], "Public IP");
}

#[tokio::test]
async fn test_check_ip_invalid() {
    let app = Router::new().route("/:ip", get(check_ip));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/invalid_ip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "Invalid IP address");
    assert_eq!(json["data"], "");
}



fn main() {
    let ipv4_private = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    let ipv4_public = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
    let ipv6_private = IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1));
    let ipv6_public = IpAddr::V6(Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 1));

    assert!(is_private_ip(&ipv4_private));
    assert!(!is_private_ip(&ipv4_public));
    assert!(is_private_ip(&ipv6_private));
    assert!(!is_private_ip(&ipv6_public));

    println!("All tests passed!");
}