use axum::{body::Body, http::{Request, StatusCode}, Router, routing::get, Extension};
use std::sync::Arc;
use tower::ServiceExt;
use serde_json::Value;

// Helper to create a test app with mock state
fn create_test_app() -> Router {
    use ip_ranger::db::{DbFile, StoredEntry, Metadata};
    use ip_ranger::server::{build_state_from_db, check_ip};

    // Create comprehensive test database with real-world IP ranges
    let test_entries = vec![
        // AWS IPv4
        StoredEntry {
            prefix: "13.32.0.0/15".to_string(),
            provider: "aws".to_string(),
            region: Some("GLOBAL".to_string()),
            service: Some("AMAZON".to_string()),
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // AWS IPv6
        StoredEntry {
            prefix: "2600:9000:5206::/48".to_string(),
            provider: "aws".to_string(),
            region: Some("GLOBAL".to_string()),
            service: Some("AMAZON".to_string()),
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // Azure China
        StoredEntry {
            prefix: "40.72.0.0/16".to_string(),
            provider: "azure-china".to_string(),
            region: Some("".to_string()),
            service: Some("ActionGroup".to_string()),
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // GitHub
        StoredEntry {
            prefix: "192.30.252.0/22".to_string(),
            provider: "github".to_string(),
            region: None,
            service: Some("hooks".to_string()),
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        StoredEntry {
            prefix: "140.82.112.0/20".to_string(),
            provider: "github".to_string(),
            region: None,
            service: Some("git".to_string()),
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // Cloudflare IPv4
        StoredEntry {
            prefix: "104.16.0.0/13".to_string(),
            provider: "cloudflare".to_string(),
            region: None,
            service: None,
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // Cloudflare IPv6
        StoredEntry {
            prefix: "2606:4700::/32".to_string(),
            provider: "cloudflare".to_string(),
            region: None,
            service: None,
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // Grafana single IP normalized to /32
        StoredEntry {
            prefix: "34.140.222.135/32".to_string(),
            provider: "grafana-hosted-alerts".to_string(),
            region: None,
            service: None,
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
        // Google Cloud
        StoredEntry {
            prefix: "8.8.8.0/24".to_string(),
            provider: "google".to_string(),
            region: Some("us-central1".to_string()),
            service: Some("DNS".to_string()),
            metadata: Metadata { fields: std::collections::HashMap::new() },
        },
    ];

    let db = DbFile {
        entries: test_entries,
        source_commit: None,
        files: vec![],
    };

    let state = build_state_from_db(&db);
    let state = Arc::new(state);

    Router::new()
        .route("/{ip}", get(check_ip))
        .layer(Extension(state))
}

#[tokio::test]
async fn test_check_ip_private_ipv4() {
    let app = create_test_app();

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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "private");
}

#[tokio::test]
async fn test_check_ip_known_provider() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/13.32.1.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "aws");
    assert_eq!(json["data"]["region"], "GLOBAL");
    assert_eq!(json["data"]["service"], "AMAZON");
}

#[tokio::test]
async fn test_check_ip_unknown() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/1.1.1.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "unknown");
}

#[tokio::test]
async fn test_check_ip_invalid() {
    let app = create_test_app();

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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "Invalid IP address");
}

#[tokio::test]
async fn test_check_ip_private_ipv6() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/fc00::1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "private");
}

// ===== Cloud Provider Tests =====

#[tokio::test]
async fn test_check_ip_azure_china() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/40.72.175.176")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "azure-china");
    assert_eq!(json["data"]["service"], "ActionGroup");
}

#[tokio::test]
async fn test_check_ip_github_hooks() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/192.30.252.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "github");
    assert_eq!(json["data"]["service"], "hooks");
}

#[tokio::test]
async fn test_check_ip_github_git() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/140.82.112.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "github");
    assert_eq!(json["data"]["service"], "git");
}

#[tokio::test]
async fn test_check_ip_cloudflare() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/104.17.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "cloudflare");
}

#[tokio::test]
async fn test_check_ip_grafana_single_ip() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/34.140.222.135")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    // Provider name includes the full file name after canonicalization
    assert_eq!(json["data"]["provider"], "grafana-hosted-alerts");
}

#[tokio::test]
async fn test_check_ip_google_dns() {
    let app = create_test_app();

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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "google");
    assert_eq!(json["data"]["region"], "us-central1");
    assert_eq!(json["data"]["service"], "DNS");
}

// ===== IPv6 Tests =====

#[tokio::test]
async fn test_check_ip_aws_ipv6() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/2600:9000:5206::1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "aws");
    assert_eq!(json["data"]["region"], "GLOBAL");
    assert_eq!(json["data"]["service"], "AMAZON");
}

#[tokio::test]
async fn test_check_ip_cloudflare_ipv6() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/2606:4700::1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "cloudflare");
}

// ===== Edge Case Tests =====

#[tokio::test]
async fn test_check_ip_loopback_ipv4() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/127.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    // Loopback is not detected as private by is_private_ip() - it returns unknown
    assert_eq!(json["data"]["provider"], "unknown");
}

#[tokio::test]
async fn test_check_ip_loopback_ipv6() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/::1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    // IPv6 loopback should return unknown (not currently detected as private)
    assert!(json["data"]["provider"] == "unknown" || json["data"]["provider"] == "private");
}

#[tokio::test]
async fn test_check_ip_link_local_ipv4() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/169.254.1.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    // Link-local is not detected as private by is_private_ip() - it returns unknown
    assert_eq!(json["data"]["provider"], "unknown");
}

#[tokio::test]
async fn test_check_ip_private_10_network() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "private");
}

#[tokio::test]
async fn test_check_ip_private_172_network() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/172.16.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "private");
}

#[tokio::test]
async fn test_check_ip_invalid_port() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/1.2.3.4:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "Invalid IP address");
}

#[tokio::test]
async fn test_check_ip_malformed() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/999.999.999.999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "Invalid IP address");
}
