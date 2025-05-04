use maud::{Markup, html, DOCTYPE};


pub fn default_header(page_name: &str) -> Markup {
    html!{
        (DOCTYPE)
        head {
            title { (page_name.to_owned()) }
            script src="/public/js/htmx_2.0.4/htmx.min.js" type="text/javascript" {}
        }
    }
}