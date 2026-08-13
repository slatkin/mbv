//! Live-only Audiobookshelf contract probe. It never prints credentials, URLs,
//! IDs, user/device identity, titles, hostnames, response bodies, or paths.
//! Run only against a disposable/controlled Audiobookshelf Service.

use libmpv2::{events::Event, Mpv};
use mbv_core::{
    audiobookshelf::AudiobookshelfClient,
    config::{self, ServiceKind},
};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

const DEVICE_ID: &str = "<DEVICE_ID>";
const REQUEST_BOUND: Duration = Duration::from_secs(5);
const READY_BOUND: Duration = Duration::from_secs(20);
const MPV_BOUND: Duration = Duration::from_secs(15);

struct LiveClient {
    base: String,
    token: String,
    agent: ureq::Agent,
    open_sessions: BTreeSet<String>,
}

impl LiveClient {
    fn load() -> Result<Self, String> {
        let setup = config::load_config()?
            .audiobookshelf_setup
            .ok_or("Audiobookshelf Service is not configured")?;
        let token = config::load_service_secret(ServiceKind::Audiobookshelf)
            .ok_or("Audiobookshelf Service credential is unavailable")?;
        Ok(Self {
            base: setup.server_url,
            token,
            agent: ureq::AgentBuilder::new()
                .timeout_connect(REQUEST_BOUND)
                .timeout(REQUEST_BOUND)
                .build(),
            open_sessions: BTreeSet::new(),
        })
    }

    fn post(&self, path: &str, body: Value) -> Result<(u16, String), String> {
        let request = self
            .agent
            .post(&format!("{}{}", self.base, path))
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/json");
        match request.send_json(body) {
            Ok(response) => read_response(response),
            Err(ureq::Error::Status(_, response)) => read_response(response),
            Err(_) => Err("request failed before an HTTP response".into()),
        }
    }

    fn get_status(&self, path: &str, authenticated: bool) -> Result<u16, String> {
        let mut request = self.agent.get(&format!("{}{}", self.base, path));
        if authenticated {
            request = request.set("Authorization", &format!("Bearer {}", self.token));
        }
        match request.call() {
            Ok(response) => Ok(response.status()),
            Err(ureq::Error::Status(status, _)) => Ok(status),
            Err(_) => Err("GET failed before an HTTP response".into()),
        }
    }

    fn play(&mut self, item: &str, episode: &str, transcode: bool) -> Result<Value, String> {
        let body = json!({
            "deviceInfo": {
                "deviceId": DEVICE_ID,
                "clientName": "mbv-contract-probe",
                "clientVersion": env!("CARGO_PKG_VERSION"),
                "manufacturer": "mbv",
                "model": "contract-probe"
            },
            "forceDirectPlay": !transcode,
            "forceTranscode": transcode,
            "supportedMimeTypes": ["audio/mpeg", "audio/mp4", "audio/flac", "audio/ogg"],
            "mediaPlayer": "mpv"
        });
        let (status, text) = self.post(&format!("/api/items/{item}/play/{episode}"), body)?;
        if status != 200 {
            return Err(format!("play returned HTTP {status}"));
        }
        let value: Value =
            serde_json::from_str(&text).map_err(|_| "play returned malformed JSON")?;
        let session = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or("play response omitted session id")?
            .to_string();
        self.open_sessions.insert(session);
        Ok(value)
    }

    fn session_request(
        &self,
        session: &str,
        action: &str,
        duration: f64,
    ) -> Result<(u16, String), String> {
        self.post(
            &format!("/api/session/{session}/{action}"),
            json!({
                "currentTime": 1.0,
                "timeListened": 1.0,
                "duration": duration
            }),
        )
    }

    fn close(&mut self, session: &str, duration: f64) -> Result<(u16, String), String> {
        let result = self.session_request(session, "close", duration);
        if result.as_ref().is_ok_and(|(status, _)| *status == 200) {
            self.open_sessions.remove(session);
        }
        result
    }

    fn close_all(&mut self) {
        for session in self.open_sessions.clone() {
            let _ = self.close(&session, 0.0);
        }
    }
}

impl Drop for LiveClient {
    fn drop(&mut self) {
        self.close_all();
    }
}

fn read_response(response: ureq::Response) -> Result<(u16, String), String> {
    let status = response.status();
    response
        .into_string()
        .map(|text| (status, text))
        .map_err(|_| "response body could not be read".into())
}

fn duration(value: &Value) -> Result<f64, String> {
    value
        .get("duration")
        .and_then(Value::as_f64)
        .ok_or("response omitted duration".into())
}

fn source(value: &Value) -> Result<(&str, bool), String> {
    let tracks = value
        .get("audioTracks")
        .and_then(Value::as_array)
        .ok_or("response omitted audioTracks")?;
    if tracks.len() != 1 {
        return Err(format!("response had {} audio tracks", tracks.len()));
    }
    let track = &tracks[0];
    let url = track
        .get("contentUrl")
        .and_then(Value::as_str)
        .ok_or("audio track omitted contentUrl")?;
    let hls = value.get("playMethod").and_then(Value::as_u64) == Some(2)
        || url.ends_with(".m3u8")
        || url.contains("/hls/");
    Ok((url, hls))
}

fn absolute_url(base: &str, path: &str) -> Result<String, String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        return Ok(path.to_string());
    }
    if !path.starts_with('/') {
        return Err("source path was not absolute".into());
    }
    Ok(format!("{base}{path}"))
}

fn wait_hls(client: &LiveClient, url: &str) -> Result<usize, String> {
    let start = Instant::now();
    let mut attempts = 0;
    while start.elapsed() < READY_BOUND {
        attempts += 1;
        match client.agent.get(url).call() {
            Ok(response) if response.status() == 200 => {
                let body = response
                    .into_string()
                    .map_err(|_| "playlist body unreadable")?;
                if body.starts_with("#EXTM3U") {
                    return Ok(attempts);
                }
            }
            Ok(_) | Err(ureq::Error::Status(_, _)) | Err(ureq::Error::Transport(_)) => {}
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err("REST-only HLS playlist readiness exceeded 20 seconds".into())
}

fn mpv_probe(url: &str, token: Option<&str>) -> Result<(String, String), String> {
    let header = token.map(|token| format!("Authorization: Bearer {token}"));
    let mpv = Mpv::with_initializer(|init| {
        init.set_option("config", "no")?;
        init.set_option("load-scripts", "no")?;
        init.set_option("vo", "null")?;
        init.set_option("ao", "null")?;
        init.set_option("pause", "yes")?;
        if let Some(header) = header.as_deref() {
            init.set_option("http-header-fields", header)?;
        }
        Ok(())
    })
    .map_err(|_| "libmpv initialization failed")?;
    mpv.command("loadfile", &[url, "replace", "-1", ""])
        .map_err(|_| "libmpv loadfile failed")?;
    wait_for_restart(&mpv)?;
    let before: f64 = mpv
        .get_property("time-pos")
        .map_err(|_| "libmpv omitted initial time-pos")?;
    mpv.command("seek", &["1", "absolute"])
        .map_err(|_| "libmpv seek failed")?;
    wait_for_restart(&mpv)?;
    let after: f64 = mpv
        .get_property("time-pos")
        .map_err(|_| "libmpv omitted post-seek time-pos")?;
    if after < 0.5 {
        return Err("libmpv ordinary seek did not advance".into());
    }
    Ok((format!("{before:.3}"), format!("{after:.3}")))
}

fn wait_for_restart(mpv: &Mpv) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < MPV_BOUND {
        match mpv.wait_event(0.25) {
            Some(Ok(Event::PlaybackRestart)) => return Ok(()),
            Some(Ok(Event::EndFile(reason))) => {
                return Err(format!("libmpv ended before readiness: {reason:?}"))
            }
            Some(Err(_)) => return Err("libmpv event error".into()),
            _ => {}
        }
    }
    Err("libmpv readiness exceeded 15 seconds".into())
}

fn sanitize(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let clean = if key == "serverVersion" || key == "mediaType" || key == "mimeType"
                    {
                        value.clone()
                    } else if key == "contentUrl" {
                        Value::String(
                            if value
                                .as_str()
                                .is_some_and(|s| s.ends_with(".m3u8") || s.contains("/hls/"))
                            {
                                "<HLS_PATH>".into()
                            } else {
                                "<DIRECT_PATH>".into()
                            },
                        )
                    } else if key == "id"
                        || key.ends_with("Id")
                        || key.ends_with("Path")
                        || key == "path"
                    {
                        Value::String(format!("<{}>", key.to_ascii_uppercase()))
                    } else {
                        sanitize(value)
                    };
                    (key.clone(), clean)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(sanitize).collect()),
        Value::String(_) => Value::String("<STRING>".into()),
        other => other.clone(),
    }
}

fn session_fixture(value: &Value) -> Value {
    let keys = [
        "id",
        "userId",
        "libraryId",
        "libraryItemId",
        "episodeId",
        "mediaType",
        "duration",
        "playMethod",
        "mediaPlayer",
        "currentTime",
        "audioTracks",
        "serverVersion",
    ];
    let object = value.as_object().expect("validated JSON object");
    let mut fixture = Value::Object(
        keys.into_iter()
            .filter_map(|key| {
                object
                    .get(key)
                    .map(|value| (key.to_string(), value.clone()))
            })
            .collect(),
    );
    if let Some(tracks) = fixture.get_mut("audioTracks").and_then(Value::as_array_mut) {
        for track in tracks {
            let object = track.as_object().expect("validated audio track");
            *track = Value::Object(
                ["index", "startOffset", "duration", "contentUrl", "mimeType"]
                    .into_iter()
                    .filter_map(|key| object.get(key).map(|value| (key.into(), value.clone())))
                    .collect(),
            );
        }
    }
    sanitize(&fixture)
}

fn controlled_failure(status: u16, body: &'static str) -> Result<Value, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| "loopback bind failed")?;
    let address = listener
        .local_addr()
        .map_err(|_| "loopback address failed")?;
    let worker = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let reason = if status == 500 {
                "Internal Server Error"
            } else {
                "OK"
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let client = AudiobookshelfClient::new(&format!("http://{address}"))
        .map_err(|error| error.to_string())?;
    let class = client
        .me_bounded("<LOOPBACK_CREDENTIAL>", REQUEST_BOUND)
        .expect_err("controlled failure unexpectedly succeeded")
        .class;
    let _ = worker.join();
    Ok(
        json!({"provenance":"controlled loopback HTTP response", "status":status,
        "body":body, "observedClass":format!("{class:?}")}),
    )
}

fn main() -> Result<(), String> {
    let mut live = LiveClient::load()?;
    let catalog = AudiobookshelfClient::new(&live.base).map_err(|error| error.to_string())?;
    let library = catalog
        .libraries_bounded(&live.token, REQUEST_BOUND)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|library| library.media_type == "podcast")
        .ok_or("no podcast library available")?;
    let show = catalog
        .podcast_shows_bounded(&live.token, &library.id, 0, 25, REQUEST_BOUND)
        .map_err(|error| error.to_string())?
        .items
        .into_iter()
        .next()
        .ok_or("no podcast show available")?;
    let episode = catalog
        .podcast_detail_bounded(&live.token, &show.library_item_id, REQUEST_BOUND)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or("no downloaded podcast episode available")?;

    let direct = live.play(&episode.library_item_id, &episode.episode_id, false)?;
    let direct_session = direct["id"]
        .as_str()
        .ok_or("direct session id missing")?
        .to_string();
    let direct_duration = duration(&direct)?;
    let (direct_path, direct_hls) = source(&direct)?;
    if direct_hls {
        return Err("forced direct play returned HLS".into());
    }
    let direct_url = absolute_url(&live.base, direct_path)?;
    let direct_seek = mpv_probe(&direct_url, Some(&live.token))?;
    let sync = live.session_request(&direct_session, "sync", direct_duration)?;
    let close = live.close(&direct_session, direct_duration)?;
    let direct_closed_status = live.get_status(&format!("/api/session/{direct_session}"), true)?;

    let hls = live.play(&episode.library_item_id, &episode.episode_id, true)?;
    let hls_session = hls["id"]
        .as_str()
        .ok_or("HLS session id missing")?
        .to_string();
    let hls_duration = duration(&hls)?;
    let (hls_path, is_hls) = source(&hls)?;
    if !is_hls {
        return Err("forced transcode did not return HLS".into());
    }
    let hls_url = absolute_url(&live.base, hls_path)?;
    let readiness_attempts = wait_hls(&live, &hls_url)?;
    let hls_seek = mpv_probe(&hls_url, None)?;
    let hls_close = live.close(&hls_session, hls_duration)?;
    let hls_closed_status = live.get_status(&format!("/api/session/{hls_session}"), true)?;
    let hls_after_close_status = live.get_status(hls_path, false)?;

    let auth = live
        .agent
        .get(&format!("{}/api/me", live.base))
        .set("Authorization", "Bearer <INVALID_CREDENTIAL>")
        .call();
    let auth_status = match auth {
        Err(ureq::Error::Status(status, _)) => status,
        Ok(r) => r.status(),
        Err(_) => 0,
    };

    let mut output = Map::new();
    output.insert(
        "absVersion".into(),
        direct.get("serverVersion").cloned().unwrap_or(Value::Null),
    );
    output.insert("directPlay".into(), session_fixture(&direct));
    output.insert("forcedTranscode".into(), session_fixture(&hls));
    output.insert(
        "sync".into(),
        json!({"status": sync.0, "bodyBytes": sync.1.len()}),
    );
    output.insert(
        "close".into(),
        json!({"status": close.0, "bodyBytes": close.1.len()}),
    );
    output.insert(
        "hlsClose".into(),
        json!({"status": hls_close.0, "bodyBytes": hls_close.1.len()}),
    );
    output.insert(
        "authenticationFailure".into(),
        json!({"provenance":"live ABS 2.36.0", "status":auth_status}),
    );
    output.insert(
        "serverFailure".into(),
        controlled_failure(500, "{\"error\":\"<MESSAGE>\"}")?,
    );
    output.insert("malformedResponse".into(), controlled_failure(200, "{")?);
    output.insert(
        "mpv".into(),
        json!({
            "direct": {"started": true, "seekFrom": direct_seek.0, "seekTo": direct_seek.1},
            "hls": {"restOnlyReady": true, "readinessAttempts": readiness_attempts,
                "pollIntervalMs": 250, "boundMs": READY_BOUND.as_millis(), "started": true,
                "seekFrom": hls_seek.0, "seekTo": hls_seek.1, "socketIoUsed": false}
        }),
    );
    output.insert(
        "cleanup".into(),
        json!({"openSessions": live.open_sessions.len(),
            "directSessionAfterCloseStatus":direct_closed_status,
            "hlsSessionAfterCloseStatus":hls_closed_status,
            "hlsPathAfterCloseStatus":hls_after_close_status}),
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Object(output)).unwrap()
    );
    Ok(())
}
