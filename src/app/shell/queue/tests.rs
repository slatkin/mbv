use super::*;
use crate::app::tests::{QueueViewTestExt, live_owner_channel, make_app_stub, make_items};
use mbv_emby_model::test_support::make_item;
use mbv_ui_msg::Msg;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

#[test]
fn queue_rows_follow_readopted_owner_snapshot_with_same_wire_revision_issue_836() {
    let mut first = crate::app::tests::emby_unified_state(&make_items(2), 0);
    first.active_slot = None;
    first.revision = 7;
    let mut app = make_app_stub();
    app.local_view = crate::app::QueueView::from_snapshot(&first);
    let mut model = Model::new(app);

    model.sync_queue();
    let component = queue_component(&model);
    assert_eq!(component.projected_row_states().len(), 2);

    let mut replacement = crate::app::tests::emby_unified_state(&make_items(1), 0);
    replacement.active_slot = None;
    replacement.revision = 7;
    model.app.local_view.adopt(
        &replacement,
        crate::app::state::queue_view::AdoptCause::Replacement,
    );
    model.sync_queue();

    let component = queue_component(&model);
    assert_eq!(component.projected_row_states().len(), 1);
}

#[test]
fn queue_projection_uses_adopted_pending_transition_without_progress() {
    // Owns queue-canonical-list, "Queue projection is bounded presentation data".
    let mut snapshot = crate::app::tests::emby_unified_state(&make_items(2), 0);
    snapshot.in_flight_transition = Some(mbv_ctrl::TransitionSummary {
        request_id: 1,
        generation: 1,
        target_slot: 101,
    });
    let mut app = make_app_stub();
    app.local_view = crate::app::QueueView::from_snapshot(&snapshot);
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 0;
        status.position_ticks = 500;
        status.runtime_ticks = 1_000;
    });

    let displayed = app.displayed_playback_state();
    let rows = mbv_components::queue::queue_media_rows(
        app.local_view.slots(),
        app.displayed_queue_playback_state(),
        app.pending_playback_slot(),
    );

    assert_eq!(displayed.active_idx, Some(1));
    assert_eq!(displayed.position_ticks, 0);
    assert!(matches!(
        &rows[1],
        mbv_render::components::media_list::MediaListRow::Item {
            semantic_state: mbv_render::components::media_list::MediaSemanticState::NowPlaying {
                progress: None
            },
            ..
        }
    ));
}

#[test]
fn deleting_before_playing_row_keeps_highlight_on_playing_slot() {
    // Regression (2026-10-29 queue index errors): deleting a row before
    // the playing one shifts ordinals while the status' `current_idx`
    // still names the old row. The highlight must follow the active
    // slot's identity, so the stale ordinal cannot paint a different
    // item as playing.
    let items = make_items(2);
    let mut app = make_app_stub();
    app.local_view =
        crate::app::QueueView::from_snapshot(&crate::app::tests::emby_unified_state(&items, 1));
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 1;
    });

    // The owner applied the delete of the first row: the playing slot
    // (101) is now first. The status has not refreshed yet —
    // `current_idx` still names the stale ordinal 1.
    let mut after_delete = crate::app::tests::emby_unified_state(&items[1..], 0);
    after_delete.slots[0].slot_id = 101;
    after_delete.active_slot = Some(101);
    app.local_view.adopt(
        &after_delete,
        crate::app::state::queue_view::AdoptCause::OwnAnswer,
    );

    let displayed = app.displayed_queue_playback_state();
    assert_eq!(displayed.active_idx, Some(0));
    let rows = mbv_components::queue::queue_media_rows(
        app.local_view.slots(),
        displayed,
        app.pending_playback_slot(),
    );
    assert!(matches!(
        &rows[0],
        mbv_render::components::media_list::MediaListRow::Item {
            semantic_state: mbv_render::components::media_list::MediaSemanticState::NowPlaying { .. },
            ..
        }
    ));
}

#[test]
fn deleting_playing_row_after_ordinal_shift_still_asks_now_playing_confirm() {
    // Regression (2026-10-29 queue index errors): after the delete before
    // the playing row shifts ordinals, the status' `current_idx` still
    // names the stale ordinal — an ordinal comparison no longer
    // recognizes the playing row, so it used to skip the confirmation
    // and stop playback as a plain edit. The guard resolves the owner's
    // observed active slot by identity.
    let items = make_items(2);
    let mut app = make_app_stub();
    app.local_view =
        crate::app::QueueView::from_snapshot(&crate::app::tests::emby_unified_state(&items, 1));
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = 1;
    });

    // Same post-delete adoption as the highlight regression: the playing
    // slot (101) is now first while `current_idx` still names ordinal 1.
    let mut after_delete = crate::app::tests::emby_unified_state(&items[1..], 0);
    after_delete.slots[0].slot_id = 101;
    after_delete.active_slot = Some(101);
    app.local_view.adopt(
        &after_delete,
        crate::app::state::queue_view::AdoptCause::OwnAnswer,
    );

    app.remove_from_queue(0);

    assert!(matches!(
        app.pending_overlay,
        Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
    ));
}

#[test]
fn unchanged_queue_revision_skips_projection_rebuild() {
    let mut app = make_app_stub();
    app.local_view.adopt_items(make_items(2), 0);
    let mut model = Model::new(app);
    model.sync_queue();

    let update = model.prepare_queue_projection();

    assert!(update.slots.is_none());
}

#[test]
fn lineage_change_rebuilds_same_revision_queue_rows_review_p2_1() {
    // A replacement QueueView restarts its private revision mint; lineage
    // still distinguishes its rows from the prior owner's queue.
    let mut state = crate::app::tests::emby_unified_state(&make_items(2), 0);
    state.active_slot = None;
    state.revision = 7;
    let mut app = make_app_stub();
    app.local_view = crate::app::QueueView::from_snapshot(&state);
    let mut model = Model::new(app);
    model.sync_queue();
    let previous_revision = model.app.local_view.revision();

    let mut replacement_items = make_items(2);
    replacement_items[0].id = "replacement".into();
    let mut replacement = crate::app::tests::emby_unified_state(&replacement_items, 0);
    replacement.active_slot = None;
    replacement.revision = 7;
    replacement.lineage = mbv_queue::QueueLineage(1);
    model.app.local_view = crate::app::QueueView::from_snapshot(&replacement);
    assert_eq!(model.app.local_view.revision(), previous_revision);
    let update = model.prepare_queue_projection();

    assert!(update.slots.is_some());
}

#[test]
fn queue_projection_fetches_now_playing_image_once_and_none_on_repaint() {
    // Task 3.4 (D9): the queue projection — not the painter — issues the
    // visual slot's fetch. One fetch per new now-playing key, and a
    // repaint tick (same fingerprint) starts none: the reservation set
    // and the fetch counters stay untouched. The fixture has no Emby
    // client, so `spawn_image_fetch` balances `image_fetches_active`
    // synchronously — the counters cannot see a duplicate spawn; the
    // reservation set and counters together pin the request count.
    let mut app = make_app_stub();
    let items = crate::app::tests::make_items(2);
    app.local_view.adopt_queue_items(
        items
            .into_iter()
            .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
            .collect::<Vec<_>>(),
        0,
    );
    app.panel_focus = PanelFocus::Queue;
    app.images.configure_protocol(None, true);
    {
        app.player.update_status(|status| {
            status.active = true;
            status.current_idx = 0;
        });
    };
    let mut model = Model::new(app);

    model.sync_queue();
    // `make_items` builds Movies, whose card now reserves the landscape
    // key (`{id}:QB`), not the portrait `{id}:P` (commit 55c37ea71).
    assert!(
        model.app.images.is_loading("id0:QB"),
        "the now-playing key must be reserved by the projection push"
    );
    let fetch_work = model.app.images.fetch_work_snapshot();

    // Repaint tick: nothing changed, so the push starts no new fetch.
    model.sync_queue();
    assert_eq!(model.app.images.fetch_work_snapshot(), fetch_work);

    // A new now-playing key reserves exactly one new key.
    model
        .app
        .player
        .update_status(|status| status.current_idx = 1);
    model.sync_queue();
    assert!(model.app.images.is_loading("id1:QB"));
}

#[test]
fn hidden_visual_slot_skips_artwork_fetch_until_shown() {
    let mut app = make_app_stub();
    app.local_view.adopt_items(make_items(2), 0);
    app.images.configure_protocol(None, true);
    app.visual_slot_hidden = true;
    {
        app.player.update_status(|status| {
            status.active = true;
            status.current_idx = 0;
        });
    };
    let mut model = Model::new(app);

    model.sync_queue();
    assert_eq!(model.app.images.card_image_fetch_calls(), 0);
    // `make_items` builds Movies → the landscape card key (commit 55c37ea71).
    assert!(!model.app.images.is_loading("id0:QB"));

    model.app.visual_slot_hidden = false;
    model.sync_queue();
    assert!(model.app.images.card_image_fetch_calls() > 0);
    assert!(model.app.images.is_loading("id0:QB"));
}

#[test]
fn queue_arrow_moves_component_cursor_only_not_app_follow() {
    // QueueRequest::Cursor is plain component navigation: arrowing in
    // the mounted component moves only the component's own cursor; App's
    // `queue_cursor` (the shell-owned follow position) is not written
    // (task 3.2 — the mirror in select_queue_slot is gone).
    let mut app = make_app_stub();
    app.local_view.adopt_queue_items(
        vec![
            mbv_queue::QueueItem::Emby(Box::new(make_item("one", "Movie"))),
            mbv_queue::QueueItem::Emby(Box::new(make_item("two", "Movie"))),
        ],
        0,
    );
    app.panel_focus = PanelFocus::Queue;
    let mut model = Model::new(app);
    model.sync_queue();
    let id = ComponentId::Queue;
    let component_cursor = |model: &Model| {
        model
            .application
            .get_component(&id)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<QueueComponent>()
                    .map(QueueComponent::test_cursor)
            })
            .expect("Queue component mounted")
    };
    assert_eq!(component_cursor(&model), 0);
    assert_eq!(model.app.local_view.cursor(), 0);

    let message = model
        .application
        .get_component_mut(&id)
        .expect("Queue component mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Queue(request @ QueueRequest::Cursor { .. })) = message else {
        panic!("queue navigation must emit a slot cursor request");
    };
    // The request carries the moved-to slot; the component cursor moved.
    model.handle_queue_request(request);
    assert_eq!(
        component_cursor(&model),
        1,
        "component cursor moved to row 1"
    );
    assert_eq!(
        model.app.local_view.cursor(),
        0,
        "QueueRequest::Cursor must not write App's follow cursor"
    );
}

fn emby_items(n: usize) -> Vec<mbv_queue::QueueItem> {
    (0..n)
        .map(|i| mbv_queue::QueueItem::Emby(Box::new(make_item(&format!("row-{i}"), "Movie"))))
        .collect()
}

fn queue_component(model: &Model) -> &QueueComponent {
    model
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
        .expect("Queue component mounted")
}

#[test]
fn remote_undo_falls_back_to_local_when_no_direct_remote_queue() {
    // Finding 5: after a remote disconnect the still-mounted component can
    // emit Undo { link: Peer } for a frame. With no direct remote queue
    // the visible queue is Local, so the Local undo entry is consumed and
    // its inverse edit is sent to the Local owner instead of flashing a
    // spurious remote-undo-unsupported error.
    use crate::app::state::playback::UndoEntry;
    let mut app = make_app_stub();
    let _cmd_rx = live_owner_channel(&mut app);
    app.local_view.adopt_queue_items(emby_items(2), 0);
    app.queue_undo_stack.push(UndoEntry::Remove {
        index: 0,
        item: Box::new(mbv_queue::QueueItem::Emby(Box::new(make_item(
            "restored", "Movie",
        )))),
    });
    let mut model = Model::new(app);
    assert!(!model.app.has_direct_remote_queue());

    model.handle_queue_request(QueueRequest::Undo {
        link: QueueLink::Peer,
    });

    assert_ne!(
        model.app.status_severity,
        ToastSeverity::Error,
        "no spurious remote-undo-unsupported error"
    );
    assert!(
        model.app.queue_undo_stack.is_empty(),
        "the Local undo entry was consumed"
    );
}
