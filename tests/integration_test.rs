use axum::body::Body;
use axum::body::to_bytes;
use axum::Extension;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt; // for `oneshot` method
use ip_ranger::handler::check_ip;
use ip_ranger::utils::{AppState, build_tries};

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

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json.get("error").unwrap().is_string());
    assert!(json.get("data").unwrap().is_object());
}