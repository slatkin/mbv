use crate::app::{App, LibEvent, ModelContentEvent, PanelFocus, dispatch::notify::ToastSeverity};
use mbv_ctrl::player::PlayerCommand;
use mbv_ws::WsEvent;

impl App {
    pub(in crate::app) fn handle_ws_event(&mut self, ev: WsEvent) {
        match ev {
            WsEvent::Play {
                item_ids,
                play_now,
                start_position_ticks,
                start_index,
            } => self.handle_ws_play(&item_ids, play_now, start_position_ticks, start_index),
            WsEvent::Stop => {
                self.player.stop();
            }
            WsEvent::Pause => {
                self.player.set_paused(true);
            }
            WsEvent::Unpause => {
                self.player.set_paused(false);
            }
            WsEvent::NextTrack => {
                self.player.next();
            }
            WsEvent::PreviousTrack => {
                self.player.previous();
            }
            WsEvent::TogglePause => {
                self.player.send_command(PlayerCommand::TogglePause);
            }
            WsEvent::Seek(ticks) => {
                self.player.send_command(PlayerCommand::SeekAbsolute(
                    mbv_emby_model::ticks_to_seconds(ticks),
                ));
            }
            WsEvent::SeekRelative(secs) => {
                self.player.send_command(PlayerCommand::Seek(secs));
            }
            WsEvent::SetVolume(v) => {
                let vol_max = self.player.status.lock().unwrap().volume_max;
                self.player
                    .send_command(PlayerCommand::SetVolume(v.clamp(0, vol_max)));
            }
            WsEvent::VolumeUp => {
                let st = self.player.status.lock().unwrap();
                let v = (st.volume + 5).min(st.volume_max);
                drop(st);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::VolumeDown => {
                let v = self.player.status.lock().unwrap().volume.saturating_sub(5);
                self.player.send_command(PlayerCommand::SetVolume(v));
            }
            WsEvent::SetMute(muted) => {
                self.mute_on = muted;
                self.player.send_command(PlayerCommand::SetMute(muted));
                self.save_prefs();
            }
            WsEvent::ToggleMute => {
                let muted = !self.player.status.lock().unwrap().muted;
                self.mute_on = muted;
                self.player.send_command(PlayerCommand::SetMute(muted));
                self.save_prefs();
            }
            WsEvent::SetAudio(index) => {
                self.player.send_command(PlayerCommand::SetAudio(index));
            }
            WsEvent::SetSub(index) => {
                let sid = self
                    .player
                    .status
                    .lock()
                    .unwrap()
                    .subtitle_stream_index_to_mpv_id(index);
                if let Some(sid) = sid {
                    self.player.send_command(PlayerCommand::SetSub(sid));
                }
            }
            WsEvent::UserDataChanged => {
                // The fetch runs synchronously (order-sensitive side
                // effects); the computed content travels to Model-owned
                // `home_content` via lib_tx (task 5.3d).
                if let Ok(content) = self.fetch_home() {
                    let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                        ModelContentEvent::HomeContentRefreshed(Box::new(content)),
                    ));
                }
            }
        }
    }

    fn handle_ws_play(
        &mut self,
        item_ids: &[String],
        play_now: bool,
        start_position_ticks: i64,
        start_index: usize,
    ) {
        tracing::info!(name: "ws.play.requested", target: "ws", item_count = item_ids.len(), play_now, "play request received");
        if !play_now {
            return;
        }
        self.on_queue_replace_silent();
        let items = {
            let Some(client) = self.emby_client() else {
                self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                return;
            };
            let c = client.lock().unwrap();
            match c.get_items_by_ids(item_ids) {
                Ok(v) => v,
                Err(e) => {
                    let msg = format!("Couldn't play from remote: {e}");
                    drop(c);
                    self.flash(msg, ToastSeverity::Error);
                    return;
                }
            }
        };
        if items.is_empty() {
            tracing::warn!(name: "ws.play.items_not_found", target: "ws", item_ids = %item_ids.join(","), "play request items not found");
            return;
        }
        let start_idx = start_index.min(items.len().saturating_sub(1));
        self.set_panel_focus(PanelFocus::Queue);
        self.queue_source = mbv_queue::QueueSource::Remote;
        if items.len() == 1 {
            let mut item = items[0].clone();
            if start_position_ticks > 0 {
                item.playback_position_ticks = start_position_ticks;
            }
            self.player_tab.set_items(vec![item.clone()], 0);
            self.flash(item.playback_label(), ToastSeverity::Neutral);
            self.submit_tab_queue(
                self.playing_queue_scope(),
                0,
                mbv_queue::QueueSource::Remote,
            );
        } else {
            tracing::info!(name: "ws.play.multiple_items", target: "ws", item_count = items.len(), start_index = start_idx, "playing multiple items");
            // Always hand the whole list to play_queue (not just the clicked
            // item) so the remote-controlled queue continues past start_idx.
            // play_queue already handles the "something is already playing"
            // case in place via unified queue submission.
            let mut items_with_pos = items.clone();
            if start_position_ticks > 0 {
                items_with_pos[start_idx].playback_position_ticks = start_position_ticks;
            }
            self.player_tab.set_items(items_with_pos, start_idx);
            // Keep the tab's canonical slot identities when starting
            // this replacement; do not mint a fresh sequential run.
            self.submit_tab_queue(
                self.playing_queue_scope(),
                start_idx,
                mbv_queue::QueueSource::Remote,
            );
        }
        self.save_queue_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use rstest::rstest;

    /// Install a stub Emby runtime whose transport is `http`, so the handler's
    /// synchronous fetch is served from scripted in-memory responses (no real
    /// server, per the mocks-only test policy).
    fn app_with_mock_emby(http: &mbv_net::mock_http::MockHttp) -> crate::app::App {
        let mut app = make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
        app
    }

    /// Two server items with distinct resume positions so a case can tell the
    /// honoured `start_position_ticks` apart from the value the fetch returned.
    const PLAY_TWO_ITEMS: &str = r#"{"Items":[
        {"Id":"a","Name":"A","Type":"Movie","MediaType":"Video","UserData":{"PlaybackPositionTicks":111}},
        {"Id":"b","Name":"B","Type":"Movie","MediaType":"Video","UserData":{"PlaybackPositionTicks":222}}
    ]}"#;

    /// `Play` issues a real Emby fetch (`get_items_by_ids`), so its queue
    /// replacement, `Remote` source, honoured start position, and persisted
    /// queue state are asserted end to end against the mock transport.
    #[rstest]
    #[case::multi_item(
        PLAY_TWO_ITEMS,
        vec!["a".to_string(), "b".to_string()],
        1,
        999,
        vec![("a", 111), ("b", 999)]
    )]
    fn play_replaces_queue_with_remote_source_and_persists(
        #[case] response: &str,
        #[case] item_ids: Vec<String>,
        #[case] start_index: usize,
        #[case] start_position_ticks: i64,
        #[case] expected_positions: Vec<(&str, i64)>,
    ) {
        let http = mbv_net::mock_http::MockHttp::new();
        http.respond(200, response);
        let mut app = app_with_mock_emby(&http);

        app.handle_ws_event(WsEvent::Play {
            item_ids,
            play_now: true,
            start_position_ticks,
            start_index,
        });

        let items = app.player_tab.emby_items();
        let actual: Vec<(&str, i64)> = items
            .iter()
            .map(|i| (i.id.as_str(), i.playback_position_ticks))
            .collect();
        assert_eq!(
            actual, expected_positions,
            "the queue is replaced in fetch order and only start_index honours start_position_ticks"
        );
        let cursor = start_index.min(items.len() - 1);
        assert_eq!(app.player_tab.queue_cursor, cursor);
        assert_eq!(app.queue_source, mbv_queue::QueueSource::Remote);

        let state = crate::config::load_queue_state().expect("Play persists the replaced queue");
        let persisted_items = state.emby_items();
        let persisted: Vec<(&str, i64)> = persisted_items
            .iter()
            .map(|i| (i.id.as_str(), i.playback_position_ticks))
            .collect();
        assert_eq!(persisted, expected_positions);
        assert_eq!(state.cursor, cursor);
        assert_eq!(state.source, mbv_queue::QueueSource::Remote);
    }

    #[test]
    fn user_data_changed_successful_fetch_sends_home_content_refreshed() {
        let http = mbv_net::mock_http::MockHttp::new();
        // `fetch_home` sequences: VirtualFolders, user Views, Continue Watching.
        http.respond(200, "[]");
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(200, r#"{"Items":[]}"#);
        let mut app = app_with_mock_emby(&http);

        app.handle_ws_event(WsEvent::UserDataChanged);

        match app.channels.lib_rx.try_recv() {
            Ok(LibEvent::ModelContent(ModelContentEvent::HomeContentRefreshed(content))) => {
                assert!(content.continue_items.is_empty());
            }
            Ok(_) => panic!("a successful home fetch must emit HomeContentRefreshed"),
            Err(e) => panic!("expected a HomeContentRefreshed event, got none: {e}"),
        }
    }
}
