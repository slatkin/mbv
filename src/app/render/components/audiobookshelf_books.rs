/// Hero layout plan shared by the `AudiobookshelfBookComponent` renderer
/// (`render/audiobookshelf_book.rs`) and the now-removed legacy `App` book
/// renderer. Kept here after the legacy renderer was deleted (task 5.3d.13)
/// because the component renderer still builds and constrains it.
#[derive(Clone)]
pub(in crate::app::render) struct BookHeroPlan {
    pub(in crate::app::render) image_key: Option<String>,
    pub(in crate::app::render) image_width: u16,
    pub(in crate::app::render) image_height: u16,
    pub(in crate::app::render) content_rows: u16,
}
