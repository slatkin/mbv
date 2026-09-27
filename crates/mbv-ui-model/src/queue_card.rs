#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueCardProjection {
    pub cache_key: Option<String>,
    pub images_enabled: bool,
    pub visualizer: bool,
}
