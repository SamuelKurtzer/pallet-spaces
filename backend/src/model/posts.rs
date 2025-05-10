use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};

use crate::error::Error;

use super::{DatabaseComponent, Private};

#[derive(FromRow, Serialize, Deserialize, Debug)]
pub struct Post {
    id: Private<i32>,
    title: String,
}

impl DatabaseComponent for Post {
  async fn create_table(pool: &Pool<Sqlite>) -> Result<(), Error> {
    let creation_attempt = pool.execute("
    CREATE TABLE if not exists users (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL
    )
    ").await;
    match creation_attempt {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::Database("Failed to create post database tables")),
    }
  }

  async fn insert_struct(self, pool: &Pool<Sqlite>) -> Result<(), Error> {
    let attempt = sqlx::query(
      "INSERT INTO users (title) VALUES (?1)")
      .bind(self.title)
      .execute(pool)
      .await;
    match attempt {
      Ok(_) => Ok(()),
      Err(_) => Err(Error::Database("Failed to insert post into database")),
    }
  }
}