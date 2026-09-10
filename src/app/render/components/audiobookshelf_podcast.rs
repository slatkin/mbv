use crate::app::components::media_list::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, MediaKind, MediaListRow, MediaSemanticState,
    SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::arrangements::wide_hero::{
    self, wide_hero_browser_border, wide_hero_browser_pane, PANE_PAD_X, PANE_PAD_Y,
};
use crate::app::render::components::detail_series_view::{
    SERIES_DETAIL_DIVIDER_ROWS, SERIES_DETAIL_EPISODE_ROWS_ESTIMATE,
    SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS,
};
use crate::app::render::components::hero::{
    inline_hero_text_width, selected_detail_shell, wrap_overview_lines, HeroContent, HeroImage,
    HeroLine, HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::{render_pill_bar, render_placeholder, HomeImagePaint, PillBar};
use crate::app::types_audiobookshelf_browse::{
    build_show_title_buckets, AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use mbv_core::audiobookshelf::AudiobookshelfShow;
use tuirealm::component::Component;

/// Podcast hero content row budget, shared by the narrow
/// `App` renderer and `AudiobookshelfPodcastComponent`'s narrow path so both admit
/// the same inline-detail height. Mirrors the prior
/// `App::audiobookshelf_hero_content_rows` behavior exactly: title row,
/// optional author row, blank before a nonempty description, wrapped
/// description (capped at four rows) using `wrap_overview_lines` +
/// `inline_hero_text_width` with the image dimensions, episode
/// divider/visible-or-estimated episode rows, trailing blank, and the
/// image-height minimum when images are enabled.
pub(in crate::app::render) fn podcast_hero_content_rows(
    state: &AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    width: u16,
    images_enabled: bool,
) -> u16 {
    let title_rows = HERO_TITLE_ROWS;
    let author_rows = state
        .selected_show()
        .and_then(|show| show.author.as_ref())
        .is_some() as u16;
    let mut rows = title_rows + author_rows;
    if let Some(description) = state
        .selected_show()
        .and_then(|show| show.description.as_deref())
        .filter(|description| !description.is_empty())
    {
        rows += 1;
        let (image_width, image_height) = if images_enabled {
            (SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS)
        } else {
            (0, 0)
        };
        let description_start = title_rows + author_rows + 1;
        rows += wrap_overview_lines(description, |line| {
            let row = description_start + line as u16;
            inline_hero_text_width(width, image_width, image_height, row) as usize
        })
        .len()
        .min(4) as u16;
    }
    if interaction.episode_focused {
        rows += 1 + SERIES_DETAIL_DIVIDER_ROWS as u16;
        rows += state
            .episodes
            .as_ref()
            .map(|_| state.visible_episodes(interaction.episode_filter).len())
            .unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE) as u16;
    }
    rows += SERIES_DETAIL_TRAILING_BLANK_ROWS as u16;
    if images_enabled {
        rows = rows.max(SERIES_IMAGE_ROWS + 1);
    }
    rows
}
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

/// The component-owned interaction values the podcast renderer needs, passed
/// in rather than read off the projected content type
/// (split-browse-state-interaction-fields task 3.2). `episode_focused` is the
/// parent-owned episode-pane focus, separate from the episode owner's selected
/// row (design.md D5).
#[derive(Clone, Copy)]
pub(in crate::app) struct PodcastInteraction {
    pub episode_filter: AudiobookshelfEpisodeFilter,
    pub episode_focused: bool,
}

/// The active show-row presentation the podcast destination paints this frame
/// (design.md D1/D2): the Wide presentation for the Wide hero rail or the
/// Inline presentation for inline Narrow. Exactly one is handed over per view;
/// the same shared `MediaList` owner moves between them.
pub(in crate::app) enum PodcastShowPresentation<'a> {
    Wide(&'a mut WideMediaList<String>),
    Inline(&'a mut InlineMediaBrowser<String>),
}

/// The active episode-row presentation. Episodes always paint fixed
/// one-column rows through the Wide presentation (design.md D1/D2), in the
/// wide hero and the narrow inline detail alike.
pub(in crate::app) enum PodcastEpisodePresentation<'a> {
    Wide(&'a mut WideMediaList<String>),
}

/// Geometry painted by the podcast component. Input uses this same geometry,
/// so selector and show targets cannot drift from the rendered surface.
#[derive(Default)]
pub(in crate::app) struct AudiobookshelfPodcastGeometry {
    pub selector_tabs: Vec<(Rect, usize)>,
    /// Painted list/browser area: the wide right panel, or the narrow content
    /// area below the pill bar. Mirrors the legacy `LayoutMain.left_area` so
    /// the shell can anchor overlays after render ownership moved to the
    /// component (task 5.3d.10c).
    pub list_area: Rect,
    /// Wide-only right panel rect; zero in the narrow layout.
    pub right_area: Rect,
    /// Hero rect the component painted (wide hero panel, or narrow
    /// inline-detail hero). Zero when no hero was painted.
    pub hero_area: Rect,
    /// Narrow-only inline hero rect; zero in the wide layout or when the
    /// inline hero was rejected. Equals `hero_area` when set.
    pub inline_hero_area: Rect,
    /// Selected-item rect the component painted (only the narrow inline hero
    /// shell today; `None` in the wide layout, which has no selected-item
    /// shell). Mirrors the legacy `LayoutMain.selected_item_rect`.
    pub selected_item_rect: Option<Rect>,
}

/// Canonical row projection for the podcast show list: one selectable `Item`
/// per show, keyed by its stable `library_item_id`. Podcast shows carry no
/// in-list letter headings (the alphabetical buckets are a pill row) and no
/// played/active semantic state.
pub(in crate::app) fn podcast_show_rows(shows: &[AudiobookshelfShow]) -> Vec<MediaListRow<String>> {
    shows
        .iter()
        .map(|show| MediaListRow::Item {
            target: show.library_item_id.clone(),
            primary: show.title.clone(),
            trailing: None,
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}

/// Paints the shared alphabetical bucket pill row and records its hit targets.
fn paint_bucket_pills(
    frame: &mut Frame,
    pills_area: Rect,
    state: &AudiobookshelfBrowseState,
    geometry: &mut AudiobookshelfPodcastGeometry,
) {
    let buckets = build_show_title_buckets(&state.shows);
    let selected_bucket = buckets
        .iter()
        .position(|bucket| state.cursor() >= bucket.start && state.cursor() < bucket.end)
        .unwrap_or(0);
    let labels: Vec<String> = buckets.iter().map(|bucket| bucket.label.into()).collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    geometry.selector_tabs = render_pill_bar(
        frame,
        pills_area,
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: selected_bucket,
            prefix: Some(" \u{2318} "),
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render_audiobookshelf_podcast_content(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    images_enabled: bool,
    state: &AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    show_presentation: PodcastShowPresentation<'_>,
    episode_presentation: PodcastEpisodePresentation<'_>,
    geometry: &mut AudiobookshelfPodcastGeometry,
    list_pane_width: Option<u16>,
) -> Option<HomeImagePaint> {
    *geometry = AudiobookshelfPodcastGeometry::default();
    let Some(wide_hero::WideHeroPanes {
        hero: hero_panel,
        browser: right_panel,
    }) = wide_hero::wide_hero_presentation(area, list_pane_width)
    else {
        return render_narrow_podcast(
            frame,
            area,
            focused,
            images_enabled,
            state,
            interaction,
            show_presentation,
            episode_presentation,
            geometry,
        );
    };
    let PodcastShowPresentation::Wide(show_list) = show_presentation else {
        return None;
    };

    // Wide layout: the list/browser occupies the left pane; the hero panel is
    // the painted hero on the right. No inline hero and no selected-item shell exist here.
    geometry.list_area = right_panel;
    geometry.right_area = right_panel;
    let right_pane = wide_hero_browser_pane(right_panel, right_panel);
    if !state.shows.is_empty() {
        // Bucket pills before the hero so its wide-only episode-filter pills
        // append after them in `selector_tabs`.
        paint_bucket_pills(frame, right_pane.pills_area, state, geometry);
    }

    // Wide hero: fills and insets the left pane via the shared primitive
    // (D8: this surface gains focus-green when the episode workspace holds
    // focus, mirroring TV -- never a bare `focused`). Title lives in the
    // right show-list panel, so the hero body carries only
    // author/description/image. Persistent-mode episode pills + table are
    // wide-only.
    let hero_content_area = wide_hero::wide_hero_hero_pane(
        frame,
        area,
        wide_hero::LeftPaneFocus::Workspace(focused),
        list_pane_width,
    )
    .expect("wide branch already confirmed wide_hero_presentation fits");
    let image_paint = render_podcast_hero(
        frame,
        hero_content_area,
        state,
        interaction,
        focused,
        true,
        images_enabled,
        true,
        episode_presentation,
        geometry,
    );
    if state.shows.is_empty() {
        show_list.invalidate_paint();
        render_placeholder(frame, right_panel, "No podcast shows");
        return image_paint;
    }
    if state.selected_show().is_some() {
        geometry.hero_area = hero_panel;
    }

    let list_panel = right_pane.list_panel;
    let content_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
    if list_panel.height > 0 {
        frame.render_widget(
            Block::default().style(Style::default().bg(palette::resolve_surface_focus(focused))),
            list_panel,
        );
    }
    // Paint the rail frame before the rows: the border primitive rewrites every
    // panel cell background, so it must not run after the canonical list.
    wide_hero_browser_border(frame, list_panel, focused);

    show_list.set_geometry(list_panel, content_area);
    // Highlight gating follows the cursor's sub-panel (design.md D5): the show
    // rail marks its selected show only while it, not the episode pane, holds
    // the cursor. The panel FILLS keep following the library column's focus
    // (`focused`) above.
    let rail_focused = focused && !interaction.episode_focused;
    show_list.set_paint_policy(WideMediaListPaintPolicy::new(
        rail_focused,
        SelectedRowSurface::ListBackdrop,
        None,
    ));
    Component::view(show_list, frame, content_area);
    image_paint
}

#[allow(clippy::too_many_arguments)]
fn render_narrow_podcast(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    images_enabled: bool,
    state: &AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    show_presentation: PodcastShowPresentation<'_>,
    episode_presentation: PodcastEpisodePresentation<'_>,
    geometry: &mut AudiobookshelfPodcastGeometry,
) -> Option<HomeImagePaint> {
    let PodcastShowPresentation::Inline(show_list) = show_presentation else {
        return None;
    };
    if state.shows.is_empty() {
        show_list.invalidate_paint();
        render_placeholder(
            frame,
            area,
            state.error.as_deref().unwrap_or("No podcast shows"),
        );
        // Narrow with no shows: the whole area is the (empty) browser.
        geometry.list_area = area;
        return None;
    }
    let parts = wide_hero::pill_bar_areas(area);
    paint_bucket_pills(frame, parts.pills_area, state, geometry);

    let content_area = parts.content_area;
    geometry.list_area = content_area;

    show_list.set_geometry(content_area, content_area);
    let hero_content_width = content_area
        .width
        .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
    let desired_detail_rows =
        podcast_hero_content_rows(state, interaction, hero_content_width, images_enabled) as usize
            + HERO_BLOCK_EXTRA_ROWS as usize;

    show_list.set_paint_policy(InlineMediaBrowserPaintPolicy::new(
        focused,
        SelectedRowSurface::ListBackdrop,
        desired_detail_rows,
    ));
    Component::view(show_list, frame, content_area);
    let selected_row_rect = show_list.current_selected_row_rect();
    let hero = show_list.current_detail_rect();

    let Some(hero_area) = hero else {
        // Ordinary-row fallback: no inline hero, no selected-item shell.
        geometry.selected_item_rect = selected_row_rect;
        return None;
    };
    selected_detail_shell(frame, hero_area, hero_area.height, focused);
    // Narrow inline hero admitted: the painted hero is both the inline hero and
    // the selected-item shell the shell anchors overlays to.
    geometry.hero_area = hero_area;
    geometry.inline_hero_area = hero_area;
    geometry.selected_item_rect = Some(hero_area);
    // Narrow inline hero: title is painted (the selected show row is replaced);
    // persistent-mode episode pills + table are suppressed.
    render_podcast_hero(
        frame,
        hero_area,
        state,
        interaction,
        focused,
        true,
        images_enabled,
        false,
        episode_presentation,
        geometry,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_podcast_hero(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    focused: bool,
    show_title: bool,
    images_enabled: bool,
    wide: bool,
    episode_presentation: PodcastEpisodePresentation<'_>,
    geometry: &mut AudiobookshelfPodcastGeometry,
) -> Option<HomeImagePaint> {
    let show = state.selected_show()?;
    let mut lines = Vec::new();
    if let Some(author) = &show.author {
        lines.push(HeroLine::Plain(author.clone()));
    }
    if let Some(description) = &show.description {
        if !description.is_empty() {
            lines.push(HeroLine::Plain(String::new()));
            lines.push(HeroLine::Plain(description.clone()));
        }
    }
    lines.push(HeroLine::Plain(String::new()));
    // Wide: `area` is already the shared-inset content rect `wide_hero_hero_pane`
    // returned to the caller. Narrow keeps its own selected-item-shell inset.
    let content_area = if wide {
        area
    } else {
        Rect {
            x: area.x + SELECTED_BLOCK_SIDE_PADDING,
            y: area.y + SELECTED_BLOCK_SIDE_PADDING,
            width: area.width.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
            height: area.height.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
        }
    };
    // HeroImage is right-aligned by the painter; use the fixed cover width so
    // the full-width overview retains room for title and metadata.
    let image = images_enabled.then_some(HeroImage {
        actual_w: SERIES_IMAGE_COLS,
        height: SERIES_IMAGE_ROWS,
    });
    let result = crate::app::render::components::hero::paint_hero_content(
        frame,
        content_area,
        &HeroContent {
            title: show_title.then_some(show.title.as_str()),
            meta_line: None,
            meta_color: palette::TEXT_SECONDARY,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &lines,
            image,
        },
        focused,
    );
    // The selected-show detail uses the shared episode presentation in both
    // hero and replacement modes. Wide keeps the workspace focus gate; inline
    // detail is visible whenever the selected show has detail.
    if (wide && interaction.episode_focused || !wide) && result.next_row < area.bottom() {
        let PodcastEpisodePresentation::Wide(episode_list) = episode_presentation;
        let listing_area = Rect {
            y: result.next_row,
            height: area.bottom().saturating_sub(result.next_row),
            ..area
        };
        let (_, listing_content_area) = wide_hero::wide_hero_hero_content_box(frame, listing_area);
        let filter = interaction.episode_filter;
        let labels: Vec<String> = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .map(|filter| filter.label().into())
            .collect();
        let ids: Vec<usize> = (0..labels.len()).collect();
        let tabs = render_pill_bar(
            frame,
            Rect {
                x: listing_content_area.x,
                y: listing_content_area.y,
                width: listing_content_area.width,
                height: listing_content_area.height.min(1),
            },
            PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos: AudiobookshelfEpisodeFilter::ALL
                    .iter()
                    .position(|candidate| *candidate == filter)
                    .unwrap_or(0),
                prefix: Some(" ⌘ "),
            },
        );
        geometry.selector_tabs.extend(tabs);
        let row_y = listing_content_area.y + 1;
        let episode_area = Rect {
            x: listing_content_area.x,
            y: row_y,
            width: listing_content_area.width,
            height: area.bottom().saturating_sub(row_y),
        };
        // The episode owner already holds the filtered rows projected before
        // view (design.md D6); the painter only paints and retains the
        // current-frame hit geometry.
        episode_list.set_geometry(episode_area, episode_area);
        // The episode pane marks its selected episode only while it holds the
        // cursor (design.md D5); the show rail's highlight is the complement.
        episode_list.set_paint_policy(WideMediaListPaintPolicy::new(
            focused && interaction.episode_focused,
            SelectedRowSurface::ListBackdrop,
            None,
        ));
        Component::view(episode_list, frame, episode_area);
    }
    (images_enabled && result.img_rect.is_some()).then(|| HomeImagePaint::AudiobookshelfCover {
        area: result.img_rect.unwrap(),
        library_item_id: show.library_item_id.clone(),
        show_placeholder: true,
    })
}

#[cfg(test)]
mod tests {
    use super::{podcast_hero_content_rows, PodcastInteraction};
    use crate::app::render::components::detail_series_view::{
        SERIES_DETAIL_DIVIDER_ROWS, SERIES_DETAIL_EPISODE_ROWS_ESTIMATE,
        SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS,
    };
    use crate::app::render::components::hero::{
        inline_hero_text_width, wrap_overview_lines, HERO_TITLE_ROWS,
    };
    use crate::app::types_audiobookshelf_browse::{
        AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
    };
    use mbv_core::audiobookshelf::{
        AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfShow,
    };

    fn interaction(episode_focused: bool) -> PodcastInteraction {
        PodcastInteraction {
            episode_filter: AudiobookshelfEpisodeFilter::All,
            episode_focused,
        }
    }

    fn make_state(show: AudiobookshelfShow) -> AudiobookshelfBrowseState {
        let library = AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        };
        let mut state = AudiobookshelfBrowseState::new(library);
        state.append_page(0, 20, 1, vec![show]);
        state.select(0);
        state
    }

    /// Independent oracle reproducing the pre-extraction
    /// `App::audiobookshelf_hero_content_rows` body, so the shared helper is
    /// proved equivalent to the legacy rule it replaces (author, long
    /// description, episode, and image cases all shift the budget exactly as
    /// before).
    fn legacy_hero_content_rows(
        state: &AudiobookshelfBrowseState,
        interaction: PodcastInteraction,
        width: u16,
        images_enabled: bool,
    ) -> u16 {
        let title_rows = HERO_TITLE_ROWS;
        let author_rows = state
            .selected_show()
            .and_then(|show| show.author.as_ref())
            .is_some() as u16;
        let mut rows = title_rows + author_rows;
        if let Some(description) = state
            .selected_show()
            .and_then(|show| show.description.as_deref())
            .filter(|description| !description.is_empty())
        {
            rows += 1;
            let (image_width, image_height) = if images_enabled {
                (SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS)
            } else {
                (0, 0)
            };
            let description_start = title_rows + author_rows + 1;
            rows += wrap_overview_lines(description, |line| {
                let row = description_start + line as u16;
                inline_hero_text_width(width, image_width, image_height, row) as usize
            })
            .len()
            .min(4) as u16;
        }
        if interaction.episode_focused {
            rows += 1 + SERIES_DETAIL_DIVIDER_ROWS as u16;
            rows += state
                .episodes
                .as_ref()
                .map(|_| state.visible_episodes(interaction.episode_filter).len())
                .unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE) as u16;
        }
        rows += SERIES_DETAIL_TRAILING_BLANK_ROWS as u16;
        if images_enabled {
            rows = rows.max(SERIES_IMAGE_ROWS + 1);
        }
        rows
    }

    fn assert_matches_legacy(
        state: &AudiobookshelfBrowseState,
        interaction: PodcastInteraction,
        width: u16,
        images_enabled: bool,
    ) {
        let got = podcast_hero_content_rows(state, interaction, width, images_enabled);
        let expected = legacy_hero_content_rows(state, interaction, width, images_enabled);
        assert_eq!(got, expected, "shared helper must match legacy rule");
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_author_only() {
        let state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: Some("Author".into()),
            description: None,
            cover_path: None,
        });
        // title(1) + author(1) + trailing(1) = 3; no image minimum.
        assert_eq!(
            podcast_hero_content_rows(&state, interaction(false), 40, false),
            3
        );
        assert_matches_legacy(&state, interaction(false), 40, false);
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_long_description() {
        let state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: Some("Author".into()),
            description: Some("word ".repeat(80)),
            cover_path: None,
        });
        assert_matches_legacy(&state, interaction(false), 40, false);
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_episodes() {
        let mut state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: None,
            description: None,
            cover_path: None,
        });
        state.episodes = Some(vec![
            AudiobookshelfDownloadedEpisode {
                library_item_id: "s".into(),
                episode_id: "e1".into(),
                title: "E1".into(),
                published_at: None,
                duration_seconds: None,
            },
            AudiobookshelfDownloadedEpisode {
                library_item_id: "s".into(),
                episode_id: "e2".into(),
                title: "E2".into(),
                published_at: None,
                duration_seconds: None,
            },
            AudiobookshelfDownloadedEpisode {
                library_item_id: "s".into(),
                episode_id: "e3".into(),
                title: "E3".into(),
                published_at: None,
                duration_seconds: None,
            },
        ]);
        assert_matches_legacy(&state, interaction(true), 40, false);
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_images_minimum() {
        let state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: None,
            description: None,
            cover_path: None,
        });
        // Images enabled lifts even a title-only budget to SERIES_IMAGE_ROWS+1.
        assert_eq!(
            podcast_hero_content_rows(&state, interaction(false), 40, true),
            SERIES_IMAGE_ROWS + 1
        );
        assert_matches_legacy(&state, interaction(false), 40, true);
    }
}
