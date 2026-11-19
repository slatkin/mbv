//! The `toast-notification-semantics` contracts of the live toast list, all
//! against explicit `now` values: no clock, no sleep.

use super::*;
use std::time::Duration;

fn toast_list() -> (ToastList, Instant) {
    (ToastList::default(), Instant::now())
}

fn live_texts(list: &ToastList, now: Instant) -> Vec<String> {
    list.live(now).map(|entry| entry.text.clone()).collect()
}

fn fill_with_neutrals(list: &mut ToastList, now: Instant, count: usize) {
    for index in 0..count {
        list.push(format!("toast {index}"), ToastSeverity::Neutral, now);
    }
}

#[test]
fn a_second_toast_does_not_end_the_first() {
    let (mut list, t0) = toast_list();
    list.push("Download failed".into(), ToastSeverity::Warning, t0);
    list.push(
        "Copied".into(),
        ToastSeverity::Success,
        t0 + Duration::from_secs(1),
    );

    let now = t0 + Duration::from_secs(1);
    assert_eq!(live_texts(&list, now), ["Download failed", "Copied"]);
}

#[test]
fn each_toast_expires_on_its_own_severity_duration() {
    let (mut list, t0) = toast_list();
    list.push("Download failed".into(), ToastSeverity::Warning, t0);
    list.push(
        "Copied".into(),
        ToastSeverity::Success,
        t0 + Duration::from_secs(1),
    );

    // The later Success toast ends 2 s after it arrived; the earlier Warning
    // toast is still live.
    assert_eq!(
        live_texts(&list, t0 + Duration::from_secs(3)),
        ["Download failed"]
    );
    // The Warning toast ends 5 s after it arrived.
    assert_eq!(
        live_texts(&list, t0 + Duration::from_secs(5)),
        Vec::<String>::new()
    );
}

#[test]
fn a_sixth_toast_ends_the_oldest() {
    let (mut list, t0) = toast_list();
    fill_with_neutrals(&mut list, t0, MAX_LIVE_TOASTS);

    list.push("toast 5".into(), ToastSeverity::Neutral, t0);

    assert_eq!(
        live_texts(&list, t0),
        ["toast 1", "toast 2", "toast 3", "toast 4", "toast 5"]
    );
}

#[test]
fn a_repeated_toast_replaces_its_live_copy_and_becomes_newest() {
    let (mut list, t0) = toast_list();
    list.push("Emby is unavailable".into(), ToastSeverity::Warning, t0);
    let first_id = list
        .live(t0)
        .next()
        .map(|entry| entry.id)
        .expect("the first toast is live at its own arrival");

    list.push(
        "Copied".into(),
        ToastSeverity::Success,
        t0 + Duration::from_secs(1),
    );
    list.push(
        "Emby is unavailable".into(),
        ToastSeverity::Warning,
        t0 + Duration::from_secs(2),
    );

    let now = t0 + Duration::from_secs(2);
    assert_eq!(live_texts(&list, now), ["Copied", "Emby is unavailable"]);
    let repeated = list.live(now).last().expect("the repeated toast is live");
    assert_ne!(repeated.id, first_id);
    // Its duration starts again: 5 s after this arrival, not the first one.
    assert_eq!(repeated.expires, t0 + Duration::from_secs(7));
}
