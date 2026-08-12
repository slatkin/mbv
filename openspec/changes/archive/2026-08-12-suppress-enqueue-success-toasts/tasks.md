## 1. Suppress Enqueue Success Feedback

- [x] 1.1 Remove the single-item library `Added: …` success flash and its now-unused message preparation, then update the helper documentation to describe append, synchronization, persistence, and rollback without promising a confirmation toast.
- [x] 1.2 Remove the Feed entry `Added: …` success flash and its now-unused message preparation while leaving synchronization, persistence, tracking retirement, error feedback, and rollback unchanged.

## 2. Verification

- [x] 2.1 Search the application enqueue paths to confirm no `Added: …` success flashes remain and existing enqueue error flashes are still present.
- [x] 2.2 Run `rtk cargo fmt --all -- --check`, `rtk cargo check`, and `rtk make check-code-file-lines`.
