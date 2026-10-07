use ratatui_image::thread::{ResizeRequest, ResizeResponse};
use std::sync::mpsc;
use std::time::Duration;

/// Registers a per-cache-key `ResizeRequest` receiver with the resize
/// worker thread; see `spawn_resize_worker`.
pub type ResizeRegisterTx = mpsc::Sender<(String, mpsc::Receiver<ResizeRequest>)>;
/// Completed off-thread resize+encode results, tagged with the
/// `card_image_states` cache key they belong to; see `spawn_resize_worker`.
pub type ResizeResponseRx = mpsc::Receiver<(String, ResizeResponse)>;

/// Runs one `resize_encode()` request and routes the result to
/// `response_tx`. Returns `false` only when the third-party encode
/// panicked: the response is lost, and per M-PANIC-CONTINUATION the caller
/// must stop serving instead of continuing on a worker that just observed
/// impossible state. A non-panic encode error keeps the worker alive —
/// same failure mode as the request simply never arriving.
fn resize_and_send(
    key: String,
    request: ResizeRequest,
    response_tx: &mpsc::Sender<(String, ResizeResponse)>,
) -> bool {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| request.resize_encode())) {
        Ok(Ok(response)) => {
            let _ = response_tx.send((key, response));
            true
        }
        Ok(Err(_)) => true,
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("non-string panic payload");
            tracing::error!(
                name: "images.resize.worker_panic",
                target: "images",
                key = %key,
                message = %message,
                "resize_encode panicked; the resize worker stops serving"
            );
            false
        }
    }
}

/// The resize worker's body, split from `spawn_resize_worker` so the
/// panic-stop contract can be driven synchronously in tests: the loop's
/// only exits are the owner dropping its `ResizeRegisterTx` and a caught
/// `resize_encode` panic, and both `return` from here. The caller owns the
/// channel ends; the spawned thread drops them when this body returns,
/// closing the response channel for the owner.
fn serve_resize_requests(
    register_rx: &mpsc::Receiver<(String, mpsc::Receiver<ResizeRequest>)>,
    response_tx: &mpsc::Sender<(String, ResizeResponse)>,
) {
    let mut receivers: Vec<(String, mpsc::Receiver<ResizeRequest>)> = Vec::new();
    loop {
        loop {
            match register_rx.try_recv() {
                Ok(pair) => receivers.push(pair),
                Err(mpsc::TryRecvError::Empty) => break,
                // The owner is gone; nothing left to serve.
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
                    // M-PANIC-CONTINUATION: a caught panic must not be
                    // followed by continued serving. Exiting drops
                    // `response_tx`, which the owner observes as
                    // `TryRecvError::Disconnected` and uses to respawn a
                    // fresh worker; this thread's per-key registrations die
                    // with it and are re-created by the owner's lazy
                    // protocol rebuilds.
                    if !resize_and_send(key, request, response_tx) {
                        return;
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
}

/// Spawns the single background worker that performs
/// `StatefulProtocol::resize_encode()` — resample + terminal-protocol encode
/// (e.g. kitty's base64 payload) — off the render thread (#164).
///
/// `ResizeRequest`/`ResizeResponse` (from `ratatui_image::thread`) carry no
/// identifying key of their own, so a single shared request channel can't
/// tell the worker which `card_image_states` entry a given request came
/// from. Instead, each cache key gets its own dedicated `ResizeRequest`
/// channel (created by the image-cache protocol builder), registered with this
/// worker over `resize_register_tx`. The worker round-robins a `try_recv`
/// poll across all registered per-key receivers — still entirely off the
/// render thread — and tags each result with its key before sending it back
/// over the single shared `resize_response_rx`.
///
/// A per-key receiver whose sender has been dropped (its `ThreadProtocol`
/// evicted from `card_image_states`, e.g. by LRU eviction) is simply
/// removed from the poll set; it never produces a response.
///
/// A panic inside `resize_encode()` (third-party code) ends this worker:
/// per M-PANIC-CONTINUATION a per-request `catch_unwind` must not be
/// followed by continued serving on possibly-poisoned state. The worker
/// exits, which closes the shared response channel; the owner
/// (`ImageCache::respawn_resize_worker`, driven by
/// `drain_resize_responses`) observes the disconnect and respawns a fresh
/// worker, dropping every cached protocol so each entry re-encodes and
/// re-registers its per-key channel lazily on its next paint.
#[must_use]
pub fn spawn_resize_worker() -> (ResizeRegisterTx, ResizeResponseRx) {
    let (register_tx, register_rx) = mpsc::channel::<(String, mpsc::Receiver<ResizeRequest>)>();
    let (response_tx, response_rx) = mpsc::channel::<(String, ResizeResponse)>();
    std::thread::spawn(move || serve_resize_requests(&register_rx, &response_tx));
    (register_tx, response_rx)
}

#[cfg(test)]
mod tests {
    use super::serve_resize_requests;
    use ratatui_image::picker::Picker;
    use ratatui_image::thread::ThreadProtocol;
    use ratatui_image::{Resize, ResizeEncodeRender};
    use std::sync::mpsc;

    /// A per-key `ResizeRequest` receiver paired with the `ThreadProtocol`
    /// that enqueues into it, as the image-cache protocol builder wires them.
    fn thread_protocol() -> (
        mpsc::Receiver<ratatui_image::thread::ResizeRequest>,
        ThreadProtocol,
    ) {
        let (tx, rx) = mpsc::channel();
        let picker = Picker::halfblocks();
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            4,
            4,
            image::Rgba([0, 0, 0, 255]),
        ));
        let protocol = ThreadProtocol::new(tx, Some(picker.new_resize_protocol(img)));
        (rx, protocol)
    }

    /// Issue #895 (M-PANIC-CONTINUATION): a caught `resize_encode` panic must
    /// end the worker, not be swallowed so the loop keeps serving. The worker
    /// body runs synchronously here; it returns exactly when the panic stops
    #[test]
    fn a_caught_panic_ends_the_worker_and_closes_the_response_channel() {
        let (register_tx, register_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();
        let (healthy_rx, mut healthy) = thread_protocol();
        let (panicking_rx, mut panicking) = thread_protocol();
        register_tx.send(("healthy".into(), healthy_rx)).unwrap();
        register_tx
            .send(("panicking".into(), panicking_rx))
            .unwrap();
        // The injection seam: a 0x0 target size makes the protocol's encode
        // early-return without recording a result, and
        // `ResizeRequest::resize_encode` then `.expect()`s that result —
        // a deterministic panic inside third-party code.
        ResizeEncodeRender::resize_encode(
            &mut healthy,
            &Resize::Scale(None),
            ratatui::layout::Size {
                width: 2,
                height: 1,
            },
        );
        ResizeEncodeRender::resize_encode(
            &mut panicking,
            &Resize::Scale(None),
            ratatui::layout::Size {
                width: 0,
                height: 0,
            },
        );

        // The body runs synchronously and returns exactly when the caught
        // panic stops it. The spawned thread owns the channel ends and
        // drops them when the body returns, so dropping the closure here
        // reproduces the thread's exit: the response channel closes.
        let worker_body = move || serve_resize_requests(&register_rx, &response_tx);
        worker_body();
        drop(worker_body);

        let (key, _) = response_rx
            .recv()
            .expect("the healthy request must be served before the panic");
        assert_eq!(key, "healthy");
        assert!(
            response_rx.recv().is_err(),
            "the caught panic must end the worker and close the response channel"
        );
    }
}
