//! The two backends: in-process libmpv and an `mpv` child process over JSON IPC.
//! Both report events in mpv's JSON IPC event shape.

use crate::ProbeError;
use crate::scenarios::Prop;
use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv, mpv_end_file_reason};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{fs, thread};

const CONNECT_BOUND: Duration = Duration::from_secs(5);

/// The same names, formats and IDs as `mbv-player` `runtime.rs` `observe_properties`.
const OBSERVED: [(&str, Format, u64); 11] = [
    ("time-pos", Format::Double, 0),
    ("pause", Format::Flag, 1),
    ("volume", Format::Double, 2),
    ("sid", Format::String, 3),
    ("mute", Format::Flag, 4),
    ("aid", Format::String, 5),
    ("video-params/h", Format::Int64, 6),
    ("audio-codec-name", Format::String, 7),
    ("current-tracks/video/image", Format::Flag, 8),
    ("playlist-pos", Format::Int64, 9),
    ("playlist-count", Format::Int64, 10),
];

const BASE_OPTIONS: [(&str, &str); 3] = [("idle", "yes"), ("vo", "null"), ("ao", "null")];

pub(crate) enum Next {
    Event(Value),
    Timeout,
    Closed,
}

pub(crate) trait Backend {
    fn command(&mut self, args: &[&str]);
    fn set(&mut self, name: &str, value: Prop);
    fn next(&mut self, timeout: Duration) -> Next;
}

pub(crate) struct Libmpv {
    pub(crate) mpv: Mpv,
}

impl Libmpv {
    pub(crate) fn start(options: &[(&str, &str)]) -> Result<Self, ProbeError> {
        let mpv = Mpv::with_initializer(|init| {
            for (name, value) in BASE_OPTIONS.iter().chain(options) {
                init.set_option(name, (*value).to_owned())?;
            }
            Ok(())
        })
        .map_err(|e| ProbeError(format!("libmpv init failed: {e:?}")))?;
        for (name, format, id) in OBSERVED {
            mpv.observe_property(name, format, id)
                .map_err(|e| ProbeError(format!("libmpv observe {name} failed: {e:?}")))?;
        }
        Ok(Self { mpv })
    }
}

impl Backend for Libmpv {
    fn command(&mut self, args: &[&str]) {
        if let Err(e) = self.mpv.command(args[0], &args[1..]) {
            eprintln!("  libmpv command {args:?} failed: {e:?}");
        }
    }

    fn set(&mut self, name: &str, value: Prop) {
        let result = match value {
            Prop::Flag(flag) => self.mpv.set_property(name, flag),
            Prop::Int(int) => self.mpv.set_property(name, int),
        };
        if let Err(e) = result {
            eprintln!("  libmpv set {name} failed: {e:?}");
        }
    }

    fn next(&mut self, timeout: Duration) -> Next {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            // libmpv2 also returns None for a property change with no data,
            // so None ends the wait only at the deadline.
            match self.mpv.wait_event(remaining.as_secs_f64()) {
                Some(event) => return Next::Event(libmpv_shape(event)),
                None if remaining.is_zero() => return Next::Timeout,
                None => {}
            }
        }
    }
}

/// Converts a libmpv2 event into mpv's JSON IPC event shape.
fn libmpv_shape(event: Result<Event<'_>, libmpv2::Error>) -> Value {
    match event {
        Ok(Event::PropertyChange {
            name,
            change,
            reply_userdata,
        }) => json!({
            "event": "property-change",
            "id": reply_userdata,
            "name": name,
            "data": property_json(&change),
        }),
        Ok(Event::EndFile(reason)) => json!({"event": "end-file", "reason": reason_name(reason)}),
        Ok(Event::LogMessage {
            prefix,
            level,
            text,
            ..
        }) => json!({"event": "log-message", "prefix": prefix, "level": level, "text": text}),
        Ok(Event::ClientMessage(args)) => json!({"event": "client-message", "args": args}),
        Ok(Event::Deprecated(raw)) => json!({"event": deprecated_name(raw.event_id)}),
        Ok(
            Event::GetPropertyReply { .. } | Event::SetPropertyReply(_) | Event::CommandReply(_),
        ) => json!({"request_id": 0}),
        Ok(other) => json!({"event": simple_event_name(&other)}),
        Err(libmpv2::Error::Raw(code)) => {
            json!({"event": "end-file", "reason": "error", "error_code": code})
        }
        Err(e) => json!({"event": "libmpv2-error", "detail": format!("{e:?}")}),
    }
}

fn simple_event_name(event: &Event<'_>) -> &'static str {
    match event {
        Event::Shutdown => "shutdown",
        Event::StartFile => "start-file",
        Event::FileLoaded => "file-loaded",
        Event::VideoReconfig => "video-reconfig",
        Event::AudioReconfig => "audio-reconfig",
        Event::Seek => "seek",
        Event::PlaybackRestart => "playback-restart",
        Event::QueueOverflow => "queue-overflow",
        _ => "unexpected",
    }
}

fn deprecated_name(event_id: libmpv2::events::EventId) -> String {
    if event_id == libmpv2_sys::mpv_event_id_MPV_EVENT_IDLE {
        "idle".to_owned()
    } else {
        format!("deprecated-{event_id}")
    }
}

fn reason_name(reason: libmpv2::EndFileReason) -> String {
    match reason {
        mpv_end_file_reason::Eof => "eof".to_owned(),
        mpv_end_file_reason::Stop => "stop".to_owned(),
        mpv_end_file_reason::Quit => "quit".to_owned(),
        mpv_end_file_reason::Error => "error".to_owned(),
        mpv_end_file_reason::Redirect => "redirect".to_owned(),
        other => format!("reason-{other}"),
    }
}

fn property_json(data: &PropertyData<'_>) -> Value {
    match *data {
        PropertyData::Str(text) | PropertyData::OsdStr(text) => json!(text),
        PropertyData::Flag(flag) => json!(flag),
        PropertyData::Int64(int) => json!(int),
        PropertyData::Double(double) => json!(double),
    }
}

pub(crate) struct Ipc {
    child: Child,
    socket: PathBuf,
    writer: UnixStream,
    lines: mpsc::Receiver<String>,
    /// Every line mpv wrote, in arrival order.
    pub(crate) raw: Vec<String>,
    next_request: u64,
}

impl Ipc {
    pub(crate) fn start(options: &[(&str, &str)], socket: PathBuf) -> Result<Self, ProbeError> {
        let mut args = vec!["--no-config".to_owned(), "--no-terminal".to_owned()];
        for (name, value) in BASE_OPTIONS.iter().chain(options) {
            args.push(format!("--{name}={value}"));
        }
        args.push(format!("--input-ipc-server={}", socket.display()));
        let child = Command::new("mpv")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ProbeError(format!("cannot start mpv: {e}")))?;
        let writer = connect(&socket)?;
        let reader = writer
            .try_clone()
            .map_err(|e| ProbeError(format!("cannot clone IPC socket: {e}")))?;
        let (tx, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut ipc = Self {
            child,
            socket,
            writer,
            lines,
            raw: Vec::new(),
            next_request: 1,
        };
        for (name, format, id) in OBSERVED {
            // Format::String matches the IPC string observation, not the native type.
            let command = if format == Format::String {
                "observe_property_string"
            } else {
                "observe_property"
            };
            ipc.send(&json!([command, id, name]));
        }
        Ok(ipc)
    }

    fn send(&mut self, command: &Value) {
        let line = json!({"command": command, "request_id": self.next_request});
        self.next_request += 1;
        if let Err(e) = writeln!(self.writer, "{line}") {
            eprintln!("  IPC write failed: {e}");
        }
    }
}

fn connect(socket: &Path) -> Result<UnixStream, ProbeError> {
    let deadline = Instant::now() + CONNECT_BOUND;
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(stream),
            Err(e) if Instant::now() >= deadline => {
                return Err(ProbeError(format!("cannot connect to mpv IPC: {e}")));
            }
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
}

impl Backend for Ipc {
    fn command(&mut self, args: &[&str]) {
        self.send(&json!(args));
    }

    fn set(&mut self, name: &str, value: Prop) {
        let value = match value {
            Prop::Flag(flag) => json!(flag),
            Prop::Int(int) => json!(int),
        };
        self.send(&json!(["set_property", name, value]));
    }

    fn next(&mut self, timeout: Duration) -> Next {
        match self.lines.recv_timeout(timeout) {
            Ok(line) => {
                let value = serde_json::from_str::<Value>(&line)
                    .unwrap_or_else(|_| json!({"event": "unparsed", "line": line}));
                if value.get("error").is_some_and(|e| e != "success") {
                    eprintln!("  IPC reply error: {line}");
                }
                self.raw.push(line);
                Next::Event(value)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Next::Timeout,
            Err(mpsc::RecvTimeoutError::Disconnected) => Next::Closed,
        }
    }
}

impl Drop for Ipc {
    fn drop(&mut self) {
        let _ = writeln!(self.writer, r#"{{"command":["quit"]}}"#);
        let deadline = Instant::now() + CONNECT_BOUND;
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.socket);
    }
}
