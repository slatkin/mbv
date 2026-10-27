//! Shared nested-shape corpus (`shared/`): wire types that only travel
//! nested inside a root payload — one `QueueItem` per kind, `PlayerStatus`,
//! and `UnifiedQueueStateData` with its transition fields — plus the
//! tolerated legacy decode shapes: the bare `EmbyItem` queue item and the
//! bare-scalar `run_identity` (design D3, D4).
//!
//! The `QueueItem` kinds are exhausted over the production `QueueItemKind`
//! enum, so a new kind fails this build until its arm and fixture exist.
//! `PlayerStatus` and `UnifiedQueueStateData` are structs with no variant
//! enum to match, so their rows stay hand-kept like `HelloShape::ALL`.

use mbv_ctrl::player::PlayerEvent;
use mbv_queue::{QueueItem, QueueItemKind, QueueSlotId};

use super::builders::{
    GENERATION, POSITION_TICKS, abs_book_queue_item, abs_episode_queue_item, emby_queue_item,
    feed_queue_item, player_status, unified_queue_state_with_transitions,
};
use super::*;

/// Every `QueueItem` kind, one fixture row each. Exhaustive over the
/// production enum: a new kind fails this build until its arm exists here.
const QUEUE_KINDS: [QueueItemKind; 4] = [
    QueueItemKind::Emby,
    QueueItemKind::Feed,
    QueueItemKind::Audiobookshelf,
    QueueItemKind::AudiobookshelfBook,
];

/// Exhaustive kind-name match: the fixture stem suffix is the Rust variant
/// name (design D4).
fn kind_name(kind: &QueueItemKind) -> &'static str {
    match kind {
        QueueItemKind::Emby => "Emby",
        QueueItemKind::Feed => "Feed",
        QueueItemKind::Audiobookshelf => "Audiobookshelf",
        QueueItemKind::AudiobookshelfBook => "AudiobookshelfBook",
    }
}

/// The canonical `QueueItem` for one kind. Exhaustive, no wildcard arm.
fn queue_item(kind: &QueueItemKind) -> QueueItem {
    match kind {
        QueueItemKind::Emby => emby_queue_item(),
        QueueItemKind::Feed => feed_queue_item(),
        QueueItemKind::Audiobookshelf => abs_episode_queue_item(),
        QueueItemKind::AudiobookshelfBook => abs_book_queue_item(),
    }
}

/// The canonical `PlayerEvent::Stopped` the bare-run-identity fixture pins:
/// identical to the `events/Player.Stopped.json` row, except that the
/// fixture carries the tolerated bare-scalar `run_identity` instead of the
/// emitted `[request_id, generation]` pair (design D3).
fn stopped_event() -> PlayerEvent {
    PlayerEvent::Stopped {
        slot_id: Some(QueueSlotId::from_raw(1)),
        run_identity: GENERATION,
        position_ticks: POSITION_TICKS,
        played: false,
        consume: false,
        error: None,
    }
}

/// The `shared/` rows of the fixture table. Queue-item rows assert that the
/// production `kind()` mapping agrees with the fixture's kind, so the
/// serializer's emitted tag and the kind dispatch cannot drift apart.
///
/// Decode-only fixtures are tolerated legacy shapes and are never rewritten
/// by bless: their files are hand-maintained records of the legacy wire
/// bytes, and reconciliation fails if one goes missing.
pub(crate) fn rows(rows: &mut Vec<FixtureRow>) {
    for kind in &QUEUE_KINDS {
        let value = queue_item(kind);
        assert_eq!(
            value.kind(),
            *kind,
            "QueueItem.{}: the kind mapping disagrees with the fixture row",
            kind_name(kind)
        );
        rows.push(FixtureRow {
            path: format!("shared/QueueItem.{}.json", kind_name(kind)),
            direction: Direction::RoundTrip,
            schema: Some(SchemaAnchor::DefsEntry {
                root: schema::Root::Cmd,
                name: "QueueItem",
            }),
            value: fixture_value(value),
        });
    }
    rows.push(FixtureRow {
        path: String::from("shared/QueueItem.LegacyBareEmbyItem.json"),
        direction: Direction::DecodeOnly,
        schema: None,
        value: fixture_value(emby_queue_item()),
    });
    rows.push(FixtureRow {
        path: String::from("shared/PlayerEvent.StoppedBareRunIdentity.json"),
        direction: Direction::DecodeOnly,
        schema: None,
        value: fixture_value(stopped_event()),
    });
    rows.push(FixtureRow {
        path: String::from("shared/PlayerStatus.json"),
        direction: Direction::RoundTrip,
        schema: Some(SchemaAnchor::DefsEntry {
            root: schema::Root::Event,
            name: "PlayerStatus",
        }),
        value: fixture_value(player_status()),
    });
    rows.push(FixtureRow {
        path: String::from("shared/UnifiedQueueStateData.WithTransitions.json"),
        direction: Direction::RoundTrip,
        schema: Some(SchemaAnchor::DefsEntry {
            root: schema::Root::Event,
            name: "UnifiedQueueStateData",
        }),
        value: fixture_value(unified_queue_state_with_transitions()),
    });
}
