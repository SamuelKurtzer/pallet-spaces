use axum::{http::StatusCode, routing::{get, get_service}, Router};
use tower_http::services::ServeDir;
use std::net::SocketAddr;

async fn handler_404() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}


#[tokio::main]
async fn main() {
    let app = Router::new()
        .nest_service("/", ServeDir::new("../frontend"))
        .fallback(handler_404);

    let addr = SocketAddr::from(([127, 0, 0, 1], 37373));

    println!("Serving app at: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
