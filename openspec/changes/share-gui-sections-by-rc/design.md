# Design

## Context

`Sections` (`crates/mbv-gui/src/window/sections.rs`) holds one `Signal<T>` per section. `write`
does `signal.set(S::from(section.value.clone()))`: one clone per written section per batch. That
clone stays. The cost is on the read side: a component reads with `signal.cloned()`, which
subscribes and deep-clones the whole value on every render. See proposal.md - Why.

Constraints found in the code:

- `GuiSectionBatch` crosses a channel from the core thread, so it holds plain `Send` values. The
  `Rc` is made on the window side, inside `write`.
- `library_pair` and the `library_music_list`/`library_*_list` narrowing functions already take
  `Option<&GuiLibraryList>` and `Option<&GuiLibraryDetail>`. Borrowing from a handle needs no
  change to them.
- Dioxus props need `Clone + PartialEq + 'static`. The Hero and pill painters take owned detail
  parts today, so each render clones them.
- `peek()` readers in `window/library/` (keys, state, destination modules) and
  `window/settings/state.rs` read through a borrow, or clone out once per key. They keep working
  through `Deref`.
- A section value at one revision is immutable (documented on `library_list_revision`).
- The window is single-threaded (Blitz, one task). `Rc` is sound; no `Arc` is needed.

## Goals / Non-Goals

**Goals:**

- One deep copy of a section per section update, however many components read it.
- A prop memo check that costs O(1) in the size of the section.

**Non-Goals:**

- No change to revisions, `apply_batch` gating, or the batch type.
- No change to `tabs` or `pills` (small, bounded by tab and pill count).
- No change to the Client core, ctrl, or any crate other than `mbv-gui`.

## Decisions

**D1. A `SectionRef<T>` newtype over `Rc<T>`, in `window/sections/shared.rs`.**
`Deref<Target = T>`, `Clone`, and a `PartialEq` that is `Rc::ptr_eq`. Alternatives:

- Bare `Rc<T>`. `Rc<T>: PartialEq` shortcuts on pointer equality only when `T: Eq`. The Library
  frames derive `PartialEq` only, so a bare `Rc` would still walk every row in a prop memo
  check. Rejected.
- `Arc<T>`. `AGENTS.md` prefers no new `Arc`/`Rc`, and the window is single-threaded. `Rc` is the
  narrower exception. The user approved `Rc` for the section signals.
- Hold `read()` guards and pass `&T` down. A guard cannot cross a prop boundary and keeps the
  signal borrowed through the whole render. Used only inside one function, where
  `library_pair` already does so.
- `use_memo` per field. It clones once per change, but it adds a memo per field and per
  component, and the memo's equality check walks the value. Rejected for the large sections.

Pointer equality is sound because a section at one revision is immutable: the same pointer is
the same content. A new revision always allocates a new `Rc`, so a changed section never
compares equal.

**D2. Four signals change type.**

| Signal | Before | After |
|---|---|---|
| `library_list` | `Signal<Option<GuiLibraryList>>` | `Signal<Option<SectionRef<GuiLibraryList>>>` |
| `library_detail` | `Signal<Option<GuiLibraryDetail>>` | `Signal<Option<SectionRef<GuiLibraryDetail>>>` |
| `queue` | `Signal<GuiQueue>` | `Signal<SectionRef<GuiQueue>>` |
| `settings` | `Signal<Option<GuiSettingsFrame>>` | `Signal<Option<SectionRef<GuiSettingsFrame>>>` |

`write` takes `S: From<SectionRef<T>>`-style conversions through one helper: it builds
`SectionRef::new(section.value.clone())` once and sets the signal. The other signals keep their
types (small values). `fresh_queue()` wraps in `SectionRef::new`.

**D3. Readers borrow or hold the handle.**

- `signal.cloned()` stays; it now clones a handle. Narrowing uses `as_deref()` to get
  `Option<&T>` for `library_pair` and the `library_*_list` functions.
- `LibraryPanel` passes the detail handle (not a cloned part) to the Hero painters. Each Hero
  narrows with `library_*_detail(Some(&detail))` inside its own body. The Hero props become
  `detail: SectionRef<GuiLibraryDetail>`. `LibraryFeedsHero` takes the list handle in place of
  `rows: Vec<_>`.
- Every component that takes a section value as a prop takes the handle instead.
  `LibraryStatusBar`'s `list` and `detail` props are included: they become
  `Option<SectionRef<..>>`. No call site rebuilds an owned value from a handle
  (`as_deref().cloned()` is forbidden), because that brings back the per-render deep clone.
- The destination lists keep reading `library_list` and `library_detail` from the section store
  themselves, as `LibraryList` does today. They now clone handles.
- `QueuePanel` clones the handle in the render body and in the `use_effect`. The `RowMenu`
  overlay clone stays (a small frame).
- `settings_section_view` returns a `SettingsSectionView` that holds the `SectionRef` in place of
  an owned `GuiSettingsFrame`.

**D4. One contract test, no bench.** The user declined a measurement step. The contract is
the new spec requirement. Its owner is the `window/sections` test layer (the section-store
harness), which already tests "a section at its held revision is ignored". One new test there
asserts: after a batch that names only another section, the `queue` handle is
`ptr_eq` to the handle before the batch; and two reads of one section are `ptr_eq`. Existing
component tests keep their coverage and only change in how they build fixtures
(`SectionRef::new`).

No new dependency, version, or feature. `std::rc::Rc` only.

## Risks / Trade-offs

- [A reader mutates through the handle and breaks the immutability that D1 relies on] →
  `SectionRef` exposes `Deref` only, never `DerefMut` or `Rc::make_mut`.
- [A fixture in a test sets a signal with a bare value and fails to compile] → The compiler finds
  every site; the fix is `SectionRef::new(...)`. Tests that only `peek()` need no change.
- [Wide diff: about 90 reader sites, most in tests] → The type change comes first, then the
  components, so every step compiles. Each task ends with `cargo check -p mbv-gui --all-targets`.
- [An `Rc` held in a `use_effect` closure or a spawned task keeps a stale section alive] →
  Acceptable: the handle frees when the closure drops, and the effect reads a fresh handle on
  each run.
- [`tabs` and `pills` still clone per render] → Accepted; bounded by tab and pill count.

## Migration Plan

One PR, no data or config change. Roll back by reverting the PR.
