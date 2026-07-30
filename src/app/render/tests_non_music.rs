use super::test_helpers::*;
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
    let mut app = make_power_home_video_app();
    let lib_idx = 0;

    assert!(
        !app.is_viewing_album_folders(lib_idx),
        "a homevideos library must never satisfy is_viewing_album_folders"
    );
    assert!(app.is_home_video_view(lib_idx));
    assert!(app.libs[lib_idx].album_track_focus.is_none());

    let mut layout = LayoutMain::default();
    let out = render_power_library_to_string(&mut app, &mut layout);

    assert!(
        out.contains("Birthday Clip"),
        "expected the original single-pane home-video list renderer to fire \
         unchanged:\n{out}"
    );
    assert!(
        app.album_tracks_cache.is_empty(),
        "home-video rendering must never touch the album-tracks cache added by #145"
    );
    assert!(
        app.libs[lib_idx].album_track_focus.is_none(),
        "home-video rendering must never set track-selection mode"
    );
}

#[test]
fn letter_filter_buckets_match_emby_name_range_bounds() {
    let ac = LetterFilter::for_index(0).unwrap();
    assert_eq!(ac.label, "A\u{2013}C");
    assert_eq!(ac.name_ge, Some("A"));
    assert_eq!(ac.name_lt, Some("D"));

    let vz = LetterFilter::for_index(7).unwrap();
    assert_eq!(vz.label, "V\u{2013}Z");
    assert_eq!(vz.name_ge, Some("V"));
    assert_eq!(vz.name_lt, None, "V–Z has no upper bound");

    let hash = LetterFilter::for_index(8).unwrap();
    assert_eq!(hash.label, "#");
    assert_eq!(hash.name_ge, None, "# has no lower bound");
    assert_eq!(hash.name_lt, Some("A"));

    assert!(LetterFilter::for_index(9).is_none());
    assert_eq!(LetterFilter::count(), 9);
    assert_eq!(LetterFilter::labels().len(), 9);
}

#[test]
fn letter_filter_default_is_the_first_bucket() {
    assert_eq!(
        LetterFilter::default_filter(),
        LetterFilter::for_index(0).unwrap()
    );
}

#[test]
fn letter_pills_show_only_when_library_total_exceeds_threshold() {
    let mut small = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD);
    assert!(
        !small.should_show_letter_pills(0),
        "exactly the threshold must not qualify"
    );

    let mut large = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD + 1);
    assert!(large.should_show_letter_pills(0));

    let backend = TestBackend::new(60, 20);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        large.render_main(
            f,
            Rect::new(0, 0, 60, 20),
            &mut layout,
            &mut crate::app::layout::LayoutPlayback::default(),
            &mut Rect::default(),
            &mut Rect::default(),
            0,
            false,
            &None,
        );
    })
    .unwrap();
    let out = buffer_to_string(&term);
    assert!(
        out.contains("A\u{2013}C"),
        "expected the default A–C pill to render:\n{out}"
    );
    assert!(
        !layout.selector_tabs.is_empty(),
        "expected pill hitboxes to be recorded for click dispatch"
    );

    let backend2 = TestBackend::new(60, 20);
    let mut term2 = Terminal::new(backend2).unwrap();
    let mut layout2 = LayoutMain::default();
    term2
        .draw(|f| {
            small.render_main(
                f,
                Rect::new(0, 0, 60, 20),
                &mut layout2,
                &mut crate::app::layout::LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
    let out2 = buffer_to_string(&term2);
    assert!(!out2.contains("A\u{2013}C"));
}
