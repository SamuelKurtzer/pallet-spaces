use std::fmt::Display;

use tokio::task::JoinError;

#[derive(Debug)]
pub enum Error {
    Database(String),
    SocketBind(String),
    Async(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self))
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Error::Database(format!("{:?}", value))
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        Error::Async(format!("{:?}", value))
    }
}