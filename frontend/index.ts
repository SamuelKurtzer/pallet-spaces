var form  = (<HTMLFormElement>document.getElementById('signupForm'));

form.onsubmit = function(event) {
    var xhr = new XMLHttpRequest();
    var formData = new FormData(form);

    xhr.open('POST', 'http://localhost:7000/tests/v1.0/form');
    xhr.setRequestHeader('Content-Type', 'application/json');

    xhr.send(JSON.stringify(formData));

    xhr.onreadystatechange = function() {
        if (xhr.readyState == XMLHttpRequest.DONE) {
            form.reset();
        }
    }
    return false;
}