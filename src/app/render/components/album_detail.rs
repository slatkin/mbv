pub(in crate::app::render) fn album_hero_detail_rows(images_enabled: bool) -> usize {
    let image_rows = if images_enabled { 12 } else { 0 };
    (1 + 1 + 1).max(image_rows) + 1
}
