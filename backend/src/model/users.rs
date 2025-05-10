use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};

use crate::error::Error;

use super::{DatabaseComponent};

#[derive(FromRow, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    id: u64,
    pub name: String,
    pub email: String,
    password: String,
}

impl std::fmt::Debug for User {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("User")
      .field("id", &self.id)
      .field("name", &self.name)
      .field("email", &self.email)
      .field("password", &"[REDACTED]")
      .finish()
  }
}

impl DatabaseComponent for User {
    async fn create_table(pool: &Pool<Sqlite>) -> Result<(), Error> {
      let creation_attempt = pool.execute("
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL UNIQUE
      )
      ").await;
      match creation_attempt {
          Ok(query_result) => {
            Ok(())
          },
          Err(_) => Err(Error::Database("Failed to create user database tables")),
      }
    }

    async fn insert_struct(self, pool: &Pool<Sqlite>) -> Result<(), Error> {
      let attempt = sqlx::query(
        "INSERT INTO users (name, email) VALUES (?1, ?2)")
        .bind(self.name)
        .bind(self.email)
        .execute(pool)
        .await;
      match attempt {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::Database("Failed to insert user into database")),
      }
    }
}