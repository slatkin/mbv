use super::*;

impl TvContent {
    /// Project the settled show-mode catalog into the shared tree vocabulary.
    /// Only the selected show's detail snapshot is available at this boundary;
    /// task 2.2 wires in additional shell-owned loaded details.
    pub(super) fn stable_show_target(
        show: &EmbyItem,
        duplicate_id: bool,
        occurrence: usize,
    ) -> String {
        let base = if show.id.is_empty() {
            format!("tv-name:{}:{}", show.name.len(), show.name)
        } else if duplicate_id {
            format!(
                "tv-id:{}:{}:{}:{}",
                show.id.len(),
                show.id,
                show.name.len(),
                show.name
            )
        } else {
            format!("tv-id:{}:{}", show.id.len(), show.id)
        };
        if occurrence == 0 {
            base
        } else {
            format!("{base}:{occurrence}")
        }
    }

    /// Sorted natural-order shows paired with their stable tree target,
    /// disambiguating duplicate Emby ids the same way `stable_show_target`
    /// does. The single source of the show-id-collision resolution that
    /// every show-target lookup (projection, expansion, selection) shares.
    pub(super) fn show_targets(items: &[EmbyItem]) -> Vec<(String, &EmbyItem)> {
        let mut shows: Vec<&EmbyItem> = items.iter().collect();
        shows.sort_by_key(|item| natural_sort_key(effective_sort_str(item)));
        let mut id_counts = std::collections::HashMap::new();
        for show in &shows {
            *id_counts.entry(show.id.as_str()).or_insert(0usize) += 1;
        }
        let mut occurrences = std::collections::HashMap::new();
        shows
            .into_iter()
            .map(|show| {
                let duplicate_id = id_counts.get(show.id.as_str()).copied().unwrap_or_default() > 1;
                let base = Self::stable_show_target(show, duplicate_id, 0);
                let occurrence = occurrences.entry(base).or_insert(0usize);
                let target = Self::stable_show_target(show, duplicate_id, *occurrence);
                *occurrence += 1;
                (target, show)
            })
            .collect()
    }

    /// The show matching a stable tree target string, resolved through the
    /// same collision-disambiguated ordering [`Self::show_targets`] builds.
    pub(super) fn resolve_show_target<'a>(
        items: &'a [EmbyItem],
        show_target: &str,
    ) -> Option<&'a EmbyItem> {
        Self::show_targets(items)
            .into_iter()
            .find(|(target, _)| target == show_target)
            .map(|(_, show)| show)
    }

    /// The loaded detail to project beneath `show`: the shell's per-show map
    /// entry when present, else the selected show's detail for contexts built
    /// without the map (existing unit tests).
    pub(super) fn detail_for_projection<'a>(
        context: &'a TvWideRenderCtx,
        show: &EmbyItem,
    ) -> Option<&'a crate::app::SeriesDetail> {
        context.series_details.get(&show.id).or_else(|| {
            context
                .selected_series
                .as_ref()
                .filter(|selected| selected.id == show.id && selected.name == show.name)
                .and(context.series_detail.as_ref())
        })
    }

    /// The loaded detail holding `show`'s seasons/episodes for target
    /// resolution: the shell's per-show map entry when present, else the
    /// pushed selected detail (existing single-detail contexts).
    pub(super) fn detail_for_show(&self, show: &EmbyItem) -> Option<&crate::app::SeriesDetail> {
        self.context
            .series_details
            .get(&show.id)
            .or(self.context.series_detail.as_ref())
    }

    /// Project the settled show-mode catalog into the shared tree vocabulary.
    /// Loaded details for every listed show project per show, so an expanded
    /// show keeps its children while another show is selected. Contexts built
    /// without the shell's detail map fall back to the selected show's detail.
    pub(super) fn tree_projection(context: &TvWideRenderCtx) -> Vec<TreeEntry<TvTreeTarget>> {
        let grouped = context.show_letter_pills
            || context.list.has_letter_filter()
            || context.list.true_total() >= 50;
        let bucket_total = if context.list.has_letter_filter() {
            usize::MAX
        } else {
            context.list.true_total()
        };
        let shows = Self::show_targets(&context.list.items);
        let mut entries = Vec::with_capacity(shows.len() + 8);
        let mut previous_bucket: Option<String> = None;
        for (show_id, show) in shows {
            let target = TvTreeTarget::Show(show_id.clone());
            if grouped {
                let bucket = letter_bucket(show, bucket_total);
                if previous_bucket.as_deref() != Some(bucket.as_str()) {
                    if previous_bucket.is_some() {
                        entries.push(TreeEntry::Spacer);
                    }
                    entries.push(TreeEntry::Heading(bucket.clone()));
                    previous_bucket = Some(bucket);
                }
            }
            let root_index = entries.len();
            entries.push(TreeEntry::Node(
                TreeNode::new(
                    target.clone(),
                    None,
                    show.display_name(),
                    effective_sort_str(show),
                    MediaSemanticState::from_emby(show),
                    TreeMarkPolicy::Direct,
                )
                .with_expandable(true),
            ));

            let Some(detail) = Self::detail_for_projection(context, show) else {
                continue;
            };
            // An empty completed detail has no pending children to load.
            if let TreeEntry::Node(root) = &mut entries[root_index] {
                root.expandable = !detail.seasons.is_empty();
            }
            let mut season_occurrences = std::collections::HashMap::new();
            for season in &detail.seasons {
                let occurrence = season_occurrences
                    .entry(season.id.clone())
                    .or_insert(0usize);
                let season_occurrence = *occurrence;
                let season_target = TvTreeTarget::Season {
                    show: show_id.clone(),
                    season: season.id.clone(),
                    occurrence: season_occurrence,
                };
                *occurrence += 1;
                let episodes = detail.episodes.get(&season.id);
                entries.push(TreeEntry::Node(
                    TreeNode::new(
                        season_target.clone(),
                        Some(target.clone()),
                        season.display_name(),
                        season.name.clone(),
                        MediaSemanticState::from_emby(season),
                        TreeMarkPolicy::Direct,
                    )
                    .with_expandable(episodes.is_none_or(|episodes| !episodes.is_empty())),
                ));
                if let Some(episodes) = episodes {
                    let mut episode_occurrences = std::collections::HashMap::new();
                    for (index, episode) in episodes.iter().enumerate() {
                        let episode_id = if episode.id.is_empty() {
                            upcoming_episode_target(episode)
                        } else {
                            episode.id.clone()
                        };
                        let episode_occurrence = episode_occurrences
                            .entry(episode_id.clone())
                            .or_insert(0usize);
                        let number = if episode.index_number > 0 {
                            episode.index_number
                        } else {
                            index as i64 + 1
                        };
                        let episode_target = TvTreeTarget::Episode {
                            show: show_id.clone(),
                            season: season.id.clone(),
                            season_occurrence,
                            episode: episode_id,
                            occurrence: *episode_occurrence,
                        };
                        *episode_occurrence += 1;
                        entries.push(TreeEntry::Node(TreeNode::new(
                            episode_target,
                            Some(season_target.clone()),
                            format!("{number}. {}", episode.name),
                            episode.name.clone(),
                            MediaSemanticState::from_emby(episode),
                            TreeMarkPolicy::Direct,
                        )));
                    }
                }
            }
        }
        entries
    }

    pub(crate) fn tree_expansion_source(
        &self,
        target: &TvTreeTarget,
    ) -> Option<(String, Option<String>)> {
        let (show_target, season_id) = match target {
            TvTreeTarget::Show(show) => (show, None),
            TvTreeTarget::Season { show, season, .. } => (show, Some(season.clone())),
            TvTreeTarget::Episode { .. } => return None,
        };
        self.browser.node(target)?;
        let show = Self::resolve_show_target(&self.context.list.items, show_target)?;
        (!show.id.is_empty()).then(|| (show.id.clone(), season_id))
    }

    // Kept as a typed boundary seam until task 3.1 connects mounted tree input.
    pub(crate) fn toggle_tree_expansion(&mut self, target: TvTreeTarget) -> Option<Msg> {
        let was_expanded = self.browser.is_expanded(&target);
        let transition = self
            .browser
            .apply(TreeOperation::ToggleExpansionTarget(target.clone()));
        if was_expanded
            || transition.disposition == super::super::list::tree_browser::TreeConsumed::Unhandled
        {
            return None;
        }
        Some(Msg::Shell(ShellRequest::TvTreeExpand { target }))
    }
}
