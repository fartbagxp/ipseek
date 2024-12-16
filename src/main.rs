use axum::{routing::get, Router, Extension};
use ip_ranger::handler::check_ip;
use reqwest::Error;
use std::sync::Arc;
use tokio::sync::RwLock;
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