use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct PostID(u64);

impl From<u64> for PostID {
    fn from(raw: u64) -> Self {
        PostID(raw)
    }
}

#[derive(Clone, FromRow, Serialize, Deserialize, Debug)]
pub struct Post {
    id: Option<PostID>,
    pub title: String,
    pub location: String,
    pub price: i64,
    pub user_id: i64,
    pub spaces_available: i64,
    pub available_date: String, // start date YYYY-MM-DD
    pub end_date: String,       // end date YYYY-MM-DD
    pub notes: String,
    pub visible: i64,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub geocoded_label: Option<String>,
}

impl Post {
    pub fn new(
        title: &str,
        location: &str,
        price: i64,
        user_id: i64,
        spaces_available: i64,
        available_date: &str,
        end_date: &str,
        notes: &str,
    ) -> Self {
        Self {
            id: None,
            title: title.to_string(),
            location: location.to_string(),
            price,
            user_id,
            spaces_available,
            available_date: available_date.to_string(),
            end_date: end_date.to_string(),
            notes: notes.to_string(),
            visible: 1,
            latitude: None,
            longitude: None,
            geocoded_label: None,
        }
    }

    pub fn id_raw(&self) -> Option<u64> {
        match &self.id {
            Some(id) => Some(id.0),
            None => None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct NewPost {
    pub title: String,
    pub location: String,
    pub price: i64,
    pub spaces_available: i64,
    pub available_date: String,
    pub end_date: String,
    pub notes: String,
}

mod model {
    use sqlx::Executor;
    use serde::Deserialize;

    use crate::{
        error::Error,
        model::database::{Database, DatabaseProvider},
    };

    use super::Post;
    #[derive(Deserialize)]
    pub struct EditPost {
        pub title: String,
        pub location: String,
        pub price: i64,
        pub spaces_available: i64,
        pub available_date: String,
        pub end_date: String,
        pub notes: String,
    }

    impl Post {
        #[allow(dead_code)]
        pub async fn get_all_posts(pool: &Database) -> Vec<Post> {
            match sqlx::query_as::<_, Post>("SELECT * FROM Posts ORDER BY id ASC")
                .fetch_all(&pool.0)
                .await
            {
                Ok(posts) => posts,
                Err(_) => Vec::new(),
            }
        }

        pub async fn get_posts_by_user(pool: &Database, user_id: i64) -> Vec<Post> {
            match sqlx::query_as::<_, Post>(
                "SELECT * FROM Posts WHERE user_id = ?1 ORDER BY id DESC",
            )
            .bind(user_id)
            .fetch_all(&pool.0)
            .await
            {
                Ok(posts) => posts,
                Err(_) => Vec::new(),
            }
        }

        pub async fn get_posts_filtered(
            pool: &Database,
            filter: &crate::plugins::posts::control::PostsFilter,
        ) -> Vec<Post> {
            use sqlx::{Arguments, sqlite::SqliteArguments};

            let mut sql = String::from("SELECT * FROM Posts");
            let mut args = SqliteArguments::default();
            let mut cond: Vec<&str> = Vec::new();

            // Only show visible posts on public listing
            cond.push("visible = 1");

            if let Some(ref title) = filter.title {
                cond.push("title LIKE ?");
                let _ = args.add(format!("%{}%", title));
            }
            if let Some(ref location) = filter.location {
                cond.push("location LIKE ?");
                let _ = args.add(format!("%{}%", location));
            }
            if let Some(ref max_price) = filter.max_price {
                if let Ok(v) = max_price.trim().parse::<i64>() {
                    cond.push("price <= ?");
                    let _ = args.add(v);
                }
            }
            if let Some(ref min_spaces) = filter.min_spaces_available {
                if let Ok(v) = min_spaces.trim().parse::<i64>() {
                    cond.push("spaces_available >= ?");
                    let _ = args.add(v);
                }
            }
            if let Some(ref start) = filter.start_date {
                if !start.is_empty() { cond.push("available_date >= ?"); let _ = args.add(start); }
            }
            if let Some(ref end) = filter.end_date {
                if !end.is_empty() { cond.push("end_date <= ?"); let _ = args.add(end); }
            }

            if !cond.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&cond.join(" AND "));
            }
            sql.push_str(" ORDER BY id ASC");

            match sqlx::query_as_with::<_, Post, _>(&sql, args)
                .fetch_all(&pool.0)
                .await
            {
                Ok(posts) => posts,
                Err(_) => Vec::new(),
            }
        }
    }

    impl std::fmt::Display for Post {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&format!("{:?}", self))
        }
    }

    impl DatabaseProvider for Post {
        type Database = Database;
        type Id = u32;
        async fn initialise_table(pool: Database) -> Result<Database, Error> {
            let creation_attempt = &pool
                .0
                .execute(
                    "
      CREATE TABLE if not exists Posts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        location TEXT NOT NULL,
        price INTEGER NOT NULL,
        user_id INTEGER NOT NULL DEFAULT 0,
        spaces_available INTEGER NOT NULL,
        available_date TEXT NOT NULL,
        end_date TEXT NOT NULL,
        notes TEXT NOT NULL,
        visible INTEGER NOT NULL DEFAULT 1,
        latitude REAL,
        longitude REAL,
        geocoded_label TEXT
      )
      ",
                )
                .await;
            match creation_attempt {
                Ok(_) => {
                    // Best-effort migrations to add new columns if the table already exists
                    // and lacks them. SQLite will error if the column exists; ignore errors.
                    let migrations = [
                        "ALTER TABLE Posts ADD COLUMN title TEXT NOT NULL DEFAULT ''",
                        "ALTER TABLE Posts ADD COLUMN location TEXT NOT NULL DEFAULT ''",
                        "ALTER TABLE Posts ADD COLUMN price INTEGER NOT NULL DEFAULT 0",
                        "ALTER TABLE Posts ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0",
                        "ALTER TABLE Posts ADD COLUMN spaces_available INTEGER NOT NULL DEFAULT 0",
                        "ALTER TABLE Posts ADD COLUMN available_date TEXT NOT NULL DEFAULT ''",
                        "ALTER TABLE Posts ADD COLUMN end_date TEXT NOT NULL DEFAULT ''",
                        "ALTER TABLE Posts ADD COLUMN visible INTEGER NOT NULL DEFAULT 1",
                        "ALTER TABLE Posts ADD COLUMN latitude REAL",
                        "ALTER TABLE Posts ADD COLUMN longitude REAL",
                        "ALTER TABLE Posts ADD COLUMN geocoded_label TEXT",
                    ];
                    for stmt in migrations { let _ = pool.0.execute(stmt).await; }
                    // Backfill end_date for existing rows where missing
                    let _ = pool.0.execute(
                        "UPDATE Posts SET end_date = CASE
                            WHEN (end_date IS NULL OR end_date = '') AND (available_date IS NOT NULL AND available_date <> '')
                            THEN date(available_date, '+30 day')
                            ELSE end_date
                        END"
                    ).await;
                    Ok(pool)
                }
                Err(_) => Err(Error::Database(
                    "Failed to create Post database tables".into(),
                )),
            }
        }

        async fn create(self, pool: &Database) -> Result<&Database, Error> {
            let attempt = sqlx::query(
                "INSERT INTO Posts (
                    title, location, price, user_id, spaces_available, available_date, end_date, notes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(self.title)
            .bind(self.location)
            .bind(self.price)
            .bind(self.user_id)
            .bind(self.spaces_available)
            .bind(self.available_date)
            .bind(self.end_date)
            .bind(self.notes)
                .execute(&pool.0)
                .await;
            match attempt {
                Ok(_) => Ok(pool),
                Err(_) => Err(Error::Database(
                    "Failed to insert Post into database".into(),
                )),
            }
        }

        async fn retrieve(id: Self::Id, pool: &Database) -> Result<Self, Error> {
            let attempt = sqlx::query_as::<_, Post>("SELECT * FROM Posts where id=(?1)")
                .bind(id)
                .fetch_one(&pool.0)
                .await;
            match attempt {
                Ok(post) => Ok(post),
                Err(_) => Err(Error::Database(
                    "Failed to insert Post into database".into(),
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

mod control {
    use axum::{
        Form, Router,
        extract::{Query, State},
        http::StatusCode,
        response::{IntoResponse, Redirect, Response},
        routing::{get},
    };
    use maud::Markup;
    use axum_login::AuthSession;
    use axum_login::AuthUser;
    use serde::Deserialize;

    use crate::{
        appstate::AppState,
        controller::RouteProvider,
        model::database::DatabaseProvider,
        plugins::posts::view::{new_post_failure, post_form_page},
    };

    use super::{NewPost, Post, view::{posts_index_page, post_show_page_view}};
    use crate::plugins::posts::model::EditPost;

    #[derive(Debug, Default, Deserialize)]
    pub struct PostsFilter {
        pub title: Option<String>,
        pub location: Option<String>,
        pub max_price: Option<String>,
        pub min_spaces_available: Option<String>,
        pub start_date: Option<String>,
        pub end_date: Option<String>,
    }

    impl RouteProvider for Post {
        fn provide_routes(router: Router<AppState>) -> Router<AppState> {
            router
                .route(
                    "/new_post",
                    get(Post::create_post_page).post(Post::new_post_request),
                )
                .route("/posts", get(Post::post_list))
                .route("/posts/{id}/edit", get(Post::edit_post_page))
                .route("/posts/{id}/toggle_visibility", axum::routing::post(Post::toggle_visibility))
                .route("/posts/{id}/delete", axum::routing::post(Post::delete_post))
                .route("/posts/{id}", get(Post::show_post_page).post(Post::edit_post_request))
                .route("/api/geocode", get(Post::geocode_suggest_endpoint))
        }
    }

    impl Post {
        pub async fn create_post_page(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
        ) -> (StatusCode, Markup) {
            // Require login
            let Some(user) = auth.user.as_ref() else {
                return (StatusCode::SEE_OTHER, maud::html!{ (crate::views::utils::default_header("Redirect")) body { script { "window.location='/login?next=/new_post'" } } });
            };
            // Gate on verified Connect account
            let verified = crate::plugins::users::service::is_connect_verified(&state, user.id() as i64).await;
            if !verified {
                return (StatusCode::SEE_OTHER, maud::html!{ (crate::views::utils::default_header("Redirect")) body { script { "window.location='/me'" } } });
            }
            let is_auth = true;
            // Provide sensible defaults for a new post
            let today = chrono::Local::now().date_naive();
            let start_s = today.format("%Y-%m-%d").to_string();
            let end_s = (today + chrono::Days::new(30)).format("%Y-%m-%d").to_string();
            let draft = Post::new(
                "",
                "",
                0,
                0,
                1,
                &start_s,
                &end_s,
                "",
            );
            (StatusCode::OK, post_form_page(is_auth, "Create Post", "/new_post", &draft).await)
        }

        pub async fn new_post_request(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            Form(payload): Form<NewPost>,
        ) -> Response {
            tracing::info!(target: "posts.create", title=%payload.title, location=%payload.location, price=%payload.price, "received new_post_request");
            // Require login
            let Some(user) = auth.user.as_ref() else {
                return Redirect::to("/login?next=/new_post").into_response();
            };
            // Gate on verified Connect account
            if !crate::plugins::users::service::is_connect_verified(&state, user.id() as i64).await {
                return Redirect::to("/me").into_response();
            }
            // Validate and normalize dates
            let start_date = match chrono::NaiveDate::parse_from_str(&payload.available_date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => return (StatusCode::BAD_REQUEST, super::view::new_post_failure().await).into_response(),
            };
            let end_date = match chrono::NaiveDate::parse_from_str(payload.end_date.trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => start_date + chrono::Days::new(30),
            };
            let end_date = if end_date < start_date { start_date + chrono::Days::new(30) } else { end_date };
            let start_s = start_date.format("%Y-%m-%d").to_string();
            let end_s = end_date.format("%Y-%m-%d").to_string();
            let uid = user.id() as i64;
            let post = Post::new(
                &payload.title,
                &payload.location,
                payload.price,
                uid,
                payload.spaces_available,
                &start_s,
                &end_s,
                &payload.notes,
            );
            tracing::debug!("Signing up Post {:?}", post);
            // Perform our own insert to get rowid for subsequent geocoding update
            let insert_res = sqlx::query(
                "INSERT INTO Posts (title, location, price, user_id, spaces_available, available_date, end_date, notes, visible) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)"
            )
            .bind(post.title)
            .bind(post.location)
            .bind(post.price)
            .bind(post.user_id)
            .bind(post.spaces_available)
            .bind(post.available_date)
            .bind(post.end_date)
            .bind(post.notes)
            .execute(&state.pool.0).await;
            let post_rowid: i64 = match insert_res {
                Ok(r) => {
                    let id = r.last_insert_rowid();
                    tracing::info!(target: "posts.create", post_id=id, "inserted post row");
                    id
                },
                Err(e) => {
                    tracing::error!(target: "posts.create", error=?e, "failed to insert post");
                    return (StatusCode::INTERNAL_SERVER_ERROR, new_post_failure().await).into_response();
                },
            };

            // Attempt to geocode the location and update coordinates
            tracing::debug!(target: "posts.create", post_id=post_rowid, "geocoding location");
            if let Some((lat, lon, label)) = super::service::geocode_location(&payload.location).await.unwrap_or(None) {
                tracing::info!(target: "posts.create", post_id=post_rowid, %lat, %lon, label=%label, "geocode success");
                let res = sqlx::query("UPDATE Posts SET latitude=?1, longitude=?2, geocoded_label=?3 WHERE id=?4")
                    .bind(lat)
                    .bind(lon)
                    .bind(label)
                    .bind(post_rowid)
                    .execute(&state.pool.0).await;
                if let Err(e) = res { tracing::warn!(target: "posts.create", post_id=post_rowid, error=?e, "failed to persist geocode"); }
            } else {
                tracing::info!(target: "posts.create", post_id=post_rowid, "geocode skipped or no result");
            }
            let to = format!("/posts/{}", post_rowid);
            tracing::info!(target: "posts.create", post_id=post_rowid, redirect=%to, "redirecting to new post");
            Redirect::to(&to).into_response()
        }

        pub async fn post_list(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            Query(filter): Query<PostsFilter>,
        ) -> (StatusCode, Markup) {
            tracing::debug!(target: "posts.index", ?filter, "listing posts with filter");
            let posts = Post::get_posts_filtered(&state.pool, &filter).await;
            tracing::info!(target: "posts.index", count=posts.len(), "posts index fetched");
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64);
            let is_auth = auth.user.is_some();
            let is_verified = if let Some(u) = auth.user.as_ref() { crate::plugins::users::service::is_connect_verified(&state, u.id() as i64).await } else { false };
            (StatusCode::OK, posts_index_page(is_auth, is_verified, &filter, &posts, current_uid).await)
        }

        pub async fn show_post_page(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            axum::extract::Path(id): axum::extract::Path<u32>,
        ) -> (StatusCode, Markup) {
            let post = match Post::retrieve(id, &state.pool).await {
                Ok(p) => p,
                Err(_) => return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()),
            };
            tracing::info!(target: "posts.show", post_id=id, title=%post.title, "rendering show page");
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64);
            let is_auth = auth.user.is_some();
            (StatusCode::OK, post_show_page_view(is_auth, id, &post, current_uid).await)
        }

        pub async fn geocode_suggest_endpoint(
            Query(params): Query<std::collections::HashMap<String, String>>,
        ) -> (StatusCode, Markup) {
            // Accept either `q` (generic) or `location` (form field name)
            let raw = params
                .get("q")
                .cloned()
                .or_else(|| params.get("location").cloned())
                .unwrap_or_default();
            let q = raw.trim();
            if q.is_empty() {
                return (StatusCode::OK, super::view::geocode_suggestions(&[]).await);
            }
            let suggestions = super::service::geocode_suggest(q).await.unwrap_or_default();
            (StatusCode::OK, super::view::geocode_suggestions(&suggestions).await)
        }

        pub async fn edit_post_page(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            axum::extract::Path(id): axum::extract::Path<u32>,
        ) -> (StatusCode, Markup) {
            let post = match Post::retrieve(id, &state.pool).await {
                Ok(p) => p,
                Err(_) => return (StatusCode::NOT_FOUND, crate::views::utils::page_not_found()),
            };
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64);
            if current_uid != Some(post.user_id) {
                return (StatusCode::FORBIDDEN, crate::views::utils::page_not_found());
            }
            let is_auth = auth.user.is_some();
            (StatusCode::OK, post_form_page(is_auth, "Edit Post", &format!("/posts/{}", id), &post).await)
        }

        pub async fn edit_post_request(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            axum::extract::Path(id): axum::extract::Path<u32>,
            Form(payload): Form<EditPost>,
        ) -> Response {
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64).unwrap_or(-1);
            // Validate date range
            let start_date = match chrono::NaiveDate::parse_from_str(&payload.available_date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => return (StatusCode::BAD_REQUEST, super::view::new_post_failure().await).into_response(),
            };
            let end_date = match chrono::NaiveDate::parse_from_str(payload.end_date.trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => start_date + chrono::Days::new(30),
            };
            let end_date = if end_date < start_date { start_date + chrono::Days::new(30) } else { end_date };
            let start_s = start_date.format("%Y-%m-%d").to_string();
            let end_s = end_date.format("%Y-%m-%d").to_string();
            // Ensure only owner can update
            let res = sqlx::query(
                "UPDATE Posts SET title=?, location=?, price=?, spaces_available=?, available_date=?, end_date=?, notes=? WHERE id=? AND user_id=?"
            )
            .bind(payload.title)
            .bind(payload.location)
            .bind(payload.price)
            .bind(payload.spaces_available)
            .bind(start_s)
            .bind(end_s)
            .bind(payload.notes)
            .bind(id)
            .bind(current_uid)
            .execute(&state.pool.0)
            .await;

            match res {
                Ok(r) if r.rows_affected() > 0 => {
                    Redirect::to(&format!("/posts/{}", id)).into_response()
                }
                _ => (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(),
            }
        }

        pub async fn toggle_visibility(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            axum::extract::Path(id): axum::extract::Path<u32>,
        ) -> Response {
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64).unwrap_or(-1);
            let res = sqlx::query(
                "UPDATE Posts SET visible = CASE visible WHEN 1 THEN 0 ELSE 1 END WHERE id=? AND user_id=?",
            )
            .bind(id)
            .bind(current_uid)
            .execute(&state.pool.0)
            .await;
            match res {
                Ok(_) => Redirect::to("/me").into_response(),
                Err(_) => (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(),
            }
        }

        pub async fn delete_post(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            axum::extract::Path(id): axum::extract::Path<u32>,
        ) -> Response {
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64).unwrap_or(-1);
            let res = sqlx::query("DELETE FROM Posts WHERE id=? AND user_id=?")
                .bind(id)
                .bind(current_uid)
                .execute(&state.pool.0)
                .await;
            match res {
                Ok(_) => Redirect::to("/me").into_response(),
                Err(_) => (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()).into_response(),
            }
        }
    }
}

mod service {
    // Return (lat, lon, label)
    #[cfg(any(all(feature = "maps", not(test)), all(feature = "maps", feature = "maps_live", test)))]
    pub async fn geocode_location(query: &str) -> Result<Option<(f64, f64, String)>, crate::error::Error> {
        use serde::Deserialize;
        let client = reqwest::Client::new();
        if let Ok(token) = std::env::var("MAPBOX_ACCESS_TOKEN") {
            tracing::debug!(target: "maps.geocode", provider="mapbox", %query);
            #[derive(Deserialize)]
            struct MbFeature { center: [f64; 2], place_name: String }
            #[derive(Deserialize)]
            struct MbResp { features: Vec<MbFeature> }
            let url = format!("https://api.mapbox.com/geocoding/v5/mapbox.places/{}.json?access_token={}&limit=1", urlencoding::encode(query), token);
            let resp = client.get(url).send().await;
            if let Ok(r) = resp { if r.status().is_success() {
                let v: MbResp = r.json().await.unwrap_or(MbResp{features: vec![]});
                if let Some(f) = v.features.into_iter().next() {
                    // Mapbox center is [lon, lat]
                    tracing::info!(target: "maps.geocode", provider="mapbox", %query, lat=f.center[1], lon=f.center[0], label=%f.place_name, "geocoded");
                    return Ok(Some((f.center[1], f.center[0], f.place_name)));
                }
            }}
        }
        // Fallback to Nominatim
        tracing::debug!(target: "maps.geocode", provider="nominatim", %query);
        #[derive(Deserialize)]
        struct NomItem { lat: String, lon: String, display_name: String }
        let url = format!("https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1", urlencoding::encode(query));
        let resp = client.get(url).header("User-Agent", "pallet-spaces/0.1").send().await;
        if let Ok(r) = resp { if r.status().is_success() {
            let arr: Vec<NomItem> = r.json().await.unwrap_or_default();
            if let Some(it) = arr.into_iter().next() {
                if let (Ok(lat), Ok(lon)) = (it.lat.parse::<f64>(), it.lon.parse::<f64>()) {
                    tracing::info!(target: "maps.geocode", provider="nominatim", %query, %lat, %lon, label=%it.display_name, "geocoded");
                    return Ok(Some((lat, lon, it.display_name)));
                }
            }
        }}
        tracing::info!(target: "maps.geocode", %query, "no geocode result");
        Ok(None)
    }

    // Test stub when maps feature enabled but not live
    #[cfg(all(feature = "maps", test, not(feature = "maps_live")))]
    pub async fn geocode_location(_query: &str) -> Result<Option<(f64, f64, String)>, crate::error::Error> {
        Ok(Some((1.2345, 2.3456, "Stub Place".to_string())))
    }

    // When maps feature disabled
    #[cfg(not(feature = "maps"))]
    pub async fn geocode_location(_query: &str) -> Result<Option<(f64, f64, String)>, crate::error::Error> {
        Ok(None)
    }

    // Suggest multiple results (label, lat, lon)
    #[cfg(any(all(feature = "maps", not(test)), all(feature = "maps", feature = "maps_live", test)))]
    pub async fn geocode_suggest(query: &str) -> Result<Vec<(String, f64, f64)>, crate::error::Error> {
        use serde::Deserialize;
        let client = reqwest::Client::new();
        if let Ok(token) = std::env::var("MAPBOX_ACCESS_TOKEN") {
            tracing::debug!(target: "maps.suggest", provider="mapbox", %query);
            #[derive(Deserialize)]
            struct MbFeature { center: [f64; 2], place_name: String }
            #[derive(Deserialize)]
            struct MbResp { features: Vec<MbFeature> }
            let url = format!("https://api.mapbox.com/geocoding/v5/mapbox.places/{}.json?access_token={}&limit=5", urlencoding::encode(query), token);
            if let Ok(r) = client.get(url).send().await { if r.status().is_success() {
                let v: MbResp = r.json().await.unwrap_or(MbResp{features: vec![]});
                let mut out = Vec::new();
                for f in v.features.into_iter().take(5) {
                    out.push((f.place_name, f.center[1], f.center[0]));
                }
                tracing::info!(target: "maps.suggest", provider="mapbox", %query, count=out.len());
                return Ok(out);
            }}
        }
        tracing::debug!(target: "maps.suggest", provider="nominatim", %query);
        #[derive(Deserialize)]
        struct NomItem { display_name: String, lat: String, lon: String }
        let url = format!("https://nominatim.openstreetmap.org/search?q={}&format=json&limit=5", urlencoding::encode(query));
        let mut out = Vec::new();
        if let Ok(r) = client.get(url).header("User-Agent", "pallet-spaces/0.1").send().await {
            if r.status().is_success() {
                let arr: Vec<NomItem> = r.json().await.unwrap_or_default();
                for it in arr.into_iter().take(5) {
                    if let (Ok(lat), Ok(lon)) = (it.lat.parse::<f64>(), it.lon.parse::<f64>()) {
                        out.push((it.display_name, lat, lon));
                    }
                }
            }
        }
        tracing::info!(target: "maps.suggest", provider="nominatim", %query, count=out.len());
        Ok(out)
    }

    #[cfg(all(feature = "maps", test, not(feature = "maps_live")))]
    pub async fn geocode_suggest(query: &str) -> Result<Vec<(String, f64, f64)>, crate::error::Error> {
        Ok(vec![
            (format!("{} - Example A", query), 1.23, 2.34),
            (format!("{} - Example B", query), 5.67, 6.78),
        ])
    }

    #[cfg(not(feature = "maps"))]
    pub async fn geocode_suggest(_query: &str) -> Result<Vec<(String, f64, f64)>, crate::error::Error> { Ok(vec![]) }
}

mod view {
    use maud::{Markup, html};

    use crate::views::utils::{default_header, title_and_navbar};

    fn format_date_display(s: &str) -> String {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            d.format("%d %b %Y").to_string()
        } else {
            s.to_string()
        }
    }

    pub async fn new_post_failure() -> Markup {
        html! {
            (default_header("Pallet Spaces: New Post"))
            body {
                h2 {
                    "Failed to create post"
                }
                p {
                    "Please try again"
                }
            }
        }
    }

    pub async fn geocode_suggestions(items: &[(String, f64, f64)]) -> Markup {
        html! {
            @if items.is_empty() {
                div class="help" { }
            } @else {
                ul class="list" {
                    @for (label, lat, lon) in items.iter() {
                        @let lbl = label.replace("'", "\\'");
                        @let map_url = {
                            if let Ok(token) = std::env::var("MAPBOX_ACCESS_TOKEN") {
                                format!(
                                    "https://api.mapbox.com/styles/v1/mapbox/streets-v12/static/pin-s+ff0000({lon},{lat})/{lon},{lat},14,0/400x200?access_token={}",
                                    token
                                )
                            } else {
                                format!(
                                    "https://staticmap.openstreetmap.de/staticmap.php?center={lat},{lon}&zoom=14&size=400x200&markers={lat},{lon},red-pushpin"
                                )
                            }
                        };
                        li { button type="button" class="btn btn--ghost" onclick={(format!("document.getElementById('location').value='{}'; document.getElementById('location-suggestions').innerHTML=''; document.getElementById('location-preview').innerHTML='\\x3Cimg src=\\x22{}\\x22 alt=\\x22Map preview\\x22 width=\\x22400\\x22 height=\\x22200\\x22 /\\x3E';", lbl, map_url))} { (label) } }
                    }
                }
            }
        }
    }

    // Index page for posts with filters and list
    pub async fn posts_index_page(
        is_auth: bool,
        is_verified: bool,
        filter: &super::control::PostsFilter,
        posts: &[super::Post],
        current_uid: Option<i64>,
    ) -> Markup {
        html! {
            (default_header("Pallet Spaces: Posts"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container" {
                    div class="cluster" {
                        h2 { "Available Spaces" }
                        @if is_auth && is_verified { a class="btn btn--success" href="/new_post" { "New Post" } }
                    }
                }
                form class="container card form" method="GET" action="/posts" {
                    div class="grid grid--2" {
                        div class="field" { label class="label" for="title" { "Title" } input class="input" type="text" id="title" name="title" value=(filter.title.clone().unwrap_or_default()) {} }
                        div class="field" {
                            label class="label" for="location" { "Location" }
                            input class="input" type="text" id="location" name="location" value=(filter.location.clone().unwrap_or_default())
                                hx-get="/api/geocode" hx-trigger="keyup changed delay:300ms" hx-target="#location-suggestions" hx-params="serialize" {}
                            div id="location-suggestions" class="help" {}
                            div id="location-preview" class="help" {}
                        }
                        div class="field" { label class="label" for="max_price" { "Max Price/day" } input class="input" type="number" id="max_price" name="max_price" min="0" step="1" value=(filter.max_price.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="min_spaces_available" { "Min Spaces" } input class="input" type="number" id="min_spaces_available" name="min_spaces_available" min="0" step="1" value=(filter.min_spaces_available.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="start_date" { "Start Date" } input class="input" type="date" id="start_date" name="start_date" value=(filter.start_date.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="end_date" { "End Date" } input class="input" type="date" id="end_date" name="end_date" value=(filter.end_date.clone().unwrap_or_default()) {} }
                        div style="grid-column: 1 / -1; text-align: right;" { button class="btn btn--primary" type="submit" { "Filter" } a class="btn btn--ghost" href="/posts" { "Reset" } }
                    }
                }
                @if posts.is_empty() {
                    div class="container" { p class="text-muted" { "No posts yet." } }
                } @else {
                    div class="container list" id="posts" {
                        @for p in posts {
                            div class="card post-card" {
                                @match p.id_raw() {
                                    Some(id) => h3 { a href=(format!("/posts/{}", id)) { (p.title) } },
                                    None => h3 { (p.title) }
                                }
                                @let pretty_loc = p.geocoded_label.as_ref().map(|s| s.as_str()).unwrap_or(&p.location);
                                p class="text-muted" { strong { "Location: " } (pretty_loc) }
                                @match (p.latitude, p.longitude) { (Some(lat), Some(lon)) => p class="text-muted" { a class="btn btn--ghost" href=(format!("https://www.openstreetmap.org/?mlat={}&mlon={}#map=14/{}/{}", lat, lon, lat, lon)) { "View on map" } }, _ => {} }
                                p class="text-muted" { strong { "Price: " } (p.price) " /day" }
                                @let start_disp = format_date_display(&p.available_date);
                                @let end_disp = format_date_display(&p.end_date);
                                p class="text-muted" { strong { "Availability: " } (start_disp) " → " (end_disp) }
                                p class="text-muted" { strong { "Pallet spaces available: " } (p.spaces_available) }
                                @if !p.notes.is_empty() { p class="mt-2 text-muted" { (p.notes) } }
                                @if current_uid == Some(p.user_id) {
                                    @match p.id_raw() {
                                        Some(id) => div class="mt-2" { a class="btn btn--secondary" href=(format!("/posts/{}/edit", id)) { "Edit" } },
                                        None => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Show a single post
    pub async fn post_show_page_view(
        is_auth: bool,
        id: u32,
        post: &super::Post,
        current_uid: Option<i64>,
    ) -> Markup {
        html! {
            (default_header("Pallet Spaces: Post"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container" {
                    a href="/posts" { "← Back to posts" }
                    div class="card mt-3" {
                        h2 { (post.title) }
                        @let pretty_loc = post.geocoded_label.as_ref().map(|s| s.as_str()).unwrap_or(&post.location);
                        p class="text-muted" { strong { "Location: " } (pretty_loc) }
                        @match (post.latitude, post.longitude) { (Some(lat), Some(lon)) => p class="text-muted" { a class="btn btn--ghost" href=(format!("https://www.openstreetmap.org/?mlat={}&mlon={}#map=14/{}/{}", lat, lon, lat, lon)) { "View on map" } }, _ => {} }
                        p class="text-muted" { strong { "Price: " } (post.price) " /day" }
                        @let start_disp = format_date_display(&post.available_date);
                        @let end_disp = format_date_display(&post.end_date);
                        p class="text-muted" { strong { "Availability: " } (start_disp) " → " (end_disp) }
                        p class="text-muted" { strong { "Pallet spaces available: " } (post.spaces_available) }
                        @if !post.notes.is_empty() { div class="mt-2 text-muted" { (post.notes) } }
                        @if current_uid == Some(post.user_id) {
                            a class="btn btn--secondary mt-2" href=(format!("/posts/{}/edit", id)) { "Edit" }
                        } @else {
                            a class="btn btn--primary mt-2" href=(format!("/posts/{}/rent", id)) { "Rent this space" }
                        }
                    }
                }
            }
        }
    }

    // Shared post form page (used for create and edit)
    pub async fn post_form_page(is_auth: bool, heading: &str, action: &str, post: &super::Post) -> Markup {
        html! {
            (default_header("Pallet Spaces: Post"))
            (title_and_navbar(is_auth))
            body class="page" {
                div class="container" { h2 { (heading) } }
                form class="container card form" method="POST" action=(action) {
                    div class="field" { label class="label" for="title" { "Title:" } input class="input" type="text" id="title" name="title" value=(post.title) {} }
                    div class="field" {
                        label class="label" for="location" { "Location:" }
                        input class="input" type="text" id="location" name="location" value=(post.location)
                            hx-get="/api/geocode" hx-trigger="keyup changed delay:300ms" hx-target="#location-suggestions" hx-params="serialize" {}
                        div id="location-suggestions" class="help" {}
                        div id="location-preview" class="help" {}
                    }
                    div class="field" { label class="label" for="price" { "Price (per day):" } input class="input" type="number" id="price" name="price" min="0" step="1" value=(post.price) {} }
                    div class="field" { label class="label" for="spaces_available" { "Pallet spaces available:" } input class="input" type="number" id="spaces_available" name="spaces_available" min="1" step="1" value=(post.spaces_available) {} }
                    div class="field" { label class="label" for="available_date" { "Start date:" } input class="input" type="date" id="available_date" name="available_date" value=(post.available_date) {} }
                    div class="field" { label class="label" for="end_date" { "End date:" } input class="input" type="date" id="end_date" name="end_date" value=(post.end_date) {} }
                    div class="field" { label class="label" for="notes" { "Notes:" } textarea class="textarea" id="notes" name="notes" { (post.notes) } }
                    div { button class="btn btn--primary" type="submit" { "Save" } }
                }
            }
        }
    }
}
