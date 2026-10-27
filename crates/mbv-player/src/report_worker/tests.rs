//! Worker-level tests for the FIFO barrier behind `SessionReporter::settle`
//! (design D4, #963). The gate is held by the test, so no test sleeps.

use super::*;

/// The bound a held job uses. Any bound proves the ack comes from the job
/// finishing rather than from a budget running out.
const HELD_JOB_BUDGET: Duration = Duration::from_secs(30);

/// A report job the worker cannot finish until the test drops `gate`: it joins
/// a thread blocked on the gate, and it reports nothing to Emby. The worker is
/// FIFO, so a barrier queued behind this job cannot be acked while the gate
/// stays open.
struct HeldJob {
    job: ReportJob,
    gate: mpsc::Sender<()>,
    /// Fires from inside the job, so the test knows the worker reached it
    /// before it checks that the barrier is still unacked.
    entered: mpsc::Receiver<()>,
}

fn held_job() -> HeldJob {
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let handle = thread::spawn(move || {
        let _ = entered_tx.send(());
        let _ = gate_rx.recv();
    });
    HeldJob {
        job: ReportJob::ProgressJoinThenStopped {
            handle: Some(handle),
            budget: HELD_JOB_BUDGET,
            stopped: None,
        },
        gate: gate_tx,
        entered: entered_rx,
    }
}

fn settle_reporter() -> SessionReporter {
    SessionReporter::new(
        Arc::new(EmbyClient::new(mbv_config::Config::default())),
        None,
        ItemId::empty(),
        MediaSourceId::new(""),
        EmbySessionId::new(""),
        false,
        Arc::new(Mutex::new(PlayerStatus::default())),
    )
}

/// Contract: the FIFO worker acks the barrier only after earlier jobs ran
/// (#963).
#[test]
fn barrier_acks_only_after_the_jobs_ahead_of_it_ran() {
    let (job_tx, job_rx) = mpsc::channel();
    thread::spawn(move || run_report_worker(job_rx));

    let held = held_job();
    job_tx.send(held.job).unwrap();
    let (ack_tx, ack_rx) = mpsc::channel::<()>();
    job_tx.send(ReportJob::Barrier(ack_tx)).unwrap();

    held.entered
        .recv()
        .expect("the worker never reached the held job");
    assert!(
        ack_rx.recv_timeout(Duration::ZERO).is_err(),
        "the barrier acked while the job ahead of it still ran"
    );

    drop(held.gate);
    // The receive is the seam observing the worker, not a delay: the ack is
    // already due once the held job's thread ends.
    ack_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the barrier acks once the held job finished");
}

/// Contract: a zero bound returns without the ack and records the timeout
/// (#963), so a run's exit never waits on a busy worker.
#[test]
fn zero_bound_settle_returns_without_the_ack() {
    let reporter = settle_reporter();
    let held = held_job();
    reporter.job_tx.send(held.job).unwrap();

    let lines = mbv_core::applog::test_support::capture_log_lines(|| {
        reporter.settle(Duration::ZERO);
    });

    assert!(
        lines
            .iter()
            .any(|line| line.contains("event=player.resume.settle_timed_out")),
        "settle must log the timeout and return: {lines:?}"
    );
    drop(held.gate);
}
