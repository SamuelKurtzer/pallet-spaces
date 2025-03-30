use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::appstate::AppState;

#[derive(Deserialize, Debug)]
pub struct SignupUser {
    name: String,
    email: String,
}

impl SignupUser {
    pub async fn page() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "Not Found")
    }

    pub async fn request(State(state): State<AppState>, Json(payload): Json<SignupUser>) -> (StatusCode, &'static str) {
        println!("{:?}", payload);
        //TODO send payload to database
        //TODO send message stating user has signed up
        (StatusCode::NOT_FOUND, "Not Found")
    }
}