pub mod api;
pub mod config;
pub mod dal;
pub mod error;
pub mod jobs;
pub mod models;
pub mod organizer;
pub mod scanner;
pub mod scheduler;
pub mod services;
pub mod state;
pub mod tagger;

mod app;
pub use app::build_router;
