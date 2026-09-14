use axum::routing::get;
use limit_order_book::{
    api::{router, state::AppState},
    db::connection::connect_db,
    observability::metrics::initialize_prometheus,
};
use std::{error::Error, net::SocketAddr};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load local environment variables before reading configuration.
    dotenvy::dotenv().ok();

    //Initialize structured request logging
    tracing_subscriber::fmt()
        .with_env_filter("limit_order_book=debug,tower_http=debug")
        .init();

    let db = connect_db().await?;
    let state = AppState::new(db);

    // Initialize the global recorder and HTTP metrics middleware.
    let (prometheus_layer, metric_handle) = initialize_prometheus();

    let app = router(state)
        .route(
            "/metrics",
            get(move || {
                // Axum handlers can run concurrently, so each request gets a handle clone.
                let metric_handle = metric_handle.clone();

                async move {
                    // Render the current metrics snapshot.
                    metric_handle.render()
                }
            }),
        )
        // Automatically measure request count, latency, and active requests.
        .layer(prometheus_layer)
        // Keep the existing structured request logging.
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "API server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
