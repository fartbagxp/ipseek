use axum::{routing::get, Extension, Router};
use ip_ranger::handler::check_ip;
use ip_ranger::utils::{build_tries, AppState};
use reqwest::Error;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let (tries, tries_v6) = build_tries().await?;
    let state = Arc::new(AppState {
        tries: Arc::new(RwLock::new(tries)),
        tries_v6: Arc::new(RwLock::new(tries_v6)),
    });

    // Start the server
    let app = Router::new()
        .route(
            "/",
            get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }),
        )
        .route("/:ip", get(check_ip))
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
