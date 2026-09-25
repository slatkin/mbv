use super::*;

pub(super) fn build_episode_rows(episodes: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    episodes
        .iter()
        .enumerate()
        .map(|(index, episode)| {
            let number = if episode.index_number > 0 {
                episode.index_number
            } else {
                index as i64 + 1
            };
            let trailing = (episode.runtime_ticks > 0)
                .then(|| fmt_duration_gutter(episode.runtime_ticks / TICKS_PER_SECOND))
                .map(MediaListTrailing::Gutter);
            MediaListRow::Item {
                target: episode.id.clone(),
                primary: format!("{number}. {}", episode.name),
                secondary: None,
                trailing,
                duration: None,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::from_emby(episode),
            }
        })
        .collect()
}

/// TV Latest shares Home's split episode title and provider-date gutter.
pub(super) fn build_latest_episode_rows(episodes: &[EmbyItem]) -> Vec<MediaListRow<String>> {
    episodes
        .iter()
        .map(|episode| {
            let item = QueueItem::Emby(Box::new(episode.clone()));
            let parts = item.playback_title_parts(None);
            let (primary, secondary) = match parts.context {
                Some(context) => (context.text, Some(parts.title.text)),
                None => (parts.title.text, None),
            };
            let trailing = crate::app::state::home_latest::provider_timestamp_secs(&item)
                .map(fmt_publish_date_short)
                .filter(|date| !date.is_empty())
                .map(MediaListTrailing::Gutter);
            MediaListRow::Item {
                target: episode.id.clone(),
                primary,
                secondary,
                trailing,
                duration: None,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::from_queue_item(&item),
            }
        })
        .collect()
}

fn parse_premiere_date(value: &str) -> Option<Date> {
    let date = value.split('T').next()?;
    Date::parse(
        date,
        &time::format_description::well_known::Iso8601::DEFAULT,
    )
    .ok()
}

fn upcoming_date_heading(date: Date, today: Date) -> String {
    match (today - date).whole_days() {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => format!("{}, {} {}", date.weekday(), date.month(), date.day()),
    }
}

pub(in crate::app) fn upcoming_episode_target(episode: &EmbyItem) -> String {
    if !episode.id.is_empty() {
        return episode.id.clone();
    }

    let series = if episode.series_id.is_empty() {
        &episode.series_name
    } else {
        &episode.series_id
    };
    format!(
        "upcoming:{}:{}:{}:{}:{}:{}",
        series.len(),
        series,
        episode.parent_index_number,
        episode.index_number,
        episode.name.len(),
        episode.name
    )
}

pub(super) fn upcoming_episode_rows(
    episodes: &[EmbyItem],
    today: Date,
) -> Vec<MediaListRow<String>> {
    let mut groups: Vec<(Option<Date>, Vec<&EmbyItem>)> = Vec::new();
    for episode in episodes {
        let date = parse_premiere_date(&episode.premiere_date);
        if let Some((_, rows)) = groups
            .iter_mut()
            .find(|(group_date, _)| *group_date == date)
        {
            rows.push(episode);
        } else {
            groups.push((date, vec![episode]));
        }
    }

    groups
        .into_iter()
        .flat_map(|(date, episodes)| {
            let heading = date.map(|date| MediaListRow::Heading {
                text: upcoming_date_heading(date, today),
            });
            heading
                .into_iter()
                .chain(episodes.into_iter().map(|episode| {
                    let trailing = (episode.runtime_ticks > 0)
                        .then(|| fmt_duration_gutter(episode.runtime_ticks / TICKS_PER_SECOND))
                        .map(MediaListTrailing::Gutter);
                    MediaListRow::Item {
                        target: upcoming_episode_target(episode),
                        primary: episode.series_name.clone(),
                        secondary: Some(format!(
                            "S{:02}:E{:02} — {}",
                            episode.parent_index_number, episode.index_number, episode.name
                        )),
                        trailing,
                        duration: None,
                        kind: MediaKind::Media,
                        semantic_state: MediaSemanticState::from_emby(episode),
                    }
                }))
        })
        .collect()
}

impl TvContent {
    /// The current season's episode `Vec`, `&[]` when the season has no
    /// episodes loaded yet or `series_detail`/`season_cursor` cannot resolve
    /// one.
    pub(super) fn current_season_episodes(&self) -> &[EmbyItem] {
        self.context
            .series_detail
            .as_ref()
            .and_then(|detail| {
                let season = detail.seasons.get(self.season_cursor)?;
                detail.episodes.get(&season.id)
            })
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    /// Whether the current season's episode key is present in
    /// `series_detail.episodes` at all (even mapped to an empty `Vec`) --
    /// the loaded/loading distinction the refresh guard in `set_content`
    /// needs, unlike [`Self::current_season_episodes`] which treats both as
    /// empty.
    pub(super) fn current_season_episodes_key_present(&self) -> bool {
        self.context
            .series_detail
            .as_ref()
            .and_then(|detail| {
                let season = detail.seasons.get(self.season_cursor)?;
                detail.episodes.get(&season.id)
            })
            .is_some()
    }
    /// Rebuild the embedded episode `WideMediaList`'s content from the
    /// current season's episodes (design.md D3): preserves the selected
    /// target where still present, otherwise clamps locally -- the same
    /// canonical machinery the series rail uses.
    pub(super) fn refresh_episode_rows(&mut self) {
        let rows = build_episode_rows(self.current_season_episodes());
        // The 6.1 `last_projected_rows` pattern: an identical re-projection
        // skips `set_content`, which would otherwise invalidate the painted
        // frame and make a sync-without-draw frame unclaimable by pointer
        // input (design D6 frame invalidation).
        if self.last_episode_rows.as_ref() != Some(&rows) {
            self.episodes.set_content(rows.clone());
            self.last_episode_rows = Some(rows);
        }
    }
    /// Enter episode selection (the wide second-Enter move; the same
    /// "workspace is active" state the Hero overlay's focused Workspace
    /// holds in Narrow).
    pub(in crate::app) fn enter_episode_selection(&mut self) {
        self.episodes.select_first();
        self.pane = Pane::Episodes;
    }
    /// Deep selection (task 6.1, design D6 of change
    /// `per-destination-item-navigation`): point the workspace at the
    /// navigated episode's season and select the episode with episode focus.
    /// Returns false when the season is out of range or the episode is not
    /// among the (refreshed) season rows; the caller treats a miss as
    /// absence (the landing stands, default selection, no error).
    pub(in crate::app) fn select_episode_in_season(
        &mut self,
        season_index: usize,
        episode_id: &str,
    ) -> bool {
        let Some(detail) = self.context.series_detail.as_ref() else {
            return false;
        };
        if season_index >= detail.seasons.len() {
            return false;
        }
        self.season_cursor = season_index;
        if self.current_season_episodes_key_present() {
            self.refresh_episode_rows();
        }
        let selected = self.episodes.select_target(&episode_id.to_string());
        if selected {
            self.pane = Pane::Episodes;
        }
        selected
    }
    /// The series item under the component's own cursor, cloned out of the
    /// cached render context. `handle_key`'s Series Enter attaches this to
    /// `ShellRequest::TvActivate` so the shell effect targets the component
    /// selection instead of the mirrored App browse cursor.
    pub(in crate::app) fn selected_item(&self) -> Option<EmbyItem> {
        // Flat Latest/Upcoming lists resolve by the selected row's stable
        // target: the flat rows are painted newest-first, while the ordinal
        // fallback below indexes the cursor into alphabetical order and
        // would address the wrong row (see the narrow-Latest component test).
        if self.flat_episode_mode() {
            return self.selected_episode_item();
        }
        // Resolve through the same natural/effective order used to build the
        // rail. Stable IDs normally make this equivalent to target lookup;
        // ordinal resolution also keeps malformed duplicate-ID payloads from
        // collapsing two visibly distinct rows onto the first item.
        let mut items: Vec<&EmbyItem> = self.context.list.items.iter().collect();
        items.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));
        items.get(self.carrier.cursor()).cloned().cloned()
    }
    /// The Series snapshot the shell pushed for this frame (`context
    /// .selected_series`), exposed so tests can verify the pushed detail
    /// follows the component's authoritative selection rather than the App
    /// browse cursor.
    #[cfg(test)]
    pub(in crate::app) fn selected_series_snapshot(&self) -> Option<&EmbyItem> {
        self.context.selected_series.as_ref()
    }
    /// The episode item under the episode owner's current selection, resolved
    /// from the pushed season detail (design.md D4: the component carries the
    /// stable episode identity; the shell never reads the cursor).
    pub(in crate::app) fn selected_episode_item(&self) -> Option<EmbyItem> {
        if self.flat_episode_mode() {
            return self
                .carrier
                .selected_target()
                .and_then(|target| {
                    self.context
                        .list
                        .items
                        .iter()
                        .find(|item| upcoming_episode_target(item) == *target)
                })
                .cloned();
        }
        self.current_season_episodes()
            .get(self.episodes.cursor())
            .cloned()
    }

    pub(super) fn flat_episode_mode(&self) -> bool {
        matches!(
            self.context.tv_content_mode,
            Some(
                mbv_core::config::TvContentMode::Latest | mbv_core::config::TvContentMode::Upcoming
            )
        )
    }
    /// Test-only: the episode owner's selectable cursor index, used to prove
    /// the cursor survives a loading refresh where no episode item is
    /// resolvable.
    #[cfg(test)]
    pub(in crate::app) fn episode_cursor(&self) -> usize {
        self.episodes.cursor()
    }
    /// Test-only: the episode owner's resting scroll offset, used to prove
    /// the overlay Workspace's viewport follows cursor/wheel movement.
    #[cfg(test)]
    pub(in crate::app) fn episode_scroll(&self) -> usize {
        self.episodes.scroll()
    }
    /// Test-only: whether the Episodes pane holds the local focus (the
    /// "workspace is active" state).
    #[cfg(test)]
    pub(in crate::app) fn episode_pane_focused(&self) -> bool {
        self.pane == Pane::Episodes
    }
    pub(in crate::app) fn selected_season(&self) -> Option<(String, String)> {
        let series_id = self.context.selected_series.as_ref()?.id.clone();
        let season_id = self
            .context
            .series_detail
            .as_ref()?
            .seasons
            .get(self.season_cursor)?
            .id
            .clone();
        Some((series_id, season_id))
    }
}
