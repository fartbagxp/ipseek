use ip_ranger::aws::{fetch_aws_cidr_ranges, find_matching_cidr};
use axum::{routing::get, Router, extract::Path, response::Json, Extension};
use ip_ranger::handler::check_ip;
use log::debug;
use reqwest::Error;
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::RwLock;
use iptrie::Ipv4Prefix;
use iptrie::Ipv4RTrieSet;
use ip_ranger::utils::{AppState, build_tries};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let tries = build_tries().await?;
    let state = Arc::new(AppState {
        tries: Arc::new(RwLock::new(tries)),
    });

    // Start the server
    let app = Router::new()
        .route("/", get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
        .route("/:ip", get(check_ip))
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}