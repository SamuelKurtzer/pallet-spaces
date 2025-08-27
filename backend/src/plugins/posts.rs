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
    pub notes: String,
}

impl Post {
    pub fn new(notes: &String) -> Self {
        Self {
            id: None,
            notes: notes.to_string(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct NewPost {
    pub notes: String,
}

mod model {
    use sqlx::Executor;

    use crate::{
        error::Error,
        model::database::{Database, DatabaseProvider},
    };

    use super::Post;
    impl Post {
        pub async fn get_all_posts(pool: &Database) -> Vec<Post> {
            let mut posts = vec![];
            for i in 0..20 {
                if let Ok(post) = Post::retrieve(i, pool).await {
                    posts.push(post);
                }
            }
            posts
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
        notes TEXT NOT NULL,
      )
      ",
                )
                .await;
            match creation_attempt {
                Ok(_) => Ok(pool),
                Err(_) => Err(Error::Database(
                    "Failed to create Post database tables".into(),
                )),
            }
        }

        async fn create(self, pool: &Database) -> Result<&Database, Error> {
            let attempt = sqlx::query("INSERT INTO Posts (notes) VALUES (?1)")
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
        extract::State,
        http::StatusCode,
        routing::{get},
    };
    use maud::Markup;

    use crate::{
        appstate::AppState,
        controller::RouteProvider,
        model::database::DatabaseComponent,
        plugins::posts::view::{new_post_failure, new_post_success},
    };

    use super::{NewPost, Post, view::create_post_page};

    impl RouteProvider for Post {
        fn provide_routes(router: Router<AppState>) -> Router<AppState> {
            router
                .route(
                    "/new_post",
                    get(Post::create_post_page).post(Post::new_post_request),
                )
                .route("/Posts", get(Post::post_list))
        }
    }

    impl Post {
        pub async fn create_post_page() -> (StatusCode, Markup) {
            (StatusCode::OK, create_post_page().await)
        }

        pub async fn new_post_request(
            State(state): State<AppState>,
            Form(payload): Form<NewPost>,
        ) -> (StatusCode, Markup) {
            let post = Post::new(&payload.notes);
            tracing::debug!("Signing up Post {:?}", post);
            let insert_result = state.pool.create(post).await;
            tracing::debug!("Creation success {:?}", insert_result);
            match insert_result {
                Ok(_) => (StatusCode::OK, new_post_success().await),
                Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, new_post_failure().await),
            }
        }

        pub async fn post_list(State(state): State<AppState>) -> (StatusCode, Markup) {
            let contents = maud::html! { ol {
                @for post in Post::get_all_posts(&state.pool).await {
                    li { (post) }
                }
            }};
            (StatusCode::OK, contents)
        }
    }
}

mod view {
    use maud::{Markup, html};

    use crate::views::utils::{default_header, title_and_navbar};

    pub async fn create_post_page() -> Markup {
        html! {
            (default_header("Pallet Spaces: Signup"))
            (title_and_navbar())
            body {
                form id="signupForm" action="signup" method="POST" hx-post="/signup" {
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

    pub async fn new_post_success() -> Markup {
        // This should redirect to the new post
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

    pub async fn new_post_failure() -> Markup {
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
}
