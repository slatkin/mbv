use super::*;
use crate::app::components::emby_library_content::{BrowserOwnerPush, EmbyLibraryContent};
use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
use crate::app::components::LibraryKind;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute};

#[test]
fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
    let mut model = mounted_model_at(make_home_video_app(), 60, 20);
    let lib_idx = 0;

    assert!(
        !model.app.is_viewing_album_folders(lib_idx),
        "a homevideos library must never satisfy is_viewing_album_folders"
    );
    assert!(model.app.is_home_video_view(lib_idx));

    let out = draw_mounted_frame(&mut model, 60, 20);

    assert!(
        out.contains("Birthday Clip"),
        "expected the embedded EmbyLibraryContent owner to paint the home-video list:\n{out}"
    );
    assert!(
        !out.contains("2 items"),
        "the removed home-video item count must not paint:\n{out}"
    );
    assert!(
        model.app.album_tracks_cache.is_empty(),
        "home-video rendering must never touch the album-tracks cache added by #145"
    );
}

/// `remove-migrated-surface-underpaint` 3.2 (D4): at the wide Wide hero
/// breakpoint the embedded `EmbyLibraryContent` owner owns the Movies / home-video
/// picture. Post task 3.8 the legacy `render_library` `EmbyLibrary` arm only
/// reserves the destination `left_area` and paints no row, banner, or hero —
/// the `movies_wide_*` split geometry hand-off is now published by the
/// component itself. Mirrors the Home precedent
/// `legacy_base_frame_does_not_paint_home_content_before_the_component`.
#[test]
fn wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows() {
    for (mut app, marker) in [
        (make_movie_app(), "Focused Movie"),
        (make_home_video_app(), "Birthday Clip"),
    ] {
        let mut layout = Rect::default();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        term.draw(|f| {
            app.reserve_library_area(
                f,
                ratatui::layout::Rect::new(0, 0, 120, 40),
                &mut layout,
                None,
            );
        })
        .unwrap();

        assert!(
            layout.width > 0 && layout.height > 0,
            "wide movies destination area hand-off must still be reserved: {:?}",
            layout
        );
        let output = buffer_to_string(&term);
        assert!(
            !output.contains(marker),
            "legacy base frame must not paint browser rows at the wide breakpoint: {output:?}"
        );
    }
}

#[test]
fn emby_latest_rows_marker_and_wide_hero_use_shared_panel_painters() {
    for kind in [
        LibraryKind::Movies,
        LibraryKind::Generic,
        LibraryKind::HomeVideos,
    ] {
        for (width, height, wide) in [(240, 30, true), (80, 30, false)] {
            let mut item = crate::app::tests::make_item("Latest Movie", "Movie");
            item.id = "latest-movie".into();
            item.date_added = "2023-11-14T00:00:00Z".into();
            item.overview = "Latest overview".into();
            let mut owner = EmbyLibraryContent::new(kind);
            owner.set_content(BrowserOwnerPush {
                items: Vec::new(),
                latest_items: vec![item],
                total_count: 0,
                library_total: None,
                letter_filter: None,
                loading: false,
                group_pills: kind == LibraryKind::HomeVideos,
                show_letter_pills: kind != LibraryKind::HomeVideos,
                feed_groups: Vec::new(),
                feed_group_ids: Vec::new(),
                feed_group_cursor: 0,
            });
            owner.set_latest_mode(true);
            owner.set_latest_marker(true);
            let key = LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: format!("{kind:?}"),
                kind,
            };
            let mut panel = LibraryPanel::new();
            panel.insert_owner(key.clone(), Box::new(owner));
            panel.set_active(Some(key));
            Component::attr(&mut panel, Attribute::Focus, AttrValue::Flag(true));
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, width, height)))
                .unwrap();
            let output = buffer_to_string(&terminal);
            assert!(output.contains("14 Nov"), "Latest date gutter: {output:?}");
            assert!(output.contains('•'), "Latest selector marker: {output:?}");
            if wide {
                let geometry = panel.test_wide_geometry().expect("Wide panel");
                assert!(
                    (geometry.hero_area.top()..geometry.hero_area.bottom()).any(|y| {
                        (geometry.hero_area.left()..geometry.hero_area.right())
                            .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                            .collect::<String>()
                            .contains("Latest Movie")
                    }),
                    "selected Latest detail hero paints"
                );
            }
        }
    }
}

#[test]
fn letter_filter_buckets_match_emby_name_range_bounds() {
    let ac = LetterFilter::for_index_for_kind(0, LetterFilterKind::Movie).unwrap();
    assert_eq!(ac.label, "A\u{2013}C");
    assert_eq!(ac.name_ge, Some("A"));
    assert_eq!(ac.name_lt, Some("D"));

    let vz = LetterFilter::for_index_for_kind(7, LetterFilterKind::Movie).unwrap();
    assert_eq!(vz.label, "V\u{2013}Z");
    assert_eq!(vz.name_ge, Some("V"));
    assert_eq!(vz.name_lt, None, "V–Z has no upper bound");

    let hash = LetterFilter::for_index_for_kind(8, LetterFilterKind::Movie).unwrap();
    assert_eq!(hash.label, "#");
    assert_eq!(hash.name_ge, None, "# has no lower bound");
    assert_eq!(hash.name_lt, Some("A"));

    assert!(LetterFilter::for_index_for_kind(9, LetterFilterKind::Movie).is_none());
    assert_eq!(LetterFilter::count_for_kind(LetterFilterKind::Movie), 9);
    assert_eq!(LetterFilter::labels().len(), 9);
}

#[test]
fn letter_filter_default_is_the_first_bucket() {
    assert_eq!(
        LetterFilter::default_filter_for_kind(LetterFilterKind::Movie),
        LetterFilter::for_index_for_kind(0, LetterFilterKind::Movie).unwrap()
    );
}

#[test]
fn tv_letter_filter_buckets_cover_three_ranges_and_sort_keys() {
    let ai = LetterFilter::for_index_for_kind(0, LetterFilterKind::Tv).unwrap();
    assert_eq!(ai.label, "A-I");
    assert_eq!(ai.name_ge, None);
    assert_eq!(ai.name_lt, Some("J"));

    let jr = LetterFilter::for_index_for_kind(1, LetterFilterKind::Tv).unwrap();
    assert_eq!(jr.label, "J-R");
    assert_eq!(jr.name_ge, Some("J"));
    assert_eq!(jr.name_lt, Some("S"));

    let sz = LetterFilter::for_index_for_kind(2, LetterFilterKind::Tv).unwrap();
    assert_eq!(sz.label, "S-Z");
    assert_eq!(sz.name_ge, Some("S"));
    assert_eq!(sz.name_lt, None);

    assert_eq!(
        LetterFilter::for_sort_key_for_kind("123 Title", LetterFilterKind::Tv),
        Some(ai.clone())
    );
    assert_eq!(
        LetterFilter::for_sort_key_for_kind("Zebra", LetterFilterKind::Tv),
        Some(sz)
    );
    assert_eq!(LetterFilter::count_for_kind(LetterFilterKind::Tv), 3);
    assert_eq!(
        LetterFilter::labels_for_kind(LetterFilterKind::Tv),
        vec!["A-I", "J-R", "S-Z"]
    );
}
