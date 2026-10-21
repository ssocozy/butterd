#![forbid(unsafe_code)]
pub mod engine;
pub mod hardware;
pub mod journal;
pub mod model;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
