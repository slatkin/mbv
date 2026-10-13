//! Decoder for mpv JSON IPC output (design D5). The IPC session in
//! `mpv_session/ipc.rs` calls it for each parsed event line.

use std::fmt;

use serde_json::{Map, Value};

use super::{AUDIO_OUTPUT_INIT_FAILED, EndFileReason, MpvEvent, MpvFailure, PropertyValue};

/// A line of mpv JSON IPC output that the decoder cannot read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DecodeError {
    NotAnObject,
    MissingField { event: String, field: &'static str },
    UnknownProperty(String),
    BadValue { name: String },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => f.write_str("JSON value is not an object"),
            Self::MissingField { event, field } => {
                write!(f, "{event} event has no usable `{field}` field")
            }
            Self::UnknownProperty(name) => write!(f, "unobserved property `{name}`"),
            Self::BadValue { name } => write!(f, "property `{name}` has a value of the wrong type"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl MpvEvent {
    /// Decodes one parsed line of mpv JSON IPC output.
    ///
    /// A line with no `event` key (a command reply, or a fixture header) and an
    /// event that the run loop does not act on both become [`MpvEvent::Ignored`].
    /// That includes events that a later mpv version adds.
    pub(crate) fn from_ipc_value(value: &Value) -> Result<Self, DecodeError> {
        let object = value.as_object().ok_or(DecodeError::NotAnObject)?;
        let Some(event) = object.get("event").and_then(Value::as_str) else {
            return Ok(Self::Ignored);
        };
        match event {
            "property-change" => property_change_from_ipc(object),
            "playback-restart" => Ok(Self::PlaybackRestart),
            "end-file" => end_file_from_ipc(object),
            "log-message" => Ok(Self::LogMessage {
                prefix: str_field(object, event, "prefix")?.to_owned(),
                level: str_field(object, event, "level")?.to_owned(),
                text: str_field(object, event, "text")?.to_owned(),
            }),
            "client-message" => client_message_from_ipc(object),
            "shutdown" => Ok(Self::Shutdown),
            _ => Ok(Self::Ignored),
        }
    }
}

fn str_field<'a>(
    object: &'a Map<String, Value>,
    event: &str,
    field: &'static str,
) -> Result<&'a str, DecodeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| DecodeError::MissingField {
            event: event.to_owned(),
            field,
        })
}

fn client_message_from_ipc(object: &Map<String, Value>) -> Result<MpvEvent, DecodeError> {
    let missing = || DecodeError::MissingField {
        event: "client-message".to_owned(),
        field: "args",
    };
    let args = object
        .get("args")
        .and_then(Value::as_array)
        .ok_or_else(missing)?;
    args.iter()
        .map(|arg| arg.as_str().map(str::to_owned).ok_or_else(missing))
        .collect::<Result<_, _>>()
        .map(MpvEvent::ClientMessage)
}

fn property_change_from_ipc(object: &Map<String, Value>) -> Result<MpvEvent, DecodeError> {
    let name = str_field(object, "property-change", "name")?;
    let value = match object.get("data").filter(|data| !data.is_null()) {
        None => PropertyValue::None,
        Some(data) => PropertyValue::from_ipc(name, data)?,
    };
    Ok(MpvEvent::PropertyChange {
        name: name.to_owned(),
        value,
    })
}

/// JSON IPC says why an end-file failed in `file_error`, as text. Classify the
/// one failure the run loop treats specially by comparing that text.
fn end_file_from_ipc(object: &Map<String, Value>) -> Result<MpvEvent, DecodeError> {
    let reason = str_field(object, "end-file", "reason")?;
    Ok(match reason {
        "eof" => MpvEvent::EndFile(EndFileReason::Eof),
        "stop" => MpvEvent::EndFile(EndFileReason::Stop),
        "quit" => MpvEvent::EndFile(EndFileReason::Quit),
        "redirect" => MpvEvent::EndFile(EndFileReason::Redirect),
        "error" => {
            let text = str_field(object, "end-file", "file_error")?;
            MpvEvent::Failed(if text == AUDIO_OUTPUT_INIT_FAILED {
                MpvFailure::AudioOutputInit
            } else {
                MpvFailure::Other(text.to_owned())
            })
        }
        other => MpvEvent::Failed(MpvFailure::Other(format!("end-file reason {other}"))),
    })
}

impl PropertyValue {
    /// Types a JSON value by property name, with the formats that
    /// `observe_properties` requests.
    fn from_ipc(name: &str, data: &Value) -> Result<Self, DecodeError> {
        let value = match name {
            "time-pos" | "volume" => data.as_f64().map(Self::Double),
            "pause" | "mute" | "current-tracks/video/image" => data.as_bool().map(Self::Flag),
            "video-params/h" | "playlist-pos" | "playlist-count" => data.as_i64().map(Self::Int64),
            "sid" | "aid" | "audio-codec-name" => data.as_str().map(|s| Self::Str(s.to_owned())),
            _ => return Err(DecodeError::UnknownProperty(name.to_owned())),
        };
        value.ok_or_else(|| DecodeError::BadValue {
            name: name.to_owned(),
        })
    }
}
