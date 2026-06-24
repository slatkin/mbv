use super::{SettingKey, SETTING_SECTIONS};
use crate::config::Config;

pub fn setting_label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::DaemonModeOnExit    => "Daemon mode on exit",
        SettingKey::StartOnQueue        => "Start on queue",
        SettingKey::AlwaysPlayNext      => "Always play next",
        SettingKey::ConsumeVideos           => "Consume videos",
        SettingKey::SavePlaylistOnConsume   => "Save playlist on consume",
        SettingKey::AlwaysSkipIntro     => "Always skip intro",
        SettingKey::ShowLogTab          => "Show log tab",
        SettingKey::ImageProtocol       => "Image protocol",
        SettingKey::HiddenLibraries     => "Hidden libraries",
        SettingKey::HiddenLatest        => "Hidden latest",
        SettingKey::ShowAudioWindow     => "Show audio window",
        SettingKey::UseMpvConfig        => "Use mpv config",
        SettingKey::NoScripts           => "No scripts",
        SettingKey::Autoload            => "autoload",
        SettingKey::ShowSysTrayIcon     => "Show systray icon",
        SettingKey::SystemNotifications => "System notifications",
        SettingKey::SubtitleMode        => "Subtitle mode",
        SettingKey::SubtitleLanguage    => "Subtitle language",
        SettingKey::AudioLanguage       => "Audio language",
        SettingKey::LogOut              => "Log out",
    }
}

pub fn setting_value(key: SettingKey, cfg: &Config, prefs: &crate::player::SubtitlePrefs) -> String {
    match key {
        SettingKey::DaemonModeOnExit    => bool_val(cfg.daemon_mode_on_exit),
        SettingKey::StartOnQueue        => bool_val(cfg.start_on_queue),
        SettingKey::AlwaysPlayNext      => bool_val(cfg.always_play_next),
        SettingKey::ConsumeVideos           => bool_val(cfg.consume_videos),
        SettingKey::SavePlaylistOnConsume   => bool_val(cfg.save_playlist_on_consume),
        SettingKey::AlwaysSkipIntro     => bool_val(cfg.always_skip_intro),
        SettingKey::ShowLogTab          => bool_val(cfg.show_log_tab),
        SettingKey::ImageProtocol       => cfg.image_protocol.clone().unwrap_or_else(|| "none".into()),
        SettingKey::HiddenLibraries     => fmt_hidden_list(&cfg.hidden_libraries),
        SettingKey::HiddenLatest        => fmt_hidden_list(&cfg.hidden_latest),
        SettingKey::ShowAudioWindow     => bool_val(cfg.show_audio_window),
        SettingKey::UseMpvConfig        => bool_val(cfg.use_mpv_config),
        SettingKey::NoScripts           => bool_val(cfg.no_scripts),
        SettingKey::Autoload            => bool_val(cfg.autoload),
        SettingKey::ShowSysTrayIcon     => bool_val(cfg.show_systray_icon),
        SettingKey::SystemNotifications => bool_val(cfg.system_notifications),
        SettingKey::SubtitleMode        => if prefs.mode.is_empty() { "Default".into() } else { prefs.mode.clone() },
        SettingKey::SubtitleLanguage    => if prefs.subtitle_lang.is_empty() { "any".into() } else { prefs.subtitle_lang.clone() },
        SettingKey::AudioLanguage       => if prefs.audio_lang.is_empty() { "any".into() } else { prefs.audio_lang.clone() },
        SettingKey::LogOut              => String::new(),
    }
}

pub fn fmt_hidden_list(list: &[String]) -> String {
    match list.len() {
        0 => "none".into(),
        1 => list[0].clone(),
        n => format!("{n} hidden"),
    }
}

pub fn bool_val(v: bool) -> String { if v { "on".into() } else { "off".into() } }

pub fn settings_total_rows() -> usize {
    SETTING_SECTIONS.iter().map(|(_, keys)| keys.len()).sum()
}

pub fn settings_cursor_to_key(cursor: usize) -> SettingKey {
    let mut idx = 0;
    for &(_, keys) in SETTING_SECTIONS {
        for &key in keys {
            if idx == cursor { return key; }
            idx += 1;
        }
    }
    SettingKey::LogOut
}
