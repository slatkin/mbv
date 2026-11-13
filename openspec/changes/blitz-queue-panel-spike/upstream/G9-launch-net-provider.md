# Upstream draft G9: `dioxus-native` launch installs a fixed net provider

Status: draft. Not posted. The user approves each draft before anyone posts it (decision 6).

Gap: design G9 in `design.md`.

## Target

Open a new issue in https://github.com/DioxusLabs/dioxus (the `dioxus-native` crate), if the user approves. No open or closed issue asks for an injection point. The issue links the closed issue #4898 and the merged PR #4899 as the nearest related work, and it names Blitz's `NetProvider` trait as the contract (Blitz repository, `blitz-traits`).

## What was searched (2026-11-13)

Read-only searches with `gh` and the cargo registry. Nothing was posted.

- Dioxus issues, all states, for `NetProvider`, `launch_cfg_with_props`, `launch_cfg`, `net provider`, `dioxus-native custom protocol`, `dioxus-native net`, `dioxus-native assets image`, `custom asset provider dioxus-native`, `dioxus-native image not loading`, `blitz-net`, `DioxusNativeNetProvider`, and `data-uri`. Matches on the `dioxus-native` gap:
  - #4898, `dioxus-native cannot be used without the net feature flag.` (closed). The build failed with no `net` feature because `blitz_net` was referenced in `assets.rs`.
- Dioxus pull requests for the same terms. Matches:
  - PR #4899, `Fix dioxus-native to run without the net feature` (merged 2025-11-03). It fixes #4898. The `net` gate in `assets.rs` and `lib.rs` is the result.
  - PR #5650, `feature flag blitz-shell behind new shell feature` (open). It addresses issue #5647 (the double build of winit). It does not add an injection point.
- Blitz issues and pull requests for `DataUriNetProvider`, `custom net provider`, and `NetProvider`. No match on an injection point for `dioxus-native`.
- Source of `dioxus-native` 0.8.0-alpha.3 (cargo registry):
  - `src/lib.rs:186-200`: with `net`, the launch builds `DioxusNativeNetProvider::with_inner(proxy, blitz_net::Provider)` and provides the inner provider as a root context.
  - `src/lib.rs:198-200`: without `net`, the launch calls `DioxusNativeNetProvider::shared(proxy)`.
  - `src/lib.rs:12`: `mod assets;`. The provider type is private. `src/lib.rs` does not re-export it.
  - `src/lib.rs:217`: `DocumentConfig { net_provider: Some(net_provider), .. }`. The launch takes the provider from its own body. The app has no argument for it.
  - `src/assets.rs:41-60`: `fetch` serves `dioxus:` URLs from the file system. Any other URL goes to the inner provider. With no inner provider, it logs at `warn` under the `tracing` feature only and never calls the handler.
- WPT: not applicable. This gap is a Rust API. No web-platform test covers it.

## Summary for the issue

`dioxus-native` launches a document with a provider it builds itself. The app cannot pass its own `NetProvider` to the launch. The provider type (`DioxusNativeNetProvider`) is private, and `with_inner` is public only inside that private module. An app that needs its own image or font source must copy the launch body, which is about 100 lines, and replace the provider there.

Without the `net` feature, a request for any URL other than `dioxus:` gets no response. The handler is dropped. With the `tracing` feature off, nothing is logged.

## Minimal repro

This gap is in the Rust API, so no HTML or CSS repro applies. The Rust side:

```toml
[dependencies]
dioxus-native = { version = "=0.8.0-alpha.3", default-features = false, features = ["prelude", "svg", "system-fonts", "woff", "vello-hybrid"] }
```

```rust
fn app() -> Element {
    rsx! { img { src: "https://example.invalid/cover.png", alt: "Cover" } }
}

fn main() {
    dioxus_native::launch(app);
}
```

Expected: the app has a way to supply a `NetProvider` (for example an in-process provider), or the launch fetches the image through a provider the app chooses.

Actual in 0.8.0-alpha.3: no way to pass a provider. The image is never fetched, and no response reaches the document.

Status of this repro: not run. The expected and actual results come from the source read. Run the repro before posting.

## Suggested ask

A launch option that takes an `Arc<dyn NetProvider>` (or a builder method on the launch config) and passes it to `DocumentConfig`. The default stays as it is. The `dioxus:` route can stay inside the default provider.

## Notes for the user

- The spike works around this gap with a copy of the launch body (design D6, probe P2). The gap itself stays `absent` in the pinned crate.
- Do not post this draft until the user approves it.
