mod error;
pub use error::EmbyError;
mod failure;
pub use failure::*;
mod types;
pub use types::*;
mod client_auth;
mod types_parsing;
pub use client_auth::*;
mod client_library;
pub use client_library::SortedItemsParams;
mod client_playlists;
mod client_reporting;
pub use client_playlists::*;
pub use client_reporting::ProgressReport;
mod client_sessions;

#[cfg(any(test, feature = "test"))]
pub mod test_support;

#[cfg(test)]
mod tests;
