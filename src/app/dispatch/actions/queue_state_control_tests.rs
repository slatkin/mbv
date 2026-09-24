#![allow(dead_code, unused_imports)]

use super::*;
use crate::config::tests::SYS_ENV_LOCK as XDG_HOME_LOCK;

struct XdgHomeGuard {
    dir: std::path::PathBuf,
    _state_dir: crate::config::TestStateDirGuard,
}

impl XdgHomeGuard {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("MBV_SYSTEM");
        Self {
            _state_dir: crate::config::TestStateDirGuard::new_at(dir.join("mbv")),
            dir,
        }
    }
}

impl Drop for XdgHomeGuard {
    fn drop(&mut self) {
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn cycle_sub_local_idle_cycles_subtitle_mode_not_a_track() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.player.status.lock().unwrap().active = false;
    let before = app.config.lock().unwrap().subtitle_mode.clone();

    app.cycle_sub();

    let after = app.config.lock().unwrap().subtitle_mode.clone();
    assert_ne!(
        before, after,
        "idle z has no session equivalent, so it should still cycle the default subtitle mode"
    );
}

#[test]
fn cycle_sub_local_active_does_not_fall_back_to_subtitle_mode() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.sub_tracks = vec![(1, "English".to_string(), false)];
        status.sub_id = 0;
    }
    let before = app.config.lock().unwrap().subtitle_mode.clone();

    app.cycle_sub();

    let after = app.config.lock().unwrap().subtitle_mode.clone();
    assert_eq!(
        before, after,
        "an active player has tracks to cycle and must not touch the idle subtitle-mode fallback"
    );
}
