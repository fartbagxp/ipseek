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
    /// Fetch datasets and rebuild local DB from the given GitHub contents URL
    Update { #[arg(long, default_value = "https://api.github.com/repos/fartbagxp/aas-cidr-ranges/contents/data/raw")] url: String },
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
        Commands::Update { url } => {
            println!("Running update -> {}", cli.db);
            let db = updater::run_update_from_url(&url).await?;
            save_db(&db, &cli.db)?;
            println!("run_update completed. DB written to {}", cli.db);
            return Ok(());
        }
        Commands::Check { ip } => {
            let db = db::load_db(&cli.db)?;
            let p = lookup::lookup_provider(db, &ip)?;
            println!("{} -> {}", ip, p);
            return Ok(());
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
            return Ok(());
        }
    }

    Ok(())
}
