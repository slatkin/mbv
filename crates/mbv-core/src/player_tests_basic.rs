use super::*;
use crate::config::tests::SYS_ENV_LOCK;

struct MpvConfigTestEnv {
    root: PathBuf,
    runtime_dir: PathBuf,
    user_mpv: PathBuf,
    old_runtime: Option<std::ffi::OsString>,
    old_config: Option<std::ffi::OsString>,
    old_system: Option<std::ffi::OsString>,
}

impl MpvConfigTestEnv {
    fn new(name: &str) -> Self {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "mbv-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let runtime_dir = root.join("runtime");
        let xdg_config = root.join("xdg-config");
        let user_mpv = xdg_config.join("mpv");
        let old_runtime = std::env::var_os("XDG_RUNTIME_DIR");
        let old_config = std::env::var_os("XDG_CONFIG_HOME");
        let old_system = std::env::var_os("MBV_SYSTEM");

        std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
        std::env::set_var("XDG_CONFIG_HOME", &xdg_config);
        std::env::remove_var("MBV_SYSTEM");

        Self {
            root,
            runtime_dir,
            user_mpv,
            old_runtime,
            old_config,
            old_system,
        }
    }

    fn restore_env(key: &str, previous: &Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}

impl Drop for MpvConfigTestEnv {
    fn drop(&mut self) {
        Self::restore_env("XDG_RUNTIME_DIR", &self.old_runtime);
        Self::restore_env("XDG_CONFIG_HOME", &self.old_config);
        Self::restore_env("MBV_SYSTEM", &self.old_system);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// ── private mpv config isolation ─────────────────────────────────────────

#[test]
fn sanitized_mpv_conf_removes_active_ipc_options_and_appends_mbv_ipc() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let env = MpvConfigTestEnv::new("sanitize-mpv-conf");
    std::fs::create_dir_all(&env.user_mpv).unwrap();
    let conf_path = env.user_mpv.join("mpv.conf");
    std::fs::write(
        &conf_path,
        "\
volume=75
input-ipc-server=/tmp/user.sock
--input-ipc-server=/tmp/other.sock
 input-ipc-server /tmp/spaced.sock
# input-ipc-server=/tmp/commented.sock
",
    )
    .unwrap();

    let sanitized = sanitized_mpv_conf(Some(&conf_path), "/tmp/mbv.sock");

    assert!(sanitized.contains("volume=75\n"));
    assert!(!sanitized.contains("/tmp/user.sock"));
    assert!(!sanitized.contains("/tmp/other.sock"));
    assert!(!sanitized.contains("/tmp/spaced.sock"));
    assert!(sanitized.contains("# input-ipc-server=/tmp/commented.sock\n"));
    assert!(sanitized.ends_with("input-ipc-server=/tmp/mbv.sock\n"));
}

#[test]
fn prepare_mpv_config_dir_symlinks_user_entries_but_not_mpv_or_input_conf() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let env = MpvConfigTestEnv::new("private-mpv-config");
    std::fs::create_dir_all(env.user_mpv.join("scripts")).unwrap();
    std::fs::create_dir_all(env.user_mpv.join("script-opts")).unwrap();
    std::fs::write(
        env.user_mpv.join("mpv.conf"),
        "volume=65\ninput-ipc-server=/tmp/user.sock\n",
    )
    .unwrap();
    std::fs::write(env.user_mpv.join("input.conf"), "q quit\n").unwrap();

    let private_dir = prepare_mpv_config_dir(true, "/tmp/mbv.sock").unwrap();
    let conf = std::fs::read_to_string(private_dir.join("mpv.conf")).unwrap();

    assert_eq!(private_dir, env.runtime_dir.join("mpv-config"));
    assert!(conf.contains("volume=65\n"));
    assert!(!conf.contains("/tmp/user.sock"));
    assert!(conf.contains("input-ipc-server=/tmp/mbv.sock\n"));
    assert!(std::fs::symlink_metadata(private_dir.join("scripts"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(std::fs::symlink_metadata(private_dir.join("script-opts"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!private_dir.join("input.conf").exists());
}

#[test]
fn prepare_mpv_config_dir_ignores_user_config_when_disabled() {
    let _g = SYS_ENV_LOCK.lock().unwrap();
    let env = MpvConfigTestEnv::new("private-mpv-config-disabled");
    std::fs::create_dir_all(env.user_mpv.join("scripts")).unwrap();
    std::fs::write(env.user_mpv.join("mpv.conf"), "volume=65\n").unwrap();

    let private_dir = prepare_mpv_config_dir(false, "/tmp/mbv.sock").unwrap();
    let conf = std::fs::read_to_string(private_dir.join("mpv.conf")).unwrap();

    assert_eq!(conf, "input-ipc-server=/tmp/mbv.sock\n");
    assert!(!private_dir.join("scripts").exists());
}

// ── shift_index_for_move ──────────────────────────────────────────────────

#[test]
fn shift_index_for_move_moves_the_tracked_index_itself() {
    assert_eq!(shift_index_for_move(1, 1, 3), 3);
    assert_eq!(shift_index_for_move(3, 3, 1), 1);
}

#[test]
fn shift_index_for_move_shifts_indices_between_from_and_to() {
    // Moving 1 -> 3 closes the gap it left, shifting everything in (1, 3] down.
    assert_eq!(shift_index_for_move(2, 1, 3), 1);
    assert_eq!(shift_index_for_move(3, 1, 3), 2);
    // Moving 3 -> 1 opens a gap at 1, shifting everything in [1, 3) up.
    assert_eq!(shift_index_for_move(1, 3, 1), 2);
    assert_eq!(shift_index_for_move(2, 3, 1), 3);
}

#[test]
fn shift_index_for_move_leaves_unrelated_indices_alone() {
    assert_eq!(shift_index_for_move(0, 1, 3), 0);
    assert_eq!(shift_index_for_move(4, 1, 3), 4);
}

// ── PlayerCommand serde (IPC protocol integrity) ─────────────────────────

fn make_media_item(id: &str) -> crate::api::EmbyItem {
    crate::api::EmbyItem {
        id: id.into(),
        name: "Test Episode".into(),
        item_type: "Episode".into(),
        is_folder: false,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 3600 * crate::api::TICKS_PER_SECOND,
        played: false,
        playback_position_ticks: 0,
        series_id: "series1".into(),
        series_name: "Show".into(),
        album_id: String::new(),
        album: String::new(),
        index_number: 2,
        parent_index_number: 1,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

fn make_queue_session_for_pos_tests(start_idx: usize) -> (PlaybackRun, Arc<Mutex<PlayerStatus>>) {
    let items = vec![
        make_media_item("ep1"),
        make_media_item("ep2"),
        make_media_item("ep3"),
    ];
    let status = Arc::new(Mutex::new(PlayerStatus {
        active: true,
        current_idx: start_idx,
        queue_len: items.len(),
        runtime_ticks: items[start_idx].runtime_ticks,
        title: items[start_idx].display_name(),
        ..Default::default()
    }));
    let client = Arc::new(EmbyClient::new(crate::config::Config::default()));
    let reporter = SessionReporter::new(
        client,
        None,
        ItemId::new(items[start_idx].id.clone()),
        MediaSourceId::new("msid"),
        EmbySessionId::new("sid"),
        false,
        status.clone(),
    );
    let (event_tx, _event_rx) = mpsc::channel();
    let session = PlaybackRun::new(
        items,
        start_idx,
        PlaybackOrigin::Queue,
        reporter,
        MpvRunConfig {
            headless: false,
            use_mpv_config: false,
            no_scripts: true,
            always_skip_intro: false,
            audio_pipe_path: Some("/tmp/mbv-test-pipe".into()),
            audio_pipe_samplerate: 48_000,
            audio_pipe_bitdepth: 16,
        },
        false,
        status.clone(),
        event_tx,
        Arc::new(Mutex::new(SubtitlePrefs::default())),
        Arc::new(Mutex::new(None)),
        "http://example.test".into(),
        "token".into(),
        Vec::new(),
    );
    (session, status)
}
