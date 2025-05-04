use maud::{Markup, DOCTYPE, html};
use super::utils::default_header;
pub async fn main_page() -> Markup {
    html! {
        (DOCTYPE)
        (default_header("Pallet Spaces"))
        body {
            p { "hello world" }
        }
    }
}

