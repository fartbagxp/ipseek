use crate::utils::{check_ip_type, lookup_aws_cidr, parse_ip, AppState};
use axum::{extract::Path, response::Json, Extension};
use log::debug;
use serde_json::json;
use std::sync::Arc;

pub async fn check_ip(
    Path(ip_str): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
) -> Json<serde_json::Value> {
    debug!("Requested IP: {}", ip_str);

    let response = match parse_ip(&ip_str) {
        Ok(ip) => match check_ip_type(&ip) {
            "Private IP" => json!({
                "error": "",
                "data": "Private IP"
            }),
            "Public IP" => {
                let tries = state.tries.read().await;
                let tries_v6 = state.tries_v6.read().await;

                let trie = tries.get("aws");
                let trie_v6 = tries_v6.get("aws");

                if let (Some(trie), Some(trie_v6)) = (trie, trie_v6) {
                    lookup_aws_cidr(ip, trie, trie_v6).await
                } else {
                    json!({
                        "error": "Trie not found",
                        "data": ""
                    })
                }
            }
            _ => json!({
                "error": "Unknown IP type",
                "data": ""
            }),
        },
        Err(err) => json!({
            "error": err,
            "data": ""
        }),
    };

    Json(response)
}
