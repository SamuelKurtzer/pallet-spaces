pub mod users;
pub mod posts;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};

use crate::error::Error;

#[derive(Debug, Default, Serialize, Deserialize)]
enum Private<T> {
    #[default]
    Priv,
    Pub(T),
}

pub trait DatabaseComponent where Self: Sized {
    async fn create_table(pool: &Pool<Sqlite>) -> Result<(), Error>;
    async fn insert_struct(self, pool: &Pool<Sqlite>) -> Result<(), Error>;
}