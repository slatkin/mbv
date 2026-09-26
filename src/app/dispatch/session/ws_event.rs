use crate::app::{dispatch::notify::ToastSeverity, App, LibEvent, PanelFocus};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerCommand;
use mbv_core::ws::WsEvent;

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
                self.reset_bare_transitions();
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
            #[expect(
                clippy::cast_precision_loss,
                reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
            )]
            WsEvent::Seek(ticks) => {
                self.player.send_command(PlayerCommand::SeekAbsolute(
                    ticks as f64 / TICKS_PER_SECOND as f64,
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
                    let _ = self
                        .lib_tx
                        .send(LibEvent::HomeContentRefreshed(Box::new(content)));
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
        log::info!(target: "ws", "Play: {} id(s), play_now={play_now}", item_ids.len());
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
            log::warn!(target: "ws", "Play: no items found for ids={}", item_ids.join(","));
            return;
        }
        let start_idx = start_index.min(items.len().saturating_sub(1));
        self.set_panel_focus(PanelFocus::Queue);
        self.queue_source = crate::config::QueueSource::Remote;
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
                crate::config::QueueSource::Remote,
            );
        } else {
            log::info!(target: "ws", "Play multi: count={}, start_idx={start_idx}", items.len());
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
                crate::config::QueueSource::Remote,
            );
        }
        self.save_queue_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use mbv_core::player::{PlayerCommand, PlayerStatus};
    use rstest::rstest;
    use std::sync::mpsc::Receiver;

    /// `PlayerCommand` carries no `PartialEq`, so compare structurally through
    /// its `Serialize` impl — exact variant and payload, not a hand-written
    /// field check per case.
    fn command_json(cmd: &PlayerCommand) -> serde_json::Value {
        serde_json::to_value(cmd).expect("PlayerCommand is serializable")
    }

    fn sent(rx: &Receiver<PlayerCommand>) -> PlayerCommand {
        rx.try_recv()
            .expect("the handler must send exactly one PlayerCommand")
    }

    fn set_status(app: &crate::app::App, f: impl FnOnce(&mut PlayerStatus)) {
        f(&mut app.player.status.lock().unwrap());
    }

    /// The pure command variants: fixture is the status the handler reads and
    /// the expected command is the exact payload it must emit. `Pause`/`Unpause`
    /// reach `TogglePause` only when the status is on the other side of the
    /// toggle, so `paused` is part of the fixture, not decoration.
    #[rstest]
    #[case::pause(false, 100, WsEvent::Pause, PlayerCommand::TogglePause)]
    #[case::next_track(false, 100, WsEvent::NextTrack, PlayerCommand::Next)]
    #[case::seek_absolute(false, 100, WsEvent::Seek(30 * TICKS_PER_SECOND), PlayerCommand::SeekAbsolute(30.0))]
    #[case::set_volume_clamps_low(false, 100, WsEvent::SetVolume(-5), PlayerCommand::SetVolume(0))]
    #[case::set_audio(false, 100, WsEvent::SetAudio(3), PlayerCommand::SetAudio(3))]
    fn pure_command_variants_send_the_exact_command(
        #[case] paused: bool,
        #[case] volume: i64,
        #[case] ev: WsEvent,
        #[case] expected: PlayerCommand,
    ) {
        let mut app = make_app_stub();
        set_status(&app, |st| {
            st.paused = paused;
            st.volume = volume;
        });
        let rx = app.player.spy_on_commands();

        app.handle_ws_event(ev);

        assert_eq!(command_json(&sent(&rx)), command_json(&expected));
    }

    #[test]
    fn stop_resets_bare_transitions_and_sends_no_transport_command() {
        use mbv_core::playback_queue::QueueSlotId;
        use mbv_core::playback_transition::Transition;

        let mut app = make_app_stub();
        let (request_id, generation) = app.bare_owner.mint_local_transition();
        app.bare_owner.accept_local_transition(Transition::new(
            request_id,
            generation,
            QueueSlotId::from_raw(1),
        ));
        assert!(
            app.bare_owner.in_flight_transition_slot().is_some(),
            "fixture must leave a bare transition in flight for Stop to drop"
        );
        let rx = app.player.spy_on_commands();

        app.handle_ws_event(WsEvent::Stop);

        assert!(
            app.bare_owner.in_flight_transition_slot().is_none(),
            "Stop must reset the bare owner's in-flight transition"
        );
        assert!(
            rx.try_recv().is_err(),
            "Stop is a teardown, not an mpv transport command"
        );
    }

    #[rstest]
    #[case::toggle_from_unmuted(false, WsEvent::ToggleMute, true, PlayerCommand::SetMute(true))]
    #[case::toggle_from_muted(true, WsEvent::ToggleMute, false, PlayerCommand::SetMute(false))]
    fn mute_variants_update_state_send_command_and_persist_prefs(
        #[case] status_muted: bool,
        #[case] ev: WsEvent,
        #[case] expected_mute_on: bool,
        #[case] expected: PlayerCommand,
    ) {
        let mut app = make_app_stub();
        set_status(&app, |st| st.muted = status_muted);
        let rx = app.player.spy_on_commands();

        app.handle_ws_event(ev);

        assert_eq!(app.mute_on, expected_mute_on);
        assert_eq!(command_json(&sent(&rx)), command_json(&expected));
        // `make_app_stub` installs a `TestStateDirGuard`, so this write lands in
        // the thread-local tempdir, never the developer's config.
        let prefs: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::config::prefs_path())
                .expect("mute must persist prefs into the test state dir"),
        )
        .expect("prefs file must stay valid JSON");
        assert_eq!(prefs["mute_on"].as_bool(), Some(expected_mute_on));
    }

    /// `SetSub` derives its target from player status: a stream index present in
    /// the map resolves to that mpv id, a negative index means "off", and an
    /// index the status cannot resolve legitimately sends nothing.
    #[rstest]
    #[case::resolved_stream_index(2, Some(PlayerCommand::SetSub(1)))]
    #[case::unknown_stream_index_sends_nothing(5, None)]
    fn set_sub_resolves_through_player_status(
        #[case] index: i64,
        #[case] expected: Option<PlayerCommand>,
    ) {
        let mut app = make_app_stub();
        set_status(&app, |st| st.sub_track_stream_indexes = vec![(1, 2)]);
        let rx = app.player.spy_on_commands();

        app.handle_ws_event(WsEvent::SetSub(index));

        match expected {
            Some(expected) => assert_eq!(command_json(&sent(&rx)), command_json(&expected)),
            None => assert!(
                rx.try_recv().is_err(),
                "an unresolvable stream index must not force a SetSub"
            ),
        }
    }

    /// Install a stub Emby runtime whose transport is `http`, so the handler's
    /// synchronous fetch is served from scripted in-memory responses (no real
    /// server, per the mocks-only test policy).
    fn app_with_mock_emby(http: &mbv_net::mock_http::MockHttp) -> crate::app::App {
        let mut app = make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let client = mbv_core::api::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
            std::sync::Mutex::new(client),
        ));
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
        assert_eq!(app.queue_source, crate::config::QueueSource::Remote);

        let state = crate::config::load_queue_state().expect("Play persists the replaced queue");
        let persisted_items = state.emby_items();
        let persisted: Vec<(&str, i64)> = persisted_items
            .iter()
            .map(|i| (i.id.as_str(), i.playback_position_ticks))
            .collect();
        assert_eq!(persisted, expected_positions);
        assert_eq!(state.cursor, cursor);
        assert_eq!(state.source, crate::config::QueueSource::Remote);
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

        match app.lib_rx.try_recv() {
            Ok(LibEvent::HomeContentRefreshed(content)) => {
                assert!(content.continue_items.is_empty());
            }
            Ok(_) => panic!("a successful home fetch must emit HomeContentRefreshed"),
            Err(e) => panic!("expected a HomeContentRefreshed event, got none: {e}"),
        }
    }
}
