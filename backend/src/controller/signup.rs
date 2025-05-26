use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Form, Router,
};
use maud::Markup;

use crate::{
    appstate::AppState,
    model::{
        database::DatabaseComponent,
        users::{Credential, SignupUser, User},
    },
    views::{
        signup::{email_form_html, login_page, signup_failure, signup_page, signup_success},
        utils::page_not_found,
    },
};

use super::RouteProvider;
impl RouteProvider for User {
    fn provide_routes(router: Router<AppState>) -> Router<AppState> {
        router
            .route("/signup", get(User::signup_page).post(User::signup_request))
            .route("/signup/email", post(User::email_validation))
            .route("/login", get(User::login_page).post(User::login_request))
            .route("/users", get(User::user_list))
    }
}

impl User {
    pub async fn signup_page() -> (StatusCode, Markup) {
        (StatusCode::OK, signup_page().await)
    }

    pub async fn signup_request(
        State(state): State<AppState>,
        Form(payload): Form<SignupUser>,
    ) -> (StatusCode, Markup) {
        let pw_hash = password_auth::generate_hash(&payload.password);
        let user = User::new(&payload.name, &payload.email, &pw_hash);
        tracing::debug!("Signing up user {:?}", user);
        let insert_result = state.pool.create(user).await;
        tracing::debug!("Creation success {:?}", insert_result);
        match insert_result {
            Ok(_) => (StatusCode::OK, signup_success().await),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, signup_failure().await),
        }
    }

    pub async fn email_validation(Form(payload): Form<SignupUser>) -> (StatusCode, Markup) {
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

    // Login
    pub async fn login_page() -> (StatusCode, Markup) {
        (StatusCode::OK, login_page().await)
    }

    pub async fn login_request(
        State(state): State<AppState>,
        Form(payload): Form<Credential>,
    ) -> (StatusCode, Markup) {
        let maybe_user = User::from_email(payload.email, &state.pool).await;
        let user = match maybe_user {
            Err(_) => return (StatusCode::NOT_ACCEPTABLE, login_page().await),
            Ok(user) => user,
        };
        let valid = password_auth::verify_password(&payload.password, &user.pw_hash);
        match valid {
            Ok(_) => (StatusCode::OK, login_page().await),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, page_not_found()),
        }
    }

    pub async fn user_list(State(state): State<AppState>) -> (StatusCode, Markup) {
        let contents = maud::html! { ol {
            @for user in User::get_all_users(&state.pool).await {
                li { (user) }
            }
        }};
        (StatusCode::OK, contents)
    }
}
