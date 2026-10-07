//! Live-only Audiobookshelf contract probe. It never prints credentials, URLs,
//! IDs, user/device identity, titles, hostnames, response bodies, or paths.
//! Run only against a disposable/controlled Audiobookshelf Service.

use libmpv2::{Mpv, events::Event};
use mbv_audiobookshelf::AudiobookshelfClient;
use mbv_config as config;
use mbv_queue::ServiceKind;
use serde_json::{Map, Value, json};
use std::backtrace::Backtrace;
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

#[derive(Debug)]
pub struct ProbeError {
    kind: ProbeErrorKind,
    message: String,
    backtrace: Backtrace,
}

#[derive(Debug)]
enum ProbeErrorKind {
    Contract,
}

impl ProbeError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            ProbeErrorKind::Contract => "core.contract_probe",
        }
    }

    fn contract(message: impl Into<String>) -> Self {
        Self {
            kind: ProbeErrorKind::Contract,
            message: message.into(),
            backtrace: Backtrace::capture(),
        }
    }
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ProbeErrorKind::Contract => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ProbeError {}

struct LiveClient {
    base: String,
    token: String,
    agent: ureq::Agent,
    open_sessions: BTreeSet<String>,
}

impl LiveClient {
    fn load() -> Result<Self, ProbeError> {
        let setup = config::load_config()
            .map_err(|error| ProbeError::contract(error.to_string()))?
            .audiobookshelf_setup
            .ok_or_else(|| ProbeError::contract("Audiobookshelf Service is not configured"))?;
        let token = config::load_service_secret(ServiceKind::Audiobookshelf).ok_or_else(|| {
            ProbeError::contract("Audiobookshelf Service credential is unavailable")
        })?;
        Ok(Self {
            base: setup.server_url,
            token,
            agent: ureq::Agent::config_builder()
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .provider(ureq::tls::TlsProvider::NativeTls)
                        .build(),
                )
                .timeout_connect(Some(REQUEST_BOUND))
                .timeout_global(Some(REQUEST_BOUND))
                .build()
                .into(),
            open_sessions: BTreeSet::new(),
        })
    }

    fn post(&self, path: &str, body: Value) -> Result<(u16, String), ProbeError> {
        let request = self
            .agent
            .post(&format!("{}{}", self.base, path))
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Content-Type", "application/json");
        match request.send_json(body) {
            Ok(response) => read_response(response),
            // ureq 3.x error status codes no longer carry the response body,
            // only the status code.
            Err(ureq::Error::StatusCode(status)) => {
                Ok((status, "<body unavailable after ureq 3.x upgrade>".into()))
            }
            Err(_) => Err(ProbeError::contract(
                "request failed before an HTTP response",
            )),
        }
    }

    fn get_status(&self, path: &str, authenticated: bool) -> Result<u16, ProbeError> {
        let mut request = self.agent.get(&format!("{}{}", self.base, path));
        if authenticated {
            request = request.header("Authorization", &format!("Bearer {}", self.token));
        }
        match request.call() {
            Ok(response) => Ok(response.status().into()),
            Err(ureq::Error::StatusCode(status)) => Ok(status),
            Err(_) => Err(ProbeError::contract("GET failed before an HTTP response")),
        }
    }

    fn play(&mut self, item: &str, episode: &str, transcode: bool) -> Result<Value, ProbeError> {
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
            return Err(ProbeError::contract(format!("play returned HTTP {status}")));
        }
        let value: Value = serde_json::from_str(&text).map_err(|error| {
            ProbeError::contract(format!("play returned malformed JSON: {error}"))
        })?;
        let session = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| ProbeError::contract("play response omitted session id"))?
            .to_string();
        self.open_sessions.insert(session);
        Ok(value)
    }

    fn session_request(
        &self,
        session: &str,
        action: &str,
        duration: f64,
    ) -> Result<(u16, String), ProbeError> {
        self.post(
            &format!("/api/session/{session}/{action}"),
            json!({
                "currentTime": 1.0,
                "timeListened": 1.0,
                "duration": duration
            }),
        )
    }

    fn close(&mut self, session: &str, duration: f64) -> Result<(u16, String), ProbeError> {
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

fn read_response(
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<(u16, String), ProbeError> {
    let status = response.status().into();
    response
        .body_mut()
        .read_to_string()
        .map(|text| (status, text))
        .map_err(|error| ProbeError::contract(format!("response body could not be read: {error}")))
}

fn duration(value: &Value) -> Result<f64, ProbeError> {
    value
        .get("duration")
        .and_then(Value::as_f64)
        .ok_or_else(|| ProbeError::contract("response omitted duration"))
}

fn source(value: &Value) -> Result<(&str, bool), ProbeError> {
    let tracks = value
        .get("audioTracks")
        .and_then(Value::as_array)
        .ok_or_else(|| ProbeError::contract("response omitted audioTracks"))?;
    if tracks.len() != 1 {
        return Err(ProbeError::contract(format!(
            "response had {} audio tracks",
            tracks.len()
        )));
    }
    let track = &tracks[0];
    let url = track
        .get("contentUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| ProbeError::contract("audio track omitted contentUrl"))?;
    let hls = value.get("playMethod").and_then(Value::as_u64) == Some(2)
        || url.to_ascii_lowercase().ends_with(".m3u8")
        || url.contains("/hls/");
    Ok((url, hls))
}

fn absolute_url(base: &str, path: &str) -> Result<String, ProbeError> {
    if path.starts_with("http://") || path.starts_with("https://") {
        return Ok(path.to_string());
    }
    if !path.starts_with('/') {
        return Err(ProbeError::contract("source path was not absolute"));
    }
    Ok(format!("{base}{path}"))
}

fn wait_hls(client: &LiveClient, url: &str) -> Result<usize, ProbeError> {
    let start = Instant::now();
    let mut attempts = 0;
    while start.elapsed() < READY_BOUND {
        attempts += 1;
        match client.agent.get(url).call() {
            Ok(mut response) if response.status() == 200 => {
                let body = response.body_mut().read_to_string().map_err(|error| {
                    ProbeError::contract(format!("playlist body unreadable: {error}"))
                })?;
                if body.starts_with("#EXTM3U") {
                    return Ok(attempts);
                }
            }
            Ok(_) | Err(_) => {}
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(ProbeError::contract(
        "REST-only HLS playlist readiness exceeded 20 seconds",
    ))
}

fn mpv_probe(url: &str, token: Option<&str>) -> Result<(String, String), ProbeError> {
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
    .map_err(|error| ProbeError::contract(format!("libmpv initialization failed: {error}")))?;
    mpv.command("loadfile", &[url, "replace", "-1", ""])
        .map_err(|error| ProbeError::contract(format!("libmpv loadfile failed: {error}")))?;
    wait_for_restart(&mpv)?;
    let before: f64 = mpv.get_property("time-pos").map_err(|error| {
        ProbeError::contract(format!("libmpv omitted initial time-pos: {error}"))
    })?;
    mpv.command("seek", &["1", "absolute"])
        .map_err(|error| ProbeError::contract(format!("libmpv seek failed: {error}")))?;
    wait_for_restart(&mpv)?;
    let after: f64 = mpv.get_property("time-pos").map_err(|error| {
        ProbeError::contract(format!("libmpv omitted post-seek time-pos: {error}"))
    })?;
    if after < 0.5 {
        return Err(ProbeError::contract("libmpv ordinary seek did not advance"));
    }
    Ok((format!("{before:.3}"), format!("{after:.3}")))
}

fn wait_for_restart(mpv: &Mpv) -> Result<(), ProbeError> {
    let start = Instant::now();
    while start.elapsed() < MPV_BOUND {
        match mpv.wait_event(0.25) {
            Some(Ok(Event::PlaybackRestart)) => return Ok(()),
            Some(Ok(Event::EndFile(reason))) => {
                return Err(ProbeError::contract(format!(
                    "libmpv ended before readiness: {reason:?}"
                )));
            }
            Some(Err(_)) => return Err(ProbeError::contract("libmpv event error")),
            _ => {}
        }
    }
    Err(ProbeError::contract("libmpv readiness exceeded 15 seconds"))
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
                            if value.as_str().is_some_and(|s| {
                                s.to_ascii_lowercase().ends_with(".m3u8") || s.contains("/hls/")
                            }) {
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

fn controlled_failure(status: u16, body: &'static str) -> Result<Value, ProbeError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| ProbeError::contract(format!("loopback bind failed: {error}")))?;
    let address = listener
        .local_addr()
        .map_err(|error| ProbeError::contract(format!("loopback address failed: {error}")))?;
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
    let client = AudiobookshelfClient::new(format!("http://{address}"))
        .map_err(|error| ProbeError::contract(error.to_string()))?;
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

struct PlaybackProbe {
    direct: Value,
    direct_seek: (String, String),
    sync: (u16, String),
    close: (u16, String),
    direct_closed_status: u16,
    hls: Value,
    readiness_attempts: usize,
    hls_seek: (String, String),
    hls_close: (u16, String),
    hls_closed_status: u16,
    hls_after_close_status: u16,
}

fn first_podcast_episode(
    live: &LiveClient,
) -> Result<mbv_audiobookshelf::AudiobookshelfDownloadedEpisode, ProbeError> {
    let catalog = AudiobookshelfClient::new(&live.base)
        .map_err(|error| ProbeError::contract(error.to_string()))?;
    let library = catalog
        .libraries_bounded(&live.token, REQUEST_BOUND)
        .map_err(|error| ProbeError::contract(error.to_string()))?
        .into_iter()
        .find(|library| library.media_type == "podcast")
        .ok_or_else(|| ProbeError::contract("no podcast library available"))?;
    let show = catalog
        .podcast_shows_bounded(&live.token, &library.id, 0, 25, REQUEST_BOUND)
        .map_err(|error| ProbeError::contract(error.to_string()))?
        .items
        .into_iter()
        .next()
        .ok_or_else(|| ProbeError::contract("no podcast show available"))?;
    catalog
        .podcast_detail_bounded(&live.token, &show.library_item_id, REQUEST_BOUND)
        .map_err(|error| ProbeError::contract(error.to_string()))?
        .into_iter()
        .next()
        .ok_or_else(|| ProbeError::contract("no downloaded podcast episode available"))
}

fn probe_playback(
    live: &mut LiveClient,
    episode: &mbv_audiobookshelf::AudiobookshelfDownloadedEpisode,
) -> Result<PlaybackProbe, ProbeError> {
    let direct = live.play(&episode.library_item_id, &episode.episode_id, false)?;
    let direct_session = direct["id"]
        .as_str()
        .ok_or_else(|| ProbeError::contract("direct session id missing"))?
        .to_string();
    let direct_duration = duration(&direct)?;
    let (direct_path, direct_hls) = source(&direct)?;
    if direct_hls {
        return Err(ProbeError::contract("forced direct play returned HLS"));
    }
    let direct_url = absolute_url(&live.base, direct_path)?;
    let direct_seek = mpv_probe(&direct_url, Some(&live.token))?;
    let sync = live.session_request(&direct_session, "sync", direct_duration)?;
    let close = live.close(&direct_session, direct_duration)?;
    let direct_closed_status = live.get_status(&format!("/api/session/{direct_session}"), true)?;

    let hls = live.play(&episode.library_item_id, &episode.episode_id, true)?;
    let hls_session = hls["id"]
        .as_str()
        .ok_or_else(|| ProbeError::contract("HLS session id missing"))?
        .to_string();
    let hls_duration = duration(&hls)?;
    let (hls_path, is_hls) = source(&hls)?;
    if !is_hls {
        return Err(ProbeError::contract("forced transcode did not return HLS"));
    }
    let hls_url = absolute_url(&live.base, hls_path)?;
    let readiness_attempts = wait_hls(live, &hls_url)?;
    let hls_seek = mpv_probe(&hls_url, None)?;
    let hls_close = live.close(&hls_session, hls_duration)?;
    let hls_closed_status = live.get_status(&format!("/api/session/{hls_session}"), true)?;
    let hls_after_close_status = live.get_status(hls_path, false)?;

    Ok(PlaybackProbe {
        direct,
        direct_seek,
        sync,
        close,
        direct_closed_status,
        hls,
        readiness_attempts,
        hls_seek,
        hls_close,
        hls_closed_status,
        hls_after_close_status,
    })
}

fn insert_playback_output(output: &mut Map<String, Value>, probe: &PlaybackProbe) {
    output.insert(
        "absVersion".into(),
        probe
            .direct
            .get("serverVersion")
            .cloned()
            .unwrap_or(Value::Null),
    );
    output.insert("directPlay".into(), session_fixture(&probe.direct));
    output.insert("forcedTranscode".into(), session_fixture(&probe.hls));
    output.insert(
        "mpv".into(),
        json!({
            "direct": {"started": true, "seekFrom": probe.direct_seek.0, "seekTo": probe.direct_seek.1},
            "hls": {"restOnlyReady": true, "readinessAttempts": probe.readiness_attempts,
                "pollIntervalMs": 250, "boundMs": READY_BOUND.as_millis(), "started": true,
                "seekFrom": probe.hls_seek.0, "seekTo": probe.hls_seek.1, "socketIoUsed": false}
        }),
    );
}

fn insert_session_output(output: &mut Map<String, Value>, probe: &PlaybackProbe) {
    output.insert(
        "sync".into(),
        json!({"status": probe.sync.0, "bodyBytes": probe.sync.1.len()}),
    );
    output.insert(
        "close".into(),
        json!({"status": probe.close.0, "bodyBytes": probe.close.1.len()}),
    );
    output.insert(
        "hlsClose".into(),
        json!({"status": probe.hls_close.0, "bodyBytes": probe.hls_close.1.len()}),
    );
}

fn insert_cleanup_output(
    output: &mut Map<String, Value>,
    live: &LiveClient,
    probe: &PlaybackProbe,
) {
    output.insert(
        "cleanup".into(),
        json!({"openSessions": live.open_sessions.len(),
            "directSessionAfterCloseStatus":probe.direct_closed_status,
            "hlsSessionAfterCloseStatus":probe.hls_closed_status,
            "hlsPathAfterCloseStatus":probe.hls_after_close_status}),
    );
}

fn main() -> Result<(), ProbeError> {
    let mut live = LiveClient::load()?;
    let episode = first_podcast_episode(&live)?;
    let probe = probe_playback(&mut live, &episode)?;

    let auth = live
        .agent
        .get(&format!("{}/api/me", live.base))
        .header("Authorization", "Bearer <INVALID_CREDENTIAL>")
        .call();
    let auth_status = match auth {
        Err(ureq::Error::StatusCode(status)) => status,
        Ok(r) => r.status().into(),
        Err(_) => 0,
    };

    let mut output = Map::new();
    insert_playback_output(&mut output, &probe);
    insert_session_output(&mut output, &probe);
    output.insert(
        "authenticationFailure".into(),
        json!({"provenance":"live ABS 2.36.0", "status":auth_status}),
    );
    output.insert(
        "serverFailure".into(),
        controlled_failure(500, "{\"error\":\"<MESSAGE>\"}")?,
    );
    output.insert("malformedResponse".into(), controlled_failure(200, "{")?);
    insert_cleanup_output(&mut output, &live, &probe);
    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Object(output)).unwrap()
    );
    Ok(())
}
