use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zbus::{connection, interface, zvariant};

use crate::api::TICKS_PER_SECOND;
use crate::player::{PlayerCommand, PlayerStatus};

struct MediaPlayer2;

#[interface(name = "org.mpris.MediaPlayer2")]
impl MediaPlayer2 {
    fn quit(&self) {}
    fn raise(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool { false }
    #[zbus(property)]
    fn can_raise(&self) -> bool { false }
    #[zbus(property)]
    fn has_track_list(&self) -> bool { false }
    #[zbus(property)]
    fn identity(&self) -> &str { "Emby Browser" }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> { vec![] }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> { vec![] }
}

struct MediaPlayer2Player {
    status: Arc<Mutex<PlayerStatus>>,
    /// Snapshot updated every 500ms by the polling loop so all property reads
    /// within one D-Bus call batch see consistent state.
    snapshot: Arc<Mutex<PlayerStatus>>,
    cmd_tx: Arc<dyn Fn(PlayerCommand) + Send + Sync>,
}

fn make_metadata(s: &PlayerStatus) -> HashMap<String, zvariant::Value<'static>> {
    let mut m = HashMap::new();
    let track_id = zvariant::ObjectPath::try_from(
        if s.active && !s.title.is_empty() {
            "/org/mpris/MediaPlayer2/TrackList/Track1"
        } else {
            "/org/mpris/MediaPlayer2/TrackList/NoTrack"
        }
    ).unwrap();
    m.insert("mpris:trackid".to_string(), zvariant::Value::new(track_id));
    if s.active && !s.title.is_empty() {
        m.insert("xesam:title".to_string(), zvariant::Value::new(s.title.clone()));
        if s.runtime_ticks > 0 {
            let length_us = s.runtime_ticks * 1_000_000 / TICKS_PER_SECOND;
            m.insert("mpris:length".to_string(), zvariant::Value::new(length_us));
        }
    }
    m
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MediaPlayer2Player {
    fn play(&self) {
        if self.status.lock().unwrap().paused {
            (self.cmd_tx)(PlayerCommand::TogglePause);
        }
    }

    fn pause(&self) {
        if !self.status.lock().unwrap().paused {
            (self.cmd_tx)(PlayerCommand::TogglePause);
        }
    }

    fn play_pause(&self) {
        (self.cmd_tx)(PlayerCommand::TogglePause);
    }

    fn stop(&self) {
        (self.cmd_tx)(PlayerCommand::TogglePause);
    }

    fn next(&self) {
        let idx = self.status.lock().unwrap().current_idx;
        (self.cmd_tx)(PlayerCommand::JumpTo(idx + 1));
    }

    fn previous(&self) {
        let idx = self.status.lock().unwrap().current_idx;
        if idx > 0 { (self.cmd_tx)(PlayerCommand::JumpTo(idx - 1)); }
    }

    fn seek(&self, offset_us: i64) {
        let secs = offset_us as f64 / 1_000_000.0;
        // Clamp seek to reasonable bounds (avoid seeking hours into the future).
        if secs.abs() > 86400.0 { return; }
        (self.cmd_tx)(PlayerCommand::Seek(secs));
    }

    fn set_position(&self, track_id: zvariant::ObjectPath<'_>, position_us: i64) {
        // Per MPRIS spec: ignore if track_id doesn't match current track or position is negative.
        if track_id.as_str() != "/org/mpris/MediaPlayer2/TrackList/Track1" { return; }
        if position_us < 0 { return; }
        let runtime_us = self.status.lock().unwrap().runtime_ticks * 1_000_000 / TICKS_PER_SECOND;
        if runtime_us > 0 && position_us > runtime_us { return; }
        (self.cmd_tx)(PlayerCommand::SeekAbsolute(position_us as f64 / 1_000_000.0));
    }

    fn open_uri(&self, _uri: &str) {}

    #[zbus(property)]
    fn playback_status(&self) -> String {
        let s = self.snapshot.lock().unwrap();
        if !s.active        { "Stopped".into() }
        else if s.paused    { "Paused".into()  }
        else                { "Playing".into() }
    }

    #[zbus(property)]
    fn loop_status(&self) -> &str { "None" }

    #[zbus(property)]
    fn rate(&self) -> f64 { 1.0 }

    #[zbus(property)]
    fn shuffle(&self) -> bool { false }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, zvariant::Value<'static>> {
        make_metadata(&self.snapshot.lock().unwrap())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.snapshot.lock().unwrap().volume as f64 / 100.0
    }

    #[zbus(property)]
    async fn set_volume(&self, vol: f64) {
        (self.cmd_tx)(PlayerCommand::SetVolume((vol * 100.0).round() as i64));
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        self.snapshot.lock().unwrap().position_ticks * 1_000_000 / TICKS_PER_SECOND
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn can_go_next(&self) -> bool { true }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool { true }
    #[zbus(property)]
    fn can_play(&self) -> bool { true }
    #[zbus(property)]
    fn can_pause(&self) -> bool { true }
    #[zbus(property)]
    fn can_seek(&self) -> bool { true }
    #[zbus(property)]
    fn can_control(&self) -> bool { true }
}

pub fn start(status: Arc<Mutex<PlayerStatus>>, send: impl Fn(PlayerCommand) + Send + Sync + 'static) {
    let send = Arc::new(send);
    let status_poll = status.clone();
    let snapshot = Arc::new(Mutex::new(status.lock().unwrap().clone()));
    let snapshot_poll = snapshot.clone();

    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(r) => r,
            Err(e) => { eprintln!("MPRIS tokio error: {e}"); return; }
        };
        rt.block_on(async move {
            let player_iface = MediaPlayer2Player { status, snapshot: snapshot_poll.clone(), cmd_tx: send };
            let conn = match connection::Builder::session()
                .unwrap()
                .name("org.mpris.MediaPlayer2.mbv")
                .unwrap()
                .serve_at("/org/mpris/MediaPlayer2", MediaPlayer2)
                .unwrap()
                .serve_at("/org/mpris/MediaPlayer2", player_iface)
                .unwrap()
                .build()
                .await
            {
                Ok(c) => c,
                Err(e) => { eprintln!("MPRIS D-Bus error: {e}"); return; }
            };

            let mut last_status = String::new();
            let mut last_title = String::new();
            let mut last_pos_s: i64 = -1;
            let mut last_vol: i64 = -1;

            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;

                // Snapshot PlayerStatus once per poll cycle so all property
                // reads see consistent state.
                let (cur_status, cur_title, cur_pos_us, cur_vol) = {
                    let s = status_poll.lock().unwrap();
                    *snapshot_poll.lock().unwrap() = s.clone();
                    let st = if !s.active       { "Stopped".to_string() }
                             else if s.paused   { "Paused".to_string()  }
                             else               { "Playing".to_string() };
                    let pos_us = s.position_ticks * 1_000_000 / TICKS_PER_SECOND;
                    (st, s.title.clone(), pos_us, s.volume)
                };

                let Ok(iface_ref) = conn.object_server()
                    .interface::<_, MediaPlayer2Player>("/org/mpris/MediaPlayer2")
                    .await
                else { continue };

                let ctxt = iface_ref.signal_context();
                let iface = iface_ref.get().await;

                if cur_status != last_status {
                    last_status = cur_status;
                    let _ = iface.playback_status_changed(ctxt).await;
                }

                if cur_title != last_title {
                    last_title = cur_title;
                    let _ = iface.metadata_changed(ctxt).await;
                }

                // Emit position every ~5s.
                let cur_pos_s = cur_pos_us / 1_000_000;
                if (cur_pos_s - last_pos_s).abs() >= 5 {
                    last_pos_s = cur_pos_s;
                    let _ = iface.position_changed(ctxt).await;
                }

                if cur_vol != last_vol {
                    last_vol = cur_vol;
                    let _ = iface.volume_changed(ctxt).await;
                }
            }
        });
    });
}
