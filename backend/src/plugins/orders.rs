use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct OrderID(u64);

impl From<u64> for OrderID {
    fn from(raw: u64) -> Self { OrderID(raw) }
}

#[derive(Clone, FromRow, Serialize, Deserialize, Debug)]
pub struct Order {
    id: Option<OrderID>,
    pub post_id: i64,
    pub renter_user_id: i64,
    pub renter_name: String,
    pub renter_email: String,
    pub quantity: i64,
    pub start_date: String,    // YYYY-MM-DD
    pub end_date: String,      // YYYY-MM-DD
    pub status: String,        // pending|submitted|failed
    pub shopify_order_id: Option<String>,
    pub stripe_session_id: Option<String>,
    pub stripe_checkout_url: Option<String>,
}

impl Order {
    #[allow(dead_code)]
    pub fn new(
        post_id: i64,
        renter_user_id: i64,
        renter_name: &str,
        renter_email: &str,
        quantity: i64,
        start_date: &str,
        end_date: &str,
    ) -> Self {
        Self {
            id: None,
            post_id,
            renter_user_id,
            renter_name: renter_name.to_string(),
            renter_email: renter_email.to_string(),
            quantity,
            start_date: start_date.to_string(),
            end_date: end_date.to_string(),
            status: "pending".to_string(),
            shopify_order_id: None,
            stripe_session_id: None,
            stripe_checkout_url: None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct NewOrder {
    pub quantity: i64,
    pub start_date: String,
    pub end_date: String,
}

mod model {
    use crate::{
        error::Error,
        model::database::{Database, DatabaseProvider},
    };
    use sqlx::Executor;

    use super::Order;

    impl DatabaseProvider for Order {
        type Database = Database;
        type Id = u32;
        async fn initialise_table(pool: Database) -> Result<Database, Error> {
            let creation_attempt = &pool
                .0
                .execute(
                    "
      CREATE TABLE if not exists Orders (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        post_id INTEGER NOT NULL,
        renter_user_id INTEGER NOT NULL DEFAULT 0,
        renter_name TEXT NOT NULL,
        renter_email TEXT NOT NULL,
        quantity INTEGER NOT NULL,
        start_date TEXT NOT NULL,
        end_date TEXT NOT NULL,
        status TEXT NOT NULL,
        shopify_order_id TEXT,
        stripe_session_id TEXT,
        stripe_checkout_url TEXT
      )
      ",
                )
                .await;
            match creation_attempt {
                Ok(_) => {
                    // Best-effort migrations for existing DBs
                    let _ = pool.0.execute("ALTER TABLE Orders ADD COLUMN renter_user_id INTEGER NOT NULL DEFAULT 0").await;
                    let _ = pool.0.execute("ALTER TABLE Orders ADD COLUMN stripe_session_id TEXT").await;
                    let _ = pool.0.execute("ALTER TABLE Orders ADD COLUMN stripe_checkout_url TEXT").await;
                    let _ = pool.0.execute("ALTER TABLE Orders ADD COLUMN end_date TEXT NOT NULL DEFAULT ''").await;
                    Ok(pool)
                },
                Err(_) => Err(Error::Database(
                    "Failed to create Orders database tables".into(),
                )),
            }
        }

        async fn create(self, pool: &Database) -> Result<&Database, Error> {
            let attempt = sqlx::query(
                "INSERT INTO Orders (
                    post_id, renter_user_id, renter_name, renter_email, quantity, start_date, end_date, status, shopify_order_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(self.post_id)
            .bind(self.renter_user_id)
            .bind(self.renter_name)
            .bind(self.renter_email)
            .bind(self.quantity)
            .bind(self.start_date)
            .bind(self.end_date)
            .bind(self.status)
            .bind(self.shopify_order_id)
            .execute(&pool.0)
            .await;
            match attempt {
                Ok(_) => Ok(pool),
                Err(_) => Err(Error::Database(
                    "Failed to insert Order into database".into(),
                )),
            }
        }

        async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error> {
            let attempt = sqlx::query_as::<_, Order>("SELECT * FROM Orders where id=(?1)")
                .bind(id)
                .fetch_one(&pool.0)
                .await;
            match attempt {
                Ok(order) => Ok(order),
                Err(_) => Err(Error::Database(
                    "Failed to retrieve Order from database".into(),
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
}

mod service {
    // Real HTTP integration behind the `stripe` feature.
    // In tests, enable the real calls only when `stripe_live` is also set.
    #[cfg(any(all(feature = "stripe", not(test)), all(feature = "stripe", feature = "stripe_live", test)))]
    pub async fn submit_stripe_checkout_session(
        client: &stripe::Client,
        title: &str,
        quantity: i64,
        days: i64,
        price_cents_per_day: i64,
        customer_email: &str,
        customer_id: Option<&str>,
        order_id: i64,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<Option<(String, String)>, crate::error::Error> {
        let total_units = quantity.saturating_mul(days.max(1));
        let mut line: stripe::CreateCheckoutSessionLineItems = Default::default();
        let price_data = stripe::CreateCheckoutSessionLineItemsPriceData {
            currency: stripe::Currency::USD,
            product_data: Some(stripe::CreateCheckoutSessionLineItemsPriceDataProductData { name: title.to_string(), ..Default::default() }),
            unit_amount: Some(price_cents_per_day),
            ..Default::default()
        };
        line.price_data = Some(price_data);
        line.quantity = Some(total_units.try_into().unwrap_or(0));
        let mut params = stripe::CreateCheckoutSession::new();
        params.mode = Some(stripe::CheckoutSessionMode::Payment);
        params.success_url = Some(success_url);
        params.cancel_url = Some(cancel_url);
        params.line_items = Some(vec![line]);
        if let Some(cid_str) = customer_id {
            // Try to convert into a typed CustomerId; fall back to email if invalid
            let cid: Result<stripe::CustomerId, _> = cid_str.parse();
            match cid {
                Ok(v) => params.customer = Some(v),
                Err(_) => params.customer_email = Some(customer_email),
            }
        } else {
            params.customer_email = Some(customer_email);
        }
        let mut md = std::collections::HashMap::new();
        md.insert("order_id".to_string(), order_id.to_string());
        params.metadata = Some(md);
        match stripe::CheckoutSession::create(client, params).await {
            Ok(sess) => Ok(Some((sess.id.to_string(), sess.url.unwrap_or_default()))),
            Err(e) => { tracing::warn!(?e, "stripe checkout session failed"); Ok(None) }
        }
    }

    // Test stub when `stripe` feature is enabled without `stripe_live`: avoid network calls.
    #[cfg(all(feature = "stripe", test, not(feature = "stripe_live")))]
    pub async fn submit_stripe_checkout_session(
        _client: &stripe::Client,
        _title: &str,
        _quantity: i64,
        _days: i64,
        _price_cents_per_day: i64,
        _customer_email: &str,
        _customer_id: Option<&str>,
        _order_id: i64,
        _success_url: &str,
        _cancel_url: &str,
    ) -> Result<Option<(String, String)>, crate::error::Error> {
        Ok(Some((
            "cs_test_stub".to_string(),
            std::env::var("STRIPE_TEST_REDIRECT_URL").unwrap_or_else(|_| "https://stripe.test/checkout".to_string()),
        )))
    }

    // Stub for when `stripe` feature is disabled (default).
    #[cfg(not(feature = "stripe"))]
    pub async fn submit_stripe_checkout_session(
        _client: &(),
        title: &str,
        quantity: i64,
        days: i64,
        price_cents_per_day: i64,
        customer_email: &str,
        _customer_id: Option<&str>,
        _order_id: i64,
        _success_url: &str,
        _cancel_url: &str,
    ) -> Result<Option<(String, String)>, crate::error::Error> {
        tracing::info!(target: "orders.stripe", %title, %quantity, days=%days, %price_cents_per_day, %customer_email, "stripe checkout stub invoked (feature disabled)");
        Ok(None)
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode, header::LOCATION}, Router};
    use tower::ServiceExt;
    use crate::{appstate::AppState, controller::Routes, model::database::{Database, DatabaseComponent}};

    // Only runs with `--features stripe`; uses the test stub (no network).
    #[cfg(all(feature = "stripe", not(feature = "stripe_live")))]
    #[tokio::test]
    async fn rent_request_redirects_to_stripe_when_configured() {
        // Prepare isolated test DB
        let db = Database::new_with_filename(&format!("test-{}-redir.db", nanoid::nanoid!())).await.unwrap();
        let db = db.initialise_table::<crate::plugins::users::User>().await.unwrap();
        let db = db.initialise_table::<crate::plugins::posts::Post>().await.unwrap();
        let db = db.initialise_table::<crate::plugins::orders::Order>().await.unwrap();
        // Provide a Stripe client; the test stub will be used and avoid network.
        let stripe_client = std::sync::Arc::new(stripe::Client::new("sk_test_stub".to_string()));
        let state = AppState::new_with_stripe(db.clone(), Some(stripe_client));

        // Minimal router with our routes
        let app: Router = Router::new()
            .add_routes::<crate::plugins::users::User>()
            .add_routes::<crate::plugins::posts::Post>()
            .add_routes::<crate::plugins::orders::Order>()
            .with_state(state.clone());

        // Insert a post to rent
        let today = chrono::Local::now().date_naive();
        let start_s = today.format("%Y-%m-%d").to_string();
        let end_s = (today + chrono::Days::new(30)).format("%Y-%m-%d").to_string();
        let post = crate::plugins::posts::Post::new(
            "Stripe Redirect Test",
            "Somewhere",
            25,
            0,
            5,
            &start_s,
            &end_s,
            "",
        );
        state.pool.create(post).await.unwrap();
        // Fetch the inserted post id
        let post_id: i64 = sqlx::query_scalar("SELECT id FROM Posts ORDER BY id DESC LIMIT 1")
            .fetch_one(&state.pool.0).await.unwrap_or(1);

        // Configure env for stub redirect URL
        unsafe { std::env::set_var("STRIPE_TEST_REDIRECT_URL", "https://stripe.test/checkout") };
        unsafe { std::env::set_var("BASE_URL", "http://127.0.0.1:37373") };

        // Submit rent request
        let form = format!(
            "quantity=2&start_date={}&end_date={}",
            start_s, end_s
        );
        let res = app
            .oneshot(
                Request::post(format!("/posts/{}/rent", post_id))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let loc = res.headers().get(LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(loc.starts_with("https://stripe."));
    }

    // Live test hitting Stripe: requires `--features stripe,stripe_live` and STRIPE_SECRET_KEY set.
    #[cfg(all(feature = "stripe", feature = "stripe_live"))]
    #[tokio::test]
    async fn rent_request_redirects_to_stripe_live() {
        // Skip if not configured
        let secret = match std::env::var("STRIPE_SECRET_KEY") { Ok(v) if !v.is_empty() => v, _ => return };

        let db = Database::new_with_filename(&format!("test-{}-redir-live.db", nanoid::nanoid!())).await.unwrap();
        let db = db.initialise_table::<crate::plugins::users::User>().await.unwrap();
        let db = db.initialise_table::<crate::plugins::posts::Post>().await.unwrap();
        let db = db.initialise_table::<crate::plugins::orders::Order>().await.unwrap();
        let stripe_client = std::sync::Arc::new(stripe::Client::new(secret));
        let state = AppState::new_with_stripe(db.clone(), Some(stripe_client));

        let app: Router = Router::new()
            .add_routes::<crate::plugins::users::User>()
            .add_routes::<crate::plugins::posts::Post>()
            .add_routes::<crate::plugins::orders::Order>()
            .with_state(state.clone());

        let today = chrono::Local::now().date_naive();
        let start_s = today.format("%Y-%m-%d").to_string();
        let end_s = (today + chrono::Days::new(30)).format("%Y-%m-%d").to_string();
        let post = crate::plugins::posts::Post::new(
            "Stripe Live Redirect",
            "Somewhere",
            25,
            0,
            5,
            &start_s,
            &end_s,
            "",
        );
        state.pool.create(post).await.unwrap();
        let post_id: i64 = sqlx::query_scalar("SELECT id FROM Posts ORDER BY id DESC LIMIT 1")
            .fetch_one(&state.pool.0).await.unwrap();

        // Provide base URL for success/cancel (not actually called by Stripe during creation)
        unsafe { std::env::set_var("BASE_URL", "http://127.0.0.1:37373") };

        let form = format!(
            "quantity=2&start_date={}&end_date={}",
            start_s, end_s
        );
        let res = app
            .oneshot(
                Request::post(format!("/posts/{}/rent", post_id))
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let loc = res.headers().get(LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(loc.starts_with("https://checkout.stripe.com/"));
    }
}

mod control {
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Redirect, Response},
        routing::get,
        Form, Router,
    };
    use axum_login::{AuthSession, AuthUser};
    use maud::Markup;

    use crate::{
        appstate::AppState,
        controller::RouteProvider,
        model::database::{Database, DatabaseProvider},
    };

    use super::{NewOrder, Order};

    impl RouteProvider for Order {
        fn provide_routes(router: Router<AppState>) -> Router<AppState> {
            router
                .route("/posts/{id}/rent", get(Order::rent_page).post(Order::rent_request))
                .route("/orders/{id}/confirm", get(Order::confirm_page).post(Order::confirm_submit))
                .route("/orders/{id}/cancel", axum::routing::post(Order::cancel_order))
                .route("/orders/{id}", get(Order::order_detail))
                .route("/orders", get(Order::my_orders))
        }
    }

    impl Order {
        pub async fn rent_page(
            State(state): State<AppState>,
            Path(id): Path<u32>,
            auth: AuthSession<Database>,
        ) -> Response {
            if auth.user.is_none() {
                let to = format!("/login?next=/posts/{}/rent", id);
                return axum::response::Redirect::to(&to).into_response();
            }
            let post = match crate::plugins::posts::Post::retrieve(id, &state.pool).await {
                Ok(p) => p,
                Err(_) => return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()).into_response(),
            };
            let is_auth = auth.user.is_some();
            let renter_name = auth.user.as_ref().map(|u| u.name.clone()).unwrap_or_default();
            let renter_email = auth.user.as_ref().map(|u| u.email.clone()).unwrap_or_default();
            // Default start/end based on post availability (end defaults to +30 days from start, capped by post.end_date)
            let start = chrono::NaiveDate::parse_from_str(&post.available_date, "%Y-%m-%d").unwrap_or_else(|_| chrono::Local::now().date_naive());
            let post_end = chrono::NaiveDate::parse_from_str(&post.end_date, "%Y-%m-%d").unwrap_or(start + chrono::Days::new(30));
            let default_end = (start + chrono::Days::new(30)).min(post_end);
            let start_s = start.format("%Y-%m-%d").to_string();
            let end_s = default_end.format("%Y-%m-%d").to_string();
            (StatusCode::OK, super::view::rent_form_page(is_auth, id, &post.title, &renter_name, &renter_email, &start_s, &end_s).await).into_response()
        }

        pub async fn rent_request(
            State(state): State<AppState>,
            Path(id): Path<u32>,
            auth: AuthSession<Database>,
            Form(payload): Form<NewOrder>,
        ) -> Response {
            if auth.user.is_none() {
                let to = format!("/login?next=/posts/{}/rent", id);
                return axum::response::Redirect::to(&to).into_response();
            }
            let (renter_user_id, renter_name, renter_email) = {
                let u = auth.user.as_ref().unwrap();
                (u.id() as i64, u.name.clone(), u.email.clone())
            };
            tracing::info!(target: "orders.rent", post_id=id, renter_email=%renter_email, quantity=%payload.quantity, start_date=%payload.start_date, end_date=%payload.end_date, "received rent request");
            // Validate minimal fields
            if payload.quantity <= 0
                || payload.start_date.trim().is_empty()
                || payload.end_date.trim().is_empty()
            {
                return (StatusCode::BAD_REQUEST, super::view::rent_failure().await).into_response();
            }

            // Load post to gather context for Stripe line item
            let post = match crate::plugins::posts::Post::retrieve(id, &state.pool).await {
                Ok(p) => p,
                Err(_) => return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()).into_response(),
            };
            // Validate and normalize dates; enforce within post range
            let start_date = match chrono::NaiveDate::parse_from_str(&payload.start_date, "%Y-%m-%d") { Ok(d) => d, Err(_) => return (StatusCode::BAD_REQUEST, super::view::rent_failure().await).into_response() };
            let end_date = match chrono::NaiveDate::parse_from_str(&payload.end_date, "%Y-%m-%d") { Ok(d) => d, Err(_) => return (StatusCode::BAD_REQUEST, super::view::rent_failure().await).into_response() };
            if end_date < start_date { return (StatusCode::BAD_REQUEST, super::view::rent_failure().await).into_response(); }
            let post_start = chrono::NaiveDate::parse_from_str(&post.available_date, "%Y-%m-%d").unwrap_or(start_date);
            let post_end = chrono::NaiveDate::parse_from_str(&post.end_date, "%Y-%m-%d").unwrap_or(end_date);
            if start_date < post_start || end_date > post_end { return (StatusCode::BAD_REQUEST, super::view::rent_failure().await).into_response(); }

            // Use authenticated user details

            // Insert and get inserted row id; keep in review until user confirms
            let insert_res = sqlx::query(
                "INSERT INTO Orders (post_id, renter_user_id, renter_name, renter_email, quantity, start_date, end_date, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending_review')"
            )
            .bind(id as i64)
            .bind(renter_user_id)
            .bind(&renter_name)
            .bind(&renter_email)
            .bind(payload.quantity)
            .bind(&payload.start_date)
            .bind(&payload.end_date)
            .execute(&state.pool.0).await;
            let order_rowid: i64 = match insert_res {
                Ok(res) => { let id = res.last_insert_rowid(); tracing::info!(target: "orders.rent", order_id=id, "order inserted"); id },
                Err(e) => { tracing::error!(target: "orders.rent", error=?e, "failed to insert order"); return (StatusCode::INTERNAL_SERVER_ERROR, super::view::rent_failure().await).into_response(); },
            };

            // Redirect to confirmation page
            let to = format!("/orders/{}/confirm", order_rowid);
            Redirect::to(&to).into_response()
        }

        pub async fn confirm_page(
            State(state): State<AppState>,
            Path(order_id): Path<i64>,
            auth: AuthSession<Database>,
        ) -> Response {
            let Some(user) = auth.user.as_ref() else { return Redirect::to("/login?next=/orders" ).into_response(); };
            let order: Option<super::Order> = sqlx::query_as("SELECT * FROM Orders WHERE id=?1")
                .bind(order_id)
                .fetch_optional(&state.pool.0).await.unwrap_or(None);
            let Some(order) = order else { return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()).into_response(); };
            if order.renter_user_id != user.id() as i64 { return (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(); }
            let post: Option<crate::plugins::posts::Post> = sqlx::query_as("SELECT * FROM Posts WHERE id=?1").bind(order.post_id).fetch_optional(&state.pool.0).await.unwrap_or(None);
            let title = post.as_ref().map(|p| p.title.as_str()).unwrap_or("");
            (StatusCode::OK, super::view::confirm_page(true, order_id as u32, title, &order).await).into_response()
        }

        pub async fn confirm_submit(
            State(state): State<AppState>,
            Path(order_id): Path<i64>,
            auth: AuthSession<Database>,
        ) -> Response {
            let Some(user) = auth.user.as_ref() else { return Redirect::to("/login?next=/orders" ).into_response(); };
            let order: Option<super::Order> = sqlx::query_as("SELECT * FROM Orders WHERE id=?1")
                .bind(order_id)
                .fetch_optional(&state.pool.0).await.unwrap_or(None);
            let Some(order) = order else { return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()).into_response(); };
            if order.renter_user_id != user.id() as i64 { return (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(); }

            // Load post for pricing
            let post: Option<crate::plugins::posts::Post> = sqlx::query_as("SELECT * FROM Posts WHERE id=?1").bind(order.post_id).fetch_optional(&state.pool.0).await.unwrap_or(None);
            let Some(post) = post else { return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()).into_response(); };

            // Build Stripe session now
            let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:37373".to_string());
            let success_url = format!("{}/orders", base_url);
            let cancel_url = format!("{}/orders/{}/confirm", base_url, order_id);
            let start_date = chrono::NaiveDate::parse_from_str(&order.start_date, "%Y-%m-%d").unwrap_or_else(|_| chrono::Local::now().date_naive());
            let end_date = chrono::NaiveDate::parse_from_str(&order.end_date, "%Y-%m-%d").unwrap_or(start_date);
            let days = (end_date - start_date).num_days().max(1) as i64;
            let mut submitted = false;
            let mut stripe_session_id: Option<String> = None;
            let mut stripe_checkout_url: Option<String> = None;
            #[cfg(feature = "stripe")]
            if let Some(client) = state.stripe.as_ref() {
                let price_cents_per_day = (post.price as i64) * 100;
                let renter_customer_id: Option<String> = sqlx::query_scalar::<_, Option<String>>("SELECT stripe_customer_id FROM users WHERE id=?1")
                    .bind(order.renter_user_id)
                    .fetch_one(&state.pool.0).await
                    .unwrap_or(None);
                match super::service::submit_stripe_checkout_session(
                    client,
                    &post.title,
                    order.quantity,
                    days,
                    price_cents_per_day,
                    &order.renter_email,
                    renter_customer_id.as_deref(),
                    order_id,
                    &success_url,
                    &cancel_url,
                ).await {
                    Ok(Some((sid, url))) => { submitted = true; stripe_session_id = Some(sid); stripe_checkout_url = Some(url); }
                    Ok(None) => { submitted = false; }
                    Err(_) => { submitted = false; }
                }
            }

            if submitted {
                let _ = sqlx::query("UPDATE Orders SET status='submitted', stripe_session_id=?1, stripe_checkout_url=?2 WHERE id=?3")
                    .bind(&stripe_session_id)
                    .bind(&stripe_checkout_url)
                    .bind(order_id)
                    .execute(&state.pool.0).await;
                if let Some(url) = stripe_checkout_url { return Redirect::to(&url).into_response(); }
            }
            // No Stripe configured or session creation failed: show pending
            let _ = sqlx::query("UPDATE Orders SET status='submitted' WHERE id=?1")
                .bind(order_id).execute(&state.pool.0).await;
            (StatusCode::OK, super::view::rent_received_pending().await).into_response()
        }

        pub async fn cancel_order(
            State(state): State<AppState>,
            Path(order_id): Path<i64>,
            auth: AuthSession<Database>,
        ) -> Response {
            let Some(user) = auth.user.as_ref() else { return Redirect::to("/login?next=/orders").into_response(); };
            let owner: Option<i64> = sqlx::query_scalar("SELECT renter_user_id FROM Orders WHERE id=?1")
                .bind(order_id)
                .fetch_one(&state.pool.0)
                .await
                .ok();
            if owner != Some(user.id() as i64) { return (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(); }
            let _ = sqlx::query("UPDATE Orders SET status='cancelled' WHERE id=?1")
                .bind(order_id)
                .execute(&state.pool.0).await;
            Redirect::to("/orders").into_response()
        }

        pub async fn my_orders(
            State(state): State<AppState>,
            auth: AuthSession<Database>
        ) -> axum::response::Response {
            if let Some(user) = auth.user.as_ref() {
                let email = user.email.clone();
                let uid = user.id() as i64;
                let orders = sqlx::query_as::<_, super::Order>(
                    "SELECT * FROM Orders WHERE renter_user_id=?1 OR renter_email=?2 ORDER BY id DESC LIMIT 100"
                )
                .bind(uid)
                .bind(email)
                .fetch_all(&state.pool.0).await.unwrap_or_default();
                return (StatusCode::OK, super::view::orders_list_page(true, &orders).await).into_response();
            }
            axum::response::Redirect::to("/login").into_response()
        }

        pub async fn order_detail(
            State(state): State<AppState>,
            Path(order_id): Path<i64>,
            auth: AuthSession<Database>
        ) -> axum::response::Response {
            let Some(user) = auth.user.as_ref() else { return axum::response::Redirect::to("/login").into_response(); };
            let order: Option<super::Order> = sqlx::query_as("SELECT * FROM Orders WHERE id=?1")
                .bind(order_id)
                .fetch_optional(&state.pool.0)
                .await
                .unwrap_or(None);
            let Some(order) = order else { return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()).into_response(); };
            if order.renter_user_id != user.id() as i64 { return (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(); }
            let post: Option<crate::plugins::posts::Post> = sqlx::query_as("SELECT * FROM Posts WHERE id=?1")
                .bind(order.post_id)
                .fetch_optional(&state.pool.0)
                .await
                .unwrap_or(None);
            let title = post.as_ref().map(|p| p.title.as_str()).unwrap_or("");
            (StatusCode::OK, super::view::order_detail_page(true, title, &order).await).into_response()
        }
    }
}

mod view {
    use maud::{html, Markup};
    use crate::views::utils::{default_header, title_and_navbar};

    pub async fn rent_form_page(
        is_auth: bool,
        post_id: u32,
        post_title: &str,
        renter_name: &str,
        renter_email: &str,
        start_date: &str,
        end_date: &str,
    ) -> Markup {
        html! {
            (default_header("Pallet Spaces: Rent"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container" { h2 { "Rent space: " (post_title) } }
                form class="container card form" method="POST" action={(format!("/posts/{}/rent", post_id))} {
                    div class="grid grid--2" {
                        div class="field" { label class="label" { "Your name" } p class="input" { (renter_name) } }
                        div class="field" { label class="label" { "Your email" } p class="input" { (renter_email) } }
                        div class="field" { label class="label" for="quantity" { "Pallet spaces needed" } input class="input" type="number" min="1" step="1" id="quantity" name="quantity" required value="1" {} }
                        div class="field" { label class="label" for="start_date" { "Start date" } input class="input" type="date" id="start_date" name="start_date" required value=(start_date) {} }
                        div class="field" { label class="label" for="end_date" { "End date" } input class="input" type="date" id="end_date" name="end_date" required value=(end_date) {} }
                    }
                    div { button class="btn btn--primary" type="submit" { "Send request" } }
                }
            }
        }
    }

    pub async fn confirm_page(
        is_auth: bool,
        order_id: u32,
        post_title: &str,
        order: &super::Order,
    ) -> Markup {
        let start = &order.start_date;
        let end = &order.end_date;
        let days = chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d")
            .ok()
            .and_then(|e| chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").ok().map(|s| (e - s).num_days().max(1)))
            .unwrap_or(1);
        html! {
            (default_header("Pallet Spaces: Confirm Order"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container card" {
                    h2 { "Confirm your order" }
                    p { strong { "Space:" } " " (post_title) }
                    p { strong { "Quantity:" } " " (order.quantity) }
                    p { strong { "Dates:" } " " (start) " → " (end) }
                    p { strong { "Total units:" } " " (order.quantity * days as i64) }
                    form class="mt-2" method="POST" action={(format!("/orders/{}/confirm", order_id))} {
                        button class="btn btn--primary" type="submit" { "Confirm and proceed to payment" }
                    }
                    form class="mt-2" method="POST" action={(format!("/orders/{}/cancel", order_id))} onsubmit="return confirm('Cancel this order?');" {
                        button class="btn btn--ghost" type="submit" { "Cancel" }
                    }
                }
            }
        }
    }

    pub async fn rent_received_pending() -> Markup {
        html! {
            (default_header("Pallet Spaces: Rent"))
            body { div class="container card" { h2 { "Request received" } p { "Thanks! We recorded your request. Our team will follow up shortly." } } }
        }
    }

    pub async fn rent_failure() -> Markup {
        html! {
            (default_header("Pallet Spaces: Rent"))
            body { div class="container card" { h2 { "Invalid details" } p class="error" { "Please check your inputs and try again." } } }
        }
    }

    pub async fn orders_list_page(is_auth: bool, orders: &[super::Order]) -> Markup {
        html! {
            (default_header("Pallet Spaces: Orders"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container" { h2 { "My Orders" } }
                @if orders.is_empty() {
                    div class="container" { p class="text-muted" { "No orders yet." } }
                } @else {
                    div class="container list" {
                        @for o in orders {
                            div class="card" {
                                p { strong { "Post: " } a href={(format!("/posts/{}", o.post_id))} { (format!("#{}", o.post_id)) } }
                                p class="text-muted" { strong { "Quantity: " } (o.quantity) }
                                p class="text-muted" { strong { "Dates: " } (o.start_date) " → " (o.end_date) }
                                p { strong { "Status: " } (o.status.clone()) }
                                div class="cluster" {
                                    a class="btn btn--ghost" href={(format!("/orders/{}", o.id.as_ref().map(|x| x.0).unwrap_or(0)))} { "Details" }
                                    @if o.status == "pending_review" { a class="btn btn--secondary" href={(format!("/orders/{}/confirm", o.id.as_ref().map(|x| x.0).unwrap_or(0)))} { "Review & pay" } }
                                    @if let Some(url) = &o.stripe_checkout_url { a class="btn btn--secondary" href=(url) { "Complete payment on Stripe" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn order_detail_page(
        is_auth: bool,
        post_title: &str,
        order: &super::Order,
    ) -> Markup {
        html! {
            (default_header("Pallet Spaces: Order"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container card" {
                    h2 { "Order details" }
                    p { strong { "Space:" } " " (post_title) }
                    p { strong { "Quantity:" } " " (order.quantity) }
                    p { strong { "Dates:" } " " (order.start_date) " → " (order.end_date) }
                    p { strong { "Status:" } " " (order.status) }
                    @if let Some(url) = &order.stripe_checkout_url { a class="btn btn--secondary" href=(url) { "Complete payment on Stripe" } }
                }
            }
        }
    }
}
