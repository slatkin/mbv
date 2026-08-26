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
        let episode_selection = self
            .app
            .audiobookshelf_browse
            .get(index)
            .is_some_and(|state| state.episode_selection.is_some());
        match intent {
            super::components::msg::PodcastEpisodeIntent::FocusOrPlay => {
                if episode_selection {
                    self.app.play_selected_audiobookshelf_episode(index);
                } else {
                    self.app.enter_audiobookshelf_episode_selection();
                }
            }
            super::components::msg::PodcastEpisodeIntent::OpenOrPlay => {
                if episode_selection {
                    self.app.play_selected_audiobookshelf_episode(index);
                } else if self.app.layout.main.is_wide_podcast_active() {
                    self.app.enter_audiobookshelf_episode_selection();
                } else {
                    self.app.open_podcast_selection_modal();
                }
            }
            super::components::msg::PodcastEpisodeIntent::Enqueue => {
                if episode_selection {
                    self.app.enqueue_selected_audiobookshelf_episode(index);
                }
            }
        }
    }

    fn abs_podcast_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.audiobookshelf_libraries.get(index)?;
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: library.id.clone(),
            kind: BrowserKind::AudiobookshelfPodcast,
        }))
    }

    /// Mounts / unmounts the Audiobookshelf podcast browser component to follow
    /// the active tab (task 5.3d). This is the mount lifecycle only: content is
    /// no longer mirrored into the component on every tick. The per-frame
    /// `set_content` projection was replaced by the event-scoped
    /// `push_audiobookshelf_podcast_content` at the writers of its projected
    /// inputs (active-tab, key/effect, async completion, progress,
    /// refresh/reset, and saved-position restore). Content is pushed right
    /// after a fresh mount so the newly mounted component paints the current
    /// browse snapshot.
    pub(super) fn sync_audiobookshelf_podcast(&mut self) {
        let next_id = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index)
                if matches!(
                    self.app.audiobookshelf_kind_at(index),
                    Some(AudiobookshelfBrowseKind::Podcast)
                ) =>
            {
                self.abs_podcast_component_id(index)
            }
            _ => None,
        };
        if self.abs_podcast_id != next_id {
            if let Some(id) = self.abs_podcast_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(
                        id.clone(),
                        Box::new(AudiobookshelfPodcastComponent::new()),
                        vec![],
                    )
                    .expect("mount Audiobookshelf podcast browser");
                self.application
                    .active(&id)
                    .expect("activate Audiobookshelf podcast browser");
                self.abs_podcast_id = Some(id);
                // Fresh mount: project the active tab's browse state so the
                // component is initialized with the current shows/selection
                // before it is painted (the active-tab writer).
                self.push_audiobookshelf_podcast_content();
            }
        }
    }

    /// Event-scoped projection replacing the per-frame content mirror (task
    /// 5.3d, `sync_audiobookshelf_podcast` Phase A): runs only when the active
    /// tab is the mounted podcast browser and mirrors the validated browse
    /// snapshot plus panel focus into `AudiobookshelfPodcastComponent` via
    /// `set_content` (preserving its selected-show/episode/scroll semantics
    /// exactly). Called at the writers of the projected inputs, so it is
    /// deterministic in `App` state and duplicate pushes are idempotent.
    /// `sync_audiobookshelf_podcast` keeps only mount lifecycle management.
    pub(super) fn push_audiobookshelf_podcast_content(&mut self) {
        let Some(id) = self.abs_podcast_id.as_ref() else {
            return;
        };
        let index = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index) => index,
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
        // underpaint renderer. It now runs here, at the event-scoped content
        // push that every writer seam already invokes, so the image-disabled
        // gate is preserved and no fetch is scheduled when images are off.
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
mod tests {
    use super::*;
    use crate::app::components::msg::{
        PodcastEpisodeIntent, PodcastEpisodeTransition, PodcastShowMove,
    };
    use crate::app::components::{Msg, ShellRequest};
    use crate::app::render::HomeImagePaint;
    use crate::app::tests_podcast::audiobookshelf_app;
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState;
    use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfShow};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;
    use tuirealm::component::Component;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn abs_podcast_shell_mounts_and_routes_component() {
        let mut model = Model::new(audiobookshelf_app());
        model.app.audiobookshelf_browse[0].append_page(
            1,
            20,
            2,
            vec![AudiobookshelfShow {
                library_item_id: "show-b".into(),
                title: "Show B".into(),
                author: None,
                description: None,
                cover_path: None,
            }],
        );
        model.sync_audiobookshelf_podcast();
        let id = model
            .abs_podcast_id
            .clone()
            .expect("podcast component mounted");
        let message = model
            .application
            .get_component_mut(&id)
            .expect("podcast component")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(movement))) = message
        else {
            panic!("show movement should be routed as a typed show-move request");
        };
        assert_eq!(movement, PodcastShowMove::NextRow);
        // The shell arm maps NextRow onto the legacy row-stride move and
        // re-projects content (task 5.3d.5), preserving the App target.
        model.app.move_audiobookshelf_show_rows(1);
        model.push_audiobookshelf_podcast_content();
        assert_eq!(model.app.audiobookshelf_browse[0].cursor(), 1);
    }

    #[test]
    fn abs_podcast_shell_routes_episode_transition_to_app() {
        let mut model = Model::new(audiobookshelf_app());
        model.app.audiobookshelf_browse[0].episode_selection = Some(0);
        model.sync_audiobookshelf_podcast();
        let id = model
            .abs_podcast_id
            .clone()
            .expect("podcast component mounted");
        let message = model
            .application
            .get_component_mut(&id)
            .expect("podcast component")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeTransition(transition))) =
            message
        else {
            panic!("episode movement should be routed as a typed episode transition");
        };
        assert_eq!(transition, PodcastEpisodeTransition::NextEpisode);
        // The shell arm maps NextEpisode onto the legacy App episode-cursor
        // move and re-projects content (task 5.3d.6), preserving the App
        // episode target.
        model.app.move_audiobookshelf_episode_cursor(1);
        model.push_audiobookshelf_podcast_content();
        assert_eq!(
            model.app.audiobookshelf_browse[0].episode_selection,
            Some(0)
        );
    }

    #[test]
    fn abs_podcast_shell_routes_action_intent_to_app() {
        let mut model = Model::new(audiobookshelf_app());
        // FocusOrPlay with no episode selection enters episode selection at the
        // App boundary (task 5.3d.7).
        model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::FocusOrPlay);
        assert_eq!(
            model.app.audiobookshelf_browse[0].episode_selection,
            Some(0)
        );

        // With episode selection active, the enqueue intent reaches the App
        // enqueue seam (the default fixture has one downloaded episode) while
        // leaving the App episode selection untouched.
        model.app.audiobookshelf_browse[0].episode_selection = Some(0);
        let before = model.app.audiobookshelf_browse[0].episode_selection;
        model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::Enqueue);
        assert_eq!(model.app.audiobookshelf_browse[0].episode_selection, before);
        assert_eq!(model.app.player_tab.total_queue_len(), 1);
    }

    /// Synchronous image-plan lifecycle for the podcast component (task
    /// 5.3d.10b): images disabled yields no plan, images enabled with a
    /// selected show yields the correct `AudiobookshelfCover` id + nonzero
    /// paint rect, and a subsequent disabled view clears the plan with no
    /// stale paint. No network fetch or secret is scheduled.
    #[test]
    fn abs_podcast_component_image_plan_lifecycle() {
        let mut component = AudiobookshelfPodcastComponent::new();
        let show = AudiobookshelfShow {
            library_item_id: "show-paint".into(),
            title: "Show".into(),
            author: None,
            description: None,
            cover_path: None,
        };
        let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        });
        state.append_page(0, 20, 1, vec![show]);
        state.select(0);

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 100, 40);

        // Images disabled: no plan, even with a selected show.
        component.set_content(&state, true, false);
        terminal.draw(|frame| component.view(frame, area)).unwrap();
        assert!(component.take_image_paint().is_none());

        // Images enabled with a selected show: plan carries the show id and a
        // nonzero paint rect (mirrors the ABS Book component handoff).
        component.set_content(&state, true, true);
        terminal.draw(|frame| component.view(frame, area)).unwrap();
        let plan = component.take_image_paint();
        let Some(HomeImagePaint::AudiobookshelfCover {
            area: paint_area,
            library_item_id,
            show_placeholder,
        }) = plan
        else {
            panic!("expected AudiobookshelfCover plan when images enabled");
        };
        assert_eq!(library_item_id, "show-paint");
        assert!(
            paint_area.width > 0 && paint_area.height > 0,
            "paint rect must be nonzero"
        );
        assert!(
            show_placeholder,
            "podcast hero uses the placeholder cover path"
        );

        // A subsequent disabled view must clear the plan (no stale paint).
        component.set_content(&state, true, false);
        terminal.draw(|frame| component.view(frame, area)).unwrap();
        assert!(component.take_image_paint().is_none());
    }

    /// Shell projection (task 5.3d.10d): the component owns painting and
    /// computes its geometry during `view`; the shell mirrors the still-required
    /// fields into `LayoutMain` so legacy readers stay correct. Wide keeps a
    /// nonzero podcast right area; narrow resets it to zero, including the
    /// wide-to-narrow redraw.
    #[test]
    fn abs_podcast_shell_projection_wide_narrow_right_area() {
        let backend = TestBackend::new(200, 50);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut model = Model::new(audiobookshelf_app());
        model.sync_audiobookshelf_podcast();
        model.push_audiobookshelf_podcast_content();

        // Wide presentation: area clears the two-column breakpoint and height.
        let wide = Rect::new(0, 0, 200, 50);
        model.app.layout.main.audiobookshelf_podcast_area = wide;
        terminal
            .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
            .unwrap();
        assert!(
            model
                .app
                .layout
                .main
                .audiobookshelf_podcast_right_area
                .width
                > 0
                && model
                    .app
                    .layout
                    .main
                    .audiobookshelf_podcast_right_area
                    .height
                    > 0,
            "wide podcast right area must stay nonzero"
        );

        // Narrow presentation: below the two-column width threshold.
        let narrow = Rect::new(0, 0, 60, 50);
        model.app.layout.main.audiobookshelf_podcast_area = narrow;
        terminal
            .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
            .unwrap();
        assert_eq!(
            model.app.layout.main.audiobookshelf_podcast_right_area,
            Rect::default(),
            "narrow podcast right area must reset to zero"
        );

        // Wide -> narrow redraw must also clear the right area.
        model.app.layout.main.audiobookshelf_podcast_area = wide;
        terminal
            .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
            .unwrap();
        assert!(
            model
                .app
                .layout
                .main
                .audiobookshelf_podcast_right_area
                .width
                > 0,
            "wide->narrow must re-establish a nonzero right area"
        );
        model.app.layout.main.audiobookshelf_podcast_area = narrow;
        terminal
            .draw(|frame| model.render_audiobookshelf_podcast_component(frame))
            .unwrap();
        assert_eq!(
            model.app.layout.main.audiobookshelf_podcast_right_area,
            Rect::default(),
            "wide->narrow redraw must reset the right area to zero"
        );
    }
}
