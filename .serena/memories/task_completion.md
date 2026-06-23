# Task Completion

Run these before considering a coding task done:

```sh
cargo build        # must compile clean (zero errors)
cargo clippy       # must pass with zero warnings
cargo test         # must pass
```

Fix all clippy warnings by deleting unused code — do not use `#[allow]`.

Always ask the user before committing or pushing.
