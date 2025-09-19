mod appstate;
mod controller;
mod error;
mod model;
mod plugins;
mod views;
use appstate::AppState;
use axum::{Router, routing::get};
use axum::http::{header::HeaderName, Request};
use controller::Routes;
use error::Error;
use model::database::{Database, DatabaseComponent};
use plugins::users::User;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::{services::ServeDir, trace::{TraceLayer, DefaultOnRequest, DefaultOnResponse, DefaultOnFailure}};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;
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
    // Structured tracing with env filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,axum=info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
    tracing::info!("Tracing initialised");

    let db = match create_database().await {
        Ok(db) => db,
        Err(err) => panic!("{:?}", err),
    };
    let state = AppState::new(db.clone());
    let app = create_router(state);

    // Set up session and auth layers for axum-login
    let session_layer = SessionManagerLayer::new(MemoryStore::default());
    let auth_layer = AuthManagerLayerBuilder::new(db, session_layer).build();
    // Request ID + Trace layers
    let x_request_id = HeaderName::from_static("x-request-id");
    let x_request_id_for_span = x_request_id.clone();
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(move |req: &Request<_>| {
            let method = req.method().clone();
            let uri = req.uri().clone();
            let req_id = req
                .headers()
                .get(&x_request_id_for_span)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-");
            tracing::info_span!(
                "http.request",
                method = %method,
                uri = %uri,
                request_id = %req_id
            )
        })
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        .on_failure(DefaultOnFailure::new().level(Level::ERROR));

    let app = app
        .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
        .layer(SetRequestIdLayer::new(x_request_id.clone(), MakeRequestUuid))
        .layer(trace_layer)
        .layer(auth_layer);
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
    use axum::body::to_bytes;

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

        // Signup should redirect to /me and set a session
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
    async fn me_shows_user_posts() {
        reset_db();
        let app = build_app().await;

        // Signup (auto-login + redirect)
        let body = "name=Poster&email=poster%40example.com&password=supersecret";
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
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let cookie = cookie_header_from_response(&res).expect("set-cookie");

        // Create a new post as this user
        let body = "title=WarehouseA&location=CityCenter&price=100&spaces_available=10&min_stay_value=4&min_stay_unit=weeks&available_date=2025-01-01&notes=Dry";

        let res = app
            .clone()
            .oneshot(
                Request::post("/new_post")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie.clone())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Fetch /me and assert the post title appears
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

        let body_bytes = to_bytes(res.into_body(), 64 * 1024).await.expect("body bytes");
        let body_str = String::from_utf8_lossy(&body_bytes);
        assert!(body_str.contains("My Posts"), "/me page should include My Posts section");
        assert!(body_str.contains("WarehouseA"), "/me page should list the user's post title");
    }

    fn extract_first_post_id_in_body(body: &str) -> Option<u32> {
        if let Some(href_idx) = body.find("href=\"/posts/") {
            let start = href_idx + "href=\"/posts/".len();
            let digits: String = body[start..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            return digits.parse::<u32>().ok();
        }
        None
    }

    #[tokio::test]
    async fn hide_post_excludes_from_public_list() {
        reset_db();
        let app = build_app().await;

        // Signup
        let res = app
            .clone()
            .oneshot(
                Request::post("/signup")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("name=Owner&email=owner%40example.com&password=supersecret"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let cookie = cookie_header_from_response(&res).expect("set-cookie");

        // Create post
        let body = "title=WarehouseB&location=Dock&price=200&spaces_available=5&min_stay_value=2&min_stay_unit=weeks&available_date=2025-01-01&notes=Cool";
        let _ = app
            .clone()
            .oneshot(
                Request::post("/new_post")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("cookie", cookie.clone())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Confirm visible on /posts
        let res = app
            .clone()
            .oneshot(Request::get("/posts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body_bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let list_html = String::from_utf8_lossy(&body_bytes);
        assert!(list_html.contains("WarehouseB"));

        // Get /me to extract post id
        let res = app
            .clone()
            .oneshot(
                Request::get("/me")
                    .header("cookie", cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body_bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let me_html = String::from_utf8_lossy(&body_bytes);
        let post_id = extract_first_post_id_in_body(&me_html).expect("post id in /me");

        // Toggle visibility (hide)
        let res = app
            .clone()
            .oneshot(
                Request::post(format!("/posts/{}/toggle_visibility", post_id))
                    .header("cookie", cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

        // Now WarehouseB should not appear on /posts
        let res = app
            .clone()
            .oneshot(Request::get("/posts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body_bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let list_html = String::from_utf8_lossy(&body_bytes);
        assert!(!list_html.contains("WarehouseB"));

        // But on /me it should appear with (hidden)
        let res = app
            .oneshot(
                Request::get("/me")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body_bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let me_html = String::from_utf8_lossy(&body_bytes);
        assert!(me_html.contains("WarehouseB"));
        assert!(me_html.contains("(hidden)"));
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
        assert_eq!(res.status(), StatusCode::SEE_OTHER);

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
