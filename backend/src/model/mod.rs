pub mod database;

pub mod posts;
pub mod users;
use std::ops::Deref;

use sqlx::{Pool, Sqlite};

use crate::error::Error;


