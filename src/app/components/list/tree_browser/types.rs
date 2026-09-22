use crate::app::components::media_list::MediaSemanticState;
/// Provider-neutral trailing metadata in a tree row's right gutter.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeTrailing {
    pub text: String,
}

#[allow(dead_code)]
impl TreeTrailing {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Closed per-node marking policy.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeMarkPolicy {
    /// The node itself can be included in the ordered mark set.
    #[default]
    Direct,
    /// The node displays an aggregate over directly markable descendants.
    Aggregate,
    /// The node is excluded from mark membership and aggregation.
    Excluded,
}

/// Plain destination data for one tree row.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode<Target> {
    pub target: Target,
    pub parent: Option<Target>,
    pub title: String,
    pub search_text: String,
    pub trailing: Option<TreeTrailing>,
    pub semantic_state: MediaSemanticState,
    pub mark_policy: TreeMarkPolicy,
}

#[allow(dead_code)]
impl<Target> TreeNode<Target> {
    pub fn new(
        target: Target,
        parent: Option<Target>,
        title: impl Into<String>,
        search_text: impl Into<String>,
        semantic_state: MediaSemanticState,
        mark_policy: TreeMarkPolicy,
    ) -> Self {
        Self {
            target,
            parent,
            title: title.into(),
            search_text: search_text.into(),
            trailing: None,
            semantic_state,
            mark_policy,
        }
    }

    pub fn with_trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(TreeTrailing::new(trailing));
        self
    }
}

/// The result disposition shared by semantic tree operations.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeConsumed {
    Unhandled,
    Consumed,
}

/// Stable-target external intent emitted by a tree operation.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeExternalIntent<Target> {
    Activate(Target),
    Context(Target),
    ContextSelection(Vec<Target>),
}

/// Stable selection change reported by a tree operation.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeSelectionChange<Target> {
    pub previous: Option<Target>,
    pub current: Option<Target>,
}

/// Provider-neutral summary of ordered marks and aggregate presentation.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeMarkSummary {
    pub marked_count: usize,
    pub has_partial_aggregate: bool,
}

/// Closed semantic operation vocabulary.  Behaviour is implemented by the
/// owner in the operations unit; keeping the vocabulary here prevents a
/// destination from inventing a second mutation surface.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeOperation<Target> {
    Move(i64),
    Page(i64),
    First,
    Last,
    Parent,
    Child,
    ToggleExpansion,
    Select(Target),
    ToggleMark,
    ToggleMarkTarget(Target),
    Activate,
    ActivateTarget(Target),
    Context,
    ContextTarget(Target),
    EditFilter(String),
    ClearFilter,
}

/// One complete operation result.  The independent fields let a caller
/// observe selection, marks, and external intent without replaying owner logic.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeTransition<Target> {
    pub disposition: TreeConsumed,
    pub selected_target: Option<Target>,
    pub selected_target_change: Option<TreeSelectionChange<Target>>,
    pub mark_summary: Option<TreeMarkSummary>,
    pub mark_summary_change: Option<TreeMarkSummary>,
    pub external_intent: Option<TreeExternalIntent<Target>>,
}

#[allow(dead_code)]
impl<Target> TreeTransition<Target> {
    pub fn unhandled() -> Self {
        Self {
            disposition: TreeConsumed::Unhandled,
            selected_target: None,
            selected_target_change: None,
            mark_summary: None,
            mark_summary_change: None,
            external_intent: None,
        }
    }
}
