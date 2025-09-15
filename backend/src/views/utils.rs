use maud::{DOCTYPE, Markup, html};

pub fn default_header(page_name: &str) -> Markup {
    html! {
        (DOCTYPE)
        head {
            title { (page_name.to_owned()) }
            script src="/public/js/htmx_2.0.4/htmx.min.js" type="text/javascript" {}
        }
    }
}

pub fn title_and_navbar(is_auth: bool) -> Markup {
    html! {
        h1 { "Pallet Spaces" }
        ul {
            li { a href="/" { "Home" }}
            li { a href="/posts" { "Posts" }}
            @if !is_auth {
                li { a href="/signup" { "Signup" }}
                li { a href="/login" { "Login" }}
            } @else {
                li { a href="/me" { "My Account" }}
                li { a href="/users" { "Users" }}
                li { form action="/logout" method="POST" { button type="submit" { "Logout" } } }
            }
        }
    }
}

pub fn page_not_found() -> Markup {
    html! {
        h1 { "404: Page not found" }
    }
}
