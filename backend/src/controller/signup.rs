use axum::{extract::State, http::StatusCode, Form};
use maud::Markup;

use crate::{appstate::AppState, model::{users::User, DatabaseComponent}, views::signup::{signup_failure, signup_page, signup_success}};
pub struct SignupUser;
impl SignupUser {
    pub async fn page() -> (StatusCode, Markup) {
        (StatusCode::OK, signup_page().await)
    }

    pub async fn request(State(state): State<AppState>, Form(payload): Form<User>) -> (StatusCode, Markup) {
        println!("{:?}", payload);
        let insert_result = payload.insert_struct(&state.pool).await;
        match insert_result {
            Ok(_) => (StatusCode::OK, signup_success().await),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, signup_failure().await),
        }
    }
}