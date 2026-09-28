use std::{error::Error, fmt, io, sync::mpsc};

#[derive(Debug)]
pub struct VisualizerError(VisualizerErrorKind);

#[derive(Debug)]
enum VisualizerErrorKind {
    Spawn(io::Error),
    StartupFailed(String),
    StartupTimeout(mpsc::RecvTimeoutError),
    WorkerStopped,
    BufferPoisoned,
    CaptureFrame(&'static str),
    Operation {
        name: &'static str,
        message: &'static str,
        source: Box<dyn Error + Send + Sync>,
    },
}

impl VisualizerError {
    pub(crate) fn startup_failed(message: String) -> Self {
        Self(VisualizerErrorKind::StartupFailed(message))
    }

    pub(crate) fn worker_stopped() -> Self {
        Self(VisualizerErrorKind::WorkerStopped)
    }

    pub(crate) fn buffer_poisoned() -> Self {
        Self(VisualizerErrorKind::BufferPoisoned)
    }

    pub(crate) fn capture_frame(message: &'static str) -> Self {
        Self(VisualizerErrorKind::CaptureFrame(message))
    }

    pub(crate) fn operation<E>(name: &'static str, message: &'static str, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self(VisualizerErrorKind::Operation {
            name,
            message,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match &self.0 {
            VisualizerErrorKind::Spawn(_) => "visualizer.spawn",
            VisualizerErrorKind::StartupFailed(_) => "visualizer.startup",
            VisualizerErrorKind::StartupTimeout(_) => "visualizer.startup_timeout",
            VisualizerErrorKind::WorkerStopped => "visualizer.worker_stopped",
            VisualizerErrorKind::BufferPoisoned => "visualizer.buffer_poisoned",
            VisualizerErrorKind::CaptureFrame(_) => "visualizer.capture_frame",
            VisualizerErrorKind::Operation { name, .. } => name,
        }
    }
}

impl fmt::Display for VisualizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            VisualizerErrorKind::Spawn(error) => {
                write!(f, "failed to start PipeWire worker: {error}")
            }
            VisualizerErrorKind::StartupFailed(message) => f.write_str(message),
            VisualizerErrorKind::StartupTimeout(error) => {
                write!(f, "PipeWire startup readiness timed out: {error}")
            }
            VisualizerErrorKind::WorkerStopped => {
                f.write_str("PipeWire worker stopped unexpectedly")
            }
            VisualizerErrorKind::BufferPoisoned => {
                f.write_str("PipeWire sample buffer was poisoned")
            }
            VisualizerErrorKind::CaptureFrame(message) => f.write_str(message),
            VisualizerErrorKind::Operation {
                message, source, ..
            } => {
                write!(f, "{message}: {source}")
            }
        }
    }
}

impl Error for VisualizerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.0 {
            VisualizerErrorKind::Spawn(error) => Some(error),
            VisualizerErrorKind::StartupTimeout(error) => Some(error),
            VisualizerErrorKind::Operation { source, .. } => Some(source.as_ref()),
            VisualizerErrorKind::StartupFailed(_)
            | VisualizerErrorKind::WorkerStopped
            | VisualizerErrorKind::BufferPoisoned
            | VisualizerErrorKind::CaptureFrame(_) => None,
        }
    }
}

impl From<io::Error> for VisualizerError {
    fn from(source: io::Error) -> Self {
        Self(VisualizerErrorKind::Spawn(source))
    }
}

impl From<mpsc::RecvTimeoutError> for VisualizerError {
    fn from(source: mpsc::RecvTimeoutError) -> Self {
        Self(VisualizerErrorKind::StartupTimeout(source))
    }
}
