include!("config_types_paths.rs");
include!("config_types_feed.rs");
include!("config_paths.rs");
include!("config_parse.rs");
include!("config_save.rs");

#[cfg(any(test, feature = "test-support"))]
pub mod tests {
    #[cfg(test)]
    use super::*;
    #[cfg(test)]
    use std::time::{SystemTime, UNIX_EPOCH};
    include!("config_tests_parse.rs");
    include!("config_tests_paths.rs");
}
