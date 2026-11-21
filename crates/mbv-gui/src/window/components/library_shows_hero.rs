//! The Shows Hero (`replace-tauri-with-blitz` task 7.6, the port of
//! `LibraryShowsHero`; add-gui-library-panel design D12, D14, D15):
//! the Music Hero's frame (`LibraryHeroFrame`) over the series, as the TUI
//! paints the TV and Music heroes through one hero and one Workspace. The
//! header band is the series — its genre, years, season count, and network —
//! and the detail block is the series' Primary poster beside the selected
//! season and the series' overview. The Workspace is the list band: the
//! season dropdown, with the core's selected season as its label, and that season's
//! episodes as rows. The Latest mode's Hero is the selected episode's facts
//! and carries no Workspace, and the Upcoming mode carries no Hero at all, so
//! the pane paints nothing there.
//!
//! The Workspace keeps no cursor (`gui-pointer-target-interaction` D6): the keys
//! act on the row the pointer marks, a press on a row does nothing, a
//! double-click plays the row, and the hovered row paints its play-next and
//! add-to-queue controls. Every action sends the episode's stable Service id
//! with the library index the frame carries; no control names a row position.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{
    GuiHomeRowStateFrame, GuiLibraryDetail, GuiShowsDetail, GuiShowsSeasonPillFrame,
    GuiShowsWorkspaceFrame,
};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::components::library_dropdown::{
    LibraryDropdown, LibraryDropdownMarker, LibraryDropdownRow,
};
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_frame::{HeroBlock, HeroHeader, LibraryHeroFrame};
use crate::window::components::library_workspace_table::{
    LibraryWorkspaceTable, WorkspaceAction, WorkspaceActionIcon, WorkspaceLoading, WorkspaceRow,
    WorkspaceRowState,
};
use crate::window::library::episode::{episode_progress, is_played_episode};
use crate::window::library::hero::{
    HeroLayoutFacts, hero_layout, join_meta_rows, label_segments, plain_segments,
};
use crate::window::library::list::filter_match;
use crate::window::library::parts::library_shows_detail;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerList;
use crate::window::sections::SectionRef;

/// The props of the Shows Hero (the `LibraryShowsHero` props, narrowed
/// to what the Hero reads): the library index the Shows commands name, and
/// the carried Shows detail — the selected row's Hero and its Workspace.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryShowsHeroProps {
    /// The Shows library index the Shows commands name (design D2, D12).
    pub lib_idx: usize,
    /// The selected row's Hero facts, its Workspace, and the selection the
    /// Latest Hero's actions name, as the one handle every reader clones
    /// (`share-gui-sections-by-rc` design D3).
    pub detail: SectionRef<GuiLibraryDetail>,
}

/// The Shows action a Hero button sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShowsAction {
    /// Play the target item.
    Play,
    /// Play the target item next in the queue.
    PlayNext,
    /// Add the target item to the queue.
    Queue,
}

/// Send a Shows command for `item_id` (design D12).
fn send(library: &LibraryState, command: ShowsAction, lib_idx: usize, item_id: &str) {
    let command = match command {
        ShowsAction::Play => GuiLibraryCommand::ShowsPlay {
            lib_idx,
            item_id: item_id.to_owned(),
        },
        ShowsAction::PlayNext => GuiLibraryCommand::ShowsPlayNext {
            lib_idx,
            item_id: item_id.to_owned(),
        },
        ShowsAction::Queue => GuiLibraryCommand::ShowsQueue {
            lib_idx,
            item_id: item_id.to_owned(),
        },
    };
    library.link.library(command);
}

/// Select a season (design D12): the select-season command the core answers
/// with that season's episodes.
fn select_season(library: &LibraryState, lib_idx: usize, season_id: &str) {
    library.link.library(GuiLibraryCommand::ShowsSelectSeason {
        lib_idx,
        season_id: season_id.to_owned(),
    });
}

/// Play an episode (design D12): the double-click, as in Music's tracklist.
/// The Owner owns the queue, so the shared play path replaces it.
fn play_episode(library: &LibraryState, lib_idx: usize, episode_id: &str) {
    send(library, ShowsAction::Play, lib_idx, episode_id);
}

/// Admit an episode directly after the active slot (design D12).
fn play_next_episode(library: &LibraryState, lib_idx: usize, episode_id: &str) {
    send(library, ShowsAction::PlayNext, lib_idx, episode_id);
}

/// Append an episode to the queue's tail (design D12).
fn queue_episode(library: &LibraryState, lib_idx: usize, episode_id: &str) {
    send(library, ShowsAction::Queue, lib_idx, episode_id);
}

/// The item the Hero's buttons act on (Q6 of the Hero look audit): the
/// selected season on a series Hero, and the episode on a Latest Hero. A
/// season is a folder, so the shared folder paths expand it.
fn action_target(detail: &GuiShowsDetail) -> Option<String> {
    match detail
        .hero
        .as_ref()
        .and_then(|hero| hero.workspace.as_ref())
    {
        None => detail.selected.clone(),
        Some(workspace) => workspace.selected_season.clone(),
    }
}

/// Send a Shows command for the Hero's target; nothing while it names none.
fn act_on(library: &LibraryState, command: ShowsAction, lib_idx: usize, target: Option<&str>) {
    let Some(item_id) = target else {
        return;
    };
    send(library, command, lib_idx, item_id);
}

/// The row's played state: played, active with a
/// resume percent, or ordinary.
fn row_state(state: &GuiHomeRowStateFrame) -> WorkspaceRowState {
    if is_played_episode(state) {
        return WorkspaceRowState::Played;
    }
    if episode_progress(state).is_none() {
        return WorkspaceRowState::Ordinary;
    }
    WorkspaceRowState::Active
}

/// The season rows the open dropdown paints under `filter`: the seasons whose
/// name matches by the Library filter's rule, in the core's order, with the
/// core's selected season marked.
fn season_rows(seasons: &[GuiShowsSeasonPillFrame], filter: &str) -> Vec<LibraryDropdownRow> {
    seasons
        .iter()
        .filter(|season| filter_match(&season.name, filter))
        .map(|season| LibraryDropdownRow {
            key: season.season_id.clone(),
            label: season.name.clone(),
            selected: season.selected,
        })
        .collect()
}

/// The season dropdown (user request 2026-11-20, replacing one pill per
/// season): the shared Library dropdown, leading the Workspace band, with the
/// core's selected season as its label and one radio row per season. A series
/// can hold many seasons, so the open popover carries a filter field. A row
/// press sends the select-season command and closes the popover.
fn paint_season_dropdown(
    seasons: &[GuiShowsSeasonPillFrame],
    lib_idx: usize,
    library: &LibraryState,
) -> Element {
    let filter = library.ui.cloned().shows_seasons;
    let open = filter.is_some();
    let rows = season_rows(seasons, filter.as_deref().unwrap_or_default());
    let label = seasons
        .iter()
        .find(|season| season.selected)
        .map_or_else(|| "Season".to_owned(), |season| season.name.clone());

    let toggle_library = library.clone();
    let on_toggle = Callback::new(move |()| toggle_library.toggle_shows_seasons_dropdown());
    let inside_library = library.clone();
    let on_inside_press = Callback::new(move |()| {
        inside_library.close_library_row_menu();
        inside_library.close_shows_view_dropdown();
    });
    let filter_library = library.clone();
    let on_filter =
        Callback::new(move |text: String| filter_library.set_shows_seasons_filter(text));
    let select_library = library.clone();
    let on_select = Callback::new(move |season_id: String| {
        select_season(&select_library, lib_idx, &season_id);
        select_library.close_shows_seasons_dropdown();
    });

    rsx! {
        LibraryDropdown {
            label,
            open,
            rows,
            marker: LibraryDropdownMarker::Radio,
            aria_label: "Seasons",
            trigger_attr: "data-library-shows-seasons-trigger",
            menu_attr: "data-library-shows-seasons-menu",
            row_attr: "data-library-shows-season",
            mark_attr: "data-library-shows-season-mark",
            filter,
            filter_placeholder: Some("Find a season"),
            filter_attr: Some("data-library-shows-seasons-filter"),
            on_toggle,
            on_select,
            on_filter,
            on_inside_press,
            align_end: false,
        }
    }
}

/// The Shows Hero: the frame
/// over the series or the Latest episode, with the season pills and the
/// selected season's episodes as the Workspace.
#[component]
pub fn LibraryShowsHero(props: LibraryShowsHeroProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The Shows part of the carried detail, narrowed from the handle in this
    // body, and the Hero's facts, present exactly while the pane paints.
    let Some(detail) = library_shows_detail(Some(&props.detail)) else {
        return rsx! {};
    };
    let Some(hero) = detail.hero.as_ref() else {
        return rsx! {};
    };

    // The series Hero's Workspace, or `None` on a Hero that paints none.
    let workspace: Option<&GuiShowsWorkspaceFrame> = hero.workspace.as_ref();

    // The selected season's name: the detail block's name, as the album's is
    // Music's. Empty on a Hero that paints no Workspace.
    let season_name = workspace
        .and_then(|workspace| workspace.seasons.iter().find(|season| season.selected))
        .map(|season| season.name.clone())
        .unwrap_or_default();

    // The selected season's episode count, once its episodes have landed.
    let season_facts = match workspace {
        Some(workspace) if !detail.workspace_loading && !workspace.episodes.is_empty() => {
            let count = workspace.episodes.len();
            format!(
                "{count} {}",
                if count == 1 { "episode" } else { "episodes" }
            )
        }
        _ => String::new(),
    };

    // A Latest episode's header and block (invariant 30): the series is the
    // largest unit and titles the header, and the episode is the block's
    // name. `None` on the series Hero, whose Workspace holds the seasons.
    let episode_layout = workspace.is_none().then(|| {
        let kicker = if hero.parent.as_deref().unwrap_or("").is_empty() {
            "episode"
        } else {
            "series"
        };
        hero_layout(
            &HeroLayoutFacts {
                title: &hero.title,
                parent: hero.parent.as_deref(),
                meta_rows: &hero.meta_rows,
                duration_row: hero.duration_row,
                progress_row: hero.progress_row,
                overview: hero.overview.as_deref(),
            },
            kicker,
        )
    });

    let (header, block) = match &episode_layout {
        Some(layout) => (
            HeroHeader::from(layout.header.clone()),
            HeroBlock::from(layout.block.clone()),
        ),
        None => (
            HeroHeader {
                kicker: label_segments("series"),
                title: hero.title.clone(),
                facts: join_meta_rows(&hero.meta_rows),
            },
            HeroBlock {
                label: hero.title.clone(),
                eyebrow: None,
                name: Some(season_name),
                facts: Some(plain_segments(&season_facts)),
                meta: Vec::new(),
                overview: hero.overview.clone(),
            },
        ),
    };

    // The Workspace table's rows: each episode with its played state and its
    // resume percent.
    let rows: Vec<WorkspaceRow> = workspace.map_or_else(Vec::new, |workspace| {
        workspace
            .episodes
            .iter()
            .map(|episode| WorkspaceRow {
                id: episode.episode_id.clone(),
                number: episode.number.to_string(),
                name: episode.name.clone(),
                duration: episode.duration.clone(),
                playing: false,
                state: Some(row_state(&episode.state)),
                progress: episode_progress(&episode.state).map(f32::from),
            })
            .collect()
    });

    // The actions' sends (design D12): the item the Hero's buttons act on.
    let lib_idx = props.lib_idx;
    let action_target = action_target(detail);
    let play_library = library.clone();
    let play_target = action_target.clone();
    let on_play = Callback::new(move |()| {
        act_on(
            &play_library,
            ShowsAction::Play,
            lib_idx,
            play_target.as_deref(),
        );
    });
    let next_library = library.clone();
    let next_target = action_target.clone();
    let on_play_next = Callback::new(move |()| {
        act_on(
            &next_library,
            ShowsAction::PlayNext,
            lib_idx,
            next_target.as_deref(),
        );
    });
    let queue_library = library.clone();
    let queue_target = action_target.clone();
    let on_queue = Callback::new(move |()| {
        act_on(
            &queue_library,
            ShowsAction::Queue,
            lib_idx,
            queue_target.as_deref(),
        );
    });
    let actions = rsx! {
        LibraryHeroActions {
            attr: "shows",
            on_play: Some(on_play),
            on_play_next: Some(on_play_next),
            on_queue: Some(on_queue),
        }
    };

    // The Workspace's press and play (design D12, D6): a press on a row does
    // nothing — the Workspace keeps no cursor — and the row's play gestures send
    // that episode.
    let on_press = Callback::new(|_: String| {});
    let play_library = library.clone();
    let on_play = Callback::new(move |episode_id: String| {
        play_episode(&play_library, lib_idx, &episode_id);
    });

    // The row's hover actions (design D12).
    let next_library = library.clone();
    let queue_library = library.clone();
    let row_actions = vec![
        WorkspaceAction {
            key: "playNext".to_owned(),
            label: "Play next".to_owned(),
            icon: WorkspaceActionIcon::ListEnd,
            run: Callback::new(move |episode_id: String| {
                play_next_episode(&next_library, lib_idx, &episode_id);
            }),
        },
        WorkspaceAction {
            key: "queue".to_owned(),
            label: "Add to queue".to_owned(),
            icon: WorkspaceActionIcon::Plus,
            run: Callback::new(move |episode_id: String| {
                queue_episode(&queue_library, lib_idx, &episode_id);
            }),
        },
    ];

    // The series Hero's Workspace (design D12): the season pills, then the
    // selected season's episodes.
    let loading = detail.workspace_loading.then(|| WorkspaceLoading {
        text: "Loading episodes…".to_owned(),
        attr: "data-library-shows-episodes-loading",
    });
    let workspace_library = library.clone();
    let children = workspace.map(move |workspace| {
        rsx! {
            section {
                class: "flex flex-col",
                style: "gap: 10px;",
                "data-library-shows-workspace": "",
                { paint_season_dropdown(&workspace.seasons, lib_idx, &workspace_library) }
                LibraryWorkspaceTable {
                    root: "data-library-shows-episodes",
                    label: "Episodes",
                    hook: "shows-episode",
                    noun: "Episode",
                    pointer_list: PointerList::Workspace,
                    rows,
                    actions: row_actions,
                    on_press,
                    on_play,
                    loading,
                }
            }
        }
    });

    rsx! {
        LibraryHeroFrame {
            attr: "shows",
            root: "data-library-shows-hero",
            header,
            block,
            art_shape: hero.art_shape,
            art: hero.art.clone(),
            art_title: hero.title.clone(),
            actions: Some(actions),
            children,
        }
    }
}

#[cfg(test)]
mod tests;
