pub mod users;
pub mod posts;
use sqlx::{Pool, Sqlite};

use crate::error::Error;
pub trait DatabaseComponent where Self: Sized {
    async fn create_table(pool: &Pool<Sqlite>) -> Result<(), Error>;
    async fn insert_struct(self, pool: &Pool<Sqlite>) -> Result<(), Error>;
}