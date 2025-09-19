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
    pub min_stay_value: i64,    // e.g., 4
    pub min_stay_unit: String,  // "weeks" or "months"
    pub available_date: String, // YYYY-MM-DD
    pub notes: String,
    pub visible: i64,
}

impl Post {
    pub fn new(
        title: &str,
        location: &str,
        price: i64,
        user_id: i64,
        spaces_available: i64,
        min_stay_value: i64,
        min_stay_unit: &str,
        available_date: &str,
        notes: &str,
    ) -> Self {
        Self {
            id: None,
            title: title.to_string(),
            location: location.to_string(),
            price,
            user_id,
            spaces_available,
            min_stay_value,
            min_stay_unit: min_stay_unit.to_string(),
            available_date: available_date.to_string(),
            notes: notes.to_string(),
            visible: 1,
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
    pub min_stay_value: i64,
    pub min_stay_unit: String,
    pub available_date: String,
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
        pub min_stay_value: i64,
        pub min_stay_unit: String,
        pub available_date: String,
        pub notes: String,
    }

    impl Post {
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
            if let Some(ref min_stay_val) = filter.min_stay_value {
                if let Ok(v) = min_stay_val.trim().parse::<i64>() {
                    cond.push("min_stay_value >= ?");
                    let _ = args.add(v);
                }
            }
            if let Some(ref unit) = filter.min_stay_unit {
                if !unit.is_empty() {
                    cond.push("min_stay_unit = ?");
                    let _ = args.add(unit);
                }
            }
            if let Some(ref avail_from) = filter.available_from {
                if !avail_from.is_empty() {
                    cond.push("available_date >= ?");
                    let _ = args.add(avail_from);
                }
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
        min_stay_value INTEGER NOT NULL,
        min_stay_unit TEXT NOT NULL,
        available_date TEXT NOT NULL,
        notes TEXT NOT NULL,
        visible INTEGER NOT NULL DEFAULT 1
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
                        "ALTER TABLE Posts ADD COLUMN min_stay_value INTEGER NOT NULL DEFAULT 0",
                        "ALTER TABLE Posts ADD COLUMN min_stay_unit TEXT NOT NULL DEFAULT 'weeks'",
                        "ALTER TABLE Posts ADD COLUMN available_date TEXT NOT NULL DEFAULT ''",
                        "ALTER TABLE Posts ADD COLUMN visible INTEGER NOT NULL DEFAULT 1",
                    ];
                    for stmt in migrations { let _ = pool.0.execute(stmt).await; }
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
                    title, location, price, user_id, spaces_available, min_stay_value, min_stay_unit, available_date, notes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(self.title)
            .bind(self.location)
            .bind(self.price)
            .bind(self.user_id)
            .bind(self.spaces_available)
            .bind(self.min_stay_value)
            .bind(self.min_stay_unit)
            .bind(self.available_date)
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

        async fn update(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
            todo!()
        }

        async fn delete(id: Self::Id, pool: &Database) -> Result<&Database, Error> {
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
        model::database::{DatabaseComponent, DatabaseProvider},
        plugins::posts::view::{new_post_failure, new_post_success, post_form_page},
    };

    use super::{NewPost, Post, view::{posts_index_page, post_show_page_view}};
    use crate::plugins::posts::model::EditPost;

    #[derive(Debug, Default, Deserialize)]
    pub struct PostsFilter {
        pub title: Option<String>,
        pub location: Option<String>,
        pub max_price: Option<String>,
        pub min_spaces_available: Option<String>,
        pub min_stay_value: Option<String>,
        pub min_stay_unit: Option<String>,
        pub available_from: Option<String>,
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
        }
    }

    impl Post {
        pub async fn create_post_page(
        ) -> (StatusCode, Markup) {
            // No auth state in nav for now; page link is already gated in UI
            let is_auth = false;
            // Provide sensible defaults for a new post
            let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
            let draft = Post::new(
                "",
                "",
                0,
                0,
                1,
                1,
                "weeks",
                &today,
                "",
            );
            (StatusCode::OK, post_form_page(is_auth, "Create Post", "/new_post", &draft).await)
        }

        pub async fn new_post_request(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            Form(payload): Form<NewPost>,
        ) -> (StatusCode, Markup) {
            let uid = auth
                .user
                .as_ref()
                .map(|u| u.id() as i64)
                .unwrap_or(0);
            let post = Post::new(
                &payload.title,
                &payload.location,
                payload.price,
                uid,
                payload.spaces_available,
                payload.min_stay_value,
                &payload.min_stay_unit,
                &payload.available_date,
                &payload.notes,
            );
            tracing::debug!("Signing up Post {:?}", post);
            let insert_result = state.pool.create(post).await;
            tracing::debug!("Creation success {:?}", insert_result);
            match insert_result {
                Ok(_) => (StatusCode::OK, new_post_success().await),
                Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, new_post_failure().await),
            }
        }

        pub async fn post_list(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            Query(filter): Query<PostsFilter>,
        ) -> (StatusCode, Markup) {
            let posts = Post::get_posts_filtered(&state.pool, &filter).await;
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64);
            let is_auth = auth.user.is_some();
            (StatusCode::OK, posts_index_page(is_auth, &filter, &posts, current_uid).await)
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
            let current_uid = auth.user.as_ref().map(|u| u.id() as i64);
            let is_auth = auth.user.is_some();
            (StatusCode::OK, post_show_page_view(is_auth, id, &post, current_uid).await)
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
            // Ensure only owner can update
            let res = sqlx::query(
                "UPDATE Posts SET title=?, location=?, price=?, spaces_available=?, min_stay_value=?, min_stay_unit=?, available_date=?, notes=? WHERE id=? AND user_id=?"
            )
            .bind(payload.title)
            .bind(payload.location)
            .bind(payload.price)
            .bind(payload.spaces_available)
            .bind(payload.min_stay_value)
            .bind(payload.min_stay_unit)
            .bind(payload.available_date)
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

    pub async fn new_post_success() -> Markup {
        // This should redirect to the new post
        html! {
            (default_header("Pallet Spaces: New Post"))
            body {
                h2 {
                    "Post created"
                }
                p {
                    "Your post has been created successfully."
                }
            }
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

    // Index page for posts with filters and list
    pub async fn posts_index_page(
        is_auth: bool,
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
                        @if is_auth { a class="btn btn--success" href="/new_post" { "New Post" } }
                    }
                }
                form class="container card form" method="GET" action="/posts" {
                    div class="grid grid--2" {
                        div class="field" { label class="label" for="title" { "Title" } input class="input" type="text" id="title" name="title" value=(filter.title.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="location" { "Location" } input class="input" type="text" id="location" name="location" value=(filter.location.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="max_price" { "Max Price/wk" } input class="input" type="number" id="max_price" name="max_price" min="0" step="1" value=(filter.max_price.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="min_spaces_available" { "Min Spaces" } input class="input" type="number" id="min_spaces_available" name="min_spaces_available" min="0" step="1" value=(filter.min_spaces_available.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="min_stay_value" { "Min Stay" } input class="input" type="number" id="min_stay_value" name="min_stay_value" min="0" step="1" value=(filter.min_stay_value.clone().unwrap_or_default()) {} }
                        div class="field" { label class="label" for="min_stay_unit" { "Unit" } select class="select" id="min_stay_unit" name="min_stay_unit" {
                            @let unit = filter.min_stay_unit.clone().unwrap_or_else(|| "weeks".to_string());
                            option value="" selected[unit == ""] { "Any" }
                            option value="weeks" selected[unit == "weeks"] { "Weeks" }
                            option value="months" selected[unit == "months"] { "Months" }
                        } }
                        div class="field" style="grid-column: 1 / -1;" { label class="label" for="available_from" { "Available From" } input class="input" type="date" id="available_from" name="available_from" value=(filter.available_from.clone().unwrap_or_default()) {} }
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
                                p class="text-muted" { strong { "Location: " } (p.location) }
                                p class="text-muted" { strong { "Price: " } (p.price) " /week" }
                                p class="text-muted" { strong { "Minimum stay: " } (p.min_stay_value) " " (p.min_stay_unit) }
                                @let date_disp = format_date_display(&p.available_date);
                                p class="text-muted" { strong { "Available from: " } (date_disp) }
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
                        p class="text-muted" { strong { "Location: " } (post.location) }
                        p class="text-muted" { strong { "Price: " } (post.price) " /week" }
                        p class="text-muted" { strong { "Minimum stay: " } (post.min_stay_value) " " (post.min_stay_unit) }
                        @let date_disp = format_date_display(&post.available_date);
                        p class="text-muted" { strong { "Available from: " } (date_disp) }
                        p class="text-muted" { strong { "Pallet spaces available: " } (post.spaces_available) }
                        @if !post.notes.is_empty() { div class="mt-2 text-muted" { (post.notes) } }
                        @if current_uid == Some(post.user_id) {
                            a class="btn btn--secondary mt-2" href=(format!("/posts/{}/edit", id)) { "Edit" }
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
                    div class="field" { label class="label" for="location" { "Location:" } input class="input" type="text" id="location" name="location" value=(post.location) {} }
                    div class="field" { label class="label" for="price" { "Price (per week):" } input class="input" type="number" id="price" name="price" min="0" step="1" value=(post.price) {} }
                    div class="field" { label class="label" for="spaces_available" { "Pallet spaces available:" } input class="input" type="number" id="spaces_available" name="spaces_available" min="1" step="1" value=(post.spaces_available) {} }
                    div class="field" { label class="label" for="min_stay_value" { "Minimum stay value:" } input class="input" type="number" id="min_stay_value" name="min_stay_value" min="0" step="1" value=(post.min_stay_value) {} }
                    div class="field" { label class="label" for="min_stay_unit" { "Minimum stay unit:" } select class="select" id="min_stay_unit" name="min_stay_unit" { option value="weeks" selected[post.min_stay_unit == "weeks"] { "Weeks" } option value="months" selected[post.min_stay_unit == "months"] { "Months" } } }
                    div class="field" { label class="label" for="available_date" { "Available Date:" } input class="input" type="date" id="available_date" name="available_date" value=(post.available_date) {} }
                    div class="field" { label class="label" for="notes" { "Notes:" } textarea class="textarea" id="notes" name="notes" { (post.notes) } }
                    div { button class="btn btn--primary" type="submit" { "Save" } }
                }
            }
        }
    }
}
