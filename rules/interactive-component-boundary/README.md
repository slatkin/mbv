# interactive-component-boundary

Path-scoped `ast-grep` rules enforcing the Interactive Component ownership
boundary (openspec/changes/migrate-tui-to-tuirealm design D10; spec
`interactive-component-framework` "Interactive ownership is mechanically
enforced"). Every rule scopes to `src/app/components/**` and rejects one class
of shell/runtime leak:

| Rule | Rejects |
| ---- | ------- |
| `no-impl-app` | `impl App` / `impl Trait for App` |
| `no-app-as-type` | importing or using `App` as a type |
| `no-service-client-deps` | `EmbyClient`, `AudiobookshelfClient`, `CastClient`, `SharedClient`, `PlayerProxy`, `RemotePlayer` |
| `no-mpsc-ownership` | `std::sync::mpsc` imports/usages |

## Verify

- Boundary scan (the local gate, also run by CI): `rtk ast-grep scan`
- Rule fixtures (one accepted + one rejected per rule): `rtk ast-grep test`

Fixtures live in the sibling `rules/interactive-component-boundary-tests/`
directory (registered via `testConfigs` in `sgconfig.yml`), kept outside this
`ruleDirs` entry so `ast-grep scan` never loads them as rules. Snapshots under
`rules/interactive-component-boundary-tests/__snapshots__/` lock each rejected
fixture's exact output; regenerate with `rtk ast-grep test -U` after an
intentional rule change.
