use async_trait::async_trait;
use axum_login::{AuthUser, AuthnBackend, UserId};
use password_auth::verify_password;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor};
use tokio::task;

use crate::error::Error;

use crate::model::database::{Database, DatabaseProvider};

#[derive(Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    id: u64,
    pub name: String,
    pub email: String,
    pub pw_hash: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credential {
    pub email: String,
    pub pw_hash: Vec<u8>
}

impl User {
    fn from_email(email: String, pool: Database) -> Result<Self, Error> {
        todo!()
    }

    async fn get_all_users(pool: &Database) -> Vec<User> {
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
    type Database = Database;
    type Id = u32;
    async fn initialise_table(pool: Database) -> Result<Database, Error> {
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
            Err(_) => Err(Error::Database("Failed to create user database tables".into())),
        }
    }

    async fn create(self, pool: &Database) -> Result<&Database, Error> {
        let attempt = sqlx::query("INSERT INTO users (name, email) VALUES (?1, ?2)")
            .bind(self.name)
            .bind(self.email)
            .execute(&pool.0)
            .await;
        match attempt {
            Ok(_) => Ok(pool),
            Err(_) => Err(Error::Database("Failed to insert user into database".into())),
        }
    }

    async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error> {
        let attempt = sqlx::query_as::<_, User>("SELECT * FROM users where id=(?1)")
            .bind(id)
            .fetch_one(&pool.0)
            .await;
        match attempt {
            Ok(user) => Ok(user),
            Err(_) => Err(Error::Database("Failed to insert user into database".into())),
        }
    }

    async fn update(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
        todo!()
    }

    async fn delete(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
        todo!()
    }
}

impl AuthUser for User {
    type Id = u32;

    fn id(&self) -> Self::Id {
        self.id as u32
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.pw_hash
    }
}