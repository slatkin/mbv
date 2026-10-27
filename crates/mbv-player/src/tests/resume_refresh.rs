use super::*;
use crate::resume::ResumeSeam;
use mbv_emby::EmbyError;
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

    let mut fetch = |_ids: &[String]| Ok(vec![server.clone()]);
    let refreshed = ResumeSeam::new(&mut fetch, &NO_DELAYS).refresh(items);

    assert_eq!(position(&refreshed[0]), (120 * TICKS_PER_SECOND, true));
}

#[test]
fn one_failure_then_success_uses_the_fetched_position_after_two_attempts() {
    let calls = Cell::new(0_u32);
    let server = server_item("ep1", 90 * TICKS_PER_SECOND);

    let items = vec![emby_video("ep1", TICKS_PER_SECOND, false)];
    let mut fetch = |_ids: &[String]| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            Err(EmbyError::resolve("fetch failed"))
        } else {
            Ok(vec![server.clone()])
        }
    };
    let refreshed = ResumeSeam::new(&mut fetch, &NO_DELAYS).refresh(items);

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

    let items = vec![emby_video("ep1", 30 * TICKS_PER_SECOND, true)];
    let mut fetch = |_ids: &[String]| {
        calls.set(calls.get() + 1);
        Err(EmbyError::resolve("fetch failed"))
    };
    let refreshed = ResumeSeam::new(&mut fetch, &NO_DELAYS).refresh(items);

    assert_eq!(calls.get(), 3, "attempts stop after the third failure");
    assert_eq!(position(&refreshed[0]), (0, false));
}

#[test]
fn emby_video_resume_fetches_and_the_server_position_wins() {
    let calls = Cell::new(0_u32);
    let server = server_item("ep1", 120 * TICKS_PER_SECOND);
    let item = emby_video("ep1", 10 * TICKS_PER_SECOND, true);

    let mut fetch = |_ids: &[String]| {
        calls.set(calls.get() + 1);
        Ok(vec![server.clone()])
    };
    let ticks =
        ResumeSeam::new(&mut fetch, &NO_DELAYS).jump_ticks(&item, Some(10 * TICKS_PER_SECOND));

    assert_eq!(calls.get(), 1, "an Emby video jumps through the fetch");
    assert_eq!(
        ticks,
        Some(120 * TICKS_PER_SECOND),
        "the server's position replaces the item's local one"
    );
}

#[test]
fn emby_audio_resume_never_fetches_and_starts_from_the_beginning() {
    let calls = Cell::new(0_u32);
    let mut audio = make_media_item("audio");
    audio.media_type = "Audio".into();
    audio.playback_position_ticks = audio.runtime_ticks / 2;
    let item = QueueItem::Emby(Box::new(audio));

    let mut fetch = |_ids: &[String]| {
        calls.set(calls.get() + 1);
        Ok(Vec::new())
    };
    let ticks =
        ResumeSeam::new(&mut fetch, &NO_DELAYS).jump_ticks(&item, Some(42 * TICKS_PER_SECOND));

    assert!(crate::resume::resume_start_pos(&item).abs() < f64::EPSILON);
    assert_eq!(ticks, None, "an Emby audio jump arms no resume seek");
    assert_eq!(calls.get(), 0, "an Emby audio jump never fetches");
}

#[test]
fn feed_resume_keeps_its_local_position_and_never_fetches() {
    let calls = Cell::new(0_u32);
    let mut entry = make_feed_entry("feed-1", "Podcast Episode 1");
    entry.position_ticks = 30 * TICKS_PER_SECOND; // 10% of a 300s episode
    let item = QueueItem::Feed(entry);

    let mut fetch = |_ids: &[String]| {
        calls.set(calls.get() + 1);
        Ok(Vec::new())
    };
    let local = crate::resume_ticks_for_item(&item);
    let ticks = ResumeSeam::new(&mut fetch, &NO_DELAYS).jump_ticks(&item, local);

    assert_eq!(local, Some(30 * TICKS_PER_SECOND));
    assert_eq!(ticks, local, "a Feed jump keeps the owner's local ticks");
    assert_eq!(calls.get(), 0, "a Feed jump never fetches");
}

#[test]
fn audiobookshelf_resume_never_fetches_and_yields_no_ticks() {
    let calls = Cell::new(0_u32);
    let item = super::abs_item();

    let mut fetch = |_ids: &[String]| {
        calls.set(calls.get() + 1);
        Ok(Vec::new())
    };
    let ticks =
        ResumeSeam::new(&mut fetch, &NO_DELAYS).jump_ticks(&item, Some(42 * TICKS_PER_SECOND));

    assert_eq!(
        ticks, None,
        "the session position is already baked into the prepared source"
    );
    assert_eq!(calls.get(), 0, "an Audiobookshelf jump never fetches");
}

#[test]
fn an_omitted_id_is_retried_alone() {
    let batches: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
    let server_a = server_item("ep-a", 10 * TICKS_PER_SECOND);
    let server_b = server_item("ep-b", 20 * TICKS_PER_SECOND);

    let mut fetch = |ids: &[String]| {
        batches.borrow_mut().push(ids.to_vec());
        if batches.borrow().len() == 1 {
            Ok(vec![server_a.clone()])
        } else {
            Ok(vec![server_b.clone()])
        }
    };
    let refreshed = ResumeSeam::new(&mut fetch, &NO_DELAYS).refresh(vec![
        emby_video("ep-a", 0, false),
        emby_video("ep-b", 0, false),
    ]);

    assert_eq!(batches.borrow()[0], vec!["ep-a", "ep-b"]);
    assert_eq!(
        batches.borrow()[1],
        vec!["ep-b"],
        "only the id the response omitted may be retried"
    );
    assert_eq!(position(&refreshed[0]).0, 10 * TICKS_PER_SECOND);
    assert_eq!(position(&refreshed[1]).0, 20 * TICKS_PER_SECOND);
}
