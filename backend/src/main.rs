use axum::{http::StatusCode, routing::get, Router};
use sqlx::{Executor, Pool, Sqlite};
use tokio::net::TcpListener;
use tower_http::services::ServeFile;
use std::net::SocketAddr;

mod signup;
mod appstate;

use signup::SignupUser;
use appstate::AppState;

async fn init_database() -> Pool<Sqlite>{
    let opt = sqlx::sqlite::SqliteConnectOptions::new().filename("test.db").create_if_missing(true);

    let pool = sqlx::sqlite::SqlitePool::connect_with(opt).await.unwrap();

    pool.execute("
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEST
      )
    ").await.unwrap();

    pool
}

fn init_router(state: AppState) -> Router {
    Router::new()
        .route_service("/", ServeFile::new("./frontend/htmxform.html"))
        .route("/signup", get(SignupUser::page).post(SignupUser::request))
        .route("/get_users", get(SignupUser::get_person_list))
        .with_state(state)
}

async fn init_listener() -> TcpListener {
    let addr = SocketAddr::from(([127, 0, 0, 1], 37373));
    println!("Serving app at: http://{}", addr);
    TcpListener::bind(addr).await.unwrap()
}


#[tokio::main]
async fn main() {
    let db = init_database().await;
    let state = AppState{pool: db};
    let app = init_router(state);
    let listener = init_listener().await;

    axum::serve(listener, app).await.unwrap();
}
