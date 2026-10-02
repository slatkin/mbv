use mbv_queue::{FeedKind, LibraryPositionState, QueueState, ServiceKind};

mod error;
pub use error::ConfigError;
mod types_paths;
pub(crate) use types_paths::config_dir;
pub use types_paths::{
    Config, DEFAULT_SYSTEM_DAEMON_TCP_LISTEN, DEFAULT_VIDEO_CACHE_BACK_MB,
    DEFAULT_VIDEO_CACHE_FORWARD_MB, cache_dir, default_daemon_server_tcp_listen,
    is_system_instance, is_valid_audio_device, state_dir,
};
mod types_setup;
pub use types_setup::{AudiobookshelfSetup, EmbySetup};
mod launch_state;
pub use launch_state::{
    AudiobookshelfBookBucket, AudiobookshelfPodcastFilter, AudiobookshelfSelectorKey,
    EmbyLetterBucket, EmbySelectorKey, FeedGroupKey, FeedsFilter, FeedsSelectorKey,
    HomeSelectorKey, LaunchPanelFocus, LibraryItemIdentity, SelectorIdentity,
    TUI_LAUNCH_STATE_VERSION, TabIdentity, TuiLaunchState, TuiLaunchStateError,
    load_tui_launch_state, save_tui_launch_state, tui_launch_state_path,
};
mod test_support;
#[cfg(test)]
pub(crate) use test_support::TEST_DEFAULT_STATE_DIR;
#[cfg(any(test, feature = "test"))]
pub(crate) use test_support::{TEST_CONFIG_DIR_OVERRIDE, TEST_STATE_DIR_OVERRIDE};
#[cfg(any(test, feature = "test"))]
pub use test_support::{TestStateDirGuard, TestTempDir, remove_test_env_var, set_test_env_var};
mod types_feed;
pub use types_feed::FeedSubscription;
mod paths;
pub use paths::{
    ScriptSource, config_path, control_socket_path, data_dir_system_or_local,
    home_latest_launch_path, library_position_state_path, mpv_config_dir, mpv_ipc_path,
    osc_fonts_dir, osc_fonts_source, osc_script_source, prefs_path, queue_state_path,
    resolve_script_source, stay_alive_queue_state_path, token_cache_path,
};
#[cfg(test)]
pub(crate) use paths::{checkout_fonts_dir, checkout_scripts_entry};
mod state;
pub use state::{
    LastRemoteConnection, StayAliveQueueState, clear_queue_state, legacy_queue_for_owner_if_absent,
    load_home_latest_launch, load_last_remote_connection, load_library_position_state,
    load_queue_state, load_stay_alive_queue_state, load_stay_alive_queue_state_at,
    save_home_latest_launch, save_last_remote_connection, save_library_position_state,
    save_library_position_state_result, save_queue_state, save_stay_alive_queue_state,
    save_stay_alive_queue_state_at,
};
#[cfg(test)]
pub(crate) use state::{load_last_remote_connection_at, save_last_remote_connection_at};
mod credentials;
pub(crate) use credentials::save_service_secret_at;
pub use credentials::{
    clear_control_credential, clear_control_credential_result, clear_service_secret,
    clear_service_secret_result, control_credential_path, load_control_credential,
    load_or_create_control_credential, load_service_secret, save_control_credential,
    save_service_secret, service_secret_path,
};
mod emby_admin;
pub use emby_admin::{
    migrate_legacy_emby_token, persist_emby_setup_and_secret, remove_emby_setup_and_secret,
    save_emby_setup,
};
#[cfg(test)]
pub(crate) use emby_admin::{persist_emby_setup_and_secret_at, save_emby_setup_at};
mod panel;
pub use panel::{DEFAULT_PANEL_COLS, PANEL_COLS_MIN, PanelConfig, PanelSide};
mod parse;
pub use parse::{load_config, parse_config, parse_feeds};
mod save;
pub(crate) use save::write_config_text_at;
pub use save::{ConfigSection, save_config_section, save_config_settings};
#[cfg(test)]
pub(crate) use save::{save_config_section_at, save_config_settings_at};
mod audiobookshelf_lifecycle;
#[cfg(test)]
pub(crate) use audiobookshelf_lifecycle::{
    audiobookshelf_transaction, save_audiobookshelf_setup_at,
};
pub use audiobookshelf_lifecycle::{
    persist_audiobookshelf_setup_and_secret, remove_audiobookshelf_setup_and_secret,
    remove_audiobookshelf_setup_and_secret_with_owned_state,
    replace_audiobookshelf_setup_and_secret,
};
mod emby_lifecycle;
pub use emby_lifecycle::{
    EmbyOwnedStateSnapshot, clear_emby_owned_state, replace_emby_setup_and_secret,
    restore_emby_owned_state, snapshot_emby_owned_state,
};

#[cfg(any(test, feature = "test"))]
pub mod tests;
