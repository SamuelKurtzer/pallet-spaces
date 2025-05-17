use std::ops::Deref;

use async_trait::async_trait;
use axum_login::{AuthnBackend, UserId};
use password_auth::verify_password;
use sqlx::{Pool, Sqlite};
use tokio::task;

use crate::error::Error;

use super::users::{Credential, User};

#[derive(Clone, Debug)]
pub struct Database(pub Pool<Sqlite>);

impl Database {
    pub async fn new_database() -> Result<Self, Error> {
        let opt = sqlx::sqlite::SqliteConnectOptions::new()
        .filename("test.db")
        .create_if_missing(true);
        match sqlx::sqlite::SqlitePool::connect_with(opt).await {
            Ok(pool) => Ok(Database(pool)),
            Err(_) => Err(Error::Database("Failed to create database".into())),
        }
    }
}

impl Deref for Database {
    type Target = Pool<Sqlite>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait DatabaseComponent
where
    Self: Sized,
{
    async fn initialise_table<T: DatabaseProvider>(self) -> Result<Self, Error>;
    async fn create<T: DatabaseProvider>(&self, item: T) -> Result<&Self, Error>;
}

pub trait DatabaseProvider
where
    Self: Sized,
{
    type Database;
    type Id;
    async fn initialise_table(pool: Database) -> Result<Database, Error>;
    async fn create(self, pool: &Database) -> Result<&Database, Error>;
    async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error>;
    async fn update(id: Self::Id, pool: &Database) -> Result<&Database, Error>;
    async fn delete(id: Self::Id, pool: &Database) -> Result<&Database, Error>;
}

impl DatabaseComponent for Database {
    async fn initialise_table<T: DatabaseProvider>(self) -> Result<Self, Error> {
        T::initialise_table(self).await
    }

    async fn create<T: DatabaseProvider>(&self, item: T) -> Result<&Self, Error> {
        item.create(self).await
    }
}

#[async_trait]
impl AuthnBackend for Database {
    type User = User;
    type Credentials = Credential;
    type Error = Error;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user: Option<Self::User> = sqlx::query_as("select * from users where email = ? ")
            .bind(creds.email)
            .fetch_optional(&self.0)
            .await?;

        // Verifying the password is blocking and potentially slow, so we'll do so via
        // `spawn_blocking`.
        task::spawn_blocking(|| {
            // We're using password-based authentication--this works by comparing our form
            // input with an argon2 password hash.
            Ok(user.filter(|user| verify_password(creds.pw_hash, &String::from_utf8(user.pw_hash.clone()).expect("invalid password hash")).is_ok()))
        })
        .await?
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as("select * from users where id = ?")
            .bind(user_id)
            .fetch_optional(&self.0)
            .await?;
        Ok(user)
    }
}