use maud::{Markup, DOCTYPE, html};
use super::utils::default_header;

/*
<form hx-post="/store">
    <input id="title" name="title" type="text"
        hx-post="/validate"
        hx-trigger="change"
        hx-sync="closest form:abort">
    <button type="submit">Submit</button>
</form>

<!DOCTYPE html>
<html>
<body>
<script src="index.js"></script>
<link rel="stylesheet" href="assets/style.css">
<div class="center">
<h1>Pallet Spaces</h1>
<p></p>
<form id="signupForm" action="signup" method="POST">
    <label for="email">E-mail:</label><br>
    <input type="text" id="email" name="email"><br>
    <label for="name">Name:</label><br>
    <input type="text" id="name" name="name"><br>
    <br>    
    <input type="submit" value="Submit"><br>
</form>
</div>

</body>
</html>
*/
pub async fn signup_page() -> Markup {
    html! {
        (DOCTYPE)
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