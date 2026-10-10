//! The Home tree projection (row 5.1, design D1/D6 of
//! `home-recently-added-tree`): the shell-owned Continue Watching items and
//! the shell-merged `Recently added` rows (design D2) project into the shared
//! [`TreeEntry`]/[`TreeNode`] vocabulary the Home owner reconciles. Roots and
//! age branches are the projection's only structural rows; each one is an
//! expandable aggregate over its directly markable item descendants, and an
//! empty root or age branch is omitted entirely. Item rows reuse the flat
//! lists' split title-parts (`provider` context, item title as secondary);
//! `Recently added` item rows carry the provider date in the canonical right
//! gutter.

use crate::list::tree_browser::{TreeEntry, TreeMarkPolicy, TreeNode};
use mbv_queue::QueueItem;
use mbv_render::components::media_list::MediaSemanticState;
use mbv_ui_model::home_latest::{
    HomeLatestLaunchWindow, HomeRecentRow, provider_timestamp_secs, recent_age,
};
use mbv_ui_model::targets::{HomeItemTarget, HomeRoot, HomeTreeTarget, RecentAge};
use mbv_ui_model::ui_util::fmt_publish_date_short;

/// Every root and age branch the projection can emit (design D6). The fixed
/// set is the Home owner's expansion bookkeeping: a stem the owner holds is
/// expanded when it is newly projected, and stays otherwise untouched, so a
/// user-collapsed branch keeps its collapse state through reconciliation.
pub(crate) const TREE_STEMS: [HomeTreeTarget; 5] = [
    HomeTreeTarget::Root(HomeRoot::Continue),
    HomeTreeTarget::Root(HomeRoot::RecentlyAdded),
    HomeTreeTarget::Age(RecentAge::New),
    HomeTreeTarget::Age(RecentAge::Last7Days),
    HomeTreeTarget::Age(RecentAge::Last14Days),
];

/// The spec's root labels (`home-latest-sections`): the two collapsible
/// roots, in display order.
fn root_label(root: HomeRoot) -> &'static str {
    match root {
        HomeRoot::Continue => "Continue watching",
        HomeRoot::RecentlyAdded => "Recently added",
    }
}

/// The spec's age branch labels, in display order under the `Recently added`
/// root.
fn age_label(age: RecentAge) -> &'static str {
    match age {
        RecentAge::New => "New",
        RecentAge::Last7Days => "Last 7 days",
        RecentAge::Last14Days => "Last 14 days",
    }
}

/// The flat split title-parts of one item row: the context is the primary
/// part, the item's own title the secondary part, and a title-only row carves
/// no secondary. Feed rows take the configured subscription display name as
/// their context.
fn split_parts(item: &QueueItem, feed_name: Option<&str>) -> (String, Option<String>) {
    let parts = item.playback_title_parts(feed_name);
    match parts.context {
        Some(context) => (context.text, Some(parts.title.text)),
        None => (parts.title.text, None),
    }
}

fn root_node(root: HomeRoot) -> TreeNode<HomeTreeTarget> {
    let label = root_label(root);
    TreeNode::new(
        HomeTreeTarget::Root(root),
        None,
        label,
        label,
        MediaSemanticState::Ordinary,
        TreeMarkPolicy::Aggregate,
    )
    .with_expandable(true)
}

fn age_node(age: RecentAge) -> TreeNode<HomeTreeTarget> {
    let label = age_label(age);
    TreeNode::new(
        HomeTreeTarget::Age(age),
        Some(HomeTreeTarget::Root(HomeRoot::RecentlyAdded)),
        label,
        label,
        MediaSemanticState::Ordinary,
        TreeMarkPolicy::Aggregate,
    )
    .with_expandable(true)
}

/// One Continue Watching item row: the shell-provided Emby snapshot's
/// split title, no date gutter (only `Recently added` rows carry one), and
/// the section-scoped [`HomeItemTarget::Continue`] identity.
fn continue_item_node(item: &QueueItem) -> TreeNode<HomeTreeTarget> {
    let (primary, secondary) = split_parts(item, None);
    let mut node = TreeNode::new(
        HomeTreeTarget::Item(HomeItemTarget::continue_item(item.id())),
        Some(HomeTreeTarget::Root(HomeRoot::Continue)),
        primary.clone(),
        primary,
        MediaSemanticState::from_queue_item(item),
        TreeMarkPolicy::Direct,
    );
    if let Some(secondary) = secondary {
        node = node.with_secondary(secondary);
    }
    node
}

/// One `Recently added` row: the container context and title in the shared
/// split title-parts format, the provider date in the right gutter, and the
/// section-scoped [`HomeItemTarget::Recent`] identity.
fn recent_item_node(row: &HomeRecentRow, parent: HomeTreeTarget) -> TreeNode<HomeTreeTarget> {
    let (primary, secondary) = split_parts(&row.item, row.feed_name.as_deref());
    let mut node = TreeNode::new(
        HomeTreeTarget::Item(HomeItemTarget::Recent(row.item.content_id())),
        Some(parent),
        primary.clone(),
        primary,
        MediaSemanticState::from_queue_item(&row.item),
        TreeMarkPolicy::Direct,
    )
    .with_trailing(
        provider_timestamp_secs(&row.item)
            .map(fmt_publish_date_short)
            .unwrap_or_default(),
    );
    if let Some(secondary) = secondary {
        node = node.with_secondary(secondary);
    }
    node
}

/// The `Recently added` rows bucketed into the three age branches in display
/// order. A row without a valid date (or older than the cutoff) is in no
/// bucket and is therefore omitted from the projection.
fn age_buckets(
    recent_rows: &[HomeRecentRow],
    window: HomeLatestLaunchWindow,
) -> [Vec<&HomeRecentRow>; 3] {
    let mut new = Vec::new();
    let mut last_7_days = Vec::new();
    let mut last_14_days = Vec::new();
    for row in recent_rows {
        match recent_age(provider_timestamp_secs(&row.item), window) {
            Some(RecentAge::New) => new.push(row),
            Some(RecentAge::Last7Days) => last_7_days.push(row),
            Some(RecentAge::Last14Days) => last_14_days.push(row),
            None => {}
        }
    }
    [new, last_7_days, last_14_days]
}

/// The complete Home projection: the `Continue watching` root with its item
/// rows, then the `Recently added` root with its non-empty age branches.
/// A root with no items is omitted, and empty inputs project no entries at
/// all (the panel then renders its empty placeholder).
pub(super) fn tree_entries(
    continue_items: &[QueueItem],
    recent_rows: &[HomeRecentRow],
    window: HomeLatestLaunchWindow,
) -> Vec<TreeEntry<HomeTreeTarget>> {
    let mut entries = Vec::new();
    if !continue_items.is_empty() {
        entries.push(root_node(HomeRoot::Continue).into());
        entries.extend(
            continue_items
                .iter()
                .map(|item| TreeEntry::Node(continue_item_node(item))),
        );
    }
    let buckets = age_buckets(recent_rows, window);
    if buckets.iter().any(|bucket| !bucket.is_empty()) {
        entries.push(root_node(HomeRoot::RecentlyAdded).into());
        for (age, index) in [
            (RecentAge::New, 0),
            (RecentAge::Last7Days, 1),
            (RecentAge::Last14Days, 2),
        ] {
            let bucket = &buckets[index];
            if bucket.is_empty() {
                continue;
            }
            let parent = HomeTreeTarget::Age(age);
            entries.push(TreeEntry::Node(age_node(age)));
            entries.extend(
                bucket
                    .iter()
                    .map(|row| TreeEntry::Node(recent_item_node(row, parent.clone()))),
            );
        }
    }
    entries
}

/// The projection contract (design D1/D2): itemless roots and empty age
/// branches are omitted, and the same item under both roots projects two
/// distinct section-scoped item targets (the section is part of the
/// identity). Regression guard for `home-recently-added-tree` row 5.1.
#[cfg(test)]
mod tests {
    use super::*;
    use mbv_emby_model::test_support::make_item;

    fn iso_secs(secs: u64) -> String {
        use time::format_description::well_known::Iso8601;
        time::OffsetDateTime::from_unix_timestamp(i64::try_from(secs).unwrap_or(i64::MAX))
            .expect("in-range fixture timestamp")
            .format(&Iso8601::DEFAULT)
            .expect("ISO-8601 formatting always succeeds")
    }

    fn emby_item(name: &str, age_secs_ago: u64, window: HomeLatestLaunchWindow) -> QueueItem {
        let mut item = make_item(name, "Movie");
        item.id = format!("item-{name}");
        item.date_added = iso_secs(window.current.saturating_sub(age_secs_ago));
        QueueItem::Emby(Box::new(item))
    }

    #[test]
    fn empty_stems_are_omitted_and_both_roots_keep_one_items_two_targets() {
        let day = 24 * 60 * 60;
        // A plausible recent epoch keeps the fixture arithmetic in range.
        let window = HomeLatestLaunchWindow {
            previous: Some(1_500_000_000 - 2 * day),
            current: 1_500_000_000,
        };
        // `shared` is a Continue item and a 3-day-old Recently added row, so
        // it lands under `Last 7 days`; the 20-day-old row is past the cutoff
        // and in no branch.
        let shared = emby_item("shared", 3 * day, window);
        let old = emby_item("old", 20 * day, window);
        let entries = tree_entries(
            std::slice::from_ref(&shared),
            &[
                HomeRecentRow {
                    item: shared.clone(),
                    feed_name: None,
                },
                HomeRecentRow {
                    item: old,
                    feed_name: None,
                },
            ],
            window,
        );
        let targets: Vec<HomeTreeTarget> = entries
            .iter()
            .filter_map(|entry| match entry {
                TreeEntry::Node(node) => Some(node.target.clone()),
                TreeEntry::Heading(_) | TreeEntry::Spacer => None,
            })
            .collect();
        // Both roots, but only the non-empty `Last 7 days` branch exists.
        assert!(targets.contains(&HomeTreeTarget::Root(HomeRoot::Continue)));
        assert!(targets.contains(&HomeTreeTarget::Root(HomeRoot::RecentlyAdded)));
        assert_eq!(
            targets
                .iter()
                .filter(|target| matches!(target, HomeTreeTarget::Age(_)))
                .collect::<Vec<_>>(),
            [&HomeTreeTarget::Age(RecentAge::Last7Days)],
            "empty age branches are omitted"
        );
        // One item, two section-scoped item targets.
        let continue_target = HomeTreeTarget::Item(HomeItemTarget::continue_item("item-shared"));
        let recent_target = HomeTreeTarget::Item(HomeItemTarget::Recent(
            mbv_queue::QueueItemContentId::Emby("item-shared".into()),
        ));
        assert!(targets.contains(&continue_target));
        assert!(targets.contains(&recent_target));
        assert_ne!(continue_target, recent_target);

        // Empty inputs project nothing.
        assert_eq!(tree_entries(&[], &[], window), Vec::new());
    }
}
