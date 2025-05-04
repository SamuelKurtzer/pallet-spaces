pub mod users;

use serde::{Deserialize, Serialize};
use sqlx::{Executor, Pool, Sqlite};

use crate::error::Error;

#[derive(Debug, Default, Serialize, Deserialize)]
enum Private<T> {
    #[default]
    Priv,
    Pub(T),
}

pub trait DatabaseComponent<T> where Self: Sized {
    async fn create_table(pool: Self) -> Result<Self, Error>;
    async fn insert_struct(pool: Self, item: T) -> Result<Self, Error>;
}