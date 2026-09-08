use crate::api::endpoints;
use crate::api::middleware::logging_middleware;
use axum::routing::get;
use axum::{Router, middleware};
use std::error::Error;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;

pub async fn run(listener: TcpListener) -> Result<(), Box<dyn Error>> {
    let api = Router::new()
        .route("/info", get(endpoints::info::get_info))
        .route("/products/{id}", get(endpoints::details::get_product))
        .route("/variants/{id}", get(endpoints::details::get_variant))
        .route("/{section}/products", get(endpoints::products::get_products));
    
    let app = Router::new()
        .nest("/api", api.clone())
        .merge(api)
        .merge(crate::api::frontend::routes())
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(logging_middleware));

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;

    Ok(())
}
