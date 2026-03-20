mod claude_cli;
mod error;
mod server;
#[cfg(test)]
mod tests;
mod translate;
mod types;

use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let debug = std::env::args().any(|a| a == "--debug");

    let default_directive = if debug {
        "cc_max_proxy=debug"
    } else {
        "cc_max_proxy=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(default_directive.parse()?))
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3456);
    let max_concurrent: usize = std::env::var("MAX_CONCURRENT")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1);

    let claude_path = claude_cli::resolve_claude_path()?;
    tracing::info!("Claude CLI found at: {}", claude_path);

    let state = Arc::new(server::AppState {
        claude_path,
        semaphore: Semaphore::new(max_concurrent),
    });

    let app = server::create_router(state);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("cc-max-proxy-rs listening on http://{}", addr);
    tracing::info!("Set ANTHROPIC_BASE_URL=http://{} in your tool", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
