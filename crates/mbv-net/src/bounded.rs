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
/// `spawn/recv_timeout/abandon` mechanics only need to be gotten right once,
/// and so each caller's own timeout logic can be unit-tested directly with a
/// closure -- no real socket or filesystem state required.
pub fn run_with_hard_bound<T, F, E>(f: F, hard_bound: Duration) -> Result<T, E>
where
    T: Send + 'static,
    E: From<String> + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    let timeout_message = format!("timed out after {}s", hard_bound.as_secs());
    run_with_hard_bound_or_error(f, move || E::from(timeout_message), hard_bound)
}

/// Runs a bounded operation with an owning-crate timeout error constructor.
pub fn run_with_hard_bound_or_error<T, F, E, X>(
    f: F,
    timeout_error: X,
    hard_bound: Duration,
) -> Result<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
    X: FnOnce() -> E + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(hard_bound) {
        Ok(result) => result,
        Err(_) => Err(timeout_error()),
    }
}

/// Cleanup-capable bounded operation with an owning-crate timeout constructor.
pub fn run_with_hard_bound_or_cleanup_with_error<T, F, E, C, X>(
    f: F,
    cleanup: C,
    timeout_error: X,
    hard_bound: Duration,
) -> Result<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
    C: FnOnce(T) + Send + 'static,
    X: FnOnce() -> E + Send + 'static,
{
    struct Pending<T, C: FnOnce(T)> {
        value: Option<T>,
        cleanup: Option<C>,
    }

    impl<T, C: FnOnce(T)> Pending<T, C> {
        fn accept(mut self) -> T {
            self.cleanup = None;
            self.value.take().unwrap()
        }
    }

    impl<T, C: FnOnce(T)> Drop for Pending<T, C> {
        fn drop(&mut self) {
            if let Some(value) = self.value.take()
                && let Some(cleanup) = self.cleanup.take()
            {
                cleanup(value);
            }
        }
    }

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = f().map(|value| Pending {
            value: Some(value),
            cleanup: Some(cleanup),
        });
        let _ = tx.send(result);
    });
    match rx.recv_timeout(hard_bound) {
        Ok(Ok(pending)) => Ok(pending.accept()),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(timeout_error()),
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
        // timeout/abandon mechanics in isolation. No timing is asserted
        // (only the Err value), so the bound is small: any bound far below
        // the 5s hang proves the same property.
        let result = run_with_hard_bound(
            || {
                std::thread::sleep(Duration::from_secs(5));
                Ok::<_, String>(())
            },
            Duration::from_millis(10),
        );
        assert_eq!(result, Err("timed out after 0s".to_string()));
    }
}
