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
        let episode = self
            .abs_podcast_component_mut(index)
            .and_then(|component| component.episode_selection());
        match intent {
            super::components::msg::PodcastEpisodeIntent::FocusOrPlay => {
                if let Some(episode_index) = episode {
                    self.app
                        .play_selected_audiobookshelf_episode(index, episode_index);
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
                        .play_selected_audiobookshelf_episode(index, episode_index);
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
                        .enqueue_selected_audiobookshelf_episode(index, episode_index);
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
    /// the active tab (task 5.3d), then projects current content into it so the
    /// mounted component paints the active browse snapshot on every sync (task
    /// 5.3d.11 U1). Mount/unmount keeps the existing lifecycle: inactive or
    /// non-podcast tabs leave the component unmounted and the projection is a
    /// no-op. Sync subsumes the old event-scoped explicit content push
    /// (deleted), so shell writers no longer push explicitly.
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
            }
        }
        // Post-mount projection (task 5.3d.11 U1): the complete former
        // explicit content-push body, run every sync (not only fresh mount) so
        // a mounted podcast component tracks current content.
        // When no podcast browser is mounted (inactive/non-podcast tab) this is
        // a no-op, preserving the existing unmount behavior.
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
        // underpaint renderer. It now runs here, under the post-mount projection
        // every sync, preserving the image-disabled gate (no fetch when images
        // are off).
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
        assert_eq!(model.app.audiobookshelf_browse[0].cursor(), 1);
    }

    #[test]
    fn abs_podcast_shell_routes_episode_transition_to_app() {
        let mut model = Model::new(audiobookshelf_app());
        // Two episodes so a Down move has a real second target.
        model.app.audiobookshelf_browse[0].episodes = Some(vec![
            mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
                library_item_id: "show-a".into(),
                episode_id: "episode-a".into(),
                title: "Episode A".into(),
                published_at: None,
                duration_seconds: None,
            },
            mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode {
                library_item_id: "show-a".into(),
                episode_id: "episode-b".into(),
                title: "Episode B".into(),
                published_at: None,
                duration_seconds: None,
            },
        ]);
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
        // The mounted component owns episode selection; NextEpisode already
        // moved its own selection into the second row. Assert from the
        // component accessor, not the App mirror, since the legacy App move
        // handler is removed (5.3d.11 U2) and only the shell re-projection
        // keeps the two in sync (D17).
        let component = model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted");
        assert_eq!(component.episode_selection(), Some(1));
    }

    #[test]
    fn abs_podcast_shell_routes_action_intent_to_app() {
        let mut model = Model::new(audiobookshelf_app());
        // FocusOrPlay with no episode selection enters episode selection on the
        // mounted component (task 5.3d.11 U2), so mount it first and read the
        // selection back through the U0 accessor.
        model.sync_audiobookshelf_podcast();
        model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::FocusOrPlay);
        assert_eq!(
            model
                .abs_podcast_component_mut(0)
                .expect("podcast component mounted")
                .episode_selection(),
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

    /// U5 regression: playback target resolution reads the mounted component's
    /// authoritative episode selection through the U0 accessor, never the App
    /// `episode_selection` mirror. When the component selection is present but
    /// the App mirror is stale (None), FocusOrPlay must still resolve the
    /// component-selected episode into a real play attempt rather than
    /// re-entering episode selection. Without an eligible owner the play is
    /// inert, surfacing the owner-unavailable status as the observable.
    #[test]
    fn abs_podcast_focus_play_uses_component_selection_not_stale_app_mirror() {
        let mut model = Model::new(audiobookshelf_app());
        crate::app::tests_podcast::add_emby_movie_library(&mut model.app);
        // Project a selection into the mounted component, then stale the mirror
        // to None so only the component owns the resolved target.
        model.app.audiobookshelf_browse[0].episode_selection = Some(0);
        model.sync_audiobookshelf_podcast();
        model.app.audiobookshelf_browse[0].episode_selection = None;
        let id = model
            .abs_podcast_id
            .clone()
            .expect("podcast component mounted");
        model
            .application
            .get_component_mut(&id)
            .expect("podcast component");

        // A real FocusOrPlay through the Model handler must not re-enter
        // selection: the component's owned selection resolves the play target,
        // which is inert here only because the owner is unavailable.
        model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::FocusOrPlay);
        assert!(
            model
                .app
                .status
                .contains("Audiobookshelf playback owner is unavailable"),
            "component-resolved FocusOrPlay must attempt playback, not re-enter selection"
        );
        assert_eq!(
            model
                .abs_podcast_component_mut(0)
                .expect("podcast component mounted")
                .episode_selection(),
            Some(0),
            "component selection must remain the resolved episode target"
        );
        assert_eq!(
            model.app.player_tab.total_queue_len(),
            0,
            "inert play attempt must not enqueue"
        );
        assert_eq!(
            model.app.audiobookshelf_browse[0].episode_selection, None,
            "the stale App mirror must not be written or read for the target"
        );
    }

    /// U5 regression: Enqueue resolves the component-owned episode index the
    /// same way, editing the Composed queue even when the App mirror is stale.
    #[test]
    fn abs_podcast_enqueue_uses_component_selection_over_stale_app_mirror() {
        let mut model = Model::new(audiobookshelf_app());
        model.app.audiobookshelf_browse[0].episode_selection = Some(0);
        model.sync_audiobookshelf_podcast();
        model.app.audiobookshelf_browse[0].episode_selection = None;

        model.handle_audiobookshelf_podcast_episode_intent(PodcastEpisodeIntent::Enqueue);
        assert_eq!(
            model.app.player_tab.total_queue_len(),
            1,
            "Enqueue must resolve the component-owned episode target"
        );
        assert_eq!(
            model.app.audiobookshelf_browse[0].episode_selection, None,
            "Enqueue must not read or write the stale App mirror"
        );
        let component = model
            .abs_podcast_component_mut(0)
            .expect("podcast component mounted");
        assert_eq!(component.episode_selection(), Some(0));
    }

    /// The load-bearing space/ctrl-a contract (task 5.3d.7), with an Emby
    /// library present for leash-checking, space/ctrl-a on the mounted podcast
    /// component report the typed action intents with episode selection active,
    /// and the shell's FocusOrPlay (space) play is inert on an unsupported
    /// owner while ctrl-a Enqueue still edits the Composed queue. The action
    /// resolves episode-selection and owner eligibility at the Model boundary,
    /// so the episode selection stays Some(0) throughout. Mirrors the
    /// component/shell route, not direct field assignment.
    #[test]
    fn abs_podcast_shell_space_and_ctrla_are_inert_without_owner() {
        let mut model = Model::new(audiobookshelf_app());
        crate::app::tests_podcast::add_emby_movie_library(&mut model.app);
        model.app.audiobookshelf_browse[0].episode_selection = Some(0);
        let nav_len = model.app.libs[0].nav_stack.len();
        model.sync_audiobookshelf_podcast();
        let id = model
            .abs_podcast_id
            .clone()
            .expect("podcast component mounted");

        // Space -> FocusOrPlay: reported with selection, resolved inert at the
        // App boundary without an eligible owner.
        let space = model
            .application
            .get_component_mut(&id)
            .expect("podcast component")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Char(' '),
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent))) = space
        else {
            panic!("space should report a typed action intent");
        };
        assert_eq!(intent, PodcastEpisodeIntent::FocusOrPlay);
        // Resolve the reported intent at the Model boundary; the play attempt
        // is inert on an unsupported owner, surfacing the owner-unavailable
        // status without enqueuing.
        model.handle_audiobookshelf_podcast_episode_intent(intent);
        assert_eq!(
            model.app.audiobookshelf_browse[0].episode_selection,
            Some(0)
        );
        assert_eq!(
            model.app.player_tab.total_queue_len(),
            0,
            "inert space must not enqueue"
        );
        assert!(
            model
                .app
                .status
                .contains("Audiobookshelf playback owner is unavailable"),
            "inert FocusOrPlay must surface the owner-unavailable status"
        );

        // Ctrl+A = Enqueue intent, resolved by the shell App effect.
        let ctrl_a = model
            .application
            .get_component_mut(&id)
            .expect("podcast component")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Char('a'),
                modifiers: KeyModifiers::CONTROL,
            }));
        let Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent))) = ctrl_a
        else {
            panic!("ctrl-a should report as a typed action intent");
        };
        assert_eq!(intent, PodcastEpisodeIntent::Enqueue);
        // Resolve the reported intent at the Model boundary so the Composed
        // queue is edited; the component only reports, it does not enqueue.
        model.handle_audiobookshelf_podcast_episode_intent(intent);
        assert_eq!(
            model.app.audiobookshelf_browse[0].episode_selection,
            Some(0),
            "enqueue intent must preserve the selected episode"
        );
        assert_eq!(model.app.player_tab.total_queue_len(), 1);
        assert_eq!(
            model.app.libs[0].nav_stack.len(),
            nav_len,
            "inert activation must not navigate the Emby library"
        );
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
        assert!(
            model.app.layout.main.left_area.width > 0 && model.app.layout.main.left_area.height > 0,
            "wide list_area must project a nonzero left_area"
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
        assert!(
            model.app.layout.main.left_area.width > 0 && model.app.layout.main.left_area.height > 0,
            "narrow list_area must project a nonzero left_area"
        );
        assert!(
            !model.app.layout.main.selector_tabs.is_empty(),
            "narrow pill bar must project selector_tabs"
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
