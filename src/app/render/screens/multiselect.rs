use super::super::super::App;
use super::super::super::{MultiSelectKind, MultiSelectPopup};

impl App {
    pub(crate) fn open_multiselect_popup(&mut self, kind: MultiSelectKind) {
        if matches!(kind, MultiSelectKind::MyLanguages) {
            const ALL_LANGS: &[&str] = &[
                "English",
                "French",
                "German",
                "Spanish",
                "Italian",
                "Portuguese",
                "Japanese",
                "Korean",
                "Chinese",
                "Russian",
                "Arabic",
                "Dutch",
                "Swedish",
                "Norwegian",
                "Danish",
                "Finnish",
                "Polish",
                "Czech",
                "Turkish",
            ];
            let my_langs = self.config.lock().unwrap().my_languages.clone();
            let items = ALL_LANGS
                .iter()
                .map(|&name| {
                    let selected = my_langs.iter().any(|l| l == name);
                    (name.to_lowercase(), name.to_string(), selected)
                })
                .collect();
            self.multiselect_popup = Some(MultiSelectPopup {
                kind,
                items,
                cursor: 0,
            });
            return;
        }
        let Some(client) = self.emby_client() else {
            return;
        };
        let client = client.lock().unwrap();
        let all = match kind {
            MultiSelectKind::HiddenLibraries => client.get_views().unwrap_or_default(),
            MultiSelectKind::HiddenLatest => client.get_user_views().unwrap_or_default(),
            MultiSelectKind::FeedViewLibraries => client.get_views().unwrap_or_default(),
            MultiSelectKind::MyLanguages => unreachable!(),
        };
        let config = self.config.lock().unwrap();
        let selected_list = match kind {
            MultiSelectKind::HiddenLibraries => &config.hidden_libraries,
            MultiSelectKind::HiddenLatest => &config.hidden_latest,
            MultiSelectKind::FeedViewLibraries => &config.feed_view_libraries,
            MultiSelectKind::MyLanguages => unreachable!(),
        };
        let items: Vec<(String, String, bool)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let is_hidden = selected_list.contains(&lower);
                (lower, v.name.clone(), is_hidden)
            })
            .collect();
        drop(config);
        drop(client);
        self.multiselect_popup = Some(MultiSelectPopup {
            kind,
            items,
            cursor: 0,
        });
    }

    pub(crate) fn close_multiselect_popup(&mut self) {
        let Some(popup) = self.multiselect_popup.take() else {
            return;
        };

        if matches!(popup.kind, MultiSelectKind::MyLanguages) {
            let selected: Vec<String> = popup
                .items
                .iter()
                .filter(|(_, _, is_sel)| *is_sel)
                .map(|(_, name, _)| name.clone())
                .collect();
            {
                let mut c = self.config.lock().unwrap();
                if !selected.is_empty() {
                    if !c.subtitle_lang.is_empty() && !selected.contains(&c.subtitle_lang) {
                        c.subtitle_lang = String::new();
                    }
                    if !c.audio_lang.is_empty() && !selected.contains(&c.audio_lang) {
                        c.audio_lang = String::new();
                    }
                }
                c.my_languages = selected;
            }
            let cfg = self.config.lock().unwrap().clone();
            {
                let mut p = self.player.subtitle_prefs.lock().unwrap();
                p.subtitle_lang = cfg.subtitle_lang.clone();
                p.audio_lang = cfg.audio_lang.clone();
            }
            if let Err(e) = crate::config::save_config_settings(&cfg) {
                log::warn!(target: "config", "config save failed: {e}");
            }
            return;
        }

        let hidden: Vec<String> = popup
            .items
            .iter()
            .filter(|(_, _, is_hidden)| *is_hidden)
            .map(|(lower, _, _)| lower.clone())
            .collect();
        {
            let mut c = self.config.lock().unwrap();
            match popup.kind {
                MultiSelectKind::HiddenLibraries => c.hidden_libraries = hidden.clone(),
                MultiSelectKind::HiddenLatest => c.hidden_latest = hidden.clone(),
                MultiSelectKind::FeedViewLibraries => c.feed_view_libraries = hidden.clone(),
                MultiSelectKind::MyLanguages => unreachable!(),
            }
        }
        match popup.kind {
            MultiSelectKind::HiddenLibraries => self.hidden_libraries = hidden,
            MultiSelectKind::HiddenLatest => self.hidden_latest = hidden,
            MultiSelectKind::FeedViewLibraries => {
                for lib in &mut self.libs {
                    lib.nav_stack.clear();
                }
            }
            MultiSelectKind::MyLanguages => unreachable!(),
        }
        let cfg = self.config.lock().unwrap().clone();
        if let Err(e) = crate::config::save_config_settings(&cfg) {
            log::warn!(target: "config", "config save failed: {e}");
        }
        let _ = self.fetch_home();
    }
}
