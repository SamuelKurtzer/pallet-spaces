use axum_login::{AuthUser, AuthnBackend, UserId};
use password_auth::verify_password;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Executor, Pool, Sqlite};
use tokio::task;

use crate::error::Error;

use crate::model::database::{Database, DatabaseProvider};

#[derive(Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    id: u64,
    pub name: String,
    pub email: String,
    pw_hash: Vec<u8>,
}

pub struct Credential {
    pub email: String,
    pw_hash: Vec<u8>
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
            Err(_) => Err(Error::Database("Failed to create user database tables")),
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
            Err(_) => Err(Error::Database("Failed to insert user into database")),
        }
    }

    async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error> {
        let attempt = sqlx::query_as::<_, User>("SELECT * FROM users where id=(?1)")
            .bind(id)
            .fetch_one(&pool.0)
            .await;
        match attempt {
            Ok(user) => Ok(user),
            Err(_) => Err(Error::Database("Failed to insert user into database")),
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
    type Id = u64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.pw_hash
    }
}


impl AuthnBackend for Database {
    #[doc = " Authenticating user type."]
    type User = User;

    #[doc = " Credential type used for authentication."]
    type Credentials = Credential;

    #[doc = " An error which can occur during authentication and authorization."]
    type Error = Error;

    #[doc = " Authenticates the given credentials with the backend."]
    async fn authenticate(&self, creds: Self::Credentials) ->  Result<Option<Self::User>, Self::Error> {
        let user: Option<Self::User> = sqlx::query_as("select * from users where email = ?").bind(creds.email).fetch_optional(&self.0).await?;
        task::spawn_blocking(|| {
            // We're using password-based authentication--this works by comparing our form
            // input with an argon2 password hash.
            Ok(user.filter(|user| verify_password(creds.pw_hash, &user.pw_hash).is_ok()))
        })
        .await?
    }

    #[doc = " Gets the user by provided ID from the backend."]
    async fn get_user(&self, user_id: &UserId<Self>) ->  ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Result<Option<Self::User> ,Self::Error> > + ::core::marker::Send+'async_trait> >where 'life0:'async_trait,'life1:'async_trait,Self:'async_trait {
        todo!()
    }
}