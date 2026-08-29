## Context

Three structs each carry content and interaction state together:

```
        BrowseLevel                    what actually owns it
        ───────────                    ─────────────────────
        items, total_count             shell   (fetched)
        loading, all_items             shell   (fetch lifecycle)
        sort_by, sort_order            shell   (query parameters)
        item_types, unplayed_only      shell   (query parameters)
        letter_filter, music_grouping  shell   (query parameters)
        parent_id, title               shell   (navigation identity)
        ───────────────────────────────────────────────────────
        cursor                         BOTH — see D1
        scroll                         BOTH — see D1
```

Because the component holds a clone of the whole struct, the bottom two rows
are reachable from both sides, and every projection must hand-patch them.

## Decisions

### D1 — `cursor` is two different facts wearing one name

`BrowseLevel.cursor` means two things depending on whether its level is the
visible one:

| | Live cursor | Resting position |
|---|---|---|
| Which level | The visible one | A level below the top of `nav_stack`, or a non-active library |
| Who changes it | The user, continuously | The shell, at a navigation event |
| Who needs to read it | The painter and local input | `save_default_library_position`, restore-on-entry, `go_back`'s parent re-anchor |
| Persisted | No | Yes (`LibraryPosition`) |

Conflating them is why "is this read a mirror?" cannot be answered
mechanically today — `actions_navigation.rs:244` reading `parent.cursor` after
a pop is legitimate resting-position access, while `actions.rs:139` reading
`lvl.items.get(lvl.cursor)` on the visible level is a mirror read. Both are
spelled identically.

Splitting them means the type tells you which you have, and the mirror read
stops compiling.

*Rejected:* keeping one field and adding a rule/comment about which uses are
sanctioned. That is what the tree does now, and it is why an ast-grep rule and
several warning comments exist.

### D2 — Three outcomes per reader, decided by inventory, not by guesswork

Every reader of a removed field resolves to exactly one of:

1. **Takes the value as a parameter.** The caller already knows the resolved
   item or index. This is the pattern `remove-tv-workspace-cursor-mirror`
   established with `activate_selected_series_item` and
   `remove-browser-cursor-scroll-mirror` used for `apply_lib_cursor_index`'s
   argument. Expected to be the large majority.
2. **Reads the resting position.** Persistence, restore, `go_back`'s parent
   re-anchor. Unchanged in behaviour; changed in spelling.
3. **Reads the component.** Only where the shell genuinely needs the live
   value at an event, via the existing sanctioned downcast accessors.

A reader that fits none of the three is a finding: stop and report it rather
than inventing a fourth path. #611's own history is the argument for this rule
— two of its four slices were sized wrongly because a field's reachability was
assumed rather than traced.

### D3 — Inventory is type-aware, not grep

`.cursor` appears ~74 times outside tests, but most belong to other structs
(`SelectionModal`, `Feeds`, `SearchSidebar`, and the components' own state).
The authoritative inventory comes from `rtk ast-grep`, matching field access
on the `BrowseLevel` type, not from a text search. #618's scout recorded ~37
non-test `BrowseLevel` readers; task 1 confirms or corrects that figure and
records it before any field moves.

### D4 — Migrate one struct at a time, deepest dependency first

Order: `AudiobookshelfBookBrowseState`, then `AudiobookshelfBrowseState`, then
`BrowseLevel`. The two Audiobookshelf structs are smaller, have a single
component each, and validate the split shape before it meets `BrowseLevel`'s
reader population. Each is independently shippable and independently
verifiable.

*Rejected:* one atomic change across all three. The compiler-forced edit set
for `BrowseLevel` alone is large enough that bundling it with the others
produces a diff no reviewer can hold in their head, and this project's
practice is to split large task groups across sequential agents rather than
run one oversized unit.

### D5 — Deletion, not deprecation

A field is removed in the same task that re-points its last reader. No
transitional accessor is left behind returning the old value; that would
recreate the mirror at one remove and it would survive, because nothing forces
its removal later.

## Risks

- **File-size cap.** Splitting structs and threading parameters will push
  several `src/app/*.rs` files over 800 lines. AGENTS.md requires splitting in
  the same PR. Budget for it; do not gate mid-project units on the cap
  (`rtk make check-code-file-lines` is a pre-PR check, not a per-task one).
- **A reader that needs the live cursor at a point where no component is
  mounted.** Destination components can be unmounted; a shell reader that
  needs the live value then has no source. Expected to surface in
  `save_default_library_position` on tab-switch-away. The resting position is
  the answer, but the ordering (persist before unmount) must be verified, not
  assumed.
- **Silent restore regressions.** Position restore is the behaviour most
  exposed by this change and the least covered by tests. Every struct's task
  group opens with a restore characterization test.
- **Mouse.** `mouse_gestures.rs` reads and writes these fields freely and is
  accepted-broken. Deleting its callees will require deleting or stubbing its
  call sites; that is in scope, repairing its behaviour is not.
