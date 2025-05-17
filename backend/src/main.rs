mod appstate;
mod controller;
mod error;
mod model;
mod views;

use appstate::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use controller::Routes;
use error::Error;
use model::{posts::Post, users::User, database::{Database, DatabaseComponent}};
use sqlx::{Pool, Sqlite};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use views::home::main_page;

async fn create_database() -> Result<Database, Error> {
    let pool = Database::new().await?;
    Ok(pool
        .initialise_table::<User>()
        .await?
        .initialise_table::<Post>()
        .await?)
}

fn create_router(state: AppState) -> Router {
    Router::new()
        .route_service("/", get(main_page))
        .add_routes::<User>()
        .nest_service("/public", ServeDir::new("./frontend/public/"))
        .with_state(state)
}

async fn create_listener() -> Result<TcpListener, Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 37373));
    tracing::info!("Serving app at: http://{}", addr);
    match TcpListener::bind(addr).await {
        Ok(ok) => Ok(ok),
        Err(_) => Err(Error::SocketBind("Failed to bind to specified socket".into())),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Tracing initialised.");

    let db = match create_database().await {
        Ok(db) => db,
        Err(err) => panic!("{:?}", err),
    };
    let state = AppState::new(db);
    let app = create_router(state);
    let listener = match create_listener().await {
        Ok(listener) => listener,
        Err(err) => panic!("{:?}", err),
    };

    axum::serve(listener, app).await.unwrap();
}
