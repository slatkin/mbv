use std::time::Duration;

/// Runs `f` on a worker thread, bounded by `hard_bound` wall-clock time
/// regardless of what `f` itself is doing internally (blocking network I/O,
/// TLS handshake stalls, proxy weirdness, etc. that a callee's own timeout
/// knobs don't reliably cover -- see issue #191).
///
/// On success, returns `f`'s result. On timeout, `Err` with a synthesized
/// `"timed out after {N}s"` message is returned; `std::thread` has no kill
/// primitive, so the spawned thread is simply abandoned -- it may keep
/// running in the background until it finishes or the process exits, then
/// its result is silently dropped since the receiving end is gone.
///
/// Shared by `api::EmbyClient::authenticate_bounded` and
/// `remote_player::RemotePlayer::connect_endpoint`'s handshake bound, so the
/// spawn/recv_timeout/abandon mechanics only need to be gotten right once,
/// and so each caller's own timeout logic can be unit-tested directly with a
/// closure -- no real socket or filesystem state required.
pub(crate) fn run_with_hard_bound<T, F>(f: F, hard_bound: Duration) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(hard_bound) {
        Ok(result) => result,
        Err(_) => Err(format!("timed out after {}s", hard_bound.as_secs())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_ok_when_closure_finishes_within_bound() {
        let result = run_with_hard_bound(|| Ok::<_, String>(42), Duration::from_secs(5));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn returns_err_from_closure_when_it_finishes_within_bound() {
        let result =
            run_with_hard_bound(|| Err::<i32, _>("boom".to_string()), Duration::from_secs(5));
        assert_eq!(result, Err("boom".to_string()));
    }

    #[test]
    fn times_out_when_closure_outlives_the_bound() {
        // No real socket or filesystem access -- this exercises the generic
        // timeout/abandon mechanics in isolation, in well under a second.
        let result = run_with_hard_bound(
            || {
                std::thread::sleep(Duration::from_secs(5));
                Ok::<_, String>(())
            },
            Duration::from_millis(50),
        );
        assert_eq!(result, Err("timed out after 0s".to_string()));
    }
}
