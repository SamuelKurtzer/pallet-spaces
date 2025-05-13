use std::ops::Deref;

use sqlx::{Pool, Sqlite};

use crate::error::Error;

#[derive(Clone, Debug)]
pub struct Database(pub Pool<Sqlite>);

impl Database {
    pub async fn new_database() -> Result<Self, Error> {
        let opt = sqlx::sqlite::SqliteConnectOptions::new()
        .filename("test.db")
        .create_if_missing(true);
        match sqlx::sqlite::SqlitePool::connect_with(opt).await {
            Ok(pool) => Ok(Database(pool)),
            Err(_) => Err(Error::Database("Failed to create database")),
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