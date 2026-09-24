use super::*;

#[test]
fn tv_launch_snapshot_uses_unfiltered_letter_scope_and_excludes_season_workspace() {
    let mut series = make_item("Series", "Series");
    series.id = "series-stable".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: Default::default(),
    };
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        true,
    ));

    {
        let content = owner.content();
        let workspace = content
            .hero
            .as_ref()
            .and_then(|hero| hero.workspace.as_ref())
            .expect("selected TV series mounts a Hero Workspace");
        let selector = workspace
            .selector
            .as_ref()
            .expect("season detail mounts a Workspace selector row");
        assert_eq!(selector.pills, vec!["Season 1"]);
    }

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Unfiltered,
            }),
            Some(LibraryItemIdentity::Emby {
                id: "series-stable".into(),
            }),
        )
    );
}

#[test]
fn tv_reanchor_launch_state_falls_back_to_first_series_when_item_is_missing() {
    let mut first = make_item("First", "Series");
    first.id = "first-series".into();
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first], 0),
        None,
        None,
        0,
        None,
        true,
    ));
    owner.test_set_letter_filter(2);
    let state = mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::Home,
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Emby {
            key: EmbySelectorKey::Letter(EmbyLetterBucket::GToI),
        }),
        item: Some(LibraryItemIdentity::Emby { id: "gone".into() }),
    };
    assert!(owner.reanchor_launch_state(&state));
    assert_eq!(owner.launch_snapshot().0, state.selector);
    assert_eq!(
        owner.launch_snapshot().1,
        Some(LibraryItemIdentity::Emby {
            id: "first-series".into()
        })
    );
}

#[test]
fn tv_launch_snapshot_uses_nonzero_letter_bucket_identity() {
    let mut series = make_item("Series", "Series");
    series.id = "series-stable".into();
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series], 0),
        None,
        None,
        0,
        None,
        true,
    ));
    owner.test_set_letter_filter(2);

    assert_eq!(
        owner.launch_snapshot(),
        (
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Letter(EmbyLetterBucket::GToI),
            }),
            Some(LibraryItemIdentity::Emby {
                id: "series-stable".into(),
            }),
        )
    );
}

#[test]
fn tv_launch_snapshot_reports_absence_without_letter_pills_or_series() {
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(Vec::new(), 0),
        None,
        None,
        0,
        None,
        false,
    ));

    assert_eq!(owner.launch_snapshot(), (None, None));
}
