// Throwaway manual verification tool for tasks 1.1-1.4 of the
// add-chromecast-target change. Run against real hardware:
//   cargo run -p mbv-core --example cast_spike -- <cast-device-ip>
// Deleted (or promoted) once findings are recorded in design.md.

use std::time::Duration;

use mbv_core::api::EmbyClient;
use mbv_core::audiobookshelf::AudiobookshelfClient;
use mbv_core::config;
use rust_cast::channels::media::{Media, MediaQueue, QueueItem, QueueType, StreamType};
use rust_cast::channels::receiver::CastDeviceApp;
use rust_cast::CastDevice;

fn main() {
    let host = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.106".to_string());

    println!("=== 1.1: Audiobookshelf media URL credential-in-URL check ===");
    check_audiobookshelf_url_credential();

    println!("\n=== 1.2/1.3/1.4: rust_cast against {host}:8009 ===");
    let emby_urls = emby_direct_play_urls();
    run_cast_spike(&host, &emby_urls);
}

fn check_audiobookshelf_url_credential() {
    let cfg = config::load_config().unwrap_or_default();
    let server_url = cfg
        .audiobookshelf_setup
        .as_ref()
        .map(|s| s.server_url.clone())
        .unwrap_or_default();
    let api_key = config::load_service_secret(config::ServiceKind::Audiobookshelf);
    let (Some(api_key), false) = (api_key, server_url.is_empty()) else {
        println!("skipped: no Audiobookshelf setup/secret found on this machine");
        return;
    };

    let client = match AudiobookshelfClient::new(&server_url) {
        Ok(c) => c,
        Err(e) => {
            println!("skipped: AudiobookshelfClient::new failed: {e}");
            return;
        }
    };
    let libraries = match client.libraries_bounded(&api_key, Duration::from_secs(5)) {
        Ok(l) => l,
        Err(e) => {
            println!("skipped: libraries fetch failed: {e}");
            return;
        }
    };
    let Some(podcast_library) = libraries.iter().find(|l| l.media_type == "podcast") else {
        println!("skipped: no podcast library found");
        return;
    };
    let shows = match client.podcast_shows_bounded(
        &api_key,
        &podcast_library.id,
        0,
        20,
        Duration::from_secs(5),
    ) {
        Ok(p) => p.items,
        Err(e) => {
            println!("skipped: podcast_shows fetch failed: {e}");
            return;
        }
    };
    let mut episode_ref = None;
    for show in &shows {
        if let Ok(episodes) =
            client.podcast_detail_bounded(&api_key, &show.library_item_id, Duration::from_secs(5))
        {
            if let Some(ep) = episodes.first() {
                episode_ref = Some((show.library_item_id.clone(), ep.episode_id.clone()));
                break;
            }
        }
    }
    let Some((library_item_id, episode_id)) = episode_ref else {
        println!("skipped: no downloaded podcast episode found");
        return;
    };

    let session = match client.create_playback_session_bounded(
        &api_key,
        "mbv-cast-spike",
        &library_item_id,
        &episode_id,
        false,
        Duration::from_secs(10),
    ) {
        Ok(s) => s,
        Err(e) => {
            println!("skipped: create_playback_session failed: {e}");
            return;
        }
    };

    let url_with_token = format!(
        "{}{}token={}",
        session.source.url,
        if session.source.url.contains('?') {
            "&"
        } else {
            "?"
        },
        api_key
    );

    let agent = ureq::Agent::new_with_defaults();
    let with_token = agent.get(&url_with_token).call();
    let without_credential = agent.get(&session.source.url).call();

    println!(
        "content_url (redacted host/path only): {}",
        mask_query(&session.source.url)
    );
    println!(
        "GET with ?token=<key> appended -> {}",
        describe_result(with_token)
    );
    println!(
        "GET with no Authorization header and no token -> {}",
        describe_result(without_credential)
    );
}

fn mask_query(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

fn describe_result(result: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> String {
    match result {
        Ok(resp) => format!("HTTP {}", resp.status()),
        Err(ureq::Error::StatusCode(code)) => format!("HTTP {code}"),
        Err(e) => format!("error: {e}"),
    }
}

fn emby_direct_play_urls() -> Vec<String> {
    let Some((server_url, user_id, token)) = emby_credentials() else {
        return vec![];
    };

    let mut client = EmbyClient::new(config::Config {
        server_url,
        ..Default::default()
    });
    client.user_id = user_id;
    client.token = token.clone();

    let items = client.get_continue_watching(10).unwrap_or_default();
    items
        .iter()
        .take(2)
        .map(|item| {
            let endpoint = if item.media_type == "Audio" {
                "Audio"
            } else {
                "Videos"
            };
            format!(
                "{}/{}/{}/stream?static=true&api_key={}",
                client.config.server_url, endpoint, item.id, token
            )
        })
        .collect()
}

fn emby_credentials() -> Option<(String, String, String)> {
    let cfg = config::load_config().ok()?;
    let server_url = cfg.emby_setup.as_ref().map(|s| s.server_url.clone())?;
    let user_id = cfg.emby_setup.as_ref().map(|s| s.user_id.clone())?;
    let token = config::load_service_secret(config::ServiceKind::Emby)?;
    Some((server_url, user_id, token))
}

fn run_cast_spike(host: &str, emby_urls: &[String]) {
    let device = match CastDevice::connect_without_host_verification(host, 8009) {
        Ok(d) => d,
        Err(e) => {
            println!("connect failed: {e}");
            return;
        }
    };
    println!("connect: ok");

    if let Err(e) = device.connection.connect("receiver-0") {
        println!("connect to receiver platform failed: {e}");
        return;
    }

    let app = match device
        .receiver
        .launch_app(&CastDeviceApp::DefaultMediaReceiver)
    {
        Ok(a) => a,
        Err(e) => {
            println!("launch_app failed: {e}");
            return;
        }
    };
    println!("launch_app: ok, transport_id={}", app.transport_id);

    if device
        .connection
        .connect(app.transport_id.as_str())
        .is_err()
    {
        println!("connect to app transport failed");
        return;
    }

    let Some(url) = emby_urls.first() else {
        println!("skipped load: no Emby direct-play URL available");
        return;
    };

    let to_media = |url: &str| Media {
        content_id: url.to_string(),
        stream_type: StreamType::Buffered,
        content_type: "video/mp4".to_string(),
        metadata: None,
        duration: None,
    };

    match device.media.load(
        app.transport_id.as_str(),
        app.session_id.as_str(),
        &to_media(url),
    ) {
        Ok(status) => println!("1.2 load: ok, status={status:?}"),
        Err(e) => {
            println!("1.2 load failed: {e}");
            return;
        }
    }

    for i in 0..3 {
        std::thread::sleep(Duration::from_secs(3));
        match device.media.get_status(app.transport_id.as_str(), None) {
            Ok(status) => println!("1.2 poll {i}: {status:?}"),
            Err(e) => println!("1.2 poll {i}: get_status failed: {e}"),
        }
    }

    if emby_urls.len() < 2 {
        println!("skipped 1.3 (multi-item queue): fewer than 2 Emby items available");
        return;
    }
    let queue = MediaQueue {
        items: emby_urls
            .iter()
            .map(|url| QueueItem {
                media: to_media(url),
            })
            .collect(),
        start_index: 0,
        queue_type: QueueType::Playlist,
    };
    match device
        .media
        .load_queue(app.transport_id.as_str(), app.session_id.as_str(), &queue)
    {
        Ok(status) => println!("1.3 load_queue: ok, status={status:?}"),
        Err(e) => {
            println!("1.3 load_queue failed: {e}");
            return;
        }
    }

    // Seek near the end of the first item so the unattended transition to the
    // second queue entry happens quickly instead of after the item's full runtime.
    std::thread::sleep(Duration::from_secs(3));
    if let Ok(status) = device.media.get_status(app.transport_id.as_str(), None) {
        if let Some(entry) = status.entries.first() {
            if let Some(duration) = entry.media.as_ref().and_then(|m| m.duration) {
                let near_end = (duration - 5.0).max(0.0);
                let _ = device.media.seek(
                    app.transport_id.as_str(),
                    entry.media_session_id,
                    Some(near_end),
                    None,
                );
                println!("1.3 seeked first item to {near_end}s of {duration}s to force a fast transition");
            }
        }
    }

    println!(
        "1.3 polling for unattended advance across items (up to 90s, proactive pong each tick)..."
    );
    for i in 0..9 {
        std::thread::sleep(Duration::from_secs(10));
        if let Err(e) = device.heartbeat.pong() {
            println!("1.3 pong {i} failed: {e}");
        }
        match device.media.get_status(app.transport_id.as_str(), None) {
            Ok(status) => println!("1.3 poll {i}: {status:?}"),
            Err(e) => println!("1.3 poll {i}: get_status failed: {e}"),
        }
    }
}
