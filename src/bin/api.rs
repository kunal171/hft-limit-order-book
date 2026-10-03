use axum::{Router, routing::get};
use limit_order_book::{
    api::{router, state::AppState},
    db::connection::connect_db,
    observability::metrics::initialize_prometheus,
};
use std::{error::Error, future::IntoFuture, net::SocketAddr};
use tokio::{net::TcpListener, sync::watch};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load local environment variables before reading configuration.
    dotenvy::dotenv().ok();

    // Initialize structured request logging.
    // RUST_LOG overrides the default, e.g. RUST_LOG=limit_order_book=trace.
    // `api` is this binary's own log target; without it the startup and
    // shutdown messages below are filtered out.
    let log_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("api=debug,limit_order_book=debug,tower_http=debug"));

    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let db = connect_db().await?;
    let state = AppState::new(db);

    // Initialize the global recorder and HTTP metrics middleware.
    let (prometheus_layer, metric_handle) = initialize_prometheus();

    let app = router(state)
        // Automatically measure request count, latency, and active requests.
        .layer(prometheus_layer)
        // Keep the existing structured request logging.
        .layer(TraceLayer::new_for_http());

    // Read the bind address from the environment so development,
    // Docker, and production can use different network configurations.
    // Both default to loopback; widen them only where a client must reach them.
    let addr = socket_addr_from_env("API_ADDR", "127.0.0.1:3000")?;
    let metrics_addr = socket_addr_from_env("METRICS_ADDR", "127.0.0.1:9100")?;

    // Metrics get their own listener, so Prometheus can be allowed to reach
    // them without exposing the API, and API clients cannot read them.
    let metrics_app = Router::new().route(
        "/metrics",
        get(move || {
            // Axum handlers can run concurrently, so each request gets a handle clone.
            let metric_handle = metric_handle.clone();

            async move {
                // Render the current metrics snapshot.
                metric_handle.render()
            }
        }),
    );

    let listener = TcpListener::bind(addr).await?;
    let metrics_listener = TcpListener::bind(metrics_addr).await?;

    tracing::info!(%addr, "API server listening");
    tracing::info!(%metrics_addr, "metrics server listening");

    // One signal stops both servers.
    let (shutdown_tx, shutdown_rx) = watch::channel(());
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(());
    });

    // Connect info gives the rate limiter each client's address.
    let api_server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(wait_for_shutdown(shutdown_rx.clone()))
    .into_future();

    let metrics_server = axum::serve(metrics_listener, metrics_app)
        .with_graceful_shutdown(wait_for_shutdown(shutdown_rx))
        .into_future();

    tokio::try_join!(api_server, metrics_server)?;

    Ok(())
}

/// Reads a socket address from the environment, or uses `default`.
fn socket_addr_from_env(name: &str, default: &str) -> Result<SocketAddr, Box<dyn Error>> {
    let value = std::env::var(name).unwrap_or_else(|_| default.to_string());

    value
        .parse()
        .map_err(|error| format!("{name}={value} is not a socket address: {error}").into())
}

/// Resolves once the shutdown signal has been broadcast.
async fn wait_for_shutdown(mut shutdown: watch::Receiver<()>) {
    // An error means the sender is gone, which also means shut down.
    let _ = shutdown.changed().await;
}

/// Resolves when the process is asked to stop.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Ctrl+C handler should install");
    };

    // SIGTERM is what Docker and Kubernetes send to stop a container.
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler should install")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, finishing in-flight requests");
}
