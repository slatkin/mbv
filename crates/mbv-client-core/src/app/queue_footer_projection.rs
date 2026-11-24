//! The Queue footer projection every Client consumes (`extract-client-core`
//! design D5, row 6.4's footer): the status pills the queue panel's footer
//! paints, computed once from the `App` as plain text and theme roles so both
//! Clients read the same owner, playlist, autosave, and dirty rules. The TUI
//! shell builds its footer spans from this projection at its painter edge.

use mbv_theme::Role;

use crate::app::App;

/// One footer pill's presented facts: its text (a glyph, a word, or both, per
/// the App's Nerd Font setting), the glyph-free label behind that text, and
/// the foreground role it paints in. The
/// pill's background is the `StatusBarPill` surface both Clients paint behind
/// the footer's pills.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FooterPill {
    /// The pill's text, without outer pads.
    pub text: String,
    /// The pill's text without its glyph: the label (or word) alone, for a
    /// Client that paints no glyph (the GUI frame; the GUI change's design
    /// D8). Empty when the TUI text is the glyph alone (the playlist pill
    /// with no playlist loaded).
    pub label: String,
    /// The foreground role the text paints in.
    pub role: Role,
    /// Whether the text is emphasised (bold in the TUI).
    pub bold: bool,
}

/// The Queue footer's status pills for one tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueFooterProjection {
    /// The owner indicator, always present: the Player owner whose queue the
    /// panel shows (design D7).
    pub owner: FooterPill,
    /// Whether the shown queue's Player owner is live: the Client's link to
    /// it is up. The GUI paints its session marker's dot only while this
    /// holds (`add-tauri-gui-client` design D4a: the dot marks the live
    /// Owner).
    pub owner_live: bool,
    /// The playlist source pill: the glyph, and the playlist name while one is
    /// loaded.
    pub playlist: FooterPill,
    /// The autosave indicator, present only while save-on-consume is on for a
    /// saved playlist. Independent of the dirty marker.
    pub autosave: Option<FooterPill>,
    /// The dirty indicator, present only while the queue differs from its saved
    /// playlist.
    pub dirty: Option<FooterPill>,
}

impl App {
    /// Project the Queue footer's status pills (design D5). Read-only: a Client
    /// adopts the result each tick.
    #[must_use]
    pub fn queue_footer_projection(&self) -> QueueFooterProjection {
        QueueFooterProjection {
            owner: self.owner_footer_pill(),
            owner_live: !self.shown_queue_owner_player().is_remote_disconnected(),
            playlist: self.playlist_footer_pill(),
            autosave: self.autosave_footer_pill(),
            dirty: self.dirty_footer_pill(),
        }
    }

    /// The owner indicator: the icon, a space, and the shown owner's label.
    fn owner_footer_pill(&self) -> FooterPill {
        let icon = if self.use_nerd_fonts {
            "\u{f1616}"
        } else {
            "\u{1F5A7}"
        };
        let label = self.queue_owner_label();
        FooterPill {
            text: format!("{icon} {label}"),
            label,
            role: Role::TextFocusAccent,
            bold: false,
        }
    }

    /// The playlist source pill: the glyph alone when no playlist is loaded
    /// (muted), and the glyph with the name after a gap when one is (metadata).
    fn playlist_footer_pill(&self) -> FooterPill {
        let gap = if self.use_nerd_fonts { " " } else { "  " };
        let glyph = if self.use_nerd_fonts {
            "\u{f0411}"
        } else {
            "\u{1F5AD}"
        };
        match self.playback_queue().source() {
            mbv_queue::QueueSource::Playlist { name, .. } => FooterPill {
                text: format!("{glyph}{gap}{name}"),
                label: name.clone(),
                role: Role::TextMetadata,
                bold: false,
            },
            _ => FooterPill {
                text: glyph.to_string(),
                label: String::new(),
                role: Role::TextMuted,
                bold: false,
            },
        }
    }

    /// The autosave indicator: the Nerd Font glyph or the word, while
    /// save-on-consume is on for a saved playlist.
    fn autosave_footer_pill(&self) -> Option<FooterPill> {
        let autosave_on = self.queue_is_saved_playlist() && {
            let config = self.config.lock().unwrap();
            let cfg = &*config;
            cfg.save_playlist_on_consume || cfg.save_playlist_on_consume_audio
        };
        if !autosave_on {
            return None;
        }
        Some(FooterPill {
            text: if self.use_nerd_fonts {
                "\u{f18ea}".to_string()
            } else {
                "AUTOSAVE".to_string()
            },
            label: "AUTOSAVE".to_string(),
            role: Role::Accent,
            bold: false,
        })
    }

    /// The dirty indicator: the Nerd Font glyph or the word, emphasised, while
    /// the queue differs from its saved playlist.
    fn dirty_footer_pill(&self) -> Option<FooterPill> {
        if !self.queue_dirty {
            return None;
        }
        Some(FooterPill {
            text: if self.use_nerd_fonts {
                "\u{f0f42}".to_string()
            } else {
                "UNSAVED".to_string()
            },
            label: "UNSAVED".to_string(),
            role: Role::TextFocusAccent,
            bold: true,
        })
    }
}

#[cfg(test)]
mod tests;
