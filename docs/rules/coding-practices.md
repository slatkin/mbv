# Coding Practices

## Code-file size

Every governed tracked code file must contain no more than 800 physical lines.
The limit is strict: a file at 800 lines passes, and a file at 801 lines fails.
The canonical local check is:

```text
make check-code-file-lines
```

The checker uses `git ls-files` and counts physical lines with `wc -l`. It reports
all ordinary size violations in one run and is the single source of truth used
by local checks and CI. Enumeration and filesystem errors also fail the check.

### Governed files

Tracked files are governed when they use one of these extensions:

- `.rs`
- `.lua`
- `.sh`
- `.py`
- `.js`
- `.ts`
- `.tsx`
- `.c`
- `.h`
- `.cpp`
- `.hpp`

The following extensionless or packaging paths are also governed:

- `Makefile`
- `PKGBUILD`
- `PKGBUILD-git`
- `.githooks/*`

This includes tests, `build.rs`, and code under `scripts/`.

The classifier explicitly excludes documentation and non-code artifacts,
including `docs/**`, `.github/**`, `openspec/**`, `dist/**`, `assets/**`,
`fonts/**`, `contrib/*.service`, TOML/JSON/YAML files, lockfiles, generated
output, and binary assets.

Adding a source language or extensionless code path requires updating both the
checker and this list in the same change. Governed files do not receive
permanent per-file exceptions or grandfathered status.

### Remediation

When a governed file exceeds the limit, split it at a cohesive responsibility
boundary and preserve its behavior, public API, test inventory, fixture access,
and package inclusion. Do not solve the violation with arbitrary line slicing
or a permanent exclusion.

### CI and branch protection

The `Code File Lines` workflow runs on pull requests and pushes to `main`, and
invokes the same `make check-code-file-lines` target used locally. Workflow
files cannot configure GitHub branch protection. A repository administrator
must add the `code-file-lines` check as a required status check for `main` in
GitHub Settings > Rules/Branches. Verify the setting with:

```text
gh api repos/slatkin/mbv/branches/main/protection/required_status_checks
```
