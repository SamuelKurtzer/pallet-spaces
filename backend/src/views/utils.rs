use maud::{DOCTYPE, Markup, html};

pub fn default_header(page_name: &str) -> Markup {
    html! {
        (DOCTYPE)
        head {
            title { (page_name.to_owned()) }
            meta name="viewport" content="width=device-width, initial-scale=1" {}
            link rel="stylesheet" href="/public/css/main.css" {}
            script src="/public/js/htmx_2.0.4/htmx.min.js" type="text/javascript" {}
        }
    }
}

pub fn title_and_navbar(is_auth: bool) -> Markup {
    html! {
        header class="site-header" {
            div class="container" {
                div class="cluster" {
                    // Left: brand + primary nav
                    div class="cluster" {
                        h1 { "Pallet Spaces" }
                        nav {
                            ul class="nav" {
                                li { a href="/" { "Home" }}
                                li { a href="/posts" { "Posts" }}
                                @if is_auth {
                                    li { a href="/orders" { "Orders" }}
                                }
                            }
                        }
                    }
                    // Right: auth actions
                    nav {
                        ul class="nav" {
                            @if !is_auth {
                                li { a class="btn btn--secondary" href="/signup" { "Signup" }}
                                li { a class="btn btn--primary" href="/login" { "Login" }}
                            } @else {
                                li { a class="btn btn--secondary" href="/me" { "Account" }}
                                li { form action="/logout" method="POST" { button class="btn btn--secondary" type="submit" { "Log off" } } }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn page_not_found() -> Markup {
    html! {
        h1 { "404: Page not found" }
    }
}
