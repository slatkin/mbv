use std::{backtrace::Backtrace, error::Error, ffi::NulError, fmt, io};

#[derive(Debug)]
pub struct PlayerError {
    kind: PlayerErrorKind,
    backtrace: Backtrace,
}

#[derive(Debug)]
enum PlayerErrorKind {
    RemoveDirectory {
        path: String,
        source: io::Error,
    },
    RemovePath {
        path: String,
        source: io::Error,
    },
    InspectDirectory {
        path: String,
        source: io::Error,
    },
    CreateDirectory {
        path: String,
        source: io::Error,
    },
    WriteConfig {
        path: String,
        source: io::Error,
    },
    PipeNotFifo(String),
    UnsafeFifo {
        path: String,
        detail: String,
    },
    PipePath(NulError),
    MakePipe {
        path: String,
        source: io::Error,
    },
    SetAudioDevice {
        device: String,
        source: libmpv2::Error,
    },
    MpvInit {
        message: String,
        source: libmpv2::Error,
    },
}

impl PlayerError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    pub(crate) fn remove_directory(path: String, source: io::Error) -> Self {
        Self {
            kind: PlayerErrorKind::RemoveDirectory { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn remove_path(path: String, source: io::Error) -> Self {
        Self {
            kind: PlayerErrorKind::RemovePath { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn inspect_directory(path: String, source: io::Error) -> Self {
        Self {
            kind: PlayerErrorKind::InspectDirectory { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn create_directory(path: String, source: io::Error) -> Self {
        Self {
            kind: PlayerErrorKind::CreateDirectory { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn write_config(path: String, source: io::Error) -> Self {
        Self {
            kind: PlayerErrorKind::WriteConfig { path, source },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn pipe_not_fifo(path: &str) -> Self {
        Self {
            kind: PlayerErrorKind::PipeNotFifo(path.to_owned()),
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn unsafe_fifo(path: &str, detail: String) -> Self {
        Self {
            kind: PlayerErrorKind::UnsafeFifo {
                path: path.to_owned(),
                detail,
            },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn make_pipe(path: &str, source: io::Error) -> Self {
        Self {
            kind: PlayerErrorKind::MakePipe {
                path: path.to_owned(),
                source,
            },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn set_audio_device(device: &str, source: libmpv2::Error) -> Self {
        Self {
            kind: PlayerErrorKind::SetAudioDevice {
                device: device.to_owned(),
                source,
            },
            backtrace: Backtrace::capture(),
        }
    }

    pub(crate) fn mpv_init(message: String, source: libmpv2::Error) -> Self {
        Self {
            kind: PlayerErrorKind::MpvInit { message, source },
            backtrace: Backtrace::capture(),
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.kind {
            PlayerErrorKind::RemoveDirectory { .. } => "player.remove_config_directory",
            PlayerErrorKind::RemovePath { .. } => "player.remove_config_path",
            PlayerErrorKind::InspectDirectory { .. } => "player.inspect_config_directory",
            PlayerErrorKind::CreateDirectory { .. } => "player.create_config_directory",
            PlayerErrorKind::WriteConfig { .. } => "player.write_config",
            PlayerErrorKind::PipeNotFifo(_) => "player.pipe_not_fifo",
            PlayerErrorKind::UnsafeFifo { .. } => "player.unsafe_fifo",
            PlayerErrorKind::PipePath(_) => "player.pipe_path",
            PlayerErrorKind::MakePipe { .. } => "player.make_pipe",
            PlayerErrorKind::SetAudioDevice { .. } => "player.set_audio_device",
            PlayerErrorKind::MpvInit { .. } => "player.mpv_init",
        }
    }
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PlayerErrorKind::RemoveDirectory { path, source } => {
                write!(
                    f,
                    "failed to remove private mpv config dir '{path}': {source}"
                )
            }
            PlayerErrorKind::RemovePath { path, source } => {
                write!(
                    f,
                    "failed to remove private mpv config path '{path}': {source}"
                )
            }
            PlayerErrorKind::InspectDirectory { path, source } => {
                write!(
                    f,
                    "failed to inspect private mpv config dir '{path}': {source}"
                )
            }
            PlayerErrorKind::CreateDirectory { path, source } => {
                write!(
                    f,
                    "failed to create private mpv config dir '{path}': {source}"
                )
            }
            PlayerErrorKind::WriteConfig { path, source } => {
                write!(f, "failed to write private mpv.conf in '{path}': {source}")
            }
            PlayerErrorKind::PipeNotFifo(path) => {
                write!(f, "audio pipe path '{path}' exists and is not a FIFO")
            }
            PlayerErrorKind::UnsafeFifo { path, detail } => {
                write!(
                    f,
                    "refusing audio pipe path '{path}': existing FIFO {detail}"
                )
            }
            PlayerErrorKind::PipePath(source) => source.fmt(f),
            PlayerErrorKind::MakePipe { path, source } => {
                write!(f, "mkfifo({path}) failed: {source}")
            }
            PlayerErrorKind::SetAudioDevice { device, source } => write!(
                f,
                "clocked audio output: failed to set audio-device '{device}': {}",
                super::mpv_err_str(source)
            ),
            PlayerErrorKind::MpvInit { message, .. } => f.write_str(message),
        }
    }
}

impl Error for PlayerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            PlayerErrorKind::RemoveDirectory { source, .. }
            | PlayerErrorKind::RemovePath { source, .. }
            | PlayerErrorKind::InspectDirectory { source, .. }
            | PlayerErrorKind::CreateDirectory { source, .. }
            | PlayerErrorKind::WriteConfig { source, .. }
            | PlayerErrorKind::MakePipe { source, .. } => Some(source),
            PlayerErrorKind::PipePath(source) => Some(source),
            PlayerErrorKind::SetAudioDevice { source, .. }
            | PlayerErrorKind::MpvInit { source, .. } => Some(source),
            PlayerErrorKind::PipeNotFifo(_) | PlayerErrorKind::UnsafeFifo { .. } => None,
        }
    }
}

impl From<NulError> for PlayerError {
    fn from(source: NulError) -> Self {
        Self {
            kind: PlayerErrorKind::PipePath(source),
            backtrace: Backtrace::capture(),
        }
    }
}
