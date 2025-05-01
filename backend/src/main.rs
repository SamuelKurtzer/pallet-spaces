use axum::{http::StatusCode, routing::get, serve::Listener, Router};
use sqlx::{Executor, Pool, Sqlite};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};
use std::net::SocketAddr;

mod signup;
mod appstate;
mod error;
mod pages;

use error::Error;
use signup::SignupUser;
use appstate::AppState;
use pages::{home::main_page, signup::signup_page};

async fn create_database() -> Result<Pool<Sqlite>, Error> {
    let opt = sqlx::sqlite::SqliteConnectOptions::new().filename("test.db").create_if_missing(true);

    let pool = sqlx::sqlite::SqlitePool::connect_with(opt).await.unwrap();

    let creation_attempt = pool.execute("
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEST
      )
    ").await;
    match creation_attempt {
        Ok(_) => Ok(pool),
        Err(_) => Err(Error::Database("Failed to create database tables")),
    }
}

fn create_router(state: AppState) -> Router {
    Router::new()
        .route_service("/", get(main_page))
        .route("/signup", get(signup_page).post(SignupUser::request))
        .route("/get_users", get(SignupUser::get_person_list))
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
    let state = AppState{pool: db};
    let app = create_router(state);
    let listener = match create_listener().await {
        Ok(listener) => listener,
        Err(err) => panic!("{:?}", err),
    };

    axum::serve(listener, app).await.unwrap();
}
