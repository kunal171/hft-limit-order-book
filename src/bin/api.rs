use limit_order_book::{
    api::{router, state::AppState},
    db::connection::connect_db,
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

    let app = router(state).layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "API server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
