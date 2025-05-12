use axum_login::AuthUser;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};

use crate::error::Error;

use super::DatabaseProvider;

#[derive(Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    id: u64,
    pub name: String,
    pub email: String,
    pw_hash: Vec<u8>,
}

impl User {
    fn from_email(email: String, pool: Pool<Sqlite>) -> Result<Self, Error> {
        todo!()
    }

    async fn get_all_users(pool: &Pool<Sqlite>) -> Vec<User> {
        let mut users = vec![];
        for i in 0..20 {
            if let Ok(user) = User::retrieve(i, pool).await {
                users.push(user);
            }
        }
        users
    }
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

impl DatabaseProvider for User {
    type Id = u32;
    async fn initialise_table(pool: Pool<Sqlite>) -> Result<Pool<Sqlite>, Error> {
        let creation_attempt = &pool
            .execute(
                "
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL UNIQUE
      )
      ",
            )
            .await;
        match creation_attempt {
            Ok(_) => Ok(pool),
            Err(_) => Err(Error::Database("Failed to create user database tables")),
        }
    }

    async fn create(self, pool: &Pool<Sqlite>) -> Result<&Pool<Sqlite>, Error> {
        let attempt = sqlx::query("INSERT INTO users (name, email) VALUES (?1, ?2)")
            .bind(self.name)
            .bind(self.email)
            .execute(pool)
            .await;
        match attempt {
            Ok(_) => Ok(pool),
            Err(_) => Err(Error::Database("Failed to insert user into database")),
        }
    }

    async fn retrieve(id: Self::Id, pool: &Pool<Sqlite>) -> Result<Self, Error> {
        let attempt = sqlx::query_as::<_, User>("SELECT * FROM users where id=(?1)")
            .bind(id)
            .fetch_one(pool)
            .await;
        match attempt {
            Ok(user) => Ok(user),
            Err(_) => Err(Error::Database("Failed to insert user into database")),
        }
    }

    async fn update(id: Self::Id, pool: &Pool<Sqlite>) -> Result<&Pool<Sqlite>, Error> {
        todo!()
    }

    async fn delete(id: Self::Id, pool: &Pool<Sqlite>) -> Result<&Pool<Sqlite>, Error> {
        todo!()
    }
}

impl AuthUser for User {
    type Id = u64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.pw_hash
    }
}
