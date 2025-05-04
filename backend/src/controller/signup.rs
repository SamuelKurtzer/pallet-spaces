use axum::{extract::State, http::StatusCode, Form};
use maud::Markup;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::{
    appstate::AppState,
    model::users::{User, DatabaseComponent<User>},
    views::signup::{signup_page, signup_success}
};
struct SignupUser;
impl SignupUser {
    pub async fn page() -> (StatusCode, Markup) {
        (StatusCode::OK, signup_page().await)
    }

    pub async fn request(State(state): State<AppState>, Form(payload): Form<User>) -> (StatusCode, Markup) {
        println!("{:?}", payload);
        state.pool.User::insert_struct(payload);
        sqlx::query("INSERT INTO users (name, email) VALUES (?1, ?2)").bind(payload.name).bind(payload.email).execute(&state.pool).await.unwrap();
        (StatusCode::OK, signup_success().await)
    }

    pub async fn get_person_list(State(state): State<AppState>) -> (StatusCode, String) {
        let rows: Vec<User> = sqlx::query_as("SELECT * FROM users").fetch_all(&state.pool).await.unwrap();
        let json_data = serde_json::to_string_pretty(&rows).unwrap();
        (StatusCode::OK, json_data)
    }
}