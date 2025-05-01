#[derive(Debug)]
pub enum Error {
    Database(&'static str),
    SocketBind(&'static str),
}