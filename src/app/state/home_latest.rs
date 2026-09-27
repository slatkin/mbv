use mbv_ui_model::home_latest::HomeLatestLaunchWindow;
use std::time::{SystemTime, UNIX_EPOCH};

#[must_use]
pub(crate) fn current_launch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[must_use]
pub(crate) fn capture_launch_window(current: u64) -> HomeLatestLaunchWindow {
    if current == 0 {
        log::warn!(target: "home_latest", "invalid non-positive launch cutoff; markers disabled");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    let previous = mbv_config::load_home_latest_launch();
    if let Err(error) = mbv_config::save_home_latest_launch(current) {
        log::warn!(target: "home_latest", "could not save launch cutoff: {error}");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    HomeLatestLaunchWindow { previous, current }
}
