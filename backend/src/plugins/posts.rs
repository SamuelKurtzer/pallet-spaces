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

        pub async fn get_posts_filtered(
            pool: &Database,
            filter: &crate::plugins::posts::control::PostsFilter,
        ) -> Vec<Post> {
            use sqlx::{Arguments, sqlite::SqliteArguments};

            let mut sql = String::from("SELECT * FROM Posts");
            let mut args = SqliteArguments::default();
            let mut cond: Vec<&str> = Vec::new();

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
        notes TEXT NOT NULL
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
        plugins::posts::view::{new_post_failure, new_post_success},
    };

    use super::{NewPost, Post, view::create_post_page};
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
                .route("/posts/{id}", get(Post::show_post_page).post(Post::edit_post_request))
        }
    }

    impl Post {
        pub async fn create_post_page() -> (StatusCode, Markup) {
            (StatusCode::OK, create_post_page().await)
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
            let contents = maud::html! {
                (crate::views::utils::default_header("Pallet Spaces: Posts"))
                (crate::views::utils::title_and_navbar(false))
                body style="font-family: system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif; background:#f8fafc; margin:0; padding:16px;" {
                    h2 style="max-width: 860px; margin: 8px auto 16px; font-size: 1.5rem;" { "Available Spaces" }
                    form method="GET" action="/posts" style="max-width: 860px; margin: 0 auto 16px; background:#fff; border:1px solid #e5e7eb; border-radius:10px; padding:12px; display:grid; grid-template-columns: repeat(6, 1fr); gap:8px; align-items:end;" {
                        div style="display:flex; flex-direction:column; gap:4px;" {
                            label for="title" { "Title" }
                            input type="text" id="title" name="title" value=(filter.title.clone().unwrap_or_default()) {}
                        }
                        div style="display:flex; flex-direction:column; gap:4px;" {
                            label for="location" { "Location" }
                            input type="text" id="location" name="location" value=(filter.location.clone().unwrap_or_default()) {}
                        }
                        div style="display:flex; flex-direction:column; gap:4px;" {
                            label for="max_price" { "Max Price/wk" }
                            input type="number" id="max_price" name="max_price" min="0" step="1" value=(filter.max_price.clone().unwrap_or_default()) {}
                        }
                        div style="display:flex; flex-direction:column; gap:4px;" {
                            label for="min_spaces_available" { "Min Spaces" }
                            input type="number" id="min_spaces_available" name="min_spaces_available" min="0" step="1" value=(filter.min_spaces_available.clone().unwrap_or_default()) {}
                        }
                        div style="display:flex; flex-direction:column; gap:4px;" {
                            label for="min_stay_value" { "Min Stay" }
                            input type="number" id="min_stay_value" name="min_stay_value" min="0" step="1" value=(filter.min_stay_value.clone().unwrap_or_default()) {}
                        }
                        div style="display:flex; flex-direction:column; gap:4px;" {
                            label for="min_stay_unit" { "Unit" }
                            select id="min_stay_unit" name="min_stay_unit" {
                                @let unit = filter.min_stay_unit.clone().unwrap_or_default();
                                option value="" selected[unit == ""] { "Any" }
                                option value="weeks" selected[unit == "weeks"] { "Weeks" }
                                option value="months" selected[unit == "months"] { "Months" }
                            }
                        }
                        div style="display:flex; flex-direction:column; gap:4px; grid-column: span 2;" {
                            label for="available_from" { "Available From" }
                            input type="date" id="available_from" name="available_from" value=(filter.available_from.clone().unwrap_or_default()) {}
                        }
                        div style="grid-column: span 4; text-align:right;" {
                            button type="submit" style="background:#0ea5e9; color:#fff; border:none; border-radius:8px; padding:8px 12px; cursor:pointer;" { "Filter" }
                            a href="/posts" style="margin-left:8px; color:#334155;" { "Reset" }
                        }
                    }
                    @if posts.is_empty() {
                        p style="max-width: 860px; margin: 0 auto; color:#475569;" { "No posts yet." }
                    } @else {
                        div id="posts" style="display:grid; gap:12px; grid-template-columns: 1fr; max-width: 860px; margin: 0 auto;" {
                            @for p in posts {
                                div class="post-card" style="background:#fff; border:1px solid #e5e7eb; border-radius:10px; padding:14px 16px; box-shadow: 0 1px 2px rgba(0,0,0,0.05);" {
                                    @match p.id_raw() {
                                        Some(id) => h3 style="margin:0 0 8px 0; font-size:1.1rem;" { a href=(format!("/posts/{}", id)) style="color:#0ea5e9; text-decoration:none;" { (p.title) } },
                                        None => h3 style="margin:0 0 8px 0; font-size:1.1rem; color:#0f172a;" { (p.title) }
                                    }
                                    p style="margin:4px 0; color:#334155;" { strong { "Location: " } (p.location) }
                                    p style="margin:4px 0; color:#334155;" { strong { "Price: " } (p.price) " /week" }
                                    p style="margin:4px 0; color:#334155;" { strong { "Minimum stay: " } (p.min_stay_value) " " (p.min_stay_unit) }
                                    p style="margin:4px 0; color:#334155;" { strong { "Available from: " } (p.available_date) }
                                    p style="margin:4px 0; color:#334155;" { strong { "Pallet spaces available: " } (p.spaces_available) }
                                    @if !p.notes.is_empty() {
                                        p style="margin:8px 0 0; color:#475569;" { (p.notes) }
                                    }
                                    @if current_uid == Some(p.user_id) {
                                        @match p.id_raw() {
                                            Some(id) => div style="margin-top:8px;" {
                                                a href=(format!("/posts/{}/edit", id)) style="display:inline-block; background:#64748b; color:#fff; border:none; border-radius:8px; padding:6px 10px; text-decoration:none;" { "Edit" }
                                            },
                                            None => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
            (StatusCode::OK, contents)
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
            let contents = maud::html! {
                (crate::views::utils::default_header("Pallet Spaces: Post"))
                (crate::views::utils::title_and_navbar(false))
                body style="font-family: system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif; background:#f8fafc; margin:0; padding:16px;" {
                    div style="max-width: 860px; margin: 0 auto;" {
                        a href="/posts" style="color:#0ea5e9; text-decoration:none;" { "← Back to posts" }
                        div style="background:#fff; border:1px solid #e5e7eb; border-radius:10px; padding:16px; margin-top:12px; box-shadow: 0 1px 2px rgba(0,0,0,0.05);" {
                            h2 style="margin:0 0 10px 0;" { (post.title) }
                            p style="margin:4px 0; color:#334155;" { strong { "Location: " } (post.location) }
                            p style="margin:4px 0; color:#334155;" { strong { "Price: " } (post.price) " /week" }
                            p style="margin:4px 0; color:#334155;" { strong { "Minimum stay: " } (post.min_stay_value) " " (post.min_stay_unit) }
                            p style="margin:4px 0; color:#334155;" { strong { "Available from: " } (post.available_date) }
                            p style="margin:4px 0; color:#334155;" { strong { "Pallet spaces available: " } (post.spaces_available) }
                            @if !post.notes.is_empty() {
                                div style="margin-top:8px; color:#475569; white-space:pre-wrap;" { (post.notes) }
                            }
                            @if current_uid == Some(post.user_id) {
                                a href=(format!("/posts/{}/edit", id)) style="display:inline-block; margin-top:10px; background:#64748b; color:#fff; border:none; border-radius:8px; padding:6px 10px; text-decoration:none;" { "Edit" }
                            }
                        }
                    }
                }
            };
            (StatusCode::OK, contents)
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
            let contents = maud::html! {
                (crate::views::utils::default_header("Pallet Spaces: Edit Post"))
                (crate::views::utils::title_and_navbar(false))
                body style="font-family: system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif; background:#f8fafc; margin:0; padding:16px;" {
                    h2 style="max-width: 860px; margin: 8px auto 16px; font-size: 1.5rem;" { "Edit Post" }
                    form method="POST" action=(format!("/posts/{}", id)) style="max-width: 860px; margin: 0 auto; background:#fff; border:1px solid #e5e7eb; border-radius:10px; padding:12px; display:grid; grid-template-columns: 1fr; gap:10px;" {
                        label for="title" { "Title:" }
                        input type="text" id="title" name="title" value=(post.title) {}

                        label for="location" { "Location:" }
                        input type="text" id="location" name="location" value=(post.location) {}

                        label for="price" { "Price (per week):" }
                        input type="number" id="price" name="price" min="0" step="1" value=(post.price) {}

                        label for="spaces_available" { "Pallet spaces available:" }
                        input type="number" id="spaces_available" name="spaces_available" min="1" step="1" value=(post.spaces_available) {}

                        label for="min_stay_value" { "Minimum Stay:" }
                        input type="number" id="min_stay_value" name="min_stay_value" min="1" value=(post.min_stay_value) {}
                        label for="min_stay_unit" { "Unit" }
                        select id="min_stay_unit" name="min_stay_unit" {
                            option value="weeks" selected[post.min_stay_unit == "weeks"] { "Weeks" }
                            option value="months" selected[post.min_stay_unit == "months"] { "Months" }
                        }

                        label for="available_date" { "Available Date:" }
                        input type="date" id="available_date" name="available_date" value=(post.available_date) {}

                        label for="notes" { "Notes:" }
                        textarea id="notes" name="notes" { (post.notes) }

                        div { button type="submit" style="background:#0ea5e9; color:#fff; border:none; border-radius:8px; padding:8px 12px; cursor:pointer;" { "Save" } }
                    }
                }
            };
            (StatusCode::OK, contents)
        }

        pub async fn edit_post_request(
            State(state): State<AppState>,
            auth: AuthSession<crate::model::database::Database>,
            axum::extract::Path(id): axum::extract::Path<u32>,
            Form(payload): Form<EditPost>,
        ) -> (StatusCode, Markup) {
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
                    // Redirect back to posts list (simple success page for now)
                    (StatusCode::OK, maud::html!{ p { "Post updated." }})
                }
                _ => (StatusCode::FORBIDDEN, crate::views::utils::page_not_found()),
            }
        }
    }
}

mod view {
    use maud::{Markup, html};

    use crate::views::utils::{default_header, title_and_navbar};

    pub async fn create_post_page() -> Markup {
        html! {
            (default_header("Pallet Spaces: New Post"))
            (title_and_navbar(false))
            body {
                form id="postForm" action="new_post" method="POST" hx-post="/new_post" {
                    label for="title" { "Title:" }
                    input type="text" id="title" name="title" {}
                    br {}

                    label for="location" { "Location:" }
                    input type="text" id="location" name="location" {}
                    br {}

                    label for="price" { "Price (per week):" }
                    input type="number" id="price" name="price" min="0" step="1" {}
                    br {}

                    label for="spaces_available" { "Pallet spaces available:" }
                    input type="number" id="spaces_available" name="spaces_available" min="1" step="1" {}
                    br {}

                    label for="min_stay_value" { "Minimum Stay:" }
                    input type="number" id="min_stay_value" name="min_stay_value" min="1" {}
                    select id="min_stay_unit" name="min_stay_unit" {
                        option value="weeks" { "Weeks" }
                        option value="months" { "Months" }
                    }
                    br {}

                    label for="available_date" { "Available Date:" }
                    input type="date" id="available_date" name="available_date" {}
                    br {}

                    label for="notes" { "Notes:" }
                    textarea id="notes" name="notes" {} "" 
                    br {}
                    button type="submit" { "Create Post" }
                }
            }
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
}
