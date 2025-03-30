use axum::{extract::State, http::StatusCode, Form, Json};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::appstate::AppState;

#[derive(Deserialize, Debug)]
pub struct SignupUser {
    name: String,
    email: String,
}

#[derive(FromRow, Serialize)]
pub struct User {
    id: i32,
    name: String,
}

impl SignupUser {
    pub async fn page() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "Not Found")
    }

    pub async fn request(State(state): State<AppState>, Form(payload): Form<SignupUser>) -> (StatusCode, &'static str) {
        println!("{:?}", payload);
        //TODO send payload to database
        sqlx::query("INSERT INTO users (name) VALUES (?1)").bind(payload.name).execute(&state.pool).await.unwrap();
        //TODO send message stating user has signed up
        (StatusCode::OK, "Added the bitch")
    }

    pub async fn get_person_list(State(state): State<AppState>) -> (StatusCode, String) {
        let rows: Vec<User> = sqlx::query_as("SELECT * FROM users").fetch_all(&state.pool).await.unwrap();
      
        let json_data = serde_json::to_string_pretty(&rows).unwrap();
      
        (StatusCode::OK, json_data)
    }
}