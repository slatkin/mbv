use super::test_helpers::*;
use super::*;

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
