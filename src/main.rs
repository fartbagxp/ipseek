mod db;
mod updater;
mod lookup;
mod server;

use clap::{Parser, Subcommand};
use anyhow::Result;
use db::save_db;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to local DB file
    #[arg(long, default_value = "./db.bin")]
    db: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build local DB from data/raw directory
    UpdateLocal { #[arg(long, default_value = "./data/raw")] path: String },
    /// Check a single IP address against the local DB
    Check { ip: String },
    /// Run HTTP server
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::UpdateLocal { path } => {
            println!("Running local update from {} -> {}", path, cli.db);
            let db = updater::run_update_from_local(&path)?;
            save_db(&db, &cli.db)?;
            println!("local update completed. DB written to {}", cli.db);
        }
        Commands::Check { ip } => {
            let db = db::load_db(&cli.db)?;
            let p = lookup::lookup_provider(db, &ip)?;
            println!("{} -> {}", ip, p);
        }
        Commands::Serve => {
            let db = db::load_db(&cli.db)?;
            let state = server::build_state_from_db(&db);
            let state = std::sync::Arc::new(state);

            // start server with state extension
            let app = axum::Router::new()
                .route("/", axum::routing::get(|| async { "Hello to IP ranger! Append your IP to the URL to use this service." }))
                .route("/{ip}", axum::routing::get(server::check_ip))
                .layer(axum::extract::Extension(state));

            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
            axum::serve(listener, app).await.unwrap();
        }
    }

    Ok(())
}
