use axum::body::to_bytes;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Extension;
use axum::Router;
use ipseek::handler::check_ip;
use ipseek::utils::{build_tries, AppState};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

async fn setup_app() -> Router {
    let (tries, tries_v6) = build_tries().await.unwrap();
    let state = Arc::new(AppState {
        tries: Arc::new(RwLock::new(tries)),
        tries_v6: Arc::new(RwLock::new(tries_v6)),
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

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
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
async fn test_aws_public_ip() {
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

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json.get("error").unwrap().is_string());
    assert!(json.get("data").unwrap().is_object());
    assert_eq!(
        json,
        json!({
          "data": {
            "create_date": "2024-12-16-19-13-08",
            "ip_address": "51.0.28.5",
            "ip_prefix": "51.0.28.0/24",
            "network_border_group": "eusc-de-east-1",
            "region": "eusc-de-east-1",
            "service": "AMAZON",
            "sync_token": "1734376388"
          },
          "error": ""
        })
    );
}

#[tokio::test]
async fn test_aws_ipv6_public_ip() {
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/2600:f0f0:551f:0000:0000:0000:0000:0001")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json,
        json!({
          "data": {
            "create_date": "2024-12-16-19-13-08",
            "ip_address": "2600:f0f0:551f::1",
            "ip_prefix": "2600:f0f0:551f::/48",
            "network_border_group": "us-east-2",
            "region": "us-east-2",
            "service": "AMAZON",
            "sync_token": "1734376388"
          },
          "error": ""
        })
    );
}
