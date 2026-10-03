use super::*;
use std::cell::{Cell, RefCell};

/// Two zero-length delays: the same three attempts as the production
/// `[500ms, 2s]` schedule, with the waiting injected away.
const NO_DELAYS: [Duration; 2] = [Duration::ZERO, Duration::ZERO];

fn emby_video(id: &str, position_ticks: i64, played: bool) -> QueueItem {
    let mut item = make_media_item(id);
    item.playback_position_ticks = position_ticks;
    item.played = played;
    QueueItem::Emby(Box::new(item))
}

fn server_item(id: &str, position_ticks: i64) -> EmbyItem {
    let mut item = make_media_item(id);
    item.playback_position_ticks = position_ticks;
    item.played = true;
    item
}

fn position(item: &QueueItem) -> (i64, bool) {
    let QueueItem::Emby(emby) = item else {
        panic!("expected an Emby item");
    };
    (emby.playback_position_ticks, emby.played)
}

#[test]
fn fetched_position_replaces_the_items_position() {
    let items = vec![emby_video("ep1", TICKS_PER_SECOND, false)];
    let server = server_item("ep1", 120 * TICKS_PER_SECOND);

    let refreshed = refresh_emby_resume(items, |_| Ok(vec![server.clone()]), &NO_DELAYS);

    assert_eq!(position(&refreshed[0]), (120 * TICKS_PER_SECOND, true));
}

#[test]
fn one_failure_then_success_uses_the_fetched_position_after_two_attempts() {
    let calls = Cell::new(0_u32);
    let server = server_item("ep1", 90 * TICKS_PER_SECOND);

    let refreshed = refresh_emby_resume(
        vec![emby_video("ep1", TICKS_PER_SECOND, false)],
        |_| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Err(EmbyError::resolve("fetch failed"))
            } else {
                Ok(vec![server.clone()])
            }
        },
        &NO_DELAYS,
    );

    assert_eq!(
        calls.get(),
        2,
        "a failed first attempt must be retried once"
    );
    assert_eq!(position(&refreshed[0]).0, 90 * TICKS_PER_SECOND);
}

#[test]
fn three_failures_start_from_the_beginning_after_three_attempts() {
    let calls = Cell::new(0_u32);

    let refreshed = refresh_emby_resume(
        vec![emby_video("ep1", 30 * TICKS_PER_SECOND, true)],
        |_| {
            calls.set(calls.get() + 1);
            Err(EmbyError::resolve("fetch failed"))
        },
        &NO_DELAYS,
    );

    assert_eq!(calls.get(), 3, "attempts stop after the third failure");
    assert_eq!(position(&refreshed[0]), (0, false));
}

#[test]
fn an_omitted_id_is_retried_alone() {
    let batches: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
    let server_a = server_item("ep-a", 10 * TICKS_PER_SECOND);
    let server_b = server_item("ep-b", 20 * TICKS_PER_SECOND);

    let refreshed = refresh_emby_resume(
        vec![emby_video("ep-a", 0, false), emby_video("ep-b", 0, false)],
        |ids| {
            batches.borrow_mut().push(ids.to_vec());
            if batches.borrow().len() == 1 {
                Ok(vec![server_a.clone()])
            } else {
                Ok(vec![server_b.clone()])
            }
        },
        &NO_DELAYS,
    );

    assert_eq!(batches.borrow()[0], vec!["ep-a", "ep-b"]);
    assert_eq!(
        batches.borrow()[1],
        vec!["ep-b"],
        "only the id the response omitted may be retried"
    );
    assert_eq!(position(&refreshed[0]).0, 10 * TICKS_PER_SECOND);
    assert_eq!(position(&refreshed[1]).0, 20 * TICKS_PER_SECOND);
}
