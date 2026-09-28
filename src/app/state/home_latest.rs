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
        tracing::warn!(name: "home_latest.launch_cutoff.invalid", target: "home_latest", current, "launch cutoff is not positive; markers disabled");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    let previous = mbv_config::load_home_latest_launch();
    if let Err(error) = mbv_config::save_home_latest_launch(current) {
        tracing::warn!(name: "home_latest.launch_cutoff.save_failed", target: "home_latest", { error.message = %error }, "could not save launch cutoff");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    HomeLatestLaunchWindow { previous, current }
}
