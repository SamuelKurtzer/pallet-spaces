pub mod posts;
pub mod users;
use sqlx::{Pool, Sqlite};

use crate::error::Error;

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
    type Id;
    async fn initialise_table(pool: Pool<Sqlite>) -> Result<Pool<Sqlite>, Error>;
    async fn create(self, pool: &Pool<Sqlite>) -> Result<&Pool<Sqlite>, Error>;
    async fn retrieve(id: Self::Id, pool: &Pool<Sqlite>) -> Result<Self, Error>;
    async fn update(id: Self::Id, pool: &Pool<Sqlite>) -> Result<&Pool<Sqlite>, Error>;
    async fn delete(id: Self::Id, pool: &Pool<Sqlite>) -> Result<&Pool<Sqlite>, Error>;
}

impl DatabaseComponent for Pool<Sqlite> {
    async fn initialise_table<T: DatabaseProvider>(self) -> Result<Self, Error> {
        T::initialise_table(self).await
    }

    async fn create<T: DatabaseProvider>(&self, item: T) -> Result<&Self, Error> {
        item.create(self).await
    }
}
