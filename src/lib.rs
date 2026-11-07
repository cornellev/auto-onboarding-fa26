pub mod controls;
pub mod perception;
pub mod planning;
pub mod runtime;
pub mod shared;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/**
 * Creates a shared library error from a message.
 * @param message The message describing the failure.
 * @return A boxed I/O error containing the message.
 */
pub fn error(message: impl Into<String>) -> Error {
    std::io::Error::other(message.into()).into()
}
