include!("config_types_paths.rs");
include!("config_tests_shared_data.rs");
include!("config_types_feed.rs");
include!("config_paths.rs");
include!("config_state.rs");
include!("config_credentials.rs");
include!("config_emby_admin.rs");
include!("config_parse.rs");
include!("config_save.rs");
include!("config_audiobookshelf_lifecycle.rs");
include!("config_emby_lifecycle.rs");

#[cfg(any(test, feature = "test-support"))]
pub mod tests {
    #[cfg(test)]
    use super::*;
    #[cfg(test)]
    use std::time::{SystemTime, UNIX_EPOCH};
    include!("config_tests_parse.rs");
    include!("config_tests_paths.rs");
    include!("config_tests_credentials.rs");
    include!("config_tests_paths_migration.rs");
    include!("config_tests_emby_admin.rs");
}
