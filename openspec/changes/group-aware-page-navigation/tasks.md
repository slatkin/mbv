# Tasks — group-aware-page-navigation

## 1. Owner change

- [ ] 1.1 Make `MediaList::delegate_operation`'s `Page` arm group-aware: on a row flow containing at least one Heading, a forward page selects the first selectable item after the next Heading and a backward page selects the first selectable item after the previous Heading (first selectable item of the flow when none precedes); keep the ±5 selectable-row stride on ungrouped flows. Verify with `cargo nextest run -p mbv`.

## 2. Boundary tests

- [ ] 2.1 Add a `#[case]` boundary table next to the owner's existing operation tests covering: forward page lands on the next group's first item; backward page lands on the previous group's first item; forward clamp in the last group to the flow's last item; backward clamp in the first group to the flow's first item; a multi-group (range-pill-shaped) flow pages within itself and clamps at its last item; ungrouped flow keeps the ±5 stride. Verify with `cargo nextest run -p mbv`.

## 3. Verification

- [ ] 3.1 Full check: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `cargo nextest run -p mbv` (pre-existing unrelated failures noted in the session are out of scope; the delta's scenarios must pass).
