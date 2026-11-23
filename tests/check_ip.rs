use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Extension, Router,
};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

// Helper to create a test app with mock state
fn create_test_app() -> Router {
    use ipseek::db::{DbFile, Metadata, StoredEntry};
    use ipseek::server::{build_state_from_db, check_ip};

    // Create comprehensive test database with real-world IP ranges
    let test_entries = vec![
        // AWS IPv4
        StoredEntry {
            prefix: "13.32.0.0/15".to_string(),
            provider: "aws".to_string(),
            region: Some("GLOBAL".to_string()),
            service: Some("AMAZON".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // AWS IPv6
        StoredEntry {
            prefix: "2600:9000:5206::/48".to_string(),
            provider: "aws".to_string(),
            region: Some("GLOBAL".to_string()),
            service: Some("AMAZON".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // Azure China
        StoredEntry {
            prefix: "40.72.0.0/16".to_string(),
            provider: "azure-china".to_string(),
            region: Some("".to_string()),
            service: Some("ActionGroup".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // GitHub
        StoredEntry {
            prefix: "192.30.252.0/22".to_string(),
            provider: "github".to_string(),
            region: None,
            service: Some("hooks".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        StoredEntry {
            prefix: "140.82.112.0/20".to_string(),
            provider: "github".to_string(),
            region: None,
            service: Some("git".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // Cloudflare IPv4
        StoredEntry {
            prefix: "104.16.0.0/13".to_string(),
            provider: "cloudflare".to_string(),
            region: None,
            service: None,
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // Cloudflare IPv6
        StoredEntry {
            prefix: "2606:4700::/32".to_string(),
            provider: "cloudflare".to_string(),
            region: None,
            service: None,
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // Grafana single IP normalized to /32
        StoredEntry {
            prefix: "34.140.222.135/32".to_string(),
            provider: "grafana-hosted-alerts".to_string(),
            region: None,
            service: None,
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // Google Cloud
        StoredEntry {
            prefix: "8.8.8.0/24".to_string(),
            provider: "google".to_string(),
            region: Some("us-central1".to_string()),
            service: Some("DNS".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        // Overlap: a service (Stripe /24) hosted inside a cloud (AWS /16).
        StoredEntry {
            prefix: "50.0.0.0/16".to_string(),
            provider: "aws".to_string(),
            region: Some("us-east-1".to_string()),
            service: Some("AMAZON".to_string()),
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
        },
        StoredEntry {
            prefix: "50.0.1.0/24".to_string(),
            provider: "stripe".to_string(),
            region: None,
            service: None,
            metadata: Metadata {
                fields: std::collections::HashMap::new(),
            },
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
        .oneshot(Request::builder().uri("/::1").body(Body::empty()).unwrap())
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

// ===== Layered / overlapping provider tests =====

#[tokio::test]
async fn test_check_ip_service_on_cloud_layers() {
    let app = create_test_app();

    // 50.0.1.5 is inside both Stripe's /24 (service) and AWS's /16 (cloud).
    let response = app
        .oneshot(
            Request::builder()
                .uri("/50.0.1.5")
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

    // Primary (top-level) fields point at the overlay service for back-compat.
    assert_eq!(json["data"]["provider"], "stripe");
    assert_eq!(json["data"]["kind"], "service");

    // Two layers, service first then the underlying cloud.
    let layers = json["data"]["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0]["provider"], "stripe");
    assert_eq!(layers[0]["kind"], "service");
    assert_eq!(layers[1]["provider"], "aws");
    assert_eq!(layers[1]["kind"], "cloud");
    assert_eq!(layers[1]["region"], "us-east-1");

    assert_eq!(
        json["data"]["summary"],
        "stripe (service) on aws (cloud · us-east-1 · AMAZON)"
    );
}

#[tokio::test]
async fn test_check_ip_cloud_only_single_layer() {
    let app = create_test_app();

    // 50.0.2.5 is inside AWS's /16 only (no overlay service).
    let response = app
        .oneshot(
            Request::builder()
                .uri("/50.0.2.5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["data"]["provider"], "aws");
    assert_eq!(json["data"]["kind"], "cloud");
    let layers = json["data"]["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(json["data"]["summary"], "aws (cloud · us-east-1 · AMAZON)");
}

// ===== Gov-domains ASN category tests =====

fn create_gov_asn_test_app() -> Router {
    use ipseek::db::{DbFile, Metadata, StoredEntry};
    use ipseek::server::{build_state_from_db, check_ip};

    fn meta(pairs: &[(&str, &str)]) -> Metadata {
        Metadata {
            fields: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    let entries = vec![
        // Federal: CBO 158.219.175.0/24
        StoredEntry {
            prefix: "158.219.175.0/24".to_string(),
            provider: "US Federal Government".to_string(),
            region: None,
            service: Some("Congressional Budget Office".to_string()),
            metadata: meta(&[("abbreviation", "CBO"), ("asn", "AS2274")]),
        },
        // State: California Department of Technology 165.107.0.0/16
        StoredEntry {
            prefix: "165.107.0.0/16".to_string(),
            provider: "US State Government".to_string(),
            region: None,
            service: Some("California Department of Technology".to_string()),
            metadata: meta(&[
                ("abbreviation", "California Department of Technology"),
                ("asn", "AS2642"),
            ]),
        },
        // City: City and County of Denver 169.133.0.0/16
        StoredEntry {
            prefix: "169.133.0.0/16".to_string(),
            provider: "US City Government".to_string(),
            region: None,
            service: Some("City and County of Denver".to_string()),
            metadata: meta(&[
                ("abbreviation", "City and County of Denver"),
                ("asn", "AS18815"),
            ]),
        },
        // Health IT: Athenahealth 103.252.161.0/24
        StoredEntry {
            prefix: "103.252.161.0/24".to_string(),
            provider: "US Health IT Vendors".to_string(),
            region: None,
            service: Some("Athenahealth".to_string()),
            metadata: meta(&[("abbreviation", "Athenahealth"), ("asn", "AS11174")]),
        },
        // Hospitals: ACMH Hospital 97.107.231.0/24
        StoredEntry {
            prefix: "97.107.231.0/24".to_string(),
            provider: "US Hospital Systems".to_string(),
            region: None,
            service: Some("ACMH Hospital".to_string()),
            metadata: meta(&[("abbreviation", "ACMH Hospital"), ("asn", "AS46881")]),
        },
        // Health Insurance: Aetna 206.213.192.0/18
        StoredEntry {
            prefix: "206.213.192.0/18".to_string(),
            provider: "US Health Insurance".to_string(),
            region: None,
            service: Some("Aetna, Inc.".to_string()),
            metadata: meta(&[("abbreviation", "Aetna, Inc."), ("asn", "AS6646")]),
        },
        // PBM: Diplomat Pharmacy 12.132.213.0/24
        StoredEntry {
            prefix: "12.132.213.0/24".to_string(),
            provider: "US Pharmacy Benefit Managers".to_string(),
            region: None,
            service: Some("Diplomat Pharmacy, Inc".to_string()),
            metadata: meta(&[
                ("abbreviation", "Diplomat Pharmacy, Inc"),
                ("asn", "AS396911"),
            ]),
        },
        // Academic: Argonne National Lab 130.202.0.0/16
        StoredEntry {
            prefix: "130.202.0.0/16".to_string(),
            provider: "US Academic Institutions".to_string(),
            region: None,
            service: Some("Argonne National Lab".to_string()),
            metadata: meta(&[("abbreviation", "Argonne National Lab"), ("asn", "AS683")]),
        },
    ];

    let db = DbFile {
        entries,
        source_commit: None,
        files: vec![],
    };
    let state = Arc::new(build_state_from_db(&db));
    Router::new()
        .route("/{ip}", get(check_ip))
        .layer(Extension(state))
}

async fn gov_asn_json(app: Router, ip: &str) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/{}", ip))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_gov_asn_federal() {
    let json = gov_asn_json(create_gov_asn_test_app(), "158.219.175.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-federal-government");
    assert_eq!(json["data"]["service"], "Congressional Budget Office");
    assert_eq!(json["data"]["metadata"]["abbreviation"], "CBO");
    assert_eq!(json["data"]["metadata"]["asn"], "AS2274");
}

#[tokio::test]
async fn test_gov_asn_state() {
    let json = gov_asn_json(create_gov_asn_test_app(), "165.107.1.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-state-government");
    assert_eq!(
        json["data"]["service"],
        "California Department of Technology"
    );
    assert_eq!(json["data"]["metadata"]["asn"], "AS2642");
}

#[tokio::test]
async fn test_gov_asn_city() {
    let json = gov_asn_json(create_gov_asn_test_app(), "169.133.1.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-city-government");
    assert_eq!(json["data"]["service"], "City and County of Denver");
    assert_eq!(json["data"]["metadata"]["asn"], "AS18815");
}

#[tokio::test]
async fn test_gov_asn_health_it() {
    let json = gov_asn_json(create_gov_asn_test_app(), "103.252.161.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-health-it-vendors");
    assert_eq!(json["data"]["service"], "Athenahealth");
    assert_eq!(json["data"]["metadata"]["asn"], "AS11174");
}

#[tokio::test]
async fn test_gov_asn_hospitals() {
    let json = gov_asn_json(create_gov_asn_test_app(), "97.107.231.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-hospital-systems");
    assert_eq!(json["data"]["service"], "ACMH Hospital");
    assert_eq!(json["data"]["metadata"]["asn"], "AS46881");
}

#[tokio::test]
async fn test_gov_asn_health_insurance() {
    let json = gov_asn_json(create_gov_asn_test_app(), "206.213.200.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-health-insurance");
    assert_eq!(json["data"]["service"], "Aetna, Inc.");
    assert_eq!(json["data"]["metadata"]["asn"], "AS6646");
}

#[tokio::test]
async fn test_gov_asn_pbm() {
    let json = gov_asn_json(create_gov_asn_test_app(), "12.132.213.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-pharmacy-benefit-managers");
    assert_eq!(json["data"]["service"], "Diplomat Pharmacy, Inc");
    assert_eq!(json["data"]["metadata"]["asn"], "AS396911");
}

#[tokio::test]
async fn test_gov_asn_academic() {
    let json = gov_asn_json(create_gov_asn_test_app(), "130.202.1.1").await;
    assert_eq!(json["error"], "");
    assert_eq!(json["data"]["provider"], "us-academic-institutions");
    assert_eq!(json["data"]["service"], "Argonne National Lab");
    assert_eq!(json["data"]["metadata"]["asn"], "AS683");
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

#[test]
fn compute_stats_counts_entries() {
    use ipseek::db::{compute_stats, DbFile, Metadata, StoredEntry};
    let entry = |prefix: &str, provider: &str| StoredEntry {
        prefix: prefix.to_string(),
        provider: provider.to_string(),
        region: None,
        service: None,
        metadata: Metadata::default(),
    };
    let db = DbFile {
        entries: vec![
            entry("13.32.0.0/15", "aws"),
            entry("2600:9000::/28", "aws"),
            entry("13.32.0.0/15", "cloudfront"),
        ],
        source_commit: None,
        files: vec![],
    };
    let s = compute_stats(&db);
    assert_eq!(s.entries, 3);
    assert_eq!(s.unique_prefixes, 2);
    assert_eq!(s.ipv4, 2);
    assert_eq!(s.ipv6, 1);
    assert_eq!(s.providers, 2);
    assert_eq!(s.by_provider["aws"], 2);
}
