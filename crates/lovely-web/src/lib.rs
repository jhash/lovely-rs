pub mod auth;
pub mod errors;
pub mod handlers;
pub mod router;
pub mod state;
pub mod views;

pub use errors::{WebError, WebResult};
pub use router::router;
pub use state::AppState;
