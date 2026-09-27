use std::sync::mpsc;

pub(crate) fn setup_shutdown_signal() -> (mpsc::SyncSender<()>, mpsc::Receiver<()>) {
    // Shared shutdown channel — written by SIGTERM thread and tray Quit item.
    let (shutdown_signal_tx, shutdown_signal_rx) = mpsc::sync_channel::<()>(1);

    // Block SIGTERM in all threads so sigwait() owns it exclusively.
    // SAFETY: `mask` is initialized before libc writes it; pthread_sigmask accepts a valid mask.
    unsafe {
        let mut mask = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&raw mut mask);
        libc::sigaddset(&raw mut mask, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_BLOCK, &raw const mask, std::ptr::null_mut())
    };

    // Thread that blocks on SIGTERM and forwards it as a graceful shutdown.
    {
        let tx = shutdown_signal_tx.clone();
        std::thread::spawn(move || {
            let mut sig: libc::c_int = 0;
            // SAFETY: sigset_t is a plain C signal-set value initialized by sigemptyset below.
            let mut mask = unsafe { std::mem::zeroed::<libc::sigset_t>() };
            // SAFETY: `mask` and `sig` are valid writable pointers for these libc calls.
            unsafe {
                libc::sigemptyset(&raw mut mask);
                libc::sigaddset(&raw mut mask, libc::SIGTERM);
                libc::sigwait(&raw const mask, &raw mut sig)
            };
            log::info!(target: "daemon", "received signal {sig} (SIGTERM), initiating graceful shutdown");
            let _ = tx.try_send(());
        })
    };

    (shutdown_signal_tx, shutdown_signal_rx)
}
