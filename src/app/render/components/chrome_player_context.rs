use super::chrome_player::{PlaybackRenderContext, PlaybackStripAreas};
use crate::app::{palette, App, PanelFocus, PanelMode};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{FeedEntry, PlaybackTitleParts, QueueItem};
use ratatui::layout::Rect;
use ratatui::style::Color;

impl App {
    /// Production playback panels build their own painter context from the
    /// shell's transport projection (task 3.5); this App-side builder stays
    /// as the characterization seam the frozen render tests call.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn playback_panel_context<'a>(
        &'a mut self,
        area: Rect,
        playback: &'a mut PlaybackStripAreas,
        player_h: u16,
        show_controls: bool,
        now_playing_title: &Option<(String, Color)>,
        // The resolved colour this parameter used to carry is threaded as the
        // panel's surface identity below. The parameter keeps its position
        // and type because the characterization callers in
        // `src/app/render/tests.rs` pass it positionally; production callers
        // name the surface through the mode match, so the colour is no
        // longer read.
        _panel_bg: Color,
    ) -> PlaybackRenderContext<'a> {
        // The site's own focus input: the queue column's bit, the same
        // expression the component projection path computes in
        // `shell_playback`. The Queue-only strip is a fixed chrome band, so
        // the row ignores the bit there (design D3(a)).
        let panel_focused = matches!(self.effective_panel_focus(), PanelFocus::Queue);
        let panel = match self.effective_panel_mode() {
            PanelMode::QueueOnly => palette::Surface::QueueOnlyPlaybackPanel,
            _ => palette::Surface::PlaybackPanel,
        };
        PlaybackRenderContext {
            area,
            playback,
            player_h,
            show_controls,
            now_playing_title: now_playing_title.clone(),
            panel,
            panel_focused,
            progress: self.playback_progress(),
            use_nerd_fonts: self.use_nerd_fonts,
            stop_available: self.connected_session_id.is_some()
                || self.player.status.lock().unwrap().active,
            next_available: self.transport_prev_next_available().1,
            status_indicators: self.build_status_indicator_spans(),
            throbber: self.now_playing_throbber_span(),
            title_parts: self.active_playback_title_parts(),
            idle_feed_title: self.idle_feed.as_ref().and_then(|feed| {
                feed.items.get(feed.current_index).map(|item| {
                    (
                        item.title.clone(),
                        item.link.as_deref().is_some_and(|link| !link.is_empty()),
                    )
                })
            }),
            marquee_text: &mut self.marquee_text,
            marquee_started_at: &mut self.marquee_started_at,
        }
    }

    /// The active queue slot's now-playing title parts (D4): the media-type
    /// mapping lives on `QueueItem` in core (D1) and the App layer only
    /// contributes the caller-resolved feed subscription display name.
    /// `None` when nothing in the local queue is playing (a cast target or
    /// remote Session the queue cannot address).
    pub(in crate::app) fn active_playback_title_parts(&self) -> Option<PlaybackTitleParts> {
        let playback = self.effective_playback_state();
        let idx = playback.active_idx.filter(|_| playback.active)?;
        let item = self.playback_queue().item_at(idx)?;
        Some(self.playback_title_parts(item))
    }

    /// The now-playing title parts for one queue item: core's media-type
    /// mapping, with the App-resolved feed subscription display name passed
    /// in for feed items only. The parts carry closed roles (D6); the
    /// painter's `title_part_fg` is the one site that resolves them to
    /// colours.
    pub(in crate::app) fn playback_title_parts(&self, item: &QueueItem) -> PlaybackTitleParts {
        let feed_subscription_name = item
            .as_feed()
            .and_then(|entry| self.feed_subscription_display_name(entry));
        item.playback_title_parts(feed_subscription_name.as_deref())
    }

    /// The display name of the configured subscription whose url matches the
    /// entry's `feed_id` (the subscription url recorded at fetch time, the
    /// same match the feed tab uses). `None` when the entry carries no
    /// `feed_id` or no configured subscription matches (task 3.2).
    pub(in crate::app) fn feed_subscription_display_name(
        &self,
        entry: &FeedEntry,
    ) -> Option<String> {
        let feed_id = entry.feed_id.as_deref()?;
        self.config
            .lock()
            .unwrap()
            .feeds
            .iter()
            .find(|subscription| subscription.url == feed_id)
            .map(|subscription| subscription.name.clone())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn playback_progress(&self) -> (i64, i64, bool) {
        if let Some(ref remote) = self.connected_session_state {
            let elapsed_s = self.remote_pos_at.elapsed().as_secs_f64();
            let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
            (
                (pos_s * TICKS_PER_SECOND as f64) as i64,
                remote.runtime_s * TICKS_PER_SECOND,
                self.playback_transport_paused(),
            )
        } else {
            let status = self.player.status.lock().unwrap();
            (status.position_ticks, status.runtime_ticks, status.paused)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use mbv_core::config::{FeedKind, FeedSubscription};

    fn feed_entry(feed_id: Option<&str>) -> FeedEntry {
        FeedEntry {
            guid: "g1".into(),
            title: "Entry Title".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: feed_id.map(str::to_string),
            position_ticks: 0,
            played: false,
        }
    }

    /// The feed subscription display name (task 3.2): only an entry whose
    /// `feed_id` matches a configured subscription's url resolves a context
    /// part; a non-matching entry and a `None` `feed_id` resolve `None`.
    #[test]
    fn feed_subscription_display_name_resolves_only_a_matching_entry() {
        let app = make_app_stub();
        app.config.lock().unwrap().feeds.push(FeedSubscription {
            name: "Example Daily".into(),
            url: "https://example.com/feed.xml".into(),
            kind: FeedKind::Audio,
        });

        let matching = QueueItem::Feed(feed_entry(Some("https://example.com/feed.xml")));
        let parts = app.playback_title_parts(&matching);
        assert_eq!(
            parts.context.as_ref().map(|part| part.text.as_str()),
            Some("Example Daily"),
            "a matching subscription resolves to its display name"
        );

        let non_matching = QueueItem::Feed(feed_entry(Some("https://other.example.org/rss")));
        let parts = app.playback_title_parts(&non_matching);
        assert!(
            parts.context.is_none(),
            "a non-matching entry degrades to the title part alone"
        );

        let identity_less = QueueItem::Feed(feed_entry(None));
        let parts = app.playback_title_parts(&identity_less);
        assert!(
            parts.context.is_none(),
            "a `None` feed_id degrades to the title part alone"
        );
    }
}
