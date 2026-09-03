---
name: safe-mbv-release
description: Safely prepare and publish an mbv release using the repository's release script. Use this whenever the user asks to release an mbv version, push a release tag, delete and retag a version, or asks whether mbv is ready to release. Do not use for release-note writing or ordinary version discussion that does not change tags or publish artifacts.
---

# Safe mbv Release

Use the repository's `scripts/release.sh` as the single source of truth for
release preparation. The script runs the project checks, updates `Cargo.toml`
and `Cargo.lock`, commits the release, and applies different behavior on
`main` versus a feature branch. Do not reproduce those steps manually unless
the script is unavailable or the user explicitly asks to change it.

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
- Read `scripts/release.sh` before invoking it, especially if the script has
  changed since the last release.
- Check both local and remote tag state. Stop if the release tag already exists
  locally or on `origin`; the script only protects against an existing local
  tag. Preserve unrelated work rather than committing it as part of the
  release.
- A readiness question is read-only: inspect and report the preflight state but
  do not run the release script, commit, push, or tag unless the user asks to
  perform the release.
- Stop on a dirty tree or detached `HEAD`.
- Do not bypass the script's checks or force a tag as a shortcut.

## Normal Release

Run the repository-provided command with the normalized version and summary:

```bash
scripts/release.sh <version> "<summary>"
```

The script accepts either `0.x.y` or `v0.x.y`; Cargo uses the version without
the `v` prefix and the Git tag uses `v0.x.y`.

On `main`, the script pushes the release commit, creates the tag, and pushes
the tag. Verify afterward that:

```bash
git status --short
git rev-parse HEAD
git ls-remote origin "refs/tags/v<version>"
```

The remote tag must point at the intended release commit. Use `gh` to inspect
the repository's current checks or release status when the repository exposes
them; do not assume a workflow name or that a pushed tag means the release
completed.

On a feature branch, the script only prepares and commits the version bump.
Do not tag it there. Report that the branch must be merged into `main` before
the tag is created, and keep release publishing separate from unrelated PR
work.

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
