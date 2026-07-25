use ratatui_image::thread::{ResizeRequest, ResizeResponse};
use std::sync::mpsc;
use std::time::Duration;

/// Registers a per-cache-key `ResizeRequest` receiver with the resize
/// worker thread; see `spawn_resize_worker`.
pub(super) type ResizeRegisterTx = mpsc::Sender<(String, mpsc::Receiver<ResizeRequest>)>;
/// Completed off-thread resize+encode results, tagged with the
/// `card_image_states` cache key they belong to; see `spawn_resize_worker`.
pub(super) type ResizeResponseRx = mpsc::Receiver<(String, ResizeResponse)>;

/// Spawns the single background worker that performs
/// `StatefulProtocol::resize_encode()` — resample + terminal-protocol encode
/// (e.g. kitty's base64 payload) — off the render thread (#164).
///
/// `ResizeRequest`/`ResizeResponse` (from `ratatui_image::thread`) carry no
/// identifying key of their own, so a single shared request channel can't
/// tell the worker which `card_image_states` entry a given request came
/// from. Instead, each cache key gets its own dedicated `ResizeRequest`
/// channel (created in `App::new_thread_protocol`), registered with this
/// worker over `resize_register_tx`. The worker round-robins a `try_recv`
/// poll across all registered per-key receivers — still entirely off the
/// render thread — and tags each result with its key before sending it back
/// over the single shared `resize_response_rx`.
///
/// A per-key receiver whose sender has been dropped (its `ThreadProtocol`
/// evicted from `card_image_states`, e.g. by LRU eviction) is simply
/// removed from the poll set; it never produces a response. A panic inside
/// `resize_encode()` is caught so it cannot silently stall every other
/// in-flight or future resize request on this worker — only that one
/// image's response is lost, same failure mode as the request simply never
/// arriving.
pub(super) fn spawn_resize_worker() -> (ResizeRegisterTx, ResizeResponseRx) {
    let (register_tx, register_rx) = mpsc::channel::<(String, mpsc::Receiver<ResizeRequest>)>();
    let (response_tx, response_rx) = mpsc::channel::<(String, ResizeResponse)>();
    std::thread::spawn(move || {
        let mut receivers: Vec<(String, mpsc::Receiver<ResizeRequest>)> = Vec::new();
        loop {
            loop {
                match register_rx.try_recv() {
                    Ok(pair) => receivers.push(pair),
                    Err(mpsc::TryRecvError::Empty) => break,
                    // App is gone; nothing left to serve.
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            let mut did_work = false;
            let mut i = 0;
            while i < receivers.len() {
                match receivers[i].1.try_recv() {
                    Ok(request) => {
                        did_work = true;
                        let key = receivers[i].0.clone();
                        // catch_unwind: a panic here must not kill this
                        // long-lived worker thread, which would silently
                        // stall every other key's resize requests forever.
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            request.resize_encode()
                        }));
                        if let Ok(Ok(response)) = result {
                            let _ = response_tx.send((key, response));
                        }
                        i += 1;
                    }
                    Err(mpsc::TryRecvError::Empty) => i += 1,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        receivers.remove(i);
                    }
                }
            }
            if !did_work {
                std::thread::sleep(Duration::from_millis(4));
            }
        }
    });
    (register_tx, response_rx)
}
