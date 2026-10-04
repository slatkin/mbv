use crate::DaemonRole;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConsumeKinds {
    pub videos: bool,
    pub audio: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnerSettings {
    pub stay_alive: bool,
    pub show_systray_icon: bool,
    pub consume: ConsumeKinds,
}

impl OwnerSettings {
    pub(crate) fn tray_enabled(self) -> bool {
        self.stay_alive || self.show_systray_icon
    }
}

pub(crate) type OwnerSettingsReader = Arc<dyn Fn() -> OwnerSettings + Send + Sync>;

impl From<&mbv_config::Config> for OwnerSettings {
    fn from(config: &mbv_config::Config) -> Self {
        Self {
            stay_alive: config.stay_alive,
            show_systray_icon: config.show_systray_icon,
            consume: ConsumeKinds {
                videos: config.consume_videos,
                audio: config.consume_audio,
            },
        }
    }
}

pub(crate) fn reader(role: DaemonRole, spawn_config: &mbv_config::Config) -> OwnerSettingsReader {
    let spawn_settings = OwnerSettings::from(spawn_config);
    let last_successful_read = Mutex::new(spawn_settings);
    Arc::new(move || {
        if role == DaemonRole::Packaged {
            return OwnerSettings {
                stay_alive: true,
                ..spawn_settings
            };
        }

        std::fs::read_to_string(mbv_config::config_path())
            .ok()
            .and_then(|text| mbv_config::parse_config(&text).ok())
            .map_or_else(
                || *last_successful_read.lock().unwrap(),
                |config| {
                    let settings = OwnerSettings::from(&config);
                    *last_successful_read.lock().unwrap() = settings;
                    settings
                },
            )
    })
}

#[cfg(test)]
pub(crate) fn fixed_reader(stay_alive: bool) -> OwnerSettingsReader {
    let settings = OwnerSettings {
        stay_alive,
        show_systray_icon: false,
        consume: ConsumeKinds {
            videos: false,
            audio: false,
        },
    };
    Arc::new(move || settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_owner_settings_reader_reloads_and_falls_back_to_last_successful_config() {
        let _state_dir = mbv_config::TestStateDirGuard::new();
        let spawn_config = mbv_config::Config::default();
        let settings = reader(DaemonRole::Local, &spawn_config);
        assert_eq!(settings(), OwnerSettings::from(&spawn_config));

        std::fs::write(
            mbv_config::config_path(),
            "[session]\nstay_alive = true\n[queue]\nconsume_audio = true\n",
        )
        .unwrap();
        let last_successful = settings();
        assert!(last_successful.stay_alive);
        assert!(last_successful.consume.audio);

        std::fs::write(mbv_config::config_path(), "invalid toml !!!").unwrap();
        assert_eq!(settings(), last_successful);
    }
}
