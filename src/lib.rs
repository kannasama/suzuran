pub mod config;
pub mod dal;
pub mod error;
pub mod models;
pub mod state;

mod app;
pub use app::build_router;
