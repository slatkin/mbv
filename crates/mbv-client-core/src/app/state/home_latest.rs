use mbv_ui_model::home_latest::HomeLatestLaunchWindow;
use std::time::{SystemTime, UNIX_EPOCH};

/// Capture the launch window: the launch instant, plus the previous launch
/// from the saved launch state, which is replaced by the launch instant. The
/// caller reads the clock; this function handles the saved copy.
#[must_use]
pub fn capture_launch_window() -> HomeLatestLaunchWindow {
    let current = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if current == 0 {
        tracing::warn!(name: "home_latest.launch_cutoff.invalid", target: "home_latest", current, "launch cutoff is not positive; markers disabled");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    let previous = mbv_config::load_home_latest_launch();
    if let Err(error) = mbv_config::save_home_latest_launch(current) {
        tracing::warn!(name: "home_latest.launch_cutoff.save_failed", target: "home_latest", { error = %error }, "could not save launch cutoff");
        return HomeLatestLaunchWindow {
            previous: None,
            current,
        };
    }
    HomeLatestLaunchWindow { previous, current }
}
