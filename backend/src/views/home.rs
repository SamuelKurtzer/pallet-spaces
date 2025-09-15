use crate::{
    model::database::Database,
    views::utils::title_and_navbar,
};
use axum_login::AuthSession;
use maud::{DOCTYPE, Markup, html};

use super::utils::default_header;
pub async fn main_page(auth: AuthSession<Database>) -> Markup {
    let is_auth = auth.user.is_some();
    html! {
        (DOCTYPE)
        (default_header("Pallet Spaces"))
        (title_and_navbar(is_auth))
        body {
            p { "hello world" }
        }
    }
}
