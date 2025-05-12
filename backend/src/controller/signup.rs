use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Form, Router,
};
use maud::Markup;

use crate::{
    appstate::AppState,
    model::{users::User, DatabaseComponent},
    views::signup::{email_form_html, signup_failure, signup_page, signup_success},
};

use super::RouteProvider;
pub struct SignupUser;
impl RouteProvider for SignupUser {
    fn provide_routes(router: Router<AppState>) -> Router<AppState> {
        router
            .route("/signup", get(SignupUser::page).post(SignupUser::request))
            .route("/signup/email", post(SignupUser::email_validation))
    }
}

impl SignupUser {
    pub async fn page() -> (StatusCode, Markup) {
        (StatusCode::OK, signup_page().await)
    }

    pub async fn request(
        State(state): State<AppState>,
        Form(payload): Form<User>,
    ) -> (StatusCode, Markup) {
        println!("{:?}", payload);
        let insert_result = state.pool.create(payload).await;
        match insert_result {
            Ok(_) => (StatusCode::OK, signup_success().await),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, signup_failure().await),
        }
    }

    pub async fn email_validation(Form(payload): Form<User>) -> (StatusCode, Markup) {
        // Actually a hard problem, can be better solved(see: https://david-gilbertson.medium.com/the-100-correct-way-to-validate-email-addresses-7c4818f24643)
        // but for now
        // check there exits an @
        let mut valid = payload.email.contains('@');

        // Check text is either side of the email
        let results = payload.email.split('@').collect::<Vec<&str>>();
        let mut res_iter = results.iter();
        valid &= match res_iter.next() {
            Some(a) => !a.is_empty(),
            None => false,
        };
        valid &= match res_iter.next() {
            Some(a) => !a.is_empty(),
            None => false,
        };

        (StatusCode::OK, email_form_html(valid, &payload.email))
    }
}
