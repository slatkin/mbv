#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct QueueCardProjection {
    pub(crate) cache_key: Option<String>,
    pub(crate) images_enabled: bool,
    pub(crate) visualizer: bool,
}
