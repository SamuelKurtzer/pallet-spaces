use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};

use crate::error::Error;

use super::{DatabaseComponent, Private};

#[derive(FromRow, Serialize, Deserialize, Debug)]
pub struct User {
    id: Private<i32>,
    pub name: String,
    pub email: String
}

impl DatabaseComponent<User> for Pool<Sqlite> {
    async fn create_table(pool: Self) -> Result<Self, Error> {
      let creation_attempt = pool.execute("
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL UNIQUE
      )
      ").await;
      match creation_attempt {
          Ok(_) => Ok(pool),
          Err(_) => Err(Error::Database("Failed to create database tables")),
      }
    }

    async fn insert_struct(pool: Self, item: User) -> Result<Self, Error> {
      let attempt = sqlx::query("INSERT INTO users (name, email) VALUES (?1, ?2)").bind(payload.name).bind(payload.email).execute(&state.pool).await;
      match attempt {
        Ok(_) => todo!(),
        Err(_) => todo!(),
      }
    }
}