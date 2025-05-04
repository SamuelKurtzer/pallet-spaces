use maud::{Markup, html};
use super::utils::default_header;

pub async fn signup_page() -> Markup {
    html! {
        (default_header("Pallet Spaces: Signup"))
        body {
            form id="signupForm" action="signup" method="POST" hx-post="/signup" {
                label for="email" { "E-mail:" }
                br {}
                input type="text" id="email" name="email" {}
                br {}
                label for="Fullname" { "Fullname:" }
                br {}
                input type="text" id="name" name="name" {}
                br {}
                button type="submit" { "Submit" }
            }
        }
    }
}

pub async fn signup_success() -> Markup {
    html!{
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