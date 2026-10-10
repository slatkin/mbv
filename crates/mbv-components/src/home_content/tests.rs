//! Home flat projection contract (`openspec/specs/home-latest-sections`:
//! "Recently added is one flat list within the 14-day cutoff").

use super::*;
use mbv_emby_model::test_support::make_item;
use mbv_render::components::media_list::MediaListRow;
use mbv_ui_msg::{HomeRequest, ShellRequest};

const DAY: u64 = 24 * 60 * 60;
pub(super) const NOW: u64 = 1_500_000_000;

fn iso_secs(secs: u64) -> String {
    use time::format_description::well_known::Iso8601;
    time::OffsetDateTime::from_unix_timestamp(i64::try_from(secs).unwrap_or(i64::MAX))
        .expect("in-range fixture timestamp")
        .format(&Iso8601::DEFAULT)
        .expect("ISO-8601 formatting always succeeds")
}

pub(super) fn recent_row(name: &str, days_old: u64) -> HomeRecentRow {
    let mut item = make_item(name, "Movie");
    item.id = name.to_string();
    item.date_added = iso_secs(NOW - days_old * DAY);
    HomeRecentRow {
        item: QueueItem::Emby(Box::new(item)),
        feed_name: None,
    }
}

/// Heading text or item primary text, one string per row, `-` for a spacer.
fn labels(owner: &HomeContent) -> Vec<String> {
    owner
        .carrier
        .rows()
        .iter()
        .map(|row| match row {
            MediaListRow::Heading { text } => format!("# {text}"),
            MediaListRow::Item { primary, .. } => primary.clone(),
            MediaListRow::Spacer => "-".into(),
        })
        .collect()
}

#[test]
fn recent_rows_form_one_flat_list_and_omit_stale_items() {
    let mut owner = HomeContent::new();
    owner.set_snapshot(
        Vec::new(),
        false,
        vec![
            recent_row("fresh", 1),
            recent_row("week", 13),
            recent_row("stale", 14),
        ],
        NOW,
    );
    assert_eq!(labels(&owner), ["# Recently added", "fresh", "week"]);

    owner.set_snapshot(Vec::new(), false, vec![recent_row("stale", 14)], NOW);
    assert_eq!(labels(&owner), Vec::<String>::new());
}

fn key(code: tuirealm::event::Key, modifiers: tuirealm::event::KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers }
}

fn recent_target(row: &HomeRecentRow) -> HomeItemTarget {
    HomeItemTarget::Recent(row.item.content_id())
}

/// "Home actions resolve the row's own section and Service identity":
/// marks made out of order act in display order.
#[test]
fn ctrl_enter_emits_marked_items_in_display_order() {
    use tuirealm::event::{Key, KeyModifiers};
    let (first, second) = (recent_row("first", 0), recent_row("second", 1));
    let expected = vec![recent_target(&first), recent_target(&second)];
    let mut owner = HomeContent::new();
    owner.set_snapshot(Vec::new(), false, vec![first, second], NOW);
    owner.carrier.select_target(&expected[1]);
    owner.carrier.toggle_selection(&expected[1]);
    owner.carrier.toggle_selection(&expected[0]);

    let msg = owner.handle_key(&key(Key::Enter, KeyModifiers::CONTROL));

    assert!(matches!(
        msg,
        Some(Msg::Shell(request))
            if *request == ShellRequest::Home(HomeRequest::Enqueue(expected))
    ));
}

/// The same item under both sections is two separately selectable rows.
#[test]
fn same_item_under_both_sections_has_two_targets() {
    use tuirealm::event::{Key, KeyModifiers};
    let recent = recent_row("dup", 1);
    let mut owner = HomeContent::new();
    owner.set_snapshot(vec![recent.item.clone()], false, vec![recent.clone()], NOW);

    let first = owner.selected_item_target();
    owner.handle_key(&key(Key::Down, KeyModifiers::NONE));

    assert_eq!(first, Some(HomeItemTarget::continue_item("dup")));
    assert_eq!(owner.selected_item_target(), Some(recent_target(&recent)));
}
