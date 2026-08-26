/// Hero layout plan shared by the `AudiobookshelfBookComponent` renderer
/// (`render/audiobookshelf_book.rs`) and the now-removed legacy `App` book
/// renderer. Kept here after the legacy renderer was deleted (task 5.3d.13)
/// because the component renderer still builds and constrains it.
#[derive(Clone)]
pub(in crate::app::render) struct BookHeroPlan {
    pub(in crate::app::render) image_key: Option<String>,
    pub(in crate::app::render) image_width: u16,
    pub(in crate::app::render) image_height: u16,
    pub(in crate::app::render) placeholder: bool,
    pub(in crate::app::render) content_rows: u16,
}

impl BookHeroPlan {
    pub(in crate::app::render) fn constrained_to_height(&self, height: u16) -> Self {
        let image_height = self.image_height.min(height.saturating_sub(1));
        Self {
            image_height,
            content_rows: self.content_rows.min(height),
            ..self.clone()
        }
    }
}
