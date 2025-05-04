use axum::{extract::State, http::StatusCode, Form};
use maud::Markup;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, Pool, Sqlite};

use crate::{appstate::AppState, model::DatabaseComponent, model::users::User, views::signup::{signup_page, signup_success}};
pub struct SignupUser;
impl SignupUser {
    pub async fn page() -> (StatusCode, Markup) {
        (StatusCode::OK, signup_page().await)
    }

    pub async fn request(State(state): State<AppState>, Form(payload): Form<User>) -> (StatusCode, Markup) {
        println!("{:?}", payload);
        state.pool.insert_struct(payload);
        (StatusCode::OK, signup_success().await)
    }
}