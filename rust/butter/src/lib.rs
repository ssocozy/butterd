#![forbid(unsafe_code)]
pub mod engine;
pub mod hardware;
pub mod journal;
pub mod lock;
pub mod model;
pub mod android;
pub mod presets;
pub mod runtime;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub mod events;

pub mod vendor;

pub mod display;

pub mod gpu;

pub mod audio;

pub mod preferences;

pub mod touch;

pub mod notifications;
