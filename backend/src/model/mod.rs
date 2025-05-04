pub mod users;
pub mod posts;

use std::path::Component;

use serde::{Deserialize, Serialize};
use sqlx::{Executor, Pool, Sqlite};

use crate::error::Error;

#[derive(Debug, Default, Serialize, Deserialize)]
enum Private<T> {
    #[default]
    Priv,
    Pub(T),
}

pub trait DatabaseComponent where Self: Sized {
    type Component;
    async fn create_table<Component>(self) -> Result<Self, Error> where 
        Self: DatabaseComponentImpl<Component>;
    async fn insert_struct(self, item: Component) -> Result<Self, Error>;
}

pub trait DatabaseComponentImpl<T> {
    async fn create_table_impl(&self) -> Result<(), Error>;
}