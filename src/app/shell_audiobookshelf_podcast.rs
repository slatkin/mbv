use super::components::{AudiobookshelfPodcastComponent, BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_audiobookshelf_podcast_episode_intent(
        &mut self,
        intent: super::components::msg::PodcastEpisodeIntent,
    ) {
        // Resolve every condition from current App state/layout at the Model
        // boundary, never from component state (D17). The shell arm re-projects
        // podcast content after this call, preserving the existing effect plus
        // post-request push behavior.
        let Some(index) = self.app.tab.audiobookshelf_index() else {
            return;
        };
        // The episode target is the mounted component's authoritative selection
        // (task 5.3d.11 U5), read through the U0 accessor. The App mirror is
        // neither read nor written here: a later FocusOrPlay/Space resolves the
        // explicit target instead of re-entering selection.
        // The episode index is an index into the component's *filtered* view,
        // so the resolver needs the component-owned filter too
        // (split-browse-state-interaction-fields task 3.2).
        let (episode, filter) = self
            .abs_podcast_component_mut(index)
            .map(|component| (component.episode_selection(), component.episode_filter()))
            .unwrap_or((None, Default::default()));
        match intent {
            super::components::msg::PodcastEpisodeIntent::FocusOrPlay => {
                if let Some(episode_index) = episode {
                    self.app
                        .play_selected_audiobookshelf_episode(index, episode_index, filter);
                } else if let Some(component) = self.abs_podcast_component_mut(index) {
                    // Entering episode selection is re-homed onto the mounted
                    // component (task 5.3d.11 U2); the post-request projection
                    // preserves this selection through `set_content`.
                    component.set_episode_selection(Some(0));
                }
            }
            super::components::msg::PodcastEpisodeIntent::OpenOrPlay => {
                if let Some(episode_index) = episode {
                    self.app
                        .play_selected_audiobookshelf_episode(index, episode_index, filter);
                } else if self.app.layout.main.is_wide_podcast_active() {
                    if let Some(component) = self.abs_podcast_component_mut(index) {
                        // Re-homed onto the mounted component (task 5.3d.11 U2),
                        // same as FocusOrPlay.
                        component.set_episode_selection(Some(0));
                    }
                } else {
                    self.open_podcast_selection_modal();
                }
            }
            super::components::msg::PodcastEpisodeIntent::Enqueue => {
                if let Some(episode_index) = episode {
                    self.app
                        .enqueue_selected_audiobookshelf_episode(index, episode_index, filter);
                }
            }
        }
    }

    /// Resolves and downcasts the mounted Audiobookshelf podcast browser for
    /// the given browse index, or `None` when it is not the active mounted
    /// browser (task 5.3d.11 U0). The mount path keys the single mounted
    /// browser on the active tab, so the component exists only when `index`
    /// matches that tab.
    pub(super) fn abs_podcast_component_mut(
        &mut self,
        index: usize,
    ) -> Option<&mut AudiobookshelfPodcastComponent> {
        let active_index = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index) => index,
            _ => return None,
        };
        if active_index != index {
            return None;
        }
        let id = self.abs_podcast_id.as_ref()?;
        self.application.get_component_mut(id).and_then(|comp| {
            comp.as_any_mut()
                .downcast_mut::<AudiobookshelfPodcastComponent>()
        })
    }

    /// Mounts / unmounts the Audiobookshelf podcast browser component to follow
    /// the active tab (task 5.3d). This is the mount lifecycle only: content is
    /// no longer mirrored into the component on every tick. The per-frame
    /// `set_content` projection was replaced by the event-scoped
    /// `push_audiobookshelf_podcast_content` at the writers of its projected
    /// inputs (active-tab, async completion, progress, refresh/reset, and
    /// saved-position restore). Content is pushed right after a fresh mount so
    /// the newly mounted component paints the current browse snapshot.
    pub(super) fn sync_audiobookshelf_podcast(&mut self) {
        let next_id = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index)
                if matches!(
                    self.app.audiobookshelf_kind_at(index),
                    Some(AudiobookshelfBrowseKind::Podcast)
                ) =>
            {
                let library = self.app.audiobookshelf_libraries.get(index);
                library.map(|library| {
                    ComponentId::Browser(BrowserKey {
                        service: ServiceKind::Audiobookshelf,
                        library_id: library.id.clone(),
                        kind: BrowserKind::AudiobookshelfPodcast,
                    })
                })
            }
            _ => None,
        };
        if self.abs_podcast_id != next_id {
            match next_id {
                Some(id) => {
                    if !self.application.mounted(&id) {
                        self.application
                            .mount(
                                id.clone(),
                                Box::new(AudiobookshelfPodcastComponent::new()),
                                vec![],
                            )
                            .expect("mount Audiobookshelf podcast browser");
                        self.register_destination(&id);
                    }
                    self.abs_podcast_id = Some(id);
                    // Re-point: project the active tab's browse state so the
                    // component paints the current shows/selection (the
                    // active-tab writer); keep-mounted preserves its private
                    // selection across the switch.
                    self.push_audiobookshelf_podcast_content();
                }
                None => {
                    self.abs_podcast_id = None;
                }
            }
        }
    }

    /// Event-scoped projection replacing the per-frame content mirror (task
    /// 5.3d.11 U6): runs only when the active tab is the mounted podcast
    /// browser and mirrors the validated browse snapshot plus panel focus into
    /// `AudiobookshelfPodcastComponent` via `set_content` (preserving its
    /// selected-show/episode/scroll/bucket semantics exactly), then runs the
    /// cover-fetch bridge (task 5.3d.9) for the selected show's cover.
    /// Called at the writers of the projected inputs, so it is deterministic
    /// in `App` state and duplicate pushes are idempotent.
    /// `sync_audiobookshelf_podcast` keeps only mount lifecycle management.
    pub(super) fn push_audiobookshelf_podcast_content(&mut self) {
        let Some(id) = self.abs_podcast_id.as_ref() else {
            return;
        };
        // Mirror `sync_audiobookshelf_podcast`'s active-tab guard: only
        // project while this tab's browse kind is still Podcast, so a stale
        // mounted podcast component never receives a non-Podcast snapshot
        // before mount reconciliation (task 5.3d).
        let index = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index)
                if matches!(
                    self.app.audiobookshelf_kind_at(index),
                    Some(AudiobookshelfBrowseKind::Podcast)
                ) =>
            {
                index
            }
            _ => return,
        };
        let Some(snapshot) = self.app.audiobookshelf_browse.get(index) else {
            return;
        };
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(podcast) = comp
                .as_any_mut()
                .downcast_mut::<AudiobookshelfPodcastComponent>()
            {
                podcast.set_content(snapshot, focused, self.app.images_enabled());
            }
        }
        // Cover-fetch bridge (task 5.3d.9): the selected show's cover was
        // previously fetched as an unconditional side effect inside the legacy
        // underpaint renderer. It now runs under this projection, preserving
        // the image-disabled gate (no fetch when images are off).
        if self.app.images_enabled() {
            let server = self
                .app
                .config
                .lock()
                .unwrap()
                .audiobookshelf_setup
                .as_ref()
                .map(|setup| setup.server_url.clone());
            if let Some(server) = server {
                if let Some(show) = self
                    .app
                    .audiobookshelf_browse
                    .get(index)
                    .and_then(|state| state.selected_show())
                {
                    self.app
                        .fetch_audiobookshelf_cover(server, show.library_item_id.clone());
                }
            }
        }
    }

    pub(super) fn render_audiobookshelf_podcast_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.abs_podcast_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.audiobookshelf_podcast_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
        // Component owns painting; read back its painted geometry so the
        // still-required legacy `LayoutMain` readers (interaction wide/narrow
        // gating via `is_wide_podcast_active`, library column count, and
        // overlay/menu anchors) stay correct once the legacy underpaint
        // renderer is removed (task 5.3d.10d). Wide keeps a nonzero right
        // area; narrow resets it to zero, including the wide-to-narrow
        // redraw. The legacy `render_audiobookshelf_podcasts` still projects
        // the same values this frame, so this mirror is idempotent until
        // 5.3d.10e detaches it.
        let projection = self
            .application
            .get_component_mut(id)
            .and_then(|comp| {
                comp.as_any_mut()
                    .downcast_mut::<AudiobookshelfPodcastComponent>()
            })
            .map(|component| {
                let image_paint = component.take_image_paint();
                let geometry = component.geometry();
                (
                    image_paint,
                    geometry.right_area,
                    geometry.list_area,
                    geometry.hero_area,
                    geometry.inline_hero_area,
                    geometry.selected_item_rect,
                    geometry.selector_tabs.clone(),
                )
            });
        if let Some((
            image_paint,
            right_area,
            list_area,
            hero_area,
            inline_hero_area,
            selected_item_rect,
            selector_tabs,
        )) = projection
        {
            self.app.paint_home_image(frame, image_paint);
            self.app.layout.main.audiobookshelf_podcast_right_area = right_area;
            self.app.layout.main.left_area = list_area;
            self.app.layout.main.hero_area = hero_area;
            self.app.layout.main.inline_hero_area = inline_hero_area;
            self.app.layout.main.selected_item_rect = selected_item_rect;
            self.app.layout.main.selector_tabs = selector_tabs;
        }
    }
}

#[cfg(test)]
#[path = "shell_audiobookshelf_podcast_tests.rs"]
mod tests;
