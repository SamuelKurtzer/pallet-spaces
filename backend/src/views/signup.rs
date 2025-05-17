use maud::{Markup, html};
use crate::views::utils::title_and_navbar;

use super::utils::default_header;

pub async fn signup_page() -> Markup {
    html! {
        (default_header("Pallet Spaces: Signup"))
        (title_and_navbar())
        body {
            form id="signupForm" action="signup" method="POST" hx-post="/signup" {
                (email_form_html(true, ""))
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

pub fn email_form_html(valid: bool, email: &str) -> Markup {
    let validation_class = match valid {
        false => "invalid-form-input",
        true => "valid-form-input",
    };
    html! {
        div hx-target="this" hx-swap="outerHTML" {
            label for="email" { "E-mail:" }
            input type="text" id="email" name="email" class=(validation_class) hx-post="/signup/email" value=(email) { }
            br {}
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

pub async fn signup_failure() -> Markup {
    html!{
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

pub async fn login_page() -> Markup {
    html! {
        (default_header("Pallet Spaces: Login"))
        (title_and_navbar())
        body {
            form id="loginForm" action="login" method="POST" hx-post="/login" {
                (email_form_html(true, ""))
                label for="Password" { "Password:" }
                input type="text" id="password" name="password" {}
                br {}
                button type="submit" { "Submit" }
            }
        }
    }
}