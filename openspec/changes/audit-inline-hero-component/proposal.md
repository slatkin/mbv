## Why

The inline hero — the selected row replaced by a variable-height detail block in the single-column browser — should be one cohesive component with one design across every tab. Four of nine surfaces deviate from the norm in presentation, each in a different way. Home Feed already had the correct text-only/no-image shape, but still routed its metadata through Home's bespoke painter. The access-control boundary from #593 (merged) enforces *who may own geometry* but not *what geometry is shared*. This change audits the inline-hero component across all surfaces, defines the norm, classifies presentation drift, and migrates each surface to the shared vocabulary — one PR per drift fix — while recording already-correct visual shapes with characterization tests.

## What Changes

- **Norm defined:** Model A (image-top, wrap-around) for tall images; Model B (beside-image, meta-column) for wide 16:9 thumbnails; no-image uses Model A's degenerate form. Tab-level browsing pills always in the panel, one row. Selection-level filters (e.g. podcast played/unplayed) live in the selection modal, not in the panel or the hero. No variants, no exceptions.
- **Series:** remove seasons/episodes extension from inline hero. The hero shows title + meta + overview + image only. Seasons and episodes move to a modal on Enter, reusing the existing modal-frame vocabulary.
- **Music:** remove `album_detail` workspace from inline hero. The hero shows title + meta + overview + album art (Model A). Track list moves to a modal on Enter.
- **Podcasts:** remove hand-painted author/description block and in-hero filter pills. Standard Model A hero (author/description as `HeroLine`s). Alphabetical pills in the panel (like every other library tab). Episodes move to a modal on Enter.
- **ABS Books:** move from Model B to Model A. Book covers are tall (~2:3), same shape as movie posters. The `beside_image_hero_dims` half-width + 16:9 budget doesn't fit a tall cover.
- **Home Feed:** preserve the already-correct visual shape — literal Feed items were already text-only with no artwork, matching Model A's no-image form and the dedicated Feeds tab — but route their metadata through shared `paint_hero_content` Model A instead of Home's bespoke painter. Home Audiobookshelf episode artwork remains unchanged.
- **Selection mode modal:** Enter on a surface with constituent items (seasons, tracks, episodes) opens a modal listing those items. Enter selects; Esc cancels and returns to the library. Reuses the existing modal-frame vocabulary used by confirm, multiselect, and context-menu overlays.

## Capabilities

### New Capabilities
- `inline-hero-selection-modal`: the modal that lists constituent items (seasons, tracks, episodes) when Enter is pressed on an inline-hero surface, reusing the existing modal-frame vocabulary.

### Modified Capabilities
- `library-list-hero`: the inline presentation requirement changes — the inline hero shows title + meta + overview + image only on every surface; structured lists (seasons, episodes, tracks, chapters) are no longer rendered inline but via the selection modal. Scenarios for narrow TV, narrow Music, narrow podcasts, and narrow ABS books change from "detail replaces row including structured list" to "detail replaces row with hero content only; Enter opens selection modal."
- `music-library-hero`: the narrow grouped Music requirement changes — the selected album's track list no longer renders inline; the hero shows album title + metadata + artwork only; Enter opens the track selection modal.
- `audiobookshelf-podcast-library-ui`: the podcast hero requirement changes — alphabetical browsing pills move to the panel; the hero shows title + author + description + cover only (standard Model A); episodes move to the selection modal. The played/unplayed filter lives in the selection modal, not in the panel or the hero.
- `right-panel-arrangements`: the inline presentation specification tightens — the inline hero is one content shape (title + meta + overview + image) across all surfaces, with model selection by image aspect ratio. Surfaces no longer declare bespoke inline content.
- `ui-design-language`: the closed structural vocabulary requirement extends to the inline hero — no surface may add a bespoke content path, extension block, or in-hero pill bar. The closed set is the shared `HeroContent` model (Model A) and the shared beside-image model (Model B).

## Impact

- **`src/app/render/components/detail_series_view.rs`**: the inline series detail loses its seasons/episodes extension; `render_series_inline_detail` calls `paint_hero_content` only.
- **`src/app/render/components/album_detail.rs`**: the inline album detail is replaced by a Model A hero call; the track list moves to the modal.
- **`src/app/render/components/audiobookshelf.rs`**: `render_audiobookshelf_hero` loses its hand-painted author/description block and in-hero filter pills; routes through `paint_hero_content` with author/description as `HeroLine`s. Alphabetical pills added to the panel.
- **`src/app/render/components/audiobookshelf_books.rs`**: switches from `beside_image_hero_dims` (Model B) to `paint_hero_content` (Model A) for the book cover.
- **`src/app/render/components/home_latest_row.rs`**: route non-Audiobookshelf Home items (currently literal Feed entries) through shared `paint_hero_content` Model A; preserve the Audiobookshelf-only artwork branch.
- **`src/app/render/tests_home_inline.rs`**: retain the Home Feed shape/routing characterization; the Audiobookshelf-only artwork path in `home_latest_row.rs` is not changed.
- **`src/app/render/components/hero.rs`**: `HeroContent` may need a third spacer mode (or spacer sentinel in the lines vec) to accommodate the podcast author/description block without hand-painting.
- **New modal component**: a constituent-list modal reusing `modal_frame.rs`, with item selection and Esc-to-cancel behavior.
- **`src/app/input*.rs`**: Enter key handling gains a path to open the selection modal for surfaces with constituent items.
- **Characterization tests**: each drift fix PR includes a characterization buffer test (if coverage is missing) then the migration, with the test updated to reflect the intended buffer change.
