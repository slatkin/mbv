use super::*;

/// A played episode projects the shared `Played` state, so the episode list
/// paints the one played-row colour used by every other tab.
#[test]
fn episode_rows_project_runtime_in_the_green_gutter() {
    let mut episode = make_item("Episode", "Episode");
    episode.runtime_ticks = 3_661 * mbv_emby_model::TICKS_PER_SECOND;
    let rows = build_episode_rows(&[episode]);
    let MediaListRow::Item {
        trailing, duration, ..
    } = &rows[0]
    else {
        panic!("episode rows are items");
    };
    assert_eq!(trailing, &Some(MediaListTrailing::Gutter("1:01".into())));
    assert_eq!(duration, &None);
}

fn premiere(series_name: &str, date: &str) -> UpcomingPremiere {
    UpcomingPremiere {
        series_id: mbv_ids::ItemId::new(format!("{series_name}-id")),
        series_name: series_name.into(),
        season: Some(2),
        episode: Some(7),
        title: "Pilot".into(),
        premiere_date: date.into(),
    }
}

#[test]
fn upcoming_premiere_rows_group_dates_in_first_seen_order_with_relative_headings() {
    let today = time::Date::from_calendar_date(2026, time::Month::September, 23).unwrap();
    let rows = upcoming_premiere_rows(
        &[
            premiere("Yesterday episode", "2026-09-22T00:00:00Z"),
            premiere("Today episode", "2026-09-23"),
            premiere("Another yesterday", "2026-09-22"),
            premiere("Older episode", "2026-09-21"),
        ],
        today,
    );
    let headings: Vec<&str> = rows
        .iter()
        .filter_map(|row| match row {
            MediaListRow::Heading { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(headings, ["Yesterday", "Today", "Monday, September 21"]);
    assert_eq!(
        rows.iter()
            .filter_map(|row| match row {
                MediaListRow::Item { primary, .. } => Some(primary.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            "Yesterday episode",
            "Another yesterday",
            "Today episode",
            "Older episode"
        ]
    );
}

/// Contract: tv-library-content-modes "Upcoming rows are premiere notices".
/// The owner builds Upcoming rows from `context.upcoming`, not from the
/// (empty) level items, and each row targets its premiere's stable key.
#[test]
fn upcoming_owner_rows_come_from_context_premieres_keyed_by_stable_key() {
    let first = premiere("Example Show", "");
    let mut second = first.clone();
    second.episode = None;
    let mut context = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0),
        None,
        None,
        0,
        None,
        false,
    );
    context.set_tv_content_mode(Some(mbv_queue::TvContentMode::Upcoming));
    context.upcoming = vec![first.clone(), second.clone()];
    let mut component = TvContent::new();
    component.set_content(context);
    component.carrier.select_index(1);
    assert_ne!(first.stable_key(), second.stable_key());
    assert_eq!(
        component.carrier.selected_target(),
        Some(&second.stable_key())
    );
}

/// What the user does to the selected Upcoming row.
#[derive(Clone, Copy)]
enum Gesture {
    Enter,
    DoubleClick,
    CtrlP,
}

/// A Wide owner in Upcoming mode with one painted notice row selected.
fn upcoming_owner_with_painted_row() -> TvContent {
    let mut context = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0),
        None,
        None,
        0,
        None,
        false,
    );
    context.set_tv_content_mode(Some(mbv_queue::TvContentMode::Upcoming));
    context.upcoming = vec![premiere("Example Show", "")];
    let mut owner = TvContent::new();
    owner.set_content(context);
    owner.set_is_wide(true);
    owner.set_focused(true);
    let list = ratatui::layout::Rect::new(0, 0, 40, 3);
    let geometry = owner.carrier.wide().row_geometry(3);
    owner
        .carrier
        .wide_mut()
        .finish_view(list, list, &geometry, None);
    owner
}

fn drive(owner: &mut TvContent, gesture: Gesture) -> Option<Msg> {
    match gesture {
        Gesture::Enter => owner.handle_key(&KeyEvent {
            code: Key::Enter,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }),
        Gesture::DoubleClick => owner.handle_slot_event(LibrarySlotEvent::List(
            MediaListSurfaceInput::DoubleClick(Position::new(1, 0)),
        )),
        Gesture::CtrlP => owner.handle_key(&KeyEvent {
            code: Key::Char('p'),
            modifiers: tuirealm::event::KeyModifiers::CONTROL,
        }),
    }
}

fn open_example_series() -> Msg {
    Msg::Shell(Box::new(ShellRequest::TvOpenUpcomingSeries {
        series_id: mbv_ids::ItemId::new("Example Show-id"),
        series_name: "Example Show".into(),
    }))
}

/// Contract: tv-library-content-modes "Upcoming rows are premiere notices
/// that open their series". Enter and a double-click open the row's series;
/// Ctrl+P emits nothing that plays.
#[rstest::rstest]
#[case::enter(Gesture::Enter, Some(open_example_series()))]
// Regression guard for 1bc51eacb, "Fix Upcoming placeholder mouse activation".
#[case::double_click(Gesture::DoubleClick, Some(open_example_series()))]
#[case::ctrl_p(Gesture::CtrlP, None)]
fn selected_upcoming_row_opens_its_series_and_never_plays(
    #[case] gesture: Gesture,
    #[case] expected: Option<Msg>,
) {
    let mut owner = upcoming_owner_with_painted_row();

    assert_eq!(drive(&mut owner, gesture), expected);
}
