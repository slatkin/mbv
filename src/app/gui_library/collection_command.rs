//! The GUI Library's one collection-command path (gui-collection-dropdown
//! design D5): the destination-neutral op and rejection vocabulary, the
//! per-destination site trait, and the shared apply and selector build the
//! four destinations' sites plug into. The sites live beside their
//! destination's store and side effects (`store.rs`, `podcasts.rs`,
//! `feeds.rs`, `emby.rs`); this module holds what they share.

use std::borrow::Cow;

use mbv_config::GuiGroupPick;

use super::collection::{GuiCollectionNoun, GuiCollectionSet, GuiCollectionTarget, GuiCollections};
use super::{
    GuiEmbyFeedCollectionSite, GuiFeedsCollectionSite, GuiMusicCollectionSite,
    GuiPodcastsCollectionSite,
};
use crate::app::App;
use crate::gui::GuiCore;

/// The op one collection command carries (gui-collection-dropdown design D5):
/// the three dropdown commands, destination-neutral.
pub(in crate::app::gui_library) enum CollectionOp {
    /// Check or uncheck the collection `key` names.
    Toggle { key: String },
    /// Select All: the All row's press and the dropdown's Clear.
    All,
    /// Install a saved launch selection, retained against the landed catalog.
    Restore { set: GuiCollectionSet },
}

impl CollectionOp {
    /// The name the op's reject logs (design D5): the command's own name.
    fn name(&self) -> &'static str {
        match self {
            Self::Toggle { .. } => "collectionToggle",
            Self::All => "collectionAll",
            Self::Restore { .. } => "collectionRestore",
        }
    }
}

/// Why a collection command changes nothing (design D5, D6): the log form the
/// destination's rule selects.
pub(in crate::app::gui_library) enum CollectionRejection {
    /// The command arrived, but the destination or its catalog rule rejects
    /// it: the ignored log names the reason.
    Ignored(String),
    /// A stale frame's key: the stale-item log names it.
    Stale(String),
}

/// One collection destination's command site (gui-collection-dropdown design
/// D5): the destination's own reject, staleness, catalog, and install steps
/// the one shared collection path calls. The Music, Podcasts, Feeds, and Emby
/// feed-group sites live beside their destination's store and side effects.
pub(in crate::app::gui_library) trait GuiCollectionSite {
    /// The noun the destination's selector reads (design D6).
    fn noun(&self) -> GuiCollectionNoun;

    /// The catalog the retention retains against (design D1): a key
    /// predicate over the landed catalog — `None` while the catalog has not
    /// landed, the list that would name a key stale, which keeps the stored
    /// set as it is. The predicate reads the catalog without building its id
    /// list, so a per-frame reader allocates nothing.
    fn present(&self, app: &App) -> Option<impl Fn(&str) -> bool>;

    /// The destination's scopes: each collection's stable key and label, in
    /// catalog order (design D6). The pairs convert as they are kept, so a
    /// site can stream its catalog without a freshly cloned vector between
    /// them.
    fn scopes(&self, app: &App) -> impl IntoIterator<Item = (String, String)>;

    /// The destination's stored set (design D1): the raw entry the effective
    /// set retains against the catalog. All's site borrows
    /// [`GuiCollectionSet::ALL`], so a library without a stored entry never
    /// clones a default.
    fn stored_set<'a>(&self, app: &'a App) -> &'a GuiCollectionSet;

    /// Whether the session has changed this destination's collection state
    /// (design D10): a launch restore never overwrites a choice the session
    /// made before the saved file's collections landed.
    fn has_stored_selection(&self, app: &App) -> bool;

    /// The collection album picks the session holds (design D13): only a
    /// Music destination keeps one, so the other sites save none.
    fn saved_picks(&self, _app: &App) -> Vec<GuiGroupPick> {
        Vec::new()
    }

    /// Restore the saved album picks whose collections the landed catalog
    /// holds (design D13): the shared restore filters them with the same
    /// predicate that retains the saved set's keys.
    fn install_picks(&self, _app: &mut App, _picks: &[GuiGroupPick]) {}

    /// Why the destination cannot serve a collection command at all (design
    /// D5, D6).
    fn reject_destination(&self, app: &App) -> Option<CollectionRejection>;

    /// Why the collection `key` cannot apply (design D5, D6): a stale frame's
    /// key, or a destination-level rule a toggle alone runs.
    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection>;

    /// The destination's own log for one toggle (design D5): Music logs the
    /// selected collection's change; the other destinations log none.
    fn log_toggle(&self, _key: &str) {}

    /// Write the set back, starting the destination's own loads (design D2,
    /// D5).
    fn install_set(&self, app: &mut App, set: GuiCollectionSet);

    /// The effective selection the destination paints (design D1): the stored
    /// set retained against the catalog. A set nothing drops is returned
    /// borrowed, so a per-frame reader clones nothing. The borrow is the
    /// `App`'s alone, so the caller's site value need not outlive it.
    fn effective_set<'a>(&self, app: &'a App) -> Cow<'a, GuiCollectionSet> {
        self.stored_set(app).effective(self.present(app))
    }
}

impl GuiCore {
    /// Apply one collection command for `target` (gui-collection-dropdown
    /// design D5): the one shared path — reject, stale check, toggle/All over
    /// the effective set, and the destination's own install — that the four
    /// destinations' sites plug into, so the toggle and All steps cannot
    /// drift. The match names each destination's site; everything else is
    /// shared.
    pub(in crate::app::gui_library) fn apply_collection(
        &mut self,
        target: &GuiCollectionTarget,
        op: CollectionOp,
    ) {
        apply_collection(&mut self.app, target, op);
    }
}

/// Apply one collection command for `target` over the `App` the GUI core owns
/// (gui-collection-dropdown design D5): the one shared path — reject, stale
/// check, toggle/All over the effective set, and the destination's own install
/// — that the four destinations' sites plug into, so the toggle and All steps
/// cannot drift. The match names each destination's site; everything else is
/// shared.
pub(in crate::app::gui_library) fn apply_collection(
    app: &mut App,
    target: &GuiCollectionTarget,
    op: CollectionOp,
) {
    match target {
        GuiCollectionTarget::Music(lib_idx) => {
            apply_collection_at(app, &GuiMusicCollectionSite(*lib_idx), op);
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            apply_collection_at(app, &GuiPodcastsCollectionSite(*lib_idx), op);
        }
        GuiCollectionTarget::Feeds => apply_collection_at(app, &GuiFeedsCollectionSite, op),
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            apply_collection_at(app, &GuiEmbyFeedCollectionSite(*lib_idx), op);
        }
    }
}

/// The one collection path over one destination's site (design D5): the op's
/// reject rule names the log line; a toggle reads the effective set and
/// installs the change, and All and the restore install their set.
pub(in crate::app::gui_library) fn apply_collection_at(
    app: &mut App,
    site: &impl GuiCollectionSite,
    op: CollectionOp,
) {
    let log_name = op.name();
    let rejection = match &op {
        CollectionOp::Toggle { key } => match site.reject_destination(app) {
            Some(rejection) => Some(rejection),
            None => site.reject_key(app, key),
        },
        CollectionOp::All | CollectionOp::Restore { .. } => site.reject_destination(app),
    };
    if let Some(rejection) = rejection {
        match rejection {
            CollectionRejection::Ignored(reason) => {
                super::log_ignored_library_command(log_name, format_args!("{reason}"));
            }
            CollectionRejection::Stale(key) => {
                super::log_stale_library_item(log_name, &key);
            }
        }
        return;
    }
    match op {
        CollectionOp::Toggle { key } => {
            site.log_toggle(&key);
            let next = site.effective_set(app).into_owned().toggle(&key);
            site.install_set(app, next);
        }
        CollectionOp::All => site.install_set(app, GuiCollectionSet::All),
        CollectionOp::Restore { set } => {
            // An All entry leaves the default unset, as a destination the
            // session never touched does.
            if !set.is_all() {
                site.install_set(app, set);
            }
        }
    }
}

/// The destination's collection selector (gui-collection-dropdown design D6),
/// over the `App` the GUI core owns: the one build every projection reads, so
/// the four frames cannot drift.
pub(in crate::app::gui_library) fn collections(
    app: &App,
    target: GuiCollectionTarget,
) -> GuiCollections {
    match &target {
        GuiCollectionTarget::Music(lib_idx) => {
            site_collections(app, &GuiMusicCollectionSite(*lib_idx), target)
        }
        GuiCollectionTarget::Podcasts(lib_idx) => {
            site_collections(app, &GuiPodcastsCollectionSite(*lib_idx), target)
        }
        GuiCollectionTarget::Feeds => site_collections(app, &GuiFeedsCollectionSite, target),
        GuiCollectionTarget::EmbyFeed(lib_idx) => {
            site_collections(app, &GuiEmbyFeedCollectionSite(*lib_idx), target)
        }
    }
}

/// The selector one site builds (design D6): the effective set over the
/// destination's catalog, its noun, and its scopes in catalog order.
fn site_collections(
    app: &App,
    site: &impl GuiCollectionSite,
    target: GuiCollectionTarget,
) -> GuiCollections {
    let set = site.effective_set(app);
    GuiCollections::new(target, set.as_ref(), site.noun(), site.scopes(app))
}
