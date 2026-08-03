## Why

mbv currently resumes known-runtime media after only 1 percent progress, which can preserve trivial startup progress as though the item were meaningfully in progress. Raising the boundary makes short, accidental starts restart cleanly while retaining real resume points.

## What Changes

- Treat known-runtime saved positions below 6 percent as non-resumable.
- Treat exactly 6 percent and greater as resumable.
- Preserve current positive-position behavior when runtime is unknown.

## Capabilities

### New Capabilities

- `playback-resume`: Defines the minimum saved progress that mbv treats as resumable.

### Modified Capabilities

None.

## Impact

- Changes playback startup for known-runtime items saved between 1 and 6 percent.
- Affects the shared saved-position resume decision used by current playback paths.
- Adds no dependency, configuration, or persisted-data migration.
