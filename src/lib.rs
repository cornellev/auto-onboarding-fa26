pub mod grid;
pub mod hardware;
pub mod reporting;
pub mod runner;
pub mod sim;
pub mod student;
pub mod types;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub fn error(message: impl Into<String>) -> Error {
    std::io::Error::other(message.into()).into()
}
