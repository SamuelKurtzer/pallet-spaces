use axum::{extract::State, http::StatusCode, response::IntoResponse};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        AppState { pool: pool }
    }
}