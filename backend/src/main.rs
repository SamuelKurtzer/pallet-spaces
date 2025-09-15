mod appstate;
mod controller;
mod error;
mod model;
mod plugins;
mod views;
use appstate::AppState;
use axum::{Router, routing::get};
use controller::Routes;
use error::Error;
use model::database::{Database, DatabaseComponent};
use plugins::users::User;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use views::home::main_page;

use plugins::posts::Post;
use axum_login::AuthManagerLayerBuilder;
use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};

async fn create_database() -> Result<Database, Error> {
    let pool = Database::new().await?;
    // Initialize required tables
    let pool = pool.initialise_table::<User>().await?;
    let pool = pool.initialise_table::<Post>().await?;
    Ok(pool)
}

fn create_router(state: AppState) -> Router {
    Router::new()
        .route_service("/", get(main_page))
        .add_routes::<User>()
        .add_routes::<Post>()
        .nest_service("/public", ServeDir::new("./frontend/public/"))
        .with_state(state)
}

async fn create_listener() -> Result<TcpListener, Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 37373));
    tracing::info!("Serving app at: http://{}", addr);
    println!("Serving app at: http://{}", addr);
    match TcpListener::bind(addr).await {
        Ok(ok) => Ok(ok),
        Err(_) => Err(Error::SocketBind(
            "Failed to bind to specified socket".into(),
        )),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Tracing initialised.");

    let db = match create_database().await {
        Ok(db) => db,
        Err(err) => panic!("{:?}", err),
    };
    let state = AppState::new(db.clone());
    let app = create_router(state);

    // Set up session and auth layers for axum-login
    let session_layer = SessionManagerLayer::new(MemoryStore::default());
    let auth_layer = AuthManagerLayerBuilder::new(db, session_layer).build();
    let app = app.layer(auth_layer);
    let listener = match create_listener().await {
        Ok(listener) => listener,
        Err(err) => panic!("{:?}", err),
    };

    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header::SET_COOKIE, HeaderValue, Request, StatusCode},
    };
    use tower::ServiceExt;
    use axum_login::AuthManagerLayerBuilder;
    use axum_login::tower_sessions::{MemoryStore, SessionManagerLayer};

    async fn build_app() -> Router {
        let db = create_database().await.expect("db");
        let state = AppState::new(db.clone());
        let app = create_router(state);
        let session_layer = SessionManagerLayer::new(MemoryStore::default());
        let auth_layer = AuthManagerLayerBuilder::new(db, session_layer).build();
        app.layer(auth_layer)
    }

    #[tokio::test]
    async fn get_posts_index_ok() {
        let app = build_app().await;
        let res = app
            .oneshot(Request::get("/posts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_post_show_not_found() {
        let app = build_app().await;
        let res = app
            .oneshot(Request::get("/posts/9999").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    fn reset_db() {
        let _ = std::fs::remove_file("test.db");
    }

    fn cookie_header_from_response(res: &axum::response::Response) -> Option<HeaderValue> {
        let mut cookie_kv = vec![];
        for val in res.headers().get_all(SET_COOKIE).iter() {
            if let Ok(s) = val.to_str() {
                if let Some((kv, _attrs)) = s.split_once(';') {
                    cookie_kv.push(kv.trim().to_string());
                }
            }
        }
        if cookie_kv.is_empty() {
            None
        } else {
            HeaderValue::from_str(&cookie_kv.join("; ")).ok()
        }
    }

    #[tokio::test]
    async fn signup_login_me_flow_ok() {
        reset_db();
        let app = build_app().await;

        // Signup
        let body = "name=Alice&email=alice%40example.com&password=supersecret";
        let res = app
            .clone()
            .oneshot(
                Request::post("/signup")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Login
        let body = "email=alice%40example.com&password=supersecret";
        let res = app
            .clone()
            .oneshot(
                Request::post("/login")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let cookie = cookie_header_from_response(&res).expect("set-cookie");

        // Access /me with session cookie
        let res = app
            .oneshot(
                Request::get("/me")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn signup_duplicate_email_conflict() {
        reset_db();
        let app = build_app().await;

        let body = "name=Bob&email=bob%40example.com&password=supersecret";
        let res = app
            .clone()
            .oneshot(
                Request::post("/signup")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Try again with same email
        let res = app
            .oneshot(
                Request::post("/signup")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn login_wrong_password_unauthorized() {
        reset_db();
        let app = build_app().await;

        // Create account
        let body = "name=Eve&email=eve%40example.com&password=supersecret";
        let _ = app
            .clone()
            .oneshot(
                Request::post("/signup")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Attempt login with wrong password
        let body = "email=eve%40example.com&password=wrong";
        let res = app
            .oneshot(
                Request::post("/login")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
