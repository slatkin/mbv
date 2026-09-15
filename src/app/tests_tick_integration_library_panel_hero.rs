use super::*;

#[test]
fn owner_retention_follows_the_catalog_through_the_sync_pass() {
    let mut harness = {
        let mut app = crate::app::render::make_movie_app();
        app.panel_mode = crate::app::PanelMode::LibraryOnly;
        app.panel_focus = crate::app::PanelFocus::Library;
        app.tab = crate::app::TabSelection::EmbyLibrary(0);
        let mut harness = TickHarness::new(app);
        harness.model_mut().sync_library_panel();
        harness.model_mut().push_library_owner(
            movies_key(),
            Box::new(FixtureOwner::new(Rc::new(RefCell::new(
                FixtureLog::default(),
            )))),
        );
        harness.model_mut().sync_mounted_surfaces();
        harness
    };
    assert!(
        harness.model().library_panel_has_owner(&movies_key()),
        "the in-catalog library's owner is retained"
    );

    // The library leaves the catalog: the sync pass retires its owner.
    harness.model_mut().app.libs.remove(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness.model().library_panel_has_owner(&movies_key()),
        "the retired library's owner is dropped by the retention rule"
    );
}

// ── Task 5.10: hero image projection (design D9) ────────────────────────
//
// The Home Hero header's fetch moved into `sync_library_hero_images`
// (`App::project_hero_image`); painting only reads the projected
// `HeroImageState`. These fixtures use a landscape-declared item so the
// projection reaches the Wide cover-fit box path.

/// A hero-bearing owner: the minimal owner the projection needs — one item's
/// policy-produced `HeroContentData`, mutable so a test can swap it for a
/// fresh item (a new cache key), and `set_hero_image` records every state the
/// projection delivers so a test can prove the placeholder frame count.
struct HeroFixtureOwner {
    carrier: MediaListCarrier<String>,
    hero: HeroContentData,
    image_states: Vec<HeroImageState>,
    hero_scroll: usize,
}

impl HeroFixtureOwner {
    fn new(hero: HeroContentData) -> Self {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![row("alpha")]);
        Self {
            carrier,
            hero,
            image_states: Vec::new(),
            hero_scroll: 0,
        }
    }
}

impl LibraryContentOwner for HeroFixtureOwner {
    fn content(&mut self) -> LibraryPanelContent<'_> {
        LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut self.carrier),
            hero: Some(HeroContent {
                facts: self.hero.facts.clone(),
                overview: self.hero.overview.clone(),
                credits: self.hero.credits.clone(),
                workspace: None,
            }),
        }
    }

    fn on_slot_event(&mut self, _event: LibrarySlotEvent) -> Option<Msg> {
        None
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        Some(self.hero.clone())
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.image_states.push(state.clone());
        self.hero.facts.artwork.image = state;
    }

    fn hero_scroll_offset(&self) -> usize {
        self.hero_scroll
    }

    fn hero_scroll(&mut self, delta: i16, max_offset: usize) -> bool {
        let next = if delta < 0 {
            self.hero_scroll.saturating_sub((-delta) as usize)
        } else {
            self.hero_scroll.saturating_add(delta as usize)
        }
        .min(max_offset);
        let changed = next != self.hero_scroll;
        self.hero_scroll = next;
        changed
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A movie item with a declared landscape (`Thumb`) image: the artwork
/// policy's Landscape arm, whose cover-fit box tracks the hero pane's width
/// directly (design D5), so a resize changes the box.
fn landscape_hero_item(id: &str) -> mbv_core::api::EmbyItem {
    let mut item = crate::app::tests::make_item("Hero", "Movie");
    item.id = id.into();
    item.image_tags.thumb = "tag".into();
    item
}

/// A Home tab with a `HeroFixtureOwner` installed instead of the plain
/// `FixtureOwner` (this file's other fixture has no declared artwork, so
/// `hero_data` returns `None` and the projection never fetches).
fn migrated_home_with_hero(item: mbv_core::api::EmbyItem, terminal_width: u16) -> TickHarness {
    let mut app = crate::app::render::make_movie_app();
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.tab = crate::app::TabSelection::Home;
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.terminal_width = terminal_width;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().push_library_owner(
        home_key(),
        Box::new(HeroFixtureOwner::new(hero_content_emby(&item))),
    );
    harness
}

fn migrated_movie_with_hero(item: mbv_core::api::EmbyItem) -> TickHarness {
    let mut app = crate::app::render::make_movie_app();
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.terminal_width = 160;
    app.terminal_height = 70;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().push_library_owner(
        movies_key(),
        Box::new(HeroFixtureOwner::new(hero_content_emby(&item))),
    );
    harness
}

/// Seed decoded sources without constructing a live Service. Keeping the
/// protocol map empty makes the following real shell sync/draw pass prove the
/// projection's one Wide protocol build and the normal Narrow lazy protocol
/// path, rather than testing a prebuilt painter fixture.
fn seed_cached_hero_image(
    harness: &mut TickHarness,
    cache_key: &str,
    rgba: [u8; 4],
) {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        40,
        20,
        image::Rgba(rgba),
    ));
    harness.model_mut().app.card_image_loading.remove(cache_key);
    harness.model_mut().app.card_image_states.insert(
        cache_key.to_owned(),
        crate::app::images::CachedImage {
            img: Some(image),
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        },
    );
}

fn draw_library_at(
    harness: &mut TickHarness,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    terminal
}

/// Establish the root placement, then run the production sync pass and draw
/// the settled frame. The dimensions match the App fixture so the seeded
/// image state is not mistaken for a resize reset on the initial frame.
fn settle_library_frame(
    harness: &mut TickHarness,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    drop(draw_library_at(harness, width, height));
    harness.model_mut().sync_mounted_surfaces();
    draw_library_at(harness, width, height)
}

#[test]
fn wide_landscape_movie_logo_is_one_composited_paint_and_narrow_is_undecorated() {
    let mut decorated_item = landscape_hero_item("panel-logo");
    decorated_item.image_tags.logo = "logo-tag".into();
    let mut decorated = migrated_home_with_hero(decorated_item, 160);
    seed_cached_hero_image(&mut decorated, "panel-logo:Backdrop,Primary", [20, 40, 60, 255]);
    seed_cached_hero_image(&mut decorated, "panel-logo:Logo:logo-tag", [220, 100, 20, 128]);
    let _wide_frame = settle_library_frame(&mut decorated, 160, 40);

    let wide_geometry = panel_of(&decorated)
        .and_then(|panel| panel.test_wide_geometry())
        .expect("the landscape Movie paints the Wide skeleton");
    let wide_image = wide_geometry
        .hero_image
        .as_ref()
        .expect("the Wide hero reserves one image paint");
    assert_eq!(
        decorated.model().app.image_protocol_builds.get(),
        1,
        "the Wide hero builds one protocol for the composited bitmap"
    );
    assert_eq!(
        decorated
            .model()
            .app
            .card_image_states
            .get("panel-logo:Backdrop,Primary")
            .and_then(|entry| entry.applied_logo_key.as_deref()),
        Some("panel-logo:Logo:logo-tag"),
        "the single Wide protocol records the applied Logo identity"
    );

    // The undecorated control uses the same landscape Movie and base source;
    // its artwork box must be unchanged, not merely present.
    let mut plain_item = landscape_hero_item("panel-logo");
    plain_item.image_tags.logo.clear();
    let mut plain = migrated_home_with_hero(plain_item, 160);
    seed_cached_hero_image(&mut plain, "panel-logo:Backdrop,Primary", [20, 40, 60, 255]);
    let _plain_frame = settle_library_frame(&mut plain, 160, 40);
    let plain_geometry = panel_of(&plain)
        .and_then(|panel| panel.test_wide_geometry())
        .expect("the undecorated landscape Movie paints the Wide skeleton");
    assert_eq!(
        wide_image.area,
        plain_geometry
            .hero_image
            .as_ref()
            .expect("the undecorated hero reserves one image paint")
            .area,
        "adding a Logo does not change the landscape artwork box"
    );

    // A real terminal shrink resets the image cache in the sync pass. Re-seed
    // only the decoded base after that reset, then project/draw Narrow; this
    // proves the selected Movie retains a usable Narrow protocol without
    // requesting or applying its Wide-only Logo.
    decorated.model_mut().app.terminal_width = 80;
    decorated.model_mut().app.terminal_height = 40;
    drop(draw_library_at(&mut decorated, 80, 40));
    decorated.model_mut().sync_mounted_surfaces();
    assert!(
        decorated.model().app.card_image_states.is_empty(),
        "the real responsive resize resets image state before re-projection"
    );
    seed_cached_hero_image(&mut decorated, "panel-logo:Backdrop,Primary", [20, 40, 60, 255]);
    decorated.model_mut().sync_mounted_surfaces();
    let owner_state = decorated
        .model()
        .library_owner::<HeroFixtureOwner>(&home_key())
        .map(|owner| owner.hero.facts.artwork.image.clone());
    assert!(
        matches!(owner_state, Some(HeroImageState::Ready { .. })),
        "the responsive re-projection publishes Ready to the owner: state={owner_state:?}, cache_keys={:?}, loading={:?}",
        decorated.model().app.card_image_states.keys().collect::<Vec<_>>(),
        decorated.model().app.card_image_loading
    );
    let _narrow_frame = draw_library_at(&mut decorated, 80, 40);
    let narrow_geometry = panel_of(&decorated)
        .and_then(|panel| panel.test_narrow_geometry())
        .expect("the real 80-column draw paints the Narrow skeleton");
    let narrow_image = narrow_geometry.inline_hero_image.as_ref().unwrap_or_else(|| {
        panic!("the Narrow hero keeps one usable image paint after reset: {narrow_geometry:?}")
    });
    assert!(!narrow_image.centered, "Narrow uses its inline right-aligned image");
    assert!(
        decorated
            .model()
            .app
            .card_image_states
            .get("panel-logo:Backdrop,Primary")
            .is_some_and(|entry| !entry.protocols.is_empty()),
        "the Narrow draw resolves a usable protocol from the re-seeded base"
    );
    assert!(
        decorated
            .model()
            .app
            .card_image_loading
            .iter()
            .all(|key| !key.contains(":Logo:")),
        "Narrow does not reserve the Wide-only Logo"
    );

    // Explicit negative leg: a Wide Portrait Movie with a declared Logo is
    // still a single undecorated base protocol.
    let mut portrait_item = crate::app::tests::make_item("Portrait", "Movie");
    portrait_item.id = "panel-portrait".into();
    portrait_item.image_tags.primary = "poster-tag".into();
    portrait_item.image_tags.logo = "logo-tag".into();
    let mut portrait = migrated_home_with_hero(portrait_item, 160);
    seed_cached_hero_image(&mut portrait, "panel-portrait:Primary,Backdrop", [20, 40, 60, 255]);
    seed_cached_hero_image(&mut portrait, "panel-portrait:Logo:logo-tag", [220, 100, 20, 128]);
    let _portrait_frame = settle_library_frame(&mut portrait, 160, 40);
    let portrait_geometry = panel_of(&portrait)
        .and_then(|panel| panel.test_wide_geometry())
        .expect("the Portrait Movie paints the Wide skeleton");
    assert!(portrait_geometry.hero_image.is_some());
    assert_eq!(
        portrait
            .model()
            .app
            .card_image_states
            .get("panel-portrait:Primary,Backdrop")
            .and_then(|entry| entry.applied_logo_key.as_deref()),
        None,
        "a Wide Portrait Movie remains undecorated"
    );
    assert!(
        portrait
            .model()
            .app
            .card_image_loading
            .iter()
            .all(|key| !key.contains(":Logo:")),
        "a Wide Portrait Movie does not reserve a Logo"
    );
}

#[test]
fn only_wide_landscape_movie_reserves_declared_logo_not_portrait_narrow_placeholder_or_non_movie() {
    let mut landscape = landscape_hero_item("logo-wide");
    landscape.image_tags.logo = "logo-tag".into();
    let mut wide = migrated_home_with_hero(landscape, 160);
    drop(draw_frame_sized(&mut wide));
    wide.model_mut().sync_mounted_surfaces();
    assert!(wide.model().app.card_image_loading.contains("logo-wide:Logo:logo-tag"));
    assert_eq!(wide.model().app.card_image_fetch_calls, 2);

    let mut portrait = crate::app::tests::make_item("Portrait", "Movie");
    portrait.id = "logo-portrait".into();
    portrait.image_tags.primary = "poster".into();
    portrait.image_tags.logo = "logo-tag".into();
    let mut portrait_harness = migrated_home_with_hero(portrait, 160);
    drop(draw_frame_sized(&mut portrait_harness));
    portrait_harness.model_mut().sync_mounted_surfaces();
    assert!(!portrait_harness.model().app.card_image_loading.iter().any(|key| key.contains(":Logo:")));

    let mut narrow_item = landscape_hero_item("logo-narrow");
    narrow_item.image_tags.logo = "logo-tag".into();
    let mut narrow = migrated_home_with_hero(narrow_item, 80);
    drop(draw_frame_sized(&mut narrow));
    narrow.model_mut().sync_mounted_surfaces();
    assert!(!narrow.model().app.card_image_loading.iter().any(|key| key.contains(":Logo:")));

    let mut non_movie = crate::app::tests::make_item("Series", "Series");
    non_movie.id = "logo-series".into();
    non_movie.image_tags.thumb = "thumb".into();
    non_movie.image_tags.logo = "logo-tag".into();
    let mut non_movie_harness = migrated_home_with_hero(non_movie, 160);
    drop(draw_frame_sized(&mut non_movie_harness));
    non_movie_harness.model_mut().sync_mounted_surfaces();
    assert!(!non_movie_harness.model().app.card_image_loading.iter().any(|key| key.contains(":Logo:")));

    let mut placeholder = crate::app::tests::make_item("Placeholder", "Movie");
    placeholder.id = "logo-placeholder".into();
    let mut placeholder_harness = migrated_home_with_hero(placeholder, 160);
    drop(draw_frame_sized(&mut placeholder_harness));
    placeholder_harness.model_mut().sync_mounted_surfaces();
    assert!(!placeholder_harness.model().app.card_image_loading.iter().any(|key| key.contains(":Logo:")));
}

#[test]
fn mounted_movie_hero_wheel_scrolls_overflow_and_falls_through_when_fitting() {
    let mut movie = crate::app::tests::make_item("Hero", "Movie");
    movie.image_tags.thumb = "tag".into();
    movie.overview = "A deliberately long overview that occupies several lines in the hero box. ".repeat(8);
    movie.people = (0..20)
        .map(|index| mbv_core::api::EmbyPerson {
            name: format!("Actor {index}"),
            role: "Actor".into(),
            kind: "Actor".into(),
        })
        .collect();
    let mut harness = migrated_movie_with_hero(movie);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 70)).unwrap();
    terminal.draw(|frame| harness.model_mut().draw_frame(frame, false, false)).unwrap();
    harness.model_mut().sync_mounted_surfaces();
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<crate::app::components::library_panel::LibraryPanel>())
        .expect("Library panel mounted");
    let geometry = panel.test_wide_geometry().expect("Wide geometry painted");
    assert!(geometry.hero_area.width > 0);
    let box_rect = geometry.overview_box.expect("overflowing Movie overview box");
    let max_offset = geometry.overview_content_length - geometry.overview_viewport;
    assert!(max_offset > 0);
    let before = panel.test_hero_scroll_offset();
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: box_rect.x + 1,
        row: box_rect.y + 1,
        modifiers: KeyModifiers::empty(),
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }));
    let after = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<crate::app::components::library_panel::LibraryPanel>())
        .unwrap()
        .test_hero_scroll_offset();
    assert!(after > before);
    assert!(after <= max_offset);

    let mut fitting_movie = crate::app::tests::make_item("Hero", "Movie");
    fitting_movie.image_tags.thumb = "tag".into();
    fitting_movie.overview = "Short overview".into();
    let mut fitting = migrated_movie_with_hero(fitting_movie);
    terminal.draw(|frame| fitting.model_mut().draw_frame(frame, false, false)).unwrap();
    fitting.model_mut().sync_mounted_surfaces();
    let fitting_box = fitting
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<crate::app::components::library_panel::LibraryPanel>())
        .and_then(|panel| panel.test_wide_geometry())
        .and_then(|geometry| geometry.overview_box)
        .expect("fitting Movie overview box");
    fitting.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: fitting_box.x + 1,
        row: fitting_box.y + 1,
        modifiers: KeyModifiers::empty(),
    }));
    let outcome = fitting.step();
    assert!(outcome.raw_messages.iter().all(|message| {
        !matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }));
}

/// One fetch per new hero cache key, and none on a repaint tick with the
/// same key (task 5.10's Verify clause, mirroring the queue visual slot's
/// `queue_projection_fetches_now_playing_image_once_and_none_on_repaint`,
/// task 3.4). The fixture has no Emby client, so `spawn_image_fetch`
/// balances `image_fetches_active` synchronously; the reservation set pins
/// the request count.
#[test]
fn hero_projection_fetches_image_once_and_none_on_repaint() {
    let mut harness = migrated_home_with_hero(landscape_hero_item("hero-a"), 160);
    // Establishes `root_frame.library` before the first projection reads it.
    drop(draw_frame_sized(&mut harness));
    harness.model_mut().sync_mounted_surfaces();

    let key = "hero-a:Backdrop,Primary";
    assert!(
        harness.model().app.card_image_loading.contains(key),
        "the new hero key must be reserved by the projection"
    );
    let loading = harness.model().app.card_image_loading.clone();
    let active = harness.model().app.image_fetches_active;
    let pending = harness.model().app.pending_image_fetches.len();
    let calls = harness.model().app.card_image_fetch_calls;
    assert_eq!(calls, 1, "the new hero key issues exactly one fetch call");

    // Repaint tick: nothing changed, so the projection starts no new fetch.
    // `card_image_fetch_calls` only increments past the dedup guard, so it
    // catches a broken guard even though the fixture has no Emby client
    // (`spawn_image_fetch` balances `image_fetches_active` back to its prior
    // value synchronously in that case, making the other counters blind to a
    // redundant call).
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().app.card_image_loading, loading);
    assert_eq!(harness.model().app.image_fetches_active, active);
    assert_eq!(harness.model().app.pending_image_fetches.len(), pending);
    assert_eq!(
        harness.model().app.card_image_fetch_calls,
        calls,
        "a repaint with the same hero key must not call queue_card_image_fetch again"
    );

    // A new hero item reserves exactly one new key.
    if let Some(panel) = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any_mut()
                .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
        })
    {
        if let Some(owner) = panel
            .owner_mut(&home_key())
            .and_then(|owner| owner.as_any_mut().downcast_mut::<HeroFixtureOwner>())
        {
            owner.hero = hero_content_emby(&landscape_hero_item("hero-b"));
        }
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .app
        .card_image_loading
        .contains("hero-b:Backdrop,Primary"));
}
