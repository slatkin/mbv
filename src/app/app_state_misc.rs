use super::{App, PanelFocus, POWER_LEFT_WIDTH_DEFAULT};
use std::time::{Duration, Instant};

impl App {
    pub(super) fn queue_column_width_max_for_terminal(terminal_width: u16) -> u16 {
        POWER_LEFT_WIDTH_DEFAULT.max(terminal_width.saturating_mul(3) / 5)
    }

    pub(super) fn normalize_queue_column_width(width: u16, terminal_width: u16) -> u16 {
        width.clamp(
            POWER_LEFT_WIDTH_DEFAULT,
            Self::queue_column_width_max_for_terminal(terminal_width),
        )
    }

    pub(super) fn clamp_queue_column_width(&mut self) -> bool {
        let normalized =
            Self::normalize_queue_column_width(self.queue_column_width, self.terminal_width);
        if normalized == self.queue_column_width {
            return false;
        }
        self.queue_column_width = normalized;
        true
    }

    /// Record that the terminal just regained focus, arming the
    /// refocus-click suppression window (see `handle_mouse`).
    pub(super) fn note_focus_gained(&mut self) {
        self.refocus_at = Some(Instant::now());
    }

    /// Clear any pending refocus suppression -- the window shouldn't
    /// outlive the focus session that armed it.
    pub(super) fn note_focus_lost(&mut self) {
        self.refocus_at = None;
    }

    pub(super) fn set_panel_focus(&mut self, focus: PanelFocus) {
        if self.panel_focus == focus {
            return;
        }
        if matches!(focus, PanelFocus::Queue) {
            self.focus_power_queue_initial_item();
        }
        self.panel_focus = focus;
        self.save_prefs();
    }

    pub(super) fn extrapolated_remote_position(remote_pos_s: i64, elapsed: Duration) -> i64 {
        remote_pos_s + elapsed.as_secs() as i64
    }

    pub(super) fn ui_config_snapshot(&self) -> crate::config::UiConfig {
        let indicator_style = match self.indicator_style {
            super::render::indicators::IndicatorStyle::Brackets => "brackets",
            super::render::indicators::IndicatorStyle::Chips => "chips",
            super::render::indicators::IndicatorStyle::Outlined => "outlined",
            super::render::indicators::IndicatorStyle::Dots => "dots",
            super::render::indicators::IndicatorStyle::Pipes => "pipes",
            super::render::indicators::IndicatorStyle::KeyValue => "keyvalue",
            super::render::indicators::IndicatorStyle::Powerline => "powerline",
        };
        crate::config::UiConfig {
            image_protocol: self.image_protocol.clone(),
            image_cache_size: self.image_cache_size,
            use_nerd_fonts: self.use_nerd_fonts,
            indicator_style: indicator_style.to_string(),
        }
    }

    /// Whether the run loop should touch the terminal this tick. `false`
    /// only while a stay-alive session is detached (`self.attached ==
    /// false`) — see the `attached` field doc for why `Terminal::clear()`
    /// must never be called in that state (issue #156). Skipping renders
    /// while detached loses nothing: the next attach's reattach-refresh
    /// (`take_attach_pending()`) forces `force_clear` and a full repaint.
    pub(super) fn wants_terminal_render(
        &self,
        had_events: bool,
        last_render: Instant,
        render_interval: Duration,
    ) -> bool {
        self.attached
            && (had_events || self.force_clear || last_render.elapsed() >= render_interval)
    }

    /// How often the run loop should repaint while otherwise idle (no key
    /// events, no completed fetches to react to). Fast (150 ms) whenever
    /// something is visibly in motion -- active local/remote playback, or a
    /// card image fetch in flight -- so states that only resolve with the
    /// passage of time (like a loading placeholder swapping in once its box
    /// should be reserved) actually get painted instead of being skipped
    /// between "just started" and "just finished" with nothing in between.
    /// Falls back to a slow 1 s cadence when nothing is changing, to avoid
    /// spinning the terminal for no reason.
    pub(super) fn render_interval(&self) -> Duration {
        let playback = self.effective_playback_state();
        if playback.active
            || self.connected_session_state.is_some()
            || !self.card_image_loading.is_empty()
        {
            Duration::from_millis(150)
        } else {
            Duration::from_secs(1)
        }
    }
}
