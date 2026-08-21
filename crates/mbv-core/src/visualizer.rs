use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, TryLockError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::spa::param::audio::{AudioFormat, AudioInfoRaw};
use pw::spa::param::format::{MediaSubtype, MediaType};
use pw::spa::pod::Pod;

const STARTUP_TIMEOUT: Duration = Duration::from_millis(500);
const SAMPLE_WINDOW_MS: usize = 33;
const MAX_SAMPLE_PAIRS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StereoSampleWindow {
    pub generation: u64,
    pub samples: Vec<StereoSample>,
}

#[derive(Debug)]
pub(crate) struct StereoSampleBuffer {
    samples: VecDeque<StereoSample>,
    capacity: usize,
    generation: u64,
}

impl StereoSampleBuffer {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
            generation: 0,
        }
    }

    fn with_sample_rate(rate: u32) -> Self {
        let capacity = ((rate as usize * SAMPLE_WINDOW_MS) / 1_000).clamp(2, MAX_SAMPLE_PAIRS);
        Self::with_capacity(capacity)
    }

    #[cfg(test)]
    pub(crate) fn push_interleaved(&mut self, values: &[f32]) {
        let mut appended = false;
        for pair in values.chunks_exact(2) {
            if !pair[0].is_finite() || !pair[1].is_finite() {
                continue;
            }
            self.push_pair(StereoSample {
                left: pair[0],
                right: pair[1],
            });
            appended = true;
        }
        if appended {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn push_pcm_bytes(&mut self, bytes: &[u8], channels: u32) {
        if channels != 2 {
            return;
        }
        let mut appended = false;
        for pair in bytes.chunks_exact(8) {
            let left = f32::from_le_bytes(pair[0..4].try_into().unwrap());
            let right = f32::from_le_bytes(pair[4..8].try_into().unwrap());
            if !left.is_finite() || !right.is_finite() {
                continue;
            }
            self.push_pair(StereoSample { left, right });
            appended = true;
        }
        if appended {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn push_pair(&mut self, sample: StereoSample) {
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub(crate) fn snapshot(&self) -> StereoSampleWindow {
        StereoSampleWindow {
            generation: self.generation,
            samples: self.samples.iter().copied().collect(),
        }
    }

    fn clear(&mut self) {
        self.samples.clear();
        self.generation = self.generation.wrapping_add(1);
    }
}

enum Startup {
    Ready,
    Failed(String),
}

enum Control {
    Stop,
}

struct WorkerData {
    format: AudioInfoRaw,
    streaming: bool,
    buffer: Arc<Mutex<StereoSampleBuffer>>,
    startup_tx: Option<Sender<Startup>>,
    failure_tx: Sender<String>,
}

pub struct PipeWireWorker {
    stop_tx: pw::channel::Sender<Control>,
    failure_rx: Receiver<String>,
    buffer: Arc<Mutex<StereoSampleBuffer>>,
    handle: Option<JoinHandle<()>>,
}

impl PipeWireWorker {
    pub fn start() -> Result<Self, String> {
        let (stop_tx, stop_rx) = pw::channel::channel();
        let (startup_tx, startup_rx) = mpsc::channel();
        let (failure_tx, failure_rx) = mpsc::channel();
        let buffer = Arc::new(Mutex::new(StereoSampleBuffer::with_capacity(1)));
        let worker_buffer = buffer.clone();
        let handle = thread::Builder::new()
            .name("mbv-pipewire-visualizer".into())
            .spawn(move || run_worker(stop_rx, startup_tx, failure_tx, worker_buffer))
            .map_err(|error| format!("failed to start PipeWire worker: {error}"))?;

        match startup_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Startup::Ready) => Ok(Self {
                stop_tx,
                failure_rx,
                buffer,
                handle: Some(handle),
            }),
            Ok(Startup::Failed(error)) => {
                let _ = stop_tx.send(Control::Stop);
                join_worker(handle);
                Err(error)
            }
            Err(error) => {
                let _ = stop_tx.send(Control::Stop);
                join_worker(handle);
                Err(format!("PipeWire startup readiness timed out: {error}"))
            }
        }
    }

    pub fn take_latest_window(&self) -> Result<Option<StereoSampleWindow>, String> {
        match self.failure_rx.try_recv() {
            Ok(error) => return Err(error),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err("PipeWire worker stopped unexpectedly".into())
            }
        }

        match self.buffer.try_lock() {
            Ok(buffer) => Ok(Some(buffer.snapshot())),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Poisoned(_)) => Err("PipeWire sample buffer was poisoned".into()),
        }
    }

    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(Control::Stop);
        if let Some(handle) = self.handle.take() {
            join_worker(handle);
        }
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.clear();
        }
    }
}

impl Drop for PipeWireWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn capture_frame_bytes<'a>(
    format: &AudioInfoRaw,
    bytes: &'a [u8],
    offset: usize,
    size: usize,
    stride: i32,
    corrupted: bool,
) -> Result<&'a [u8], &'static str> {
    if format.format() != AudioFormat::F32LE || format.channels() != 2 || format.rate() == 0 {
        return Err("negotiated format is not interleaved stereo F32LE");
    }
    if corrupted {
        return Err("PipeWire supplied a corrupted capture chunk");
    }
    if stride != 8 || !size.is_multiple_of(8) {
        return Err("PipeWire supplied an invalid stereo frame layout");
    }
    let end = offset
        .checked_add(size)
        .filter(|end| *end <= bytes.len())
        .ok_or("PipeWire capture chunk exceeds its buffer")?;
    Ok(&bytes[offset..end])
}

fn run_worker(
    stop_rx: pw::channel::Receiver<Control>,
    startup_tx: Sender<Startup>,
    failure_tx: Sender<String>,
    buffer: Arc<Mutex<StereoSampleBuffer>>,
) {
    pw::init();

    let mainloop = match pw::main_loop::MainLoopRc::new(None) {
        Ok(mainloop) => mainloop,
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!(
                "failed to create PipeWire main loop: {error}"
            )));
            return;
        }
    };
    let context = match pw::context::ContextRc::new(&mainloop, None) {
        Ok(context) => context,
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!(
                "failed to create PipeWire context: {error}"
            )));
            return;
        }
    };
    let core = match context.connect_rc(None) {
        Ok(core) => core,
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!(
                "failed to connect to PipeWire: {error}"
            )));
            return;
        }
    };

    let _stop_listener = stop_rx.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        move |_| mainloop.quit()
    });

    let stream = match pw::stream::StreamBox::new(
        &core,
        "mbv-system-audio-visualizer",
        properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::STREAM_CAPTURE_SINK => "true",
        },
    ) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = startup_tx.send(Startup::Failed(format!(
                "failed to create PipeWire capture stream: {error}"
            )));
            return;
        }
    };
    let startup_error_tx = startup_tx.clone();

    let listener = stream
        .add_local_listener_with_user_data(WorkerData {
            format: AudioInfoRaw::new(),
            streaming: false,
            buffer,
            startup_tx: Some(startup_tx),
            failure_tx,
        })
        .state_changed({
            let mainloop = mainloop.clone();
            move |_, data, _, new| match new {
                pw::stream::StreamState::Streaming => {
                    data.streaming = true;
                    if data.format.format() == AudioFormat::F32LE
                        && data.format.channels() == 2
                        && data.format.rate() != 0
                    {
                        if let Some(startup_tx) = data.startup_tx.take() {
                            let _ = startup_tx.send(Startup::Ready);
                        }
                    }
                }
                pw::stream::StreamState::Error(error) => {
                    let message = format!("PipeWire capture stream failed: {error}");
                    if let Some(startup_tx) = data.startup_tx.take() {
                        let _ = startup_tx.send(Startup::Failed(message));
                    } else {
                        let _ = data.failure_tx.send(message);
                    }
                    if let Ok(mut buffer) = data.buffer.try_lock() {
                        buffer.clear();
                    }
                    mainloop.quit();
                }
                _ => {}
            }
        })
        .param_changed({
            let mainloop = mainloop.clone();
            move |_, data, id, param| {
                let Some(param) = param else {
                    return;
                };
                if id != pw::spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let result = (|| {
                    let (media_type, media_subtype) =
                        pw::spa::param::format_utils::parse_format(param).map_err(|error| {
                            format!("failed to parse PipeWire media format: {error}")
                        })?;
                    if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                        return Err("PipeWire negotiated a non-raw-audio format".to_string());
                    }
                    let mut format = AudioInfoRaw::new();
                    format.parse(param).map_err(|error| {
                        format!("failed to parse PipeWire audio format: {error}")
                    })?;
                    if format.format() != AudioFormat::F32LE
                        || format.channels() != 2
                        || format.rate() == 0
                    {
                        return Err(format!("unsupported PipeWire capture format: {format:?}"));
                    }
                    let mut buffer = data
                        .buffer
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *buffer = StereoSampleBuffer::with_sample_rate(format.rate());
                    data.format = format;
                    if data.streaming {
                        if let Some(startup_tx) = data.startup_tx.take() {
                            let _ = startup_tx.send(Startup::Ready);
                        }
                    }
                    Ok(())
                })();
                if let Err(message) = result {
                    if let Some(startup_tx) = data.startup_tx.take() {
                        let _ = startup_tx.send(Startup::Failed(message));
                    } else {
                        let _ = data.failure_tx.send(message);
                    }
                    mainloop.quit();
                }
            }
        })
        .process({
            let mainloop = mainloop.clone();
            move |stream, data| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let Some(raw) = buffer.datas_mut().first_mut() else {
                    return;
                };
                let offset = raw.chunk().offset() as usize;
                let size = raw.chunk().size() as usize;
                let stride = raw.chunk().stride();
                let corrupted = raw
                    .chunk()
                    .flags()
                    .contains(pw::spa::buffer::ChunkFlags::CORRUPTED);
                let Some(bytes) = raw.data() else {
                    return;
                };
                match capture_frame_bytes(&data.format, bytes, offset, size, stride, corrupted) {
                    Ok(frame) => {
                        if let Ok(mut sample_buffer) = data.buffer.try_lock() {
                            sample_buffer.push_pcm_bytes(frame, data.format.channels());
                        }
                    }
                    Err(error) => {
                        if let Ok(mut sample_buffer) = data.buffer.try_lock() {
                            sample_buffer.clear();
                        }
                        let _ = data.failure_tx.send(error.into());
                        mainloop.quit();
                    }
                }
            }
        })
        .register();

    let listener = match listener {
        Ok(listener) => listener,
        Err(error) => {
            let _ = startup_error_tx.send(Startup::Failed(format!(
                "failed to register PipeWire listener: {error}"
            )));
            return;
        }
    };

    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    audio_info.set_channels(2);
    let object = pw::spa::pod::Object {
        type_: pw::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: pw::spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    let values = match pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(object),
    ) {
        Ok((values, _)) => values.into_inner(),
        Err(error) => {
            let _ = startup_error_tx.send(Startup::Failed(format!(
                "failed to serialize PipeWire format: {error}"
            )));
            return;
        }
    };
    let Some(pod) = Pod::from_bytes(&values) else {
        let _ = startup_error_tx.send(Startup::Failed(
            "failed to create PipeWire format pod".into(),
        ));
        return;
    };
    let mut params = [pod];
    if let Err(error) = stream.connect(
        spa::utils::Direction::Input,
        None,
        pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS,
        &mut params,
    ) {
        let _ = startup_error_tx.send(Startup::Failed(format!(
            "failed to connect PipeWire capture stream: {error}"
        )));
        return;
    }

    mainloop.run();
    let _ = stream.disconnect();
    drop(listener);
}

fn join_worker(handle: JoinHandle<()>) {
    if handle.join().is_err() {
        log::warn!(target: "visualizer", "PipeWire worker thread panicked");
    }
}

#[cfg(test)]
mod tests {
    use super::{capture_frame_bytes, PipeWireWorker, StereoSample, StereoSampleBuffer};
    use pipewire::spa::param::audio::{AudioFormat, AudioInfoRaw};
    use std::time::{Duration, Instant};

    #[test]
    fn overwrite_buffer_keeps_newest_complete_stereo_pairs() {
        let mut buffer = StereoSampleBuffer::with_capacity(2);

        buffer.push_interleaved(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);

        let window = buffer.snapshot();
        assert_eq!(
            window.samples,
            vec![
                StereoSample {
                    left: 0.3,
                    right: 0.4,
                },
                StereoSample {
                    left: 0.5,
                    right: 0.6,
                },
            ]
        );
    }

    #[test]
    fn overwrite_buffer_discards_incomplete_channel_pair() {
        let mut buffer = StereoSampleBuffer::with_capacity(2);

        buffer.push_interleaved(&[0.1, 0.2, 0.3]);

        assert_eq!(
            buffer.snapshot().samples,
            vec![StereoSample {
                left: 0.1,
                right: 0.2,
            }]
        );
    }

    #[test]
    fn pcm_buffer_preserves_finite_sample_amplitudes() {
        let mut buffer = StereoSampleBuffer::with_capacity(2);
        let mut bytes = Vec::new();
        for value in [1.5_f32, -1.5] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        buffer.push_pcm_bytes(&bytes, 2);

        assert_eq!(
            buffer.snapshot().samples,
            vec![StereoSample {
                left: 1.5,
                right: -1.5,
            }]
        );
    }

    #[test]
    fn capture_frame_requires_interleaved_stereo_f32le_frames() {
        let mut format = AudioInfoRaw::new();
        format.set_format(AudioFormat::F32LE);
        format.set_channels(2);
        format.set_rate(48_000);
        let bytes = [0u8; 8];
        assert!(capture_frame_bytes(&format, &bytes, 0, 8, 8, false).is_ok());

        format.set_format(AudioFormat::S16LE);
        assert!(capture_frame_bytes(&format, &bytes, 0, 8, 8, false).is_err());
        format.set_format(AudioFormat::F32LE);
        format.set_channels(1);
        assert!(capture_frame_bytes(&format, &bytes, 0, 8, 8, false).is_err());
        format.set_channels(2);
        assert!(capture_frame_bytes(&format, &bytes, 0, 8, 16, false).is_err());
        assert!(capture_frame_bytes(&format, &bytes, 0, 7, 8, false).is_err());
        assert!(capture_frame_bytes(&format, &bytes, 1, 8, 8, false).is_err());
        assert!(capture_frame_bytes(&format, &bytes, 0, 8, 8, true).is_err());
        format.set_rate(0);
        assert!(capture_frame_bytes(&format, &bytes, 0, 8, 8, false).is_err());
    }

    #[test]
    fn sample_window_capacity_uses_negotiated_rate() {
        assert_eq!(StereoSampleBuffer::with_sample_rate(44_100).capacity, 1_455);
        assert_eq!(StereoSampleBuffer::with_sample_rate(48_000).capacity, 1_584);
    }

    #[test]
    fn worker_failure_is_reported_or_shutdown_is_bounded() {
        match PipeWireWorker::start() {
            Ok(mut worker) => {
                let started = Instant::now();
                worker.stop();
                assert!(started.elapsed() < Duration::from_secs(2));
            }
            Err(error) => assert!(!error.is_empty()),
        }
    }
}
