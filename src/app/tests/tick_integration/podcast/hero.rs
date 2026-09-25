use super::*;

/// The shell's hero image projection (row 4.1, design D7): the podcast
/// episode hero's cover is projected keyed by the parent show's
/// `library_item_id` — the episode's own identity, not a show selection —
/// and the Ready state lands on the owner through the shell sync pass. The
/// image cache is pre-seeded under exactly that key so the proof is
/// hermetic: the projection finds the cache hit the episode's identity
/// dictates and never issues a fetch.
#[test]
fn podcast_hero_image_projection_keys_the_parent_show_cover_through_the_sync_pass() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].shows[0].cover_path = Some("cover".into());
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("http://abs.test"),
    );
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    let cache_key = crate::app::images::audiobookshelf_hero_cover_cache_key(
        "http://abs.test",
        "show-a",
        app.current_protocol_suffix(),
    );

    let mut harness = TickHarness::new(app);
    // The first draw settles the terminal size (its sync pass also clears
    // the image caches on the resize) and registers the podcast owner; the
    // seed below lands after that, so the projection that follows reads a
    // warm cache instead of issuing a fetch.
    draw(&mut harness, 80);
    harness.model_mut().app.card_image_states.insert(
        cache_key.clone(),
        crate::app::images::CachedImage {
            img: Some(image::DynamicImage::ImageRgba8(
                image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255])),
            )),
            protocols: std::collections::HashMap::new(),
            cover_box: None,
            applied_logo_key: None,
        },
    );
    harness.model_mut().sync_mounted_surfaces();

    let image = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.active_hero_data())
        .expect("the podcast hero projects")
        .facts
        .artwork
        .image;
    assert!(
        matches!(
            &image,
            crate::app::components::library_panel::HeroImageState::Ready {
                cache_key: key,
                ..
            } if key == &cache_key
        ),
        "the hero image state is Ready under the parent show's cover key: {image:?}"
    );
    // The pre-seeded cache entry means the projection issued no fetch: the
    // once-per-key discipline holds through the sync pass.
    assert!(harness.model().app.card_image_loading.is_empty());
}

/// The playing podcast's cover must not be one cache entry serving two
/// derivations. The Wide Library hero re-encodes its entry from a cover-fit
/// crop of its artwork box (`ensure_hero_cover_protocol`), while the queue
/// card renders the same show cover through `Resize::Scale`; sharing the entry
/// made each consumer `resize_encode` (take) the other's `ThreadProtocol` every
/// frame, so the hero painted its placeholder block instead of the image — the
/// reported "wide library hero image flashes constantly" while an ABS podcast
/// plays. A non-playing show never collided: its `library_item_id` gave the
/// hero a different key.
#[test]
fn playing_show_cover_keeps_the_hero_and_queue_card_entries_apart() {
    let mut app = audiobookshelf_app();
    app.audiobookshelf_browse[0].shows[0].cover_path = Some("cover".into());
    app.config.lock().unwrap().audiobookshelf_setup = Some(
        mbv_core::config::AudiobookshelfSetup::new("http://abs.test"),
    );
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    // The playing episode belongs to the show whose cover the selected hero
    // draws, which is exactly the collision case.
    app.player_tab
        .queue
        .append(mbv_core::playback_queue::QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfQueueItem {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
                title: "Episode A".into(),
                show_title: Some("Show A".into()),
                author: None,
                description: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: Some("cover".into()),
            },
        ));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
    }
    let suffix = app.current_protocol_suffix();
    let hero_key = crate::app::images::audiobookshelf_hero_cover_cache_key(
        "http://abs.test",
        "show-a",
        suffix,
    );
    let card_key =
        crate::app::images::audiobookshelf_cover_cache_key("http://abs.test", "show-a", suffix);
    assert_ne!(
        hero_key, card_key,
        "the hero and the queue card must not share an artwork entry"
    );

    let mut harness = TickHarness::new(app);
    // Settle the terminal size first: the resize pass clears the image caches,
    // so the seeds below must land after it.
    draw(&mut harness, 160);
    for key in [&hero_key, &card_key] {
        harness.model_mut().app.card_image_states.insert(
            key.clone(),
            crate::app::images::CachedImage {
                img: Some(image::DynamicImage::ImageRgba8(
                    image::RgbaImage::from_pixel(40, 20, image::Rgba([10, 20, 30, 255])),
                )),
                protocols: std::collections::HashMap::new(),
                cover_box: None,
                applied_logo_key: None,
            },
        );
    }
    harness.model_mut().app.refresh_queue_card_image();
    draw(&mut harness, 160);

    let panel = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .expect("library panel");
    assert!(
        panel.test_wide_geometry().is_some(),
        "the fixture must paint the Wide skeleton, where the hero's cover-fit box rebuilds the protocol"
    );
    let hero_image = panel
        .active_hero_data()
        .expect("the podcast hero projects")
        .facts
        .artwork
        .image;
    assert!(
        matches!(
            &hero_image,
            crate::app::components::library_panel::HeroImageState::Ready {
                cache_key: key,
                ..
            } if key == &hero_key
        ),
        "the hero image state is Ready under the hero-scoped cover key: {hero_image:?}"
    );

    let entries = &harness.model().app.card_image_states;
    assert!(
        entries[&hero_key].cover_box.is_some(),
        "the hero re-encodes its own entry with the cover-fit box"
    );
    assert!(
        entries[&card_key].cover_box.is_none(),
        "the queue card's entry keeps its plain encoding: the hero's crop must not reach it"
    );
    assert_eq!(
        harness
            .model()
            .app
            .queue_card_projection
            .cache_key
            .as_deref(),
        Some(card_key.as_str()),
        "the queue card paints its own key, not the hero's crop"
    );
}

/// Narrow-geometry Enter on a podcast episode plays immediately: no Library
/// Hero overlay opens (row 3.4, design D6 — podcast episodes are not
/// hero-bearing rows).
#[test]
fn narrow_enter_plays_the_selected_episode_without_an_overlay() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 80);
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel");
    assert!(panel.test_narrow_geometry().is_some());
    assert!(!panel.test_hero_overlay_open());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
       message,
       Msg::Shell(ref shell_boxed)
    if matches!(shell_boxed.as_ref(), ShellRequest::AudiobookshelfPodcastEpisodeIntent(
           crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(_))
       )))));
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel");
    assert!(!panel.test_hero_overlay_open(), "no overlay opened");
}

/// Narrow-geometry double-click activates the episode directly (row 3.4):
/// the not-hero-bearing owner skips the overlay attempt and the resolved
/// row's OpenOrPlay intent crosses the shell.
#[test]
fn narrow_double_click_plays_the_selected_episode_without_an_overlay() {
    let mut harness = TickHarness::new(audiobookshelf_app());
    draw(&mut harness, 80);
    let list = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel")
        .test_list_rect()
        .expect("podcast list paints");
    let point = (list.x + 1, list.y + 1);

    harness.inject(mouse(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    harness.step();
    harness.inject(mouse(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
       message,
       Msg::Shell(ref shell_boxed)
    if matches!(shell_boxed.as_ref(), ShellRequest::AudiobookshelfPodcastEpisodeIntent(
           crate::app::components::msg::PodcastEpisodeIntent::OpenOrPlay(Some(_))
       )))));
    let panel = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel");
    assert!(!panel.test_hero_overlay_open(), "no overlay opened");
}

fn hero_scroll_offset(harness: &TickHarness) -> usize {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .expect("library panel")
        .test_hero_scroll_offset()
}

/// The Wide hero's overview box is scrollable: the panel claims the wheel
/// against the box it painted and turns the owner's offset, so the box's
/// painted scrollbar follows the wheel. The hero describes one episode, so
/// selecting another starts its description at the top again.
#[test]
fn wide_hero_overview_wheel_scrolls_the_episode_description() {
    let mut app = audiobookshelf_app();
    {
        let episodes = app.audiobookshelf_browse[0]
            .detail_cache
            .get_mut("show-a")
            .expect("the fixture caches show-a's episodes");
        episodes[0].description = Some(
            "A deliberately long episode description that overflows the hero box. ".repeat(80),
        );
        episodes.push(episode("show-a", "episode-b"));
    }
    let mut harness = TickHarness::new(app);
    draw(&mut harness, 160);
    let (box_rect, max_offset) = {
        let panel = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .expect("library panel");
        let geometry = panel.test_wide_geometry().expect("wide panel");
        let box_rect = geometry
            .overview_box
            .expect("the episode hero paints an overview box");
        assert_eq!(panel.test_hero_scroll_offset(), 0);
        (
            box_rect,
            geometry.overview_content_length - geometry.overview_viewport,
        )
    };
    assert!(
        max_offset > 0,
        "the long description overflows the hero box"
    );

    harness.inject(mouse(
        MouseEventKind::ScrollDown,
        box_rect.x + 1,
        box_rect.y + 1,
    ));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed)
    )));
    let scrolled = hero_scroll_offset(&harness);
    assert!(
        scrolled > 0 && scrolled <= max_offset,
        "one wheel step scrolled the overview: {scrolled}"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    assert_eq!(
        podcast(&mut harness)
            .selected_episode_target()
            .expect("an episode is selected")
            .episode_id(),
        "episode-b"
    );
    assert_eq!(
        hero_scroll_offset(&harness),
        0,
        "the newly selected episode starts at the top of its description"
    );
}
