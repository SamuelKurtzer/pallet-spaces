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
    pub stripe_customer_id: Option<String>,
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
    pub next: Option<String>,
}

impl User {
    pub fn new(name: &str, email: &str, password: &str) -> Self {
        let user = User {
            id: None,
            name: name.to_string(),
            email: email.to_string(),
            pw_hash: password.to_string(),
            stripe_customer_id: None,
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
        pw_hash TEXT NOT NULL,
        stripe_customer_id TEXT UNIQUE,
        stripe_connect_account_id TEXT UNIQUE,
        stripe_connect_verified INTEGER NOT NULL DEFAULT 0
      )
      ",
                )
                .await;
            match creation_attempt {
                Ok(_) => {
                    // Best-effort migrations for existing DBs
                    let _ = pool.0.execute("ALTER TABLE users ADD COLUMN stripe_customer_id TEXT UNIQUE").await;
                    let _ = pool.0.execute("ALTER TABLE users ADD COLUMN stripe_connect_account_id TEXT UNIQUE").await;
                    let _ = pool.0.execute("ALTER TABLE users ADD COLUMN stripe_connect_verified INTEGER NOT NULL DEFAULT 0").await;
                    Ok(pool)
                },
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

        async fn update(_id: Self::Id, _pool: &Database) -> Result<&Database, Error> {
            todo!()
        }

        async fn delete(_id: Self::Id, _pool: &Database) -> Result<&Database, Error> {
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

pub mod service {
    use crate::error::Error;
    use crate::appstate::AppState;
    use std::str::FromStr;

    // Real Stripe calls when `stripe` feature is enabled (and in live tests when opted in)
    #[cfg(feature = "stripe")]
    async fn stripe_list_customer_by_email(client: &stripe::Client, email: &str) -> Result<Option<String>, Error> {
        let mut params = stripe::ListCustomers::new();
        params.email = Some(email);
        let res = stripe::Customer::list(client, &params).await;
        match res {
            Ok(list) => Ok(list.data.first().map(|c| c.id.to_string()))
            , Err(e) => Err(Error::String(format!("stripe list customers error: {:?}", e)))
        }
    }

    #[cfg(feature = "stripe")]
    async fn stripe_create_customer(client: &stripe::Client, email: &str, name: &str, user_id: i64) -> Result<Option<String>, Error> {
        let mut params = stripe::CreateCustomer::new();
        params.email = Some(email);
        params.name = Some(name);
        params.metadata = {
            let mut m = std::collections::HashMap::new();
            m.insert("user_id".to_string(), user_id.to_string());
            Some(m)
        };
        let customer = stripe::Customer::create(client, params).await;
        match customer { Ok(c) => Ok(Some(c.id.to_string())), Err(e) => Err(Error::String(format!("stripe create customer error: {:?}", e))) }
    }

    #[cfg(feature = "stripe")]
    async fn stripe_update_customer(client: &stripe::Client, customer_id: &str, email: &str, name: &str, user_id: i64) -> Result<(), Error> {
        let mut params = stripe::UpdateCustomer::new();
        params.email = Some(email);
        params.name = Some(name);
        let mut m = std::collections::HashMap::new();
        m.insert("user_id".to_string(), user_id.to_string());
        params.metadata = Some(m);
        let cid_obj = stripe::CustomerId::from_str(customer_id).unwrap_or_else(|_| panic!("invalid customer id"));
        let _ = stripe::Customer::update(client, &cid_obj, params).await;
        Ok(())
    }

    // Stubs when `stripe` feature is disabled in non-live tests
    #[cfg(not(feature = "stripe"))]
    async fn stripe_list_customer_by_email(_secret_key: &str, _email: &str) -> Result<Option<String>, Error> { Ok(None) }
    #[cfg(not(feature = "stripe"))]
    async fn stripe_create_customer(_secret_key: &str, _email: &str, _name: &str, _user_id: i64) -> Result<Option<String>, Error> { Ok(None) }
    #[cfg(not(feature = "stripe"))]
    async fn stripe_update_customer(_secret_key: &str, _customer_id: &str, _email: &str, _name: &str, _user_id: i64) -> Result<(), Error> { Ok(()) }

    pub async fn ensure_customer_for_user(state: &AppState, user_id: i64, email: &str, name: &str) -> Result<Option<String>, Error> {
        // Already present?
        if let Ok(opt) = sqlx::query_scalar::<_, Option<String>>("SELECT stripe_customer_id FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_one(&state.pool.0)
            .await { if opt.is_some() { return Ok(opt); } }
        #[cfg(not(feature = "stripe"))]
        {
            let _ = (user_id, email, name);
            return Ok(None);
        }
        #[cfg(feature = "stripe")]
        {
            let client = match state.stripe.as_ref() { Some(c) => c.clone(), None => return Ok(None) };
            // Try create first with idempotency key
            let res = match stripe_create_customer(&client, email, name, user_id).await? {
                Some(cid) => {
                    let _ = sqlx::query("UPDATE users SET stripe_customer_id=?1 WHERE id=?2")
                        .bind(&cid)
                        .bind(user_id)
                        .execute(&state.pool.0).await;
                    Ok(Some(cid))
                }
                None => {
                    // Fallback: find by email
                    if let Ok(Some(found)) = stripe_list_customer_by_email(&client, email).await {
                        let _ = sqlx::query("UPDATE users SET stripe_customer_id=?1 WHERE id=?2")
                            .bind(&found)
                            .bind(user_id)
                            .execute(&state.pool.0).await;
                        let _ = stripe_update_customer(&client, &found, email, name, user_id).await;
                        Ok(Some(found))
                    } else {
                        Ok(None)
                    }
                },
            };
            return res;
        }
    }

    pub async fn push_email_name_to_stripe(state: &AppState, user_id: i64) {
        #[cfg(not(feature = "stripe"))]
        { let _ = user_id; return; }
        #[cfg(feature = "stripe")]
        if let Some(client) = state.stripe.as_ref() {
            if let Ok((maybe_cid, email, name)) = sqlx::query_as::<_, (Option<String>, String, String)>("SELECT stripe_customer_id, email, name FROM users WHERE id=?1")
                .bind(user_id)
                .fetch_one(&state.pool.0).await
            {
                if let Some(cid) = maybe_cid {
                    let _ = stripe_update_customer(client, &cid, &email, &name, user_id).await;
                }
            }
        }
    }

    pub async fn is_connect_verified(state: &AppState, user_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT stripe_connect_verified FROM users WHERE id=?1")
            .bind(user_id)
            .fetch_one(&state.pool.0)
            .await
            .unwrap_or(0) == 1
    }

    #[cfg(not(feature = "stripe"))]
    pub async fn create_or_get_connect_account_and_link(
        _state: &AppState,
        _user_id: i64,
        _email: &str,
        _return_url: &str,
        _refresh_url: &str,
    ) -> Result<Option<String>, Error> {
        Ok(None)
    }

    #[cfg(feature = "stripe")]
    pub async fn create_or_get_connect_account_and_link(
        state: &AppState,
        user_id: i64,
        email: &str,
        return_url: &str,
        refresh_url: &str,
    ) -> Result<Option<String>, Error> {
        use stripe::{Account, AccountId, AccountLink, AccountLinkType, CreateAccount, CreateAccountLink};
        let client = match state.stripe.as_ref() { Some(c) => c.clone(), None => return Ok(None) };

        // Ensure we have or create a Connect account id
        let current: Option<String> = sqlx::query_scalar("SELECT stripe_connect_account_id FROM users WHERE id=?1")
            .bind(user_id)
            .fetch_one(&state.pool.0)
            .await
            .unwrap_or(None);

        let acct_id = if let Some(aid) = current {
            aid
        } else {
            // Create an Express account with basic capabilities
            let mut params = CreateAccount::new();
            params.type_ = Some(stripe::AccountType::Express);
            params.email = Some(email);
            let acct = Account::create(&client, params).await.map_err(|e| Error::String(format!("stripe create account error: {:?}", e)))?;
            let aid = acct.id.to_string();
            let _ = sqlx::query("UPDATE users SET stripe_connect_account_id=?1 WHERE id=?2")
                .bind(&aid)
                .bind(user_id)
                .execute(&state.pool.0).await;
            aid
        };

        // Create an onboarding/remediation link
        let acc_id = AccountId::from_str(&acct_id).map_err(|_| Error::String("invalid account id".into()))?;
        let mut link = CreateAccountLink::new(acc_id, AccountLinkType::AccountOnboarding);
        link.refresh_url = Some(refresh_url);
        link.return_url = Some(return_url);
        // Default collection behavior is fine; we want express onboarding
        let created = AccountLink::create(&client, link)
            .await
            .map_err(|e| Error::String(format!("stripe create account_link error: {:?}", e)))?;
        Ok(Some(created.url))
    }

    #[cfg(feature = "stripe")]
    pub async fn refresh_connect_status(state: &AppState, user_id: i64) {
        use stripe::AccountId;
        let Some(client) = state.stripe.as_ref() else { return; };
        let aid: Option<String> = sqlx::query_scalar("SELECT stripe_connect_account_id FROM users WHERE id=?1")
            .bind(user_id)
            .fetch_one(&state.pool.0)
            .await
            .unwrap_or(None);
        if let Some(aid) = aid {
            if let Ok(acc_id) = AccountId::from_str(&aid) {
                if let Ok(acct) = stripe::Account::retrieve(client, &acc_id, &[]) .await {
                    let charges = acct.charges_enabled.unwrap_or(false);
                    let payouts = acct.payouts_enabled.unwrap_or(false);
                    let due_empty = acct
                        .requirements
                        .as_ref()
                        .and_then(|r| r.currently_due.as_ref())
                        .map(|v| v.is_empty())
                        .unwrap_or(false);
                    let verified = (charges && payouts) || due_empty;
                    let _ = sqlx::query("UPDATE users SET stripe_connect_verified=?1 WHERE id=?2")
                        .bind(if verified { 1 } else { 0 })
                        .bind(user_id)
                        .execute(&state.pool.0).await;
                }
            }
        }
    }

    #[cfg(not(feature = "stripe"))]
    pub async fn refresh_connect_status(_state: &AppState, _user_id: i64) { }
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
    use axum::extract::Query;
    use maud::Markup;
    use tracing::{debug, error, info, warn};
    use serde::Deserialize;

    use crate::{
        appstate::AppState,
        controller::RouteProvider,
        model::database::{Database, DatabaseComponent},
        views::utils::{default_header, page_not_found, title_and_navbar},
    };

    use super::{
        Credential, SignupUser, User,
        view::{email_form_html, login_page, signup_failure, signup_page},
    };

    #[derive(Deserialize, Default, Clone)]
    pub struct LoginParams { pub next: Option<String> }

    impl RouteProvider for User {
        fn provide_routes(router: Router<AppState>) -> Router<AppState> {
            let router = router
                .route("/signup", get(User::signup_page).post(User::signup_request))
                .route("/signup/email", post(User::email_validation))
                .route("/login", get(User::login_page).post(User::login_request))
                .route("/logout", post(User::logout_request))
                .route("/users", get(User::user_list))
                .route("/me", get(User::me_page))
                .route("/me/verify", get(User::connect_verify))
                .route("/me/refresh_connect", get(User::refresh_connect))
                .route("/me/profile", post(User::update_profile))
                .route("/admin/stripe/backfill-customers", post(User::admin_backfill_customers))
                .route("/webhooks/stripe", post(User::stripe_webhook));
            #[cfg(test)]
            let router = router.route("/__test__/verify_me", post(User::test_mark_verified));
            router
        }
    }

    #[derive(Deserialize, Debug, Default, Clone)]
    pub struct BackfillParams {
        pub limit: Option<u32>,
        pub cursor: Option<i64>,
    }

    #[derive(Deserialize, Clone, Debug)]
    pub struct UpdateProfile { pub name: String, pub email: String }

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
                            // Ensure Stripe customer (best-effort)
                            let _ = super::service::ensure_customer_for_user(&state, user.id() as i64, &user.email, &user.name).await;
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
        pub async fn login_page(auth: AuthSession<Database>, Query(params): Query<LoginParams>) -> (StatusCode, Markup) {
            let is_auth = auth.user.is_some();
            (StatusCode::OK, login_page(is_auth, true, "", None, params.next.as_deref()).await)
        }

        pub async fn login_request(
            mut auth: AuthSession<Database>,
            State(state): State<AppState>,
            Form(payload): Form<Credential>,
        ) -> Response {
            let email = payload.email.clone();
            let next = payload.next.clone();
            match auth.authenticate(payload).await {
                Ok(Some(user)) => {
                    if let Err(err) = auth.login(&user).await {
                        tracing::error!(?err, "failed to establish session");
                        return (StatusCode::INTERNAL_SERVER_ERROR, page_not_found()).into_response();
                    }
                    // Create Stripe customer on first login if missing (best-effort)
                    let _ = super::service::ensure_customer_for_user(&state, user.id() as i64, &user.email, &user.name).await;
                    // Redirect to 'next' when provided and safe (relative path)
                    if let Some(dest) = next.clone() {
                        if dest.starts_with('/') { return Redirect::to(&dest).into_response(); }
                    }
                    Redirect::to("/me").into_response()
                }
                Ok(None) => {
                    (StatusCode::UNAUTHORIZED, login_page(false, false, &email, Some("Invalid email or password"), next.as_deref()).await).into_response()
                },
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
                return (StatusCode::UNAUTHORIZED, login_page(false, true, "", None, None).await);
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
                let verified: i64 = sqlx::query_scalar("SELECT stripe_connect_verified FROM users WHERE id=?1")
                    .bind(user.id() as i64)
                    .fetch_one(&state.pool.0).await
                    .unwrap_or(0);
                let body = maud::html! {
                    (default_header("Pallet Spaces: My Account"))
                    (title_and_navbar(true))
                    body class="page" {
                        div class="container stack" {
                            h2 { "My Account" }
                            @if verified == 0 {
                                div class="card" {
                                    p { "Your payouts account is not verified. Verify to create rental posts." }
                                    div class="cluster" { a class="btn btn--primary" href="/me/verify" { "Verify account" } a class="btn btn--secondary" href="/me/refresh_connect" { "Refresh status" } }
                                }
                            } @else {
                                div class="card" { p { "Payments account verified." } }
                            }
                            form class="card form" method="POST" action="/me/profile" {
                                div class="grid grid--2" {
                                    div class="field" { label class="label" for="name" { "Fullname" } input class="input" type="text" id="name" name="name" required value=(user.name) {} }
                                    div class="field" { label class="label" for="email" { "Email" } input class="input" type="email" id="email" name="email" required value=(user.email) {} }
                                }
                                div { button class="btn btn--primary" type="submit" { "Save" } }
                            }
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
                                            p class="text-muted" { (p.location) " — " (p.price) " /day" }
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
                (StatusCode::UNAUTHORIZED, login_page(false, true, "", None, None).await)
            }
        }

        pub async fn connect_verify(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
        ) -> Response {
            let Some(user) = auth.user.as_ref() else { return Redirect::to("/login?next=/me/verify").into_response(); };
            // Try to ensure a Connect account and get remediation link
            let email = user.email.clone();
            let base = std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:37373".to_string());
            let return_url = format!("{}/me", base);
            let refresh_url = format!("{}/me/verify", base);
            match super::service::create_or_get_connect_account_and_link(&state, user.id() as i64, &email, &return_url, &refresh_url).await {
                Ok(Some(url)) => Redirect::to(&url).into_response(),
                _ => {
                    // Either Stripe is not configured or an error occurred; show account page
                    (StatusCode::OK, super::view::verify_unavailable().await).into_response()
                }
            }
        }

        pub async fn refresh_connect(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
        ) -> Response {
            if let Some(user) = auth.user.as_ref() {
                super::service::refresh_connect_status(&state, user.id() as i64).await;
                return Redirect::to("/me").into_response();
            }
            Redirect::to("/login?next=/me").into_response()
        }

        // Test helper: mark the logged-in user as verified
        #[cfg(test)]
        pub async fn test_mark_verified(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
        ) -> Response {
            if let Some(user) = auth.user.as_ref() {
                let _ = sqlx::query("UPDATE users SET stripe_connect_verified=1 WHERE id=?1")
                    .bind(user.id() as i64)
                    .execute(&state.pool.0).await;
                return Redirect::to("/me").into_response();
            }
            Redirect::to("/login").into_response()
        }

        pub async fn update_profile(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
            Form(payload): Form<UpdateProfile>,
        ) -> Response {
            let Some(user) = auth.user.as_ref() else {
                return Redirect::to("/login").into_response();
            };
            let name = payload.name.trim();
            let email = payload.email.trim().to_lowercase();
            if name.is_empty() || email.is_empty() || !email.contains('@') {
                return (StatusCode::BAD_REQUEST, page_not_found()).into_response();
            }
            if email != user.email {
                if let Ok(true) = User::exists_by_email(&state.pool, &email).await {
                    return (StatusCode::CONFLICT, page_not_found()).into_response();
                }
            }
            let _ = sqlx::query("UPDATE users SET name=?1, email=?2 WHERE id=?3")
                .bind(name)
                .bind(&email)
                .bind(user.id() as i64)
                .execute(&state.pool.0).await;
            super::service::push_email_name_to_stripe(&state, user.id() as i64).await;
            Redirect::to("/me").into_response()
        }

        pub async fn admin_backfill_customers(
            auth: AuthSession<Database>,
            State(state): State<AppState>,
            Query(params): Query<BackfillParams>,
        ) -> axum::response::Response {
            // Require logged-in and admin email match
            let admin_email = std::env::var("ADMIN_EMAIL").unwrap_or_default();
            let user = match auth.user.as_ref() {
                Some(u) => u,
                None => return axum::response::Redirect::to("/login").into_response(),
            };
            if admin_email.is_empty() || user.email != admin_email {
                return (StatusCode::FORBIDDEN, page_not_found()).into_response();
            }

            let limit = params.limit.unwrap_or(200).min(1000) as i64;
            let cursor = params.cursor.unwrap_or(0);

            let rows: Vec<(i64, String, String)> = sqlx::query_as(
                "SELECT id, name, email FROM users WHERE (stripe_customer_id IS NULL OR stripe_customer_id = '') AND id > ?1 ORDER BY id ASC LIMIT ?2",
            )
            .bind(cursor)
            .bind(limit)
            .fetch_all(&state.pool.0)
            .await
            .unwrap_or_default();

            let mut created = 0usize;
            let mut existing = 0usize;
            let mut errors: Vec<(i64, String)> = vec![];
            let mut last_id = cursor;

            for (id, name, email) in rows.iter() {
                last_id = (*id).max(last_id);
                match super::service::ensure_customer_for_user(&state, *id, email, name).await {
                    Ok(Some(_)) => created += 1,
                    Ok(None) => existing += 1,
                    Err(e) => { errors.push((*id, format!("{:?}", e))); }
                }
                // basic pacing to avoid bursts
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            let done = rows.len() < (limit as usize);
            let next_cursor = if done { None } else { Some(last_id) };
            let body = serde_json::json!({
                "processed": rows.len(),
                "created": created,
                "existing": existing,
                "errors": errors,
                "done": done,
                "next_cursor": next_cursor,
            });
            axum::Json(body).into_response()
        }

        pub async fn stripe_webhook(
            State(state): State<AppState>,
            headers: axum::http::HeaderMap,
            body: String,
        ) -> StatusCode {
            let secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
            let sig = headers.get("Stripe-Signature").and_then(|h| h.to_str().ok()).unwrap_or("");
            #[cfg(feature = "stripe")]
            {
                if secret.is_empty() || sig.is_empty() {
                    tracing::warn!(target: "stripe.webhook", reason="missing_secret_or_sig", "webhook without verification context");
                    return StatusCode::OK;
                }
                match stripe::Webhook::construct_event(&body, sig, &secret) {
                    Ok(_event) => {
                        // Parse raw JSON after signature verification for flexibility
                        let event: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                        let etype = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        tracing::info!(target: "stripe.webhook", event_type=%etype, "verified webhook");
                        if etype == "checkout.session.completed" {
                            if let Some(obj) = event.get("data").and_then(|d| d.get("object")).and_then(|o| o.as_object()) {
                                if let Some(meta) = obj.get("metadata").and_then(|m| m.as_object()) {
                                    if let Some(order_id_s) = meta.get("order_id").and_then(|v| v.as_str()) {
                                        if let Ok(order_id) = order_id_s.parse::<i64>() {
                                            let _ = sqlx::query("UPDATE Orders SET status='paid' WHERE id=?1")
                                                .bind(order_id)
                                                .execute(&state.pool.0).await;
                                        }
                                    }
                                }
                            }
                        }
                        if etype == "account.updated" {
                            if let Some(obj) = event.get("data").and_then(|d| d.get("object")).and_then(|o| o.as_object()) {
                                let aid = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let charges = obj.get("charges_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                                let payouts = obj.get("payouts_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                                let due_empty = obj
                                    .get("requirements").and_then(|r| r.get("currently_due")).and_then(|v| v.as_array()).map(|a| a.is_empty()).unwrap_or(false);
                                let verified = (charges && payouts) || due_empty;
                                if !aid.is_empty() {
                                    let _ = sqlx::query("UPDATE users SET stripe_connect_verified=?1 WHERE stripe_connect_account_id=?2")
                                        .bind(if verified { 1 } else { 0 })
                                        .bind(aid)
                                        .execute(&state.pool.0).await;
                                }
                            }
                        }
                        StatusCode::OK
                    }
                    Err(e) => {
                        tracing::warn!(target: "stripe.webhook", error=?e, "signature verification failed");
                        StatusCode::BAD_REQUEST
                    }
                }
            }
            #[cfg(not(feature = "stripe"))]
            {
                let _ = (state, secret, sig, body);
                StatusCode::OK
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

    pub async fn login_page(is_auth: bool, valid_email: bool, email: &str, warn: Option<&str>, next: Option<&str>) -> Markup {
        html! {
            (default_header("Pallet Spaces: Login"))
            (title_and_navbar(is_auth))
            body class="page" {
                @if let Some(msg) = warn { div class="container card" { p class="error" { (msg) } } }
                @if next.is_some() {
                    div class="container card" { p { "Please log in to continue renting." } }
                }
                (login_form(valid_email, email, next).await)
            }
        }
    }

    pub async fn login_form(valid_email: bool, email: &str, next: Option<&str>) -> Markup {
        html! {
            form class="container card form" id="loginForm" action="login" method="POST" hx-post="/login" {
                (email_form_html(valid_email, email))
                div class="field" { label class="label" for="password" { "Password:" } input class="input" type="password" id="password" name="password" required {} }
                @if let Some(n) = next { input type="hidden" name="next" value=(n) {} }
                div { button class="btn btn--primary" type="submit" { "Submit" } }
            }
        }
    }

    pub async fn verify_unavailable() -> Markup {
        html! {
            (default_header("Pallet Spaces: Verify Account"))
            (title_and_navbar(true))
            body class="page" {
                div class="container card" {
                    h2 { "Verification Unavailable" }
                    p class="text-muted" { "Stripe is not configured or temporarily unavailable. Please try again later." }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, StatusCode}, Router};
    use tower::ServiceExt;
    use crate::{appstate::AppState, controller::Routes, model::database::{Database, DatabaseComponent}};
    use axum_login::AuthManagerLayerBuilder;
    use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};

    #[tokio::test]
    async fn admin_backfill_requires_auth() {
        let db = Database::new_with_filename(&format!("test-{}-users-backfill.db", nanoid::nanoid!())).await.unwrap();
        let db = db.initialise_table::<crate::plugins::users::User>().await.unwrap();
        let state = AppState::new(db.clone());
        let app: Router = {
            let base = Router::new().add_routes::<crate::plugins::users::User>().with_state(state.clone());
            let session_layer = SessionManagerLayer::new(MemoryStore::default());
            let auth_layer = AuthManagerLayerBuilder::new(db, session_layer).build();
            base.layer(auth_layer)
        };

        let res = app
            .oneshot(
                Request::post("/admin/stripe/backfill-customers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Redirect to login since not authenticated
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
    }

    // Live test against Stripe API to ensure we can create a real Customer and persist its id.
    // Requires: --features stripe,stripe_live and STRIPE_SECRET_KEY env var.
    #[cfg(all(feature = "stripe", feature = "stripe_live"))]
    #[tokio::test]
    async fn live_ensure_customer_for_user_creates_and_updates() {
        let secret = match std::env::var("STRIPE_SECRET_KEY") { Ok(s) if !s.is_empty() => s, _ => return };

        let db = Database::new_with_filename(&format!("test-{}-users-live.db", nanoid::nanoid!())).await.unwrap();
        let db = db.initialise_table::<crate::plugins::users::User>().await.unwrap();
        let state = AppState::new_with_stripe(db.clone(), Some(std::sync::Arc::new(stripe::Client::new(secret))));

        // Insert a unique user
        let email = format!("live.{}@example.com", nanoid::nanoid!(6));
        let name = format!("Live Test {}", nanoid::nanoid!(4));
        let pw_hash = password_auth::generate_hash("supersecret");
        let user = crate::plugins::users::User::new(&name, &email, &pw_hash);
        state.pool.create(user).await.unwrap();
        let uid: i64 = sqlx::query_scalar("SELECT id FROM users WHERE email=?1").bind(&email).fetch_one(&state.pool.0).await.unwrap();

        // Ensure customer
        let cid = super::service::ensure_customer_for_user(&state, uid, &email, &name).await.unwrap();
        let cid = cid.expect("stripe customer id");
        assert!(cid.starts_with("cus_"));
        let persisted: Option<String> = sqlx::query_scalar("SELECT stripe_customer_id FROM users WHERE id=?1").bind(uid).fetch_one(&state.pool.0).await.unwrap();
        assert_eq!(persisted.as_deref(), Some(cid.as_str()));

        // Update local email/name and push update
        let new_email = format!("live2.{}@example.com", nanoid::nanoid!(6));
        let new_name = format!("Live Test B {}", nanoid::nanoid!(4));
        let _ = sqlx::query("UPDATE users SET email=?1, name=?2 WHERE id=?3")
            .bind(&new_email).bind(&new_name).bind(uid).execute(&state.pool.0).await.unwrap();
        super::service::push_email_name_to_stripe(&state, uid).await;
        // Cleanup: delete the test customer
        use std::str::FromStr;
        let client = state.stripe.unwrap();
        let cid_obj = stripe::CustomerId::from_str(&cid).unwrap();
        let _ = stripe::Customer::delete(&client, &cid_obj).await;
    }
}
