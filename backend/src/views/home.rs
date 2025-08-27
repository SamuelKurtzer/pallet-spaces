use crate::views::utils::title_and_navbar;
use maud::{DOCTYPE, Markup, html};

use super::utils::default_header;
pub async fn main_page() -> Markup {
    html! {
        (DOCTYPE)
        (default_header("Pallet Spaces"))
        (title_and_navbar())
        body {
            p { "hello world" }
        }
    }
}
