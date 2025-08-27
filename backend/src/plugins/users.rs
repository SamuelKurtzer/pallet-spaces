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
            tracing::info!("{}", email);
            let user: User = sqlx::query_as("select * from users where email = ? ")
                .bind(email)
                .fetch_one(&pool.0)
                .await?;
            tracing::debug!("{:?}", user);
            Ok(user)
        }

        pub async fn get_all_users(pool: &Database) -> Vec<User> {
            let mut users = vec![];
            for i in 0..20 {
                if let Ok(user) = User::retrieve(i, pool).await {
                    users.push(user);
                }
            }
            users
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
        Form, Router,
        extract::State,
        http::StatusCode,
        routing::{get, post},
    };
    use maud::Markup;

    use crate::{
        appstate::AppState, controller::RouteProvider, model::database::DatabaseComponent,
        views::utils::page_not_found,
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
}

mod view {
    use maud::{Markup, html};

    use crate::views::utils::{default_header, title_and_navbar};

    pub async fn signup_page() -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            (title_and_navbar())
            body {
                form id="signupForm" action="signup" method="POST" hx-post="/signup" {
                    (email_form_html(true, ""))
                    label for="Fullname" { "Fullname:" }
                    input type="text" id="name" name="name" {}
                    br {}
                    label for="Password" { "Password:" }
                    input type="text" id="password" name="password" {}
                    br {}
                    button type="submit" { "Submit" }
                }
            }
        }
    }

    pub fn email_form_html(valid: bool, email: &str) -> Markup {
        let validation_class = match valid {
            false => "invalid-form-input",
            true => "valid-form-input",
        };
        html! {
            div hx-target="this" hx-swap="outerHTML" {
                label for="email" { "E-mail:" }
                input type="text" id="email" name="email" class=(validation_class) hx-post="/signup/email" value=(email) { }
                br {}
            }
        }
    }

    pub async fn signup_success() -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            body {
                h2 {
                    "Thanks for signing up"
                }
                p {
                    "We'll be in touch soon if theres enough interest"
                }
            }
        }
    }

    pub async fn signup_failure() -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            body {
                h2 {
                    "Attempted signup failed"
                }
                p {
                    "Please try again"
                }
            }
        }
    }

    pub async fn login_page() -> Markup {
        html! {
            (default_header("Pallet Spaces: Login"))
            (title_and_navbar())
            body {
                (login_form().await)
            }
        }
    }

    pub async fn login_form() -> Markup {
        html! {
            form id="loginForm" action="login" method="POST" hx-post="/login" {
                (email_form_html(true, ""))
                label for="Password" { "Password:" }
                input type="text" id="password" name="password" {}
                br {}
                button type="submit" { "Submit" }
            }
        }
    }
}
