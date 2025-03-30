use axum::{http::StatusCode, routing::{get, get_service, post}, Json, Router};
use serde::Deserialize;
use tower_http::services::{ServeDir, ServeFile};
use std::net::SocketAddr;

async fn handler_404() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[derive(Deserialize, Debug)]
struct SignupUser {
    name: String,
    email: String,
}

async fn signup_page() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

async fn signup_request(Json(payload): Json<SignupUser>) -> (StatusCode, &'static str) {
    //TODO send payload to database
    //TODO send message stating user has signed up
    println!("{:?}", payload);
    (StatusCode::NOT_FOUND, "Not Found")
}


#[tokio::main]
async fn main() {
    let app = Router::new()
        .route_service("/", ServeFile::new("./frontend/index.html"))
        .route("/signup", post(signup_request))
        .fallback(handler_404);

    let addr = SocketAddr::from(([127, 0, 0, 1], 37373));

    println!("Serving app at: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
