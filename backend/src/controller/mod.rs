use axum::Router;

use crate::appstate::AppState;

pub mod signup;

pub trait Routes {
    fn add_routes<T: RouteProvider>(self) -> Self;
}

pub trait RouteProvider {
    fn provide_routes(router: Router<AppState>) -> Router<AppState>;
}

impl Routes for Router<AppState> {
    fn add_routes<T: RouteProvider>(self) -> Self {
        T::provide_routes(self)
    }
}
