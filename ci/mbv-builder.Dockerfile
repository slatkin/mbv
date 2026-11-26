# mbv-builder: the frozen CI build environment for mbv.
#
# Every build.yml and aur.yml job runs inside this image. It carries the
# complete toolchain — pacman makedeps, the pinned rustup toolchain, and the
# cargo tools — so CI performs zero provisioning at runtime: no pacman, no
# rustup downloads, no install-actions. Nothing in a CI run depends on the
# Arch mirrors, the Arch Linux Archive, or any rolling package state.
#
# Rebuild and push manually (docker CLI against any host):
#
#   docker build -f ci/mbv-builder.Dockerfile \
#     -t ghcr.io/slatkin/mbv-builder:20260818-rust1.99.0 ci
#   docker push ghcr.io/slatkin/mbv-builder:20260818-rust1.99.0
#
# Refresh cadence is deliberate: bump the base image tag and the Arch Linux
# Archive date together (they must match — the mirrorlist pin below is what
# makes the image content exactly the snapshot, not a rolling upgrade), and
# bump the rustup toolchain when rust-toolchain.toml moves. Tag scheme:
# <archive-date>-rust<toolchain>. Old tags stay in GHCR, so an old commit
# always rebuilds in the environment it was written for.
#
# History note: before this image, every job ran `pacman -Syu` at runtime.
# Against the live mirrors that pulled OpenSSL 4 (libssl.so.4) into a .deb
# that then failed to load on Debian 13 (libssl.so.3 only; mbvd exit 127).
# The archive pin fixed that, but runtime provisioning stayed slow and
# flaky (archive.archlinux.org throttles). Freezing the environment into an
# image removes the whole class.

FROM archlinux/archlinux:base-devel-20260818.0.575430

# Pin the package snapshot to the base image's own date BEFORE any pacman
# operation: the Arch Linux Archive keeps the dated snapshot, so -Syu here
# converges the image to exactly the 2026-08-18 state instead of the rolling
# mirrors. Bump this date together with the base image tag above.
RUN printf 'Server = https://archive.archlinux.org/repos/2026/08/18/$repo/os/$arch\n' \
      > /etc/pacman.d/mirrorlist \
 && pacman -Syu --noconfirm \
 && pacman -S --noconfirm --needed \
      rustup clang nasm mpv pipewire openssl dpkg github-cli git openssh \
      zig wayland libxkbcommon fontconfig ttf-dejavu ast-grep \
      editorconfig-checker sccache cargo-nextest cargo-audit cargo-deb \
      cargo-hack \
 && pacman -Scc --noconfirm \
 && rm -rf /var/cache/pacman/pkg/*
# Package roles, for the next person who edits the list:
#   rustup                toolchain manager; the real toolchain is below
#   clang nasm zig        native build-script tooling (aws-lc-rs, ring, etc.)
#   mpv pipewire openssl  pkg-config probes in build scripts; libmpv linking
#   dpkg                  cargo-deb assembles .deb packages
#   github-cli git openssh  checkout, release upload, AUR push
#   wayland libxkbcommon fontconfig ttf-dejavu
#                         GUI build deps; ttf-dejavu gives Blitz headless
#                         layout tests a system font (without one, text nodes
#                         return no layout box and get_client_bounding_rect
#                         panics — CI runs on 2026-11-18/19 failed there)
#   ast-grep              repo rule tripwires
#   editorconfig-checker  the EditorConfig gate
#   sccache               RUSTC_WRAPPER object cache
#   cargo-nextest cargo-audit cargo-deb cargo-hack
#                         the cargo subcommands the gates invoke

# The pinned toolchain, baked. rust-toolchain.toml in the repo selects this
# exact channel at runtime, so rustup resolves it locally and never touches
# the network. Must move with rust-toolchain.toml's `channel`.
RUN rustup toolchain install 1.99.0 --profile minimal --component clippy,rustfmt \
 && rustup default 1.99.0 \
 && rm -rf /root/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/share/doc

# Build-time proof that every gate's entry point resolves inside the image:
# a missing package or a broken rustup shim fails the image build here, not
# a CI run later.
RUN cargo --version \
 && rustc --version \
 && cargo clippy --version \
 && cargo nextest --version \
 && cargo deb --version \
 && cargo hack --version \
 && cargo audit --version \
 && sccache --version \
 && ast-grep --version \
 && editorconfig-checker --version \
 && gh --version \
 && git --version
