use std::collections::HashSet;
use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use bl4_community::{
    db::Database, helpers::sanitize_db_url, rate_limit::RateLimiter, routes, state::AppState,
};

#[derive(Parser)]
#[command(name = "bl4-community")]
#[command(about = "Community API server for Borderlands 4 items database")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Start the API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3030")]
        port: u16,

        /// Database path
        #[arg(short, long, env = "DATABASE_URL", default_value = "share/items.db")]
        database: String,

        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            port,
            database,
            bind,
        } => {
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "bl4_community=info,tower_http=debug".into()),
                )
                .with(tracing_subscriber::fmt::layer())
                .init();

            let db_url = if database.contains("://") {
                database.clone()
            } else {
                format!("sqlite:{}?mode=rwc", database)
            };

            tracing::info!("Connecting to database: {}", sanitize_db_url(&db_url));
            let db = Database::connect(&db_url).await?;
            db.init().await?;
            tracing::info!("Database initialized");

            let allowed_ips: HashSet<String> = std::env::var("BL4_ALLOWED_IPS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if !allowed_ips.is_empty() {
                tracing::info!("Allowed IPs for attachments: {:?}", allowed_ips);
            }

            let state = Arc::new(AppState {
                db,
                write_limiter: RateLimiter::new(30),
                allowed_ips,
            });
            let app = routes::build_router(state);

            let bind_addr = format!("{}:{}", bind, port);
            tracing::info!("Starting server on {}", bind_addr);
            tracing::info!("OpenAPI spec available at /openapi.json");
            tracing::info!("Interactive docs at /scalar");

            let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
