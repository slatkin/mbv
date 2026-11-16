//! Unit tests of the pointer target's walk (design D1): the marker chain as a
//! pure function of the ancestors' attribute values. No test opens a window,
//! sleeps, or reaches the core.

use super::{PointerAncestor, PointerList, PointerTarget, PointerZone, pointer_target};

/// One ancestor of the hovered node, as its marker values.
fn ancestor<'a>(
    zone: Option<&'a str>,
    list: Option<&'a str>,
    row: Option<&'a str>,
) -> PointerAncestor<'a> {
    PointerAncestor { zone, list, row }
}

/// An ancestor that carries no marker.
fn plain() -> PointerAncestor<'static> {
    ancestor(None, None, None)
}

/// Contract (design D1): a row inside a list inside the Library zone names the
/// Library zone, the list, and the row.
#[test]
fn a_row_inside_a_list_inside_the_library_zone_names_all_three() {
    let chain = [
        plain(),
        ancestor(None, None, Some("artist-1")),
        ancestor(None, Some("music"), None),
        ancestor(Some("library"), None, None),
        plain(),
    ];

    assert_eq!(
        pointer_target(&chain),
        PointerTarget {
            zone: Some(PointerZone::Library),
            list: Some(PointerList::Music),
            row: Some("artist-1".to_owned()),
        }
    );
}

/// Contract (design D1): a node inside the Queue zone names the Queue zone,
/// the header, the filter, the rows, and the footer band included — and the
/// player block above it carries no marker.
#[test]
fn a_node_inside_the_queue_zone_names_the_queue_alone() {
    let row = [plain(), ancestor(Some("queue"), None, None), plain()];
    let player_block = [plain(), plain()];

    assert_eq!(
        pointer_target(&row),
        PointerTarget {
            zone: Some(PointerZone::Queue),
            list: None,
            row: None,
        }
    );
    assert_eq!(pointer_target(&player_block), PointerTarget::default());
}

/// Contract (design D1): a node with no marked ancestor names nothing, so the
/// key carries no target beyond the global chords.
#[test]
fn a_node_with_no_markers_names_nothing() {
    let chain = [plain(), plain(), plain()];

    assert_eq!(pointer_target(&chain), PointerTarget::default());
}

/// Contract (design D1): the row's chrome — its play control, its actions, its
/// glyph — resolves to the row, because the nearest row marker wins.
#[test]
fn a_row_chrome_child_resolves_to_its_row() {
    let chain = [
        plain(),
        plain(),
        ancestor(None, None, Some("track-2")),
        ancestor(None, Some("tracklist"), None),
        ancestor(Some("library"), None, None),
    ];

    assert_eq!(
        pointer_target(&chain),
        PointerTarget {
            zone: Some(PointerZone::Library),
            list: Some(PointerList::Tracklist),
            row: Some("track-2".to_owned()),
        }
    );
}
