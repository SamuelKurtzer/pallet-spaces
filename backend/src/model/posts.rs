use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};

use crate::error::Error;

use super::{DatabaseComponent, Private};

#[derive(FromRow, Serialize, Deserialize, Debug)]
pub struct Post {
    id: Private<i32>,
    title: String,
}

impl DatabaseComponent<Post> for Pool<Sqlite> {
    async fn create_table(self) -> Result<Self, Error> {
      let creation_attempt = self.execute("
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL UNIQUE
      )
      ").await;
      match creation_attempt {
          Ok(_) => Ok(self),
          Err(_) => Err(Error::Database("Failed to create database tables")),
      }
    }

    async fn insert_struct(self, item: Post) -> Result<Self, Error> {
      let attempt = sqlx::query("INSERT INTO users (title) VALUES (?1)").bind(item.title).execute(&self).await;
      match attempt {
        Ok(_) => todo!(),
        Err(_) => todo!(),
      }
    }
}