use axum::body::Body;
use axum::Extension;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use crate::to_bytes;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt; // for `oneshot` method
use iptrie::Ipv4RTrieSet;
use iptrie::Ipv4Prefix;
use std::collections::HashMap;
use reqwest::Error;

mod aws;
use aws::{fetch_aws_cidr_ranges, find_matching_cidr, V4CidrBlock};

struct AppState {
    tries: Arc<RwLock<HashMap<String, Ipv4RTrieSet>>>,
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

async fn setup_app() -> Router {
    let tries = build_tries().await.unwrap();
    let state = Arc::new(AppState {
        tries: Arc::new(RwLock::new(tries)),
    });

    Router::new()
        .route("/:ip", get(check_ip))
        .layer(Extension(state))
}

#[tokio::test]
async fn test_private_ip() {
    let app = setup_app().await;

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

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json,
        json!({
            "error": "",
            "data": "Private IP"
        })
    );
}

#[tokio::test]
async fn test_public_ip() {
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/51.0.28.5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json.get("error").unwrap().is_string());
    assert!(json.get("data").unwrap().is_object());
}