// cast-owner-output task 4.2: the Owner's Cast source resolve runs on a
// resolve thread and re-enters through `DaemonEvent::Cast`; a result whose
// slot and generation no longer match the pending record is dropped.

use super::cast_output::{
    cast_selection, connect_output_client, fake_connector, pump_cast_event, select_output,
};
use super::r#loop::{TestLoop, test_loop_with_queue};
use super::*;

/// A feed queue item with an enclosure URL: `prepare_cast_source` resolves it
/// with an empty resolve context (no service client is touched), so the
/// resolve-thread tests stay hermetic.
fn feed_qi_with_url(guid: &str, url: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: Some(url.into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
        feed_id: Some("feed".into()),
        position_ticks: 0,
        played: false,
    })
}

/// Arms the pending resolve record the way the row 5.1 funnel will: keyed by
/// the slot the resolve was started for and the transition generation it
/// belongs to, carrying the start position.
fn arm_pending_resolve(t: &mut TestLoop, slot: mbv_queue::QueueSlotId, generation: u64) {
    t.event_loop.owner.pending_cast_resolve = Some(crate::cast::PendingCastResolve {
        slot,
        generation,
        start_ticks: 0,
    });
}

/// A hand-stamped resolve outcome for `slot`/`generation`, the same shape the
/// resolve thread's `send` closure forwards as `DaemonEvent::Cast`.
fn resolved_event(slot: mbv_queue::QueueSlotId, generation: u64, url: &str) -> DaemonEvent {
    DaemonEvent::Cast(crate::cast::CastOwnerEvent::Resolved {
        slot,
        generation,
        result: Ok(crate::cast_resolve::CastSource {
            media: mbv_cast::client::CastMediaItem {
                url: url.to_string(),
                content_type: "audio/mpeg".to_string(),
            },
            progress: crate::CastProgressTarget::Feed {
                feed_id: Some("feed".to_string()),
                guid: slot.raw().to_string(),
            },
        }),
    })
}

/// Waits until every job already queued on the cast worker has run: a sentinel
/// job queued last fires only after the jobs ahead of it (the load) did.
fn wait_for_worker_jobs(cast: &crate::cast::PlaybackOutput) {
    let crate::cast::PlaybackOutput::Cast(cast) = cast else {
        panic!("the receiver is the output");
    };
    let (done_tx, done_rx) = mpsc::channel();
    let _ = cast.worker.send(Box::new(move |_| {
        let _ = done_tx.send(());
    }));
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the queued jobs ran before the sentinel");
}

/// Contract: cast-owner-output row 4.2 — a resolve outcome matching the
/// pending record loads the resolved media on the Cast worker at the record's
/// start position and records the loaded item against the slot.
#[test]
fn a_fresh_cast_resolve_result_loads_the_resolved_media() {
    let item = feed_qi_with_url("a", "https://feed/a.mp3");
    let mut t = test_loop_with_queue(crate::DaemonRole::Local, vec![item.clone()], 0);
    let (connector, calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let slot = t.event_loop.owner.core.queue.slots()[0].slot_id;
    arm_pending_resolve(&mut t, slot, 7);
    let merged_tx = t.event_loop.merged_tx.clone();
    crate::cast_resolve::spawn_cast_resolve(
        item,
        crate::CastResolveContext::default(),
        move |result| {
            let _ = merged_tx.send(DaemonEvent::Cast(crate::cast::CastOwnerEvent::Resolved {
                slot,
                generation: 7,
                result,
            }));
        },
    );

    let event = t.merged_rx.recv().expect("the resolve reports its outcome");
    t.event_loop.handle_event(event);
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "keep_alive".to_string(),
            "load(https://feed/a.mp3, 0)".to_string()
        ],
        "the resolved media loads at the pending record's start position"
    );
    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    let loaded = cast.loaded.as_ref().expect("the load records the item");
    assert_eq!(loaded.slot, slot);
    assert_eq!(loaded.content_id, "https://feed/a.mp3");
    assert!(
        t.event_loop.owner.pending_cast_resolve.is_none(),
        "the matched outcome consumes the pending record"
    );
}

/// Contract: cast-owner-output row 4.2 — a resolve outcome for a slot that is
/// no longer the pending target is dropped: the worker is given no load, the
/// output records no loaded item, and the newer target's resolve stays armed.
#[test]
fn a_stale_cast_resolve_result_loads_nothing() {
    let mut t = test_loop_with_queue(
        crate::DaemonRole::Local,
        vec![
            feed_qi_with_url("a", "https://feed/a.mp3"),
            feed_qi_with_url("b", "https://feed/b.mp3"),
        ],
        0,
    );
    let (connector, calls) = fake_connector();
    t.event_loop.cast_connector = Some(connector);
    let (client_id, reply_tx, _rx) = connect_output_client(&t.event_loop.ctrl_clients, true);
    select_output(&mut t, client_id, &reply_tx, cast_selection());
    pump_cast_event(&mut t);

    let slot_b = t.event_loop.owner.core.queue.slots()[1].slot_id;
    arm_pending_resolve(&mut t, slot_b, 9);

    // A result for the superseded slot a arrives.
    let slot_a = t.event_loop.owner.core.queue.slots()[0].slot_id;
    t.event_loop
        .handle_event(resolved_event(slot_a, 9, "https://feed/a.mp3"));
    wait_for_worker_jobs(&t.event_loop.owner.output);

    assert!(
        calls.lock().unwrap().is_empty(),
        "a stale resolve result sends no load"
    );
    let crate::cast::PlaybackOutput::Cast(cast) = &t.event_loop.owner.output else {
        panic!("the receiver is the output");
    };
    assert!(cast.loaded.is_none(), "a stale result loads nothing");
    let pending = t
        .event_loop
        .owner
        .pending_cast_resolve
        .as_ref()
        .expect("the newer target's resolve stays armed");
    assert_eq!(pending.slot, slot_b);
    assert_eq!(pending.generation, 9);
}
