use sqlx::SqlitePool;

use crate::model::database::Database;

#[derive(Clone)]
pub struct AppState {
    pub pool: Database,
}

impl AppState {
    pub fn new(pool: Database) -> Self {
        AppState { pool: pool}
    }
}