mod appstate;
mod error;
mod views;
mod model;
mod controller;

use axum::{routing::get, Router};
use controller::signup::SignupUser;
use sqlx::{Executor, Pool, Sqlite};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use std::net::SocketAddr;
use error::Error;
use model::{posts::Post, users::User, DatabaseComponent};
use appstate::AppState;
use views::{home::main_page};

async fn create_database() -> Result<Pool<Sqlite>, Error> {
    let opt = sqlx::sqlite::SqliteConnectOptions::new().filename("test.db").create_if_missing(true);

    let pool = sqlx::sqlite::SqlitePool::connect_with(opt).await.unwrap(); 
    pool.create_table::<User>()
        .create_table::<Post>()
        .await
}

fn create_router(state: AppState) -> Router {
    Router::new()
        .route_service("/", get(main_page))
        .route("/signup", get(SignupUser::page).post(SignupUser::request))
        .nest_service("/public", ServeDir::new("./frontend/public/"))
        .with_state(state)
}

async fn create_listener() -> Result<TcpListener, Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 37373));
    println!("Serving app at: http://{}", addr);
    match TcpListener::bind(addr).await {
        Ok(ok) => Ok(ok),
        Err(_) => Err(Error::SocketBind("failed to bind to specified socket")),
    }
}


#[tokio::main]
async fn main() {
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
