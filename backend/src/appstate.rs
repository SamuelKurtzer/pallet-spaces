use crate::model::database::Database;
use std::sync::Arc;

#[cfg(feature = "stripe")]
type StripeClientType = stripe::Client;
#[cfg(not(feature = "stripe"))]
struct StripeClientType;

#[derive(Clone)]
pub struct AppState {
    pub pool: Database,
    #[allow(dead_code)]
    pub stripe: Option<Arc<StripeClientType>>,
}

impl AppState {
    pub fn new(pool: Database) -> Self {
        AppState { pool, stripe: None }
    }

    pub fn new_with_stripe(pool: Database, stripe: Option<Arc<StripeClientType>>) -> Self {
        AppState { pool, stripe }
    }
}
