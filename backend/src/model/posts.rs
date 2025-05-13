use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};

use crate::error::Error;

use crate::model::database::{Database, DatabaseProvider};

#[derive(FromRow, Serialize, Deserialize, Debug)]
pub struct Post {
    #[serde(default)]
    id: i32,
    title: String,
}

impl DatabaseProvider for Post {
    type Database = Database;
    type Id = u32;

    async fn initialise_table(pool: Database) -> Result<Database, Error> {
        let creation_attempt = pool
            .execute(
                "
    CREATE TABLE if not exists users (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL
    )
    ",
            )
            .await;
        match creation_attempt {
            Ok(_) => Ok(pool),
            Err(_) => Err(Error::Database("Failed to create post database tables")),
        }
    }

    async fn create(self, pool: &Database) -> Result<&Database, Error> {
        let attempt = sqlx::query("INSERT INTO users (title) VALUES (?1)")
            .bind(self.title)
            .execute(&pool.0)
            .await;
        match attempt {
            Ok(_) => Ok(pool),
            Err(_) => Err(Error::Database("Failed to insert post into database")),
        }
    }

    async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error> {
        todo!()
    }

    async fn update(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
        todo!()
    }

    async fn delete(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
        todo!()
    }
}
