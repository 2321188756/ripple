mod config;
mod routes;
mod state;
mod worker;

use config::ServiceConfig;
use ripple_knowledge_ingest::LocalObjectStore;
use ripple_knowledge_store::{AuthConfig, KnowledgeStore};
use state::AppState;
use std::io;
use tokio::signal;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = ServiceConfig::from_env().map_err(|error| {
        tracing::error!(error = %error, "knowledge service configuration rejected");
        error
    })?;
    std::fs::create_dir_all(&config.data_root)?;

    let store = match KnowledgeStore::connect(&config.database_url, config.max_connections).await {
        Ok(store) => match store.initialize().await {
            Ok(()) => Some(store),
            Err(error) => {
                warn!(
                    code = error.code(),
                    "knowledge service started without ready database dependencies"
                );
                None
            }
        },
        Err(error) => {
            warn!(
                code = error.code(),
                "knowledge service started without database connection"
            );
            None
        }
    };

    let object_store = LocalObjectStore::new(config.data_root.join("objects"));
    if let Some(ready_store) = store.clone() {
        ready_store.recover_expired_ingestion_leases().await?;
        worker::spawn_worker(
            ready_store,
            object_store.clone(),
            format!("worker-{}", uuid::Uuid::new_v4()),
        );
    }

    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    info!(listen_addr = %config.listen_addr, "knowledge service listening");

    axum::serve(
        listener,
        routes::router(AppState {
            store,
            object_store: object_store.clone(),
            bootstrap_token_digest: AppState::bootstrap_digest(&config.bootstrap_token),
            auth: AuthConfig {
                access_ttl: config.access_ttl,
                refresh_ttl: config.refresh_ttl,
            },
        }),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RIPPLE_KNOWLEDGE_LOG")
                .unwrap_or_else(|_| "ripple_knowledge_server=info,warn".to_owned()),
        )
        .with_target(true)
        .with_ansi(false)
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::warn!(error = %error, "knowledge service Ctrl+C handler unavailable");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::warn!(error = %error, "knowledge service SIGTERM handler unavailable")
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[allow(dead_code)]
fn io_error_is_safe_to_report(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "permission_denied",
        io::ErrorKind::NotFound => "not_found",
        _ => "io_failure",
    }
}
