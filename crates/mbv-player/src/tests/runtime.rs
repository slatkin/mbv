//! Contracts of the child start (design D9 of `mpv-child-process`). No mpv
//! process runs: the spawner's worker is a stub that records each request.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::mpv_error::MpvError;
use crate::mpv_session::spawn::{Spawner, StartRequest};
use crate::runtime::{MpvRunConfig, ResolvedSpawn, start_session};

fn stub_spawner(recorded: Arc<Mutex<Vec<StartRequest>>>) -> Spawner {
    Spawner::with_start(Box::new(move |request| {
        recorded.lock().unwrap().push(request);
        Err(MpvError::Spawn("stub start".to_owned()))
    }))
}

fn paths() -> ResolvedSpawn {
    ResolvedSpawn::for_test("/run/mbv/mbv-mpv.sock", Path::new("/run/mbv/mpv-config"))
}

fn config(headless: bool, audio_pipe: Option<&str>) -> MpvRunConfig {
    MpvRunConfig {
        headless,
        use_mpv_config: false,
        video_cache_forward_mb: 0,
        video_cache_back_mb: 0,
        no_scripts: false,
        always_skip_intro: false,
        audio_pipe_path: audio_pipe.map(str::to_owned),
        audio_pipe_samplerate: 48_000,
        audio_pipe_bitdepth: 16,
        audio_device: None,
    }
}

/// `pre_warm` starts a headless pipe run; the on-demand start in
/// `run_player_thread` starts a normal run with no pre-warmed session. Both
/// must reach the controller's one spawner (design D5).
#[test]
fn pre_warm_and_on_demand_runs_both_start_through_the_spawner() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let spawner = stub_spawner(Arc::clone(&recorded));

    let _ = start_session(&spawner, &config(true, Some("/run/mbv/mbv-pipe")), &paths());
    let _ = start_session(&spawner, &config(false, None), &paths());

    let requests = recorded.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].program, "mpv");
    assert_eq!(requests[1].program, "mpv");
}
