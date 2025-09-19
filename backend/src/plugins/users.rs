use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use tracing::debug;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct UserID(u64);

impl From<u64> for UserID {
    fn from(raw: u64) -> Self {
        UserID(raw)
    }
}

#[derive(Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    id: Option<UserID>,
    pub name: String,
    pub email: String,
    pub pw_hash: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SignupUser {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credential {
    pub email: String,
    pub password: String,
}

impl User {
    pub fn new(name: &str, email: &str, password: &str) -> Self {
        let user = User {
            id: None,
            name: name.to_string(),
            email: email.to_string(),
            pw_hash: password.to_string(),
        };
        debug!("Made new user {:?}", user);
        user
    }
}

mod model {
    use axum_login::AuthUser;
    use sqlx::Executor;

    use crate::{
        error::Error,
        model::database::{Database, DatabaseProvider},
    };

    use super::User;
    impl User {
        pub async fn from_email(email: String, pool: &Database) -> Result<Self, Error> {
            tracing::debug!(email = %email, "lookup user by email");
            let user: User = sqlx::query_as("select * from users where email = ? ")
                .bind(email)
                .fetch_one(&pool.0)
                .await?;
            tracing::debug!(?user, "user loaded");
            Ok(user)
        }

        pub async fn get_all_users(pool: &Database) -> Vec<User> {
            match sqlx::query_as::<_, User>(
                "SELECT id, name, email, pw_hash FROM users ORDER BY id DESC LIMIT 100",
            )
            .fetch_all(&pool.0)
            .await
            {
                Ok(list) => list,
                Err(err) => {
                    tracing::warn!(?err, "failed to list users");
                    vec![]
                }
            }
        }

        pub async fn exists_by_email(pool: &Database, email: &str) -> Result<bool, Error> {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM users WHERE email = ?1 LIMIT 1",
            )
            .bind(email)
            .fetch_optional(&pool.0)
            .await?;
            Ok(exists.is_some())
        }
    }

    impl std::fmt::Debug for User {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("User")
                .field("id", &self.id)
                .field("name", &self.name)
                .field("email", &self.email)
                .field("password", &"[REDACTED]")
                .finish()
        }
    }

    impl std::fmt::Display for User {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&format!("{:?}", self))
        }
    }

    impl DatabaseProvider for User {
        type Database = Database;
        type Id = u32;
        async fn initialise_table(pool: Database) -> Result<Database, Error> {
            let creation_attempt = &pool
                .0
                .execute(
                    "
      CREATE TABLE if not exists users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL UNIQUE,
        pw_hash TEXT NOT NULL
      )
      ",
                )
                .await;
            match creation_attempt {
                Ok(_) => Ok(pool),
                Err(_) => Err(Error::Database(
                    "Failed to create user database tables".into(),
                )),
            }
        }

        async fn create(self, pool: &Database) -> Result<&Database, Error> {
            let attempt =
                sqlx::query("INSERT INTO users (name, email, pw_hash) VALUES (?1, ?2, ?3)")
                    .bind(self.name)
                    .bind(self.email)
                    .bind(self.pw_hash)
                    .execute(&pool.0)
                    .await;
            match attempt {
                Ok(_) => Ok(pool),
                Err(_) => Err(Error::Database(
                    "Failed to insert user into database".into(),
                )),
            }
        }

        async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error> {
            let attempt = sqlx::query_as::<_, User>("SELECT * FROM users where id=(?1)")
                .bind(id)
                .fetch_one(&pool.0)
                .await;
            match attempt {
                Ok(user) => Ok(user),
                Err(_) => Err(Error::Database(
                    "Failed to insert user into database".into(),
                )),
            }
        }

        async fn update(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
            todo!()
        }

        async fn delete(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
            todo!()
        }
    }

    impl AuthUser for User {
        type Id = u32;

        fn id(&self) -> Self::Id {
            match &self.id {
                Some(a) => a.0 as u32,
                None => 0,
            }
        }

        fn session_auth_hash(&self) -> &[u8] {
            &self.pw_hash.as_bytes()
        }
    }
}

mod control {
    use axum::{
        extract::State,
        http::StatusCode,
        routing::{get, post},
        Form, Router,
    };
    use axum_login::{AuthSession, AuthUser};
    use axum::response::{IntoResponse, Redirect, Response};
    use maud::Markup;
    use tracing::{debug, error, info, warn};

    use crate::{
        appstate::AppState,
        controller::RouteProvider,
        model::database::{Database, DatabaseComponent},
        views::utils::{default_header, page_not_found, title_and_navbar},
    };

    use super::{
        Credential, SignupUser, User,
        view::{email_form_html, login_page, signup_failure, signup_page, signup_success},
    };

    impl RouteProvider for User {
        fn provide_routes(router: Router<AppState>) -> Router<AppState> {
            router
                .route("/signup", get(User::signup_page).post(User::signup_request))
                .route("/signup/email", post(User::email_validation))
                .route("/login", get(User::login_page).post(User::login_request))
                .route("/logout", post(User::logout_request))
                .route("/users", get(User::user_list))
                .route("/me", get(User::me_page))
        }
    }

    impl User {
        pub async fn signup_page(auth: AuthSession<Database>) -> (StatusCode, Markup) {
            let is_auth = auth.user.is_some();
            (StatusCode::OK, signup_page(is_auth).await)
        }

        pub async fn signup_request(
            mut auth: AuthSession<Database>,
            State(state): State<AppState>,
            Form(payload): Form<SignupUser>,
        ) -> Response {
            // Normalize and validate
            let email = payload.email.trim().to_lowercase();
            let name = payload.name.trim().to_string();
            let pw_len = payload.password.len();
            info!(target: "user.signup", %email, %name, pw_len, "signup request received");
            if email.is_empty() || name.is_empty() || pw_len < 8 {
                warn!(target: "user.signup", %email, %name, pw_len, reason = "invalid_input", "signup rejected");
                return (StatusCode::BAD_REQUEST, signup_failure().await).into_response();
            }

            // Prevent duplicate accounts
            match User::exists_by_email(&state.pool, &email).await {
                Ok(true) => {
                    warn!(target: "user.signup", %email, %name, reason = "duplicate_email", "signup rejected");
                    return (StatusCode::CONFLICT, signup_failure().await).into_response();
                }
                Ok(false) => debug!(target: "user.signup", %email, %name, "email available"),
                Err(err) => {
                    error!(target: "user.signup", %email, %name, ?err, reason = "exists_check_failed", "signup failed at duplicate check");
                    return (StatusCode::INTERNAL_SERVER_ERROR, signup_failure().await).into_response();
                }
            }

            let pw_hash = password_auth::generate_hash(&payload.password);
            let user = User::new(&name, &email, &pw_hash);
            debug!(target: "user.signup", user = ?user, "creating user");
            let insert_result = state.pool.create(user).await;
            debug!(target: "user.signup", res = ?insert_result, "insert result");
            match insert_result {
                Ok(_) => {
                    // Load full user (with id) and establish session
                    match User::from_email(email.clone(), &state.pool).await {
                        Ok(user) => {
                            if let Err(err) = auth.login(&user).await {
                                error!(target: "user.signup", %email, %name, ?err, reason = "login_failed", "auto-login failed after signup");
                                return (StatusCode::INTERNAL_SERVER_ERROR, signup_failure().await).into_response();
                            }
                            info!(target: "user.signup", %email, %name, "signup success, redirecting to /me");
                            return Redirect::to("/me").into_response();
                        }
                        Err(err) => {
                            error!(target: "user.signup", %email, %name, ?err, reason = "lookup_failed", "failed to load user after signup");
                            return (StatusCode::INTERNAL_SERVER_ERROR, signup_failure().await).into_response();
                        }
                    }
                }
                Err(err) => {
                    error!(target: "user.signup", %email, %name, ?err, reason = "db_insert_failed", "signup failed");
                    (StatusCode::CONFLICT, signup_failure().await).into_response()
                }
            }
        }

        pub async fn email_validation(
            State(state): State<AppState>,
            Form(payload): Form<SignupUser>,
        ) -> (StatusCode, Markup) {
            // Actually a hard problem, can be better solved(see: https://david-gilbertson.medium.com/the-100-correct-way-to-validate-email-addresses-7c4818f24643)
            // but for now
            // check there exits an @
            let mut valid = payload.email.contains('@');

            // Check text is either side of the email
            let email = payload.email.trim().to_lowercase();
            let results = email.split('@').collect::<Vec<&str>>();
            let mut res_iter = results.iter();
            valid &= match res_iter.next() {
                Some(a) => !a.is_empty(),
                None => false,
            };
            valid &= match res_iter.next() {
                Some(a) => !a.is_empty(),
                None => false,
            };

            // Duplicate check against DB
            let mut duplicate = false;
            if valid {
                match User::exists_by_email(&state.pool, &email).await {
                    Ok(true) => { duplicate = true; valid = false; }
                    Ok(false) => {}
                    Err(err) => warn!(target: "user.signup", %email, ?err, reason = "exists_check_failed", "email validation fallback to format only"),
                }
            }
            info!(target: "user.signup", %email, valid_format = (results.len() == 2), duplicate, final_valid = valid, "email validation");

            (StatusCode::OK, email_form_html(valid, &email))
        }

        // Login
        pub async fn login_page(auth: AuthSession<Database>) -> (StatusCode, Markup) {
            let is_auth = auth.user.is_some();
            (StatusCode::OK, login_page(is_auth, true, "", None).await)
        }

        pub async fn login_request(
            mut auth: AuthSession<Database>,
            Form(payload): Form<Credential>,
        ) -> Response {
            let email = payload.email.clone();
            match auth.authenticate(payload).await {
                Ok(Some(user)) => {
                    if let Err(err) = auth.login(&user).await {
                        tracing::error!(?err, "failed to establish session");
                        return (StatusCode::INTERNAL_SERVER_ERROR, page_not_found()).into_response();
                    }
                    Redirect::to("/me").into_response()
                }
                Ok(None) => (
                    StatusCode::UNAUTHORIZED,
                    login_page(false, false, &email, Some("Invalid email or password")).await,
                )
                    .into_response(),
                Err(err) => {
                    tracing::error!(?err, "authentication error");
                    (StatusCode::INTERNAL_SERVER_ERROR, page_not_found()).into_response()
                }
            }
        }

        pub async fn logout_request(mut auth: AuthSession<Database>) -> StatusCode {
            if let Err(err) = auth.logout().await {
                tracing::warn!(?err, "logout failed");
            }
            StatusCode::NO_CONTENT
        }

        pub async fn user_list(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
        ) -> (StatusCode, Markup) {
            if auth.user.as_ref().is_none() {
                return (StatusCode::UNAUTHORIZED, login_page(false, true, "", None).await);
            }
            let contents = maud::html! {
                (default_header("Pallet Spaces: Users"))
                (title_and_navbar(true))
                body class="page" {
                    div class="container" {
                        h2 { "Users" }
                        ol {
                            @for user in User::get_all_users(&state.pool).await {
                                li { (format!("{} <{}>", user.name, user.email)) }
                            }
                        }
                    }
                }
            };
            (StatusCode::OK, contents)
        }

        pub async fn me_page(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
        ) -> (StatusCode, Markup) {
            if let Some(user) = auth.user.clone() {
                let posts = crate::plugins::posts::Post::get_posts_by_user(&state.pool, user.id() as i64).await;
                let body = maud::html! {
                    (default_header("Pallet Spaces: My Account"))
                    (title_and_navbar(true))
                    body class="page" {
                        div class="container stack" {
                            h2 { "My Account" }
                            p { (format!("Name: {}", user.name)) }
                            p { (format!("Email: {}", user.email)) }
                            h3 { "My Posts" }
                            @if posts.is_empty() {
                                p class="text-muted" { "You have not created any posts yet." }
                            } @else {
                                div class="list" {
                                    @for p in posts {
                                        div class="card" {
                                            @match p.id_raw() {
                                                Some(id) => h3 { a href=(format!("/posts/{}", id)) { (p.title.clone()) } }
                                                None => h3 { (p.title.clone()) }
                                            }
                                            p class="text-muted" { (p.location) " — " (p.price) " /week" }
                                            @if p.visible == 0 { span class="badge badge--hidden" { "(hidden)" } }
                                            @match p.id_raw() {
                                                Some(id) => div class="cluster mt-2" {
                                                    a class="btn btn--secondary" href=(format!("/posts/{}/edit", id)) { "Edit" }
                                                    form method="POST" action=(format!("/posts/{}/toggle_visibility", id)) {
                                                        @let is_hidden = p.visible == 0;
                                                        button class="btn btn--ghost" type="submit" { (if is_hidden { "Show" } else { "Hide" }) }
                                                    }
                                                    form method="POST" action=(format!("/posts/{}/delete", id)) onsubmit="return confirm('Delete this post?');" {
                                                        button class="btn btn--danger" type="submit" { "Delete" }
                                                    }
                                                }
                                                None => {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                };
                (StatusCode::OK, body)
            } else {
                (StatusCode::UNAUTHORIZED, login_page(false, true, "", None).await)
            }
        }
    }
}

mod view {
    use maud::{Markup, html};

    use crate::views::utils::{default_header, title_and_navbar};

    pub async fn signup_page(is_auth: bool) -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            (title_and_navbar(is_auth))
            body class="page" {
                form class="container card form" id="signupForm" action="signup" method="POST" hx-post="/signup" {
                    (email_form_html(true, ""))
                    div class="field" { label class="label" for="name" { "Fullname:" } input class="input" type="text" id="name" name="name" {} }
                    div class="field" { label class="label" for="password" { "Password:" } input class="input" type="password" id="password" name="password" minlength="8" required {} }
                    div { button class="btn btn--primary" type="submit" { "Submit" } }
                }
            }
        }
    }

    pub fn email_form_html(valid: bool, email: &str) -> Markup {
        html! {
            div class="field" hx-target="this" hx-swap="outerHTML" {
                label class="label" for="email" { "E-mail:" }
                input class="input" type="text" id="email" name="email" hx-post="/signup/email" value=(email) aria-invalid=(!valid) {}
                @if !valid { p class="help" { "Please enter a valid, unused email." } }
            }
        }
    }

    pub async fn signup_success() -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            body class="page" {
                div class="container card" {
                    h2 { "Thanks for signing up" }
                    p class="text-muted" { "We\'ll be in touch soon if there\'s enough interest." }
                }
            }
        }
    }

    pub async fn signup_failure() -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            body class="page" {
                div class="container card" {
                    h2 { "Attempted signup failed" }
                    p class="text-muted" { "Please try again" }
                }
            }
        }
    }

    pub async fn login_page(is_auth: bool, valid_email: bool, email: &str, warn: Option<&str>) -> Markup {
        html! {
            (default_header("Pallet Spaces: Login"))
            (title_and_navbar(is_auth))
            body class="page" {
                @if let Some(msg) = warn { div class="container card" { p class="error" { (msg) } } }
                (login_form(valid_email, email).await)
            }
        }
    }

    pub async fn login_form(valid_email: bool, email: &str) -> Markup {
        html! {
            form class="container card form" id="loginForm" action="login" method="POST" hx-post="/login" {
                (email_form_html(valid_email, email))
                div class="field" { label class="label" for="password" { "Password:" } input class="input" type="password" id="password" name="password" required {} }
                div { button class="btn btn--primary" type="submit" { "Submit" } }
            }
        }
    }
}
