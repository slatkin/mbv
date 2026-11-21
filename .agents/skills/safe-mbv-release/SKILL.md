---
name: safe-mbv-release
description: Safely prepare and publish an mbv release using the repository's release script. Use this whenever the user asks to release an mbv version, push a release tag, delete and retag a version, or asks whether mbv is ready to release. Do not use for release-note writing or ordinary version discussion that does not change tags or publish artifacts.
---

# Safe mbv Release

Use the repository's `tools/release.sh` as the single source of truth for
release preparation. The script runs the project checks (`cargo nextest run`,
`cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`), updates
`Cargo.toml`, `Cargo.lock`, and `PKGBUILD` (`pkgver`, `sha256sums='SKIP'` —
build.yml fills the real hash when it builds the release tarball), commits the
release, and pushes. On `main` it also tags and pushes the tag itself. Do not
reproduce those steps manually unless the script is unavailable or the user
explicitly asks to change it.

Releases never go through a pull request. The release commit pushes directly
to `main` and the tag pushes directly to `origin`. Never open a release PR,
and never merge release prep through one.

## Preflight

Before changing anything, inspect:

```bash
git status --short
git branch --show-current
git log -1 --oneline
git tag --sort=-version:refname | head -10
```

- Confirm the requested version and release summary. If the summary is absent,
  derive a concise one from commits since the previous tag and show it before
  running the release command; do not invent product claims.
- Read `tools/release.sh` before invoking it, especially if the script has
  changed since the last release.
- Old CI runs on `main` are not a release gate: the release commit is not on
  `main` yet, and the release script runs the full gate set itself. Do not
  poll `gh run list` before running the script.
- Check both local and remote tag state. Stop if the release tag already exists
  locally or on `origin`; the script only protects against an existing local
  tag. Preserve unrelated work rather than committing it as part of the
  release.
- A readiness question is read-only: inspect and report the preflight state but
  do not run the release script, commit, push, or tag unless the user asks to
  perform the release.
- Stop on uncommitted changes to *tracked* files or detached `HEAD`. Untracked
  files never enter the release commit, but only untracked `*.md` notes are
  tolerated: any other untracked file could be a source the tag would silently
  omit, so it also stops the release (the script enforces the same rule).
- Do not bypass the script's checks or force a tag as a shortcut.

## Normal Release

Run the repository-provided command with the normalized version and summary:

```bash
tools/release.sh <version> "<summary>"
```

The script accepts either `0.x.y` or `v0.x.y`; Cargo uses the version without
the `v` prefix and the Git tag uses `v0.x.y`.

The script runs the CI gate set first (the same checks, in the same order, as
`.github/workflows/build.yml`): fmt, clippy, ast-grep, `cargo audit`, the
release builds, release tests for the workspace and `mbv-gui`, and the
feature matrix. A failure there means the release push would fail CI the
same way; fix the cause and rerun the script. Do not skip or shorten the
gate set, and never bypass it with a manual tag.

On `main`, the script pushes the release commit to `main`, tags it `vX.Y.Z`,
and pushes the tag — no PR, no release branch. The tag push triggers the
PKGBUILD publish run (tarball, debs, GitHub release, AUR hand-off). Then
verify:

```bash
git status --short
git ls-remote origin "refs/tags/v<version>"
gh run list --repo slatkin/mbv --limit 3
```

The remote tag must point at the release commit on `main`, and the publish run
must be running. Use `gh` to inspect checks or release status when the
repository exposes them; do not assume a workflow name or that a pushed tag
means the release completed.

On a feature branch, the script only prepares and commits the version bump.
Do not tag it there. Report that the branch must be merged into `main`
before the tag is created (then `tools/release-tag.sh <version>`), and keep
release publishing separate from unrelated PR work.

Never wait for the release github action to complete as part of this process.

## Retagging

Treat deletion of a published tag as destructive. Only do it when the user
explicitly requests the retag.

1. Resolve the local and remote tag targets before deleting anything:

   ```bash
   git rev-list -n 1 "v<version>"
   git ls-remote origin "refs/tags/v<version>"
   ```

2. Confirm which commit the replacement tag should reference.
3. Delete the local tag and the exact remote tag explicitly. Do not force-move
   a tag without an explicit user instruction.
4. Recreate and push the tag from the intended commit.
5. Verify the local and remote tag targets match and report both values.

If the tag is consumed by published artifacts, check the repository's release
status after retagging and call out that already-published artifacts may still
reference the old commit.

## Verification Report

End with a compact factual report containing:

- normalized version and tag;
- commit that carries the release;
- branch used;
- checks run by the release script and their result;
- whether the commit and tag were pushed;
- remote tag verification and any pending CI or release status;
- any step intentionally not performed, such as tagging from a feature branch.

Do not claim a release succeeded based only on a successful local command. A
release is complete only after the relevant remote commit/tag and available CI
or release status have been checked.
