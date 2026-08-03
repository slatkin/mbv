## Context

Saved-position resume is centralized by the media item's resume decision. The current known-runtime boundary is an integer 1 percent comparison; unknown runtime accepts any positive position.

## Goals / Non-Goals

**Goals:**

- Change the known-runtime boundary once for every existing caller.
- Preserve exact inclusive boundary behavior without floating-point rounding.

**Non-Goals:**

- Adding configuration or media-type-specific thresholds.
- Changing progress reporting, watched-state, or near-end completion.

## Decisions

Replace the existing integer 1 percent comparison with an inclusive 6 percent comparison in the shared resume decision. Use overflow-conscious integer arithmetic and preserve the existing positive-position fallback when runtime is unknown.

Alternative considered: make the threshold configurable. Rejected because this change establishes one product policy and does not justify another setting.

## Risks / Trade-offs

- **[Risk] Users lose a desired resume point below 6 percent** → The item restarts from the beginning; no saved data is deleted, and the boundary is explicit and covered at exactly 6 percent.
- **[Risk] Arithmetic changes produce boundary drift** → Cover immediately below, exactly at, and immediately above 6 percent with integer-boundary tests.

## Migration Plan

Apply the shared decision change with no persisted-data migration. Rollback restores the 1 percent comparison.
