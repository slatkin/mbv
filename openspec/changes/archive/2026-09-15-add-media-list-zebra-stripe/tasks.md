## 1. Add zebra policy type and paint-policy field

- [x] 1.1 Add `ZebraStripe { focused: Color, unfocused: Color }` struct to `src/app/components/media_list/mod.rs` and add an `Option<ZebraStripe>` field to `WideMediaListPaintPolicy`, defaulting to `None` in `new()`, `for_queue()`, and `for_library_workspace()`. Add a `with_zebra(self, zebra: ZebraStripe) -> Self` builder. Expose a `pub(crate) fn zebra_bg(&self) -> Option<Color>` that resolves the focused/unfocused colour from the stored policy and `self.focused`. Verify: `cargo check -p mbv-core` if the type is there, else `cargo check -p mbv`.

## 2. Thread zebra background through the row painter

- [x] 2.1 In `render_wide_media_list` (`src/app/render/components/media_list/wide.rs`), resolve `policy.zebra_bg()` once before the row loop. Maintain a `visible_item_index: usize` counter that increments only when the source row is a selectable `Item`. Pass `zebra_bg.filter(|_| visible_item_index % 2 == 1)` as a new `Option<Color>` parameter to `media_list_row`. Increment the counter after each `Item` row. Verify: `cargo check -p mbv`.

- [x] 2.2 In `media_list_row` (`src/app/render/components/media_list/row.rs`), add an `alternate_bg: Option<Color>` parameter. In the `ListItem::style` expression, change the non-selected branch from `Style::default()` to `alternate_bg.map_or(Style::default(), |bg| Style::default().bg(bg))`. Verify: `cargo check -p mbv`.

## 3. Enable on queue

- [x] 3.1 In `src/app/render/components/queue.rs:31`, chain `.with_zebra(ZebraStripe { focused: Color::from_u32(0x003c4841), unfocused: Color::from_u32(0x00333c43) })` onto the `WideMediaListPaintPolicy::for_queue(focused)` call. Verify: `cargo check -p mbv` and `cargo nextest run -p mbv` pass. Run the app and confirm queue rows alternate backgrounds.

## 4. Test

- [x] 4.1 Add a unit test in the media-list render test module that constructs a `WideMediaList` with 4+ selectable items, paints with a zebra-enabled policy, and asserts the 2nd and 4th visible item rows have the zebra background while the 1st and 3rd do not. Verify: `cargo nextest run -p mbv` passes the new test.
