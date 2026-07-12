use super::super::super::super::layout::LayoutLibrary;
use super::super::super::super::ui_util::trunc_overview;
use super::super::super::super::App;
use super::meta::{library_is_audio, library_is_episode_like};
use super::{LibraryTableContext, LIB_AUDIO_IMG_W, LIB_EPISODE_IMG_W, LIB_SELECTED_IMG_W};
use mbv_core::api::MediaItem;
use ratatui::layout::{Rect, Size};
use textwrap::wrap;

impl App {
    pub(super) fn build_library_table_context(
        &mut self,
        lib_idx: usize,
        display_items: &[(usize, MediaItem)],
        cursor: usize,
    ) -> LibraryTableContext {
        let audio_img_h = self
            .image_picker
            .as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_AUDIO_IMG_W as f32 * fs.width as f32) / fs.height as f32).ceil() as u16
            })
            .unwrap_or(12);
        let selected_img_h = self
            .image_picker
            .as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_SELECTED_IMG_W as f32 * fs.width as f32 * 1.5) / fs.height as f32).ceil()
                    as u16
            })
            .unwrap_or(12)
            .min(12);
        let episode_img_h = self
            .image_picker
            .as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_EPISODE_IMG_W as f32 * fs.width as f32 * (9.0 / 16.0)) / fs.height as f32)
                    .ceil() as u16
            })
            .unwrap_or(9);
        let at_album_folders =
            self.is_viewing_album_folders(lib_idx) && self.libs[lib_idx].search.is_none();
        let at_music_groups = {
            let lib = &self.libs[lib_idx];
            lib.library.collection_type == "music"
                && !self.music_levels.is_empty()
                && !lib.nav_stack.is_empty()
                && self
                    .music_levels
                    .get(lib.nav_stack.len() - 1)
                    .map(|s| s == "group")
                    .unwrap_or(false)
        };
        let is_feed_lib = {
            let c = self.client.lock().unwrap();
            c.config
                .feed_view_libraries
                .contains(&self.libs[lib_idx].library.name.to_lowercase())
        };
        let playback = self.effective_playback_state();
        let now_playing_id = if playback.active {
            self.playback_queue()
                .items
                .get(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };
        let images_enabled = self.images_enabled();
        let actual_sel_img_h = self.actual_selected_library_image_height(
            display_items,
            cursor,
            images_enabled,
            at_album_folders,
            is_feed_lib,
            selected_img_h,
            episode_img_h,
        );

        LibraryTableContext {
            images_enabled,
            at_album_folders,
            at_music_groups,
            is_feed_lib,
            now_playing_id,
            audio_img_h,
            selected_img_h,
            episode_img_h,
            actual_sel_img_h,
        }
    }

    fn actual_selected_library_image_height(
        &mut self,
        display_items: &[(usize, MediaItem)],
        cursor: usize,
        images_enabled: bool,
        at_album_folders: bool,
        is_feed_lib: bool,
        selected_img_h: u16,
        episode_img_h: u16,
    ) -> u16 {
        if !images_enabled {
            return 0;
        }
        let Some((_, item)) = display_items.get(cursor) else {
            return 0;
        };
        let is_audio = library_is_audio(item);
        let is_album_folder = at_album_folders && item.is_folder;
        if is_audio || is_album_folder {
            return 0;
        }
        let is_episode_like = library_is_episode_like(item, is_feed_lib);
        let (img_w, img_h) = if is_episode_like {
            (LIB_EPISODE_IMG_W, episode_img_h)
        } else {
            (LIB_SELECTED_IMG_W, selected_img_h)
        };
        let cache_key = format!("{}:lib", item.id);
        if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
            // `size_for` is `None` while resize+encode is in-flight on the
            // worker thread; fall back to the reserved height like an
            // absent/unfetched entry.
            state
                .size_for(
                    ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)),
                    Size {
                        width: img_w,
                        height: img_h,
                    },
                )
                .map(|s| s.height)
                .unwrap_or(img_h)
        } else {
            img_h
        }
    }

    pub(super) fn library_row_height(
        &self,
        area: Rect,
        item: &MediaItem,
        idx: usize,
        cursor: usize,
        ctx: &LibraryTableContext,
    ) -> u16 {
        let is_audio = library_is_audio(item);
        let is_album_folder = ctx.at_album_folders && item.is_folder;
        let base: u16 =
            if item.is_folder && item.item_type != "Series" && item.item_type != "Season" {
                if is_album_folder && idx == cursor {
                    if ctx.images_enabled {
                        ctx.audio_img_h.max(3)
                    } else {
                        3
                    }
                } else {
                    1
                }
            } else if is_audio {
                if idx == cursor {
                    ctx.audio_img_h.max(3)
                } else {
                    3
                }
            } else if ctx.images_enabled && idx == cursor {
                let is_episode_like = library_is_episode_like(item, ctx.is_feed_lib);
                let (sel_img_w, sel_img_h) = if is_episode_like {
                    (LIB_EPISODE_IMG_W, ctx.episode_img_h)
                } else {
                    (LIB_SELECTED_IMG_W, ctx.selected_img_h)
                };
                let ew = area.width.saturating_sub(2 + sel_img_w) as usize;
                let overview = trunc_overview(&item.overview);
                let ov_lines = if overview.is_empty() {
                    0
                } else {
                    wrap(&overview, ew.max(1)).len() as u16
                };
                let dir_lines: u16 = if item.item_type == "Movie" && !item.director.is_empty() {
                    2
                } else {
                    0
                };
                let tech: u16 = match (item.video_info.is_empty(), item.audio_info.is_empty()) {
                    (true, true) => 0,
                    _ => 1,
                };
                let img_h_for_layout = if ctx.actual_sel_img_h > 0 {
                    ctx.actual_sel_img_h
                } else {
                    sel_img_h
                };
                (2 + tech + ov_lines + dir_lines).max(img_h_for_layout)
            } else {
                2
            };
        base + 1
    }

    pub(super) fn library_table_scroll(
        &mut self,
        area: Rect,
        lib_idx: usize,
        cursor: usize,
        all_heights: &[u16],
        layout: &mut LayoutLibrary,
    ) -> usize {
        let scroll = if self.libs[lib_idx].search.is_some() {
            let mut s = layout
                .lib_scroll
                .get(lib_idx)
                .copied()
                .unwrap_or(0)
                .min(cursor);
            loop {
                let visible_h: u16 = all_heights[s..=cursor].iter().sum();
                if visible_h <= area.height {
                    break;
                }
                s += 1;
            }
            s
        } else {
            cursor
        };
        if let Some(v) = layout.lib_scroll.get_mut(lib_idx) {
            *v = scroll;
        }
        scroll
    }

    pub(super) fn prefetch_library_table_assets(
        &mut self,
        display_items: &[(usize, MediaItem)],
        cursor: usize,
        ctx: &LibraryTableContext,
    ) {
        if !self.list_image_fetches_allowed() {
            return;
        }
        let prefetch_start = cursor.saturating_sub(3);
        let prefetch_end = (cursor + 3).min(display_items.len().saturating_sub(1));
        for pi in prefetch_start..=prefetch_end {
            if let Some((_, item)) = display_items.get(pi) {
                let is_audio = library_is_audio(item);
                let is_album_folder = ctx.at_album_folders && item.is_folder;
                if ctx.images_enabled || is_audio || is_album_folder {
                    let cache_key = format!("{}:lib", item.id);
                    let img_types: &[&str] = if is_album_folder {
                        &["AudioChild", "Primary"]
                    } else {
                        &["Primary"]
                    };
                    self.fetch_list_card_image_when_idle(
                        cache_key,
                        item.id.clone(),
                        String::new(),
                        img_types,
                    );
                }
                if is_album_folder && item.production_year == 0 {
                    self.fetch_album_year(item.id.clone());
                }
            }
        }
    }
}
