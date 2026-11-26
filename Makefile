BINARY     := mbv
# The AUR layout: the binaries resolve /usr/share/mbv and /etc/mbv at runtime,
# so PREFIX stays /usr. DESTDIR stages the tree, as makepkg's $pkgdir does.
PREFIX     ?= /usr
TAILWINDCSS ?= tailwindcss

.PHONY: all build install uninstall clean clean-dev gui-css check-code-file-lines test-check-code-file-lines dev dev-release checks format format-check test run ast audit build-release test-release feature-matrix release release-preflight release-tag

all: build

# Never run make under sudo. Make as root rebuilds the whole tree as root, and
# root's cargo cannot use the build user's sccache (it cannot read /root), so
# every rustc spawn fails with EACCES. The copies in `install` ask for the
# password themselves, and only when they write outside DESTDIR.
ifneq ($(SUDO_USER),)
$(error Run make as yourself; it uses sudo only for the final file copies)
endif

# Make passes a command-line DESTDIR down through MAKEFLAGS; OpenSSL's vendored
# build reads it and installs its libs into the staging tree, where openssl-sys
# cannot find them.
CARGO   = env -u DESTDIR -u MAKEFLAGS -u MAKEOVERRIDES -u MFLAGS cargo
# Staging (DESTDIR) needs no privileges; a system install does.
INSTALL = $(if $(DESTDIR),install,sudo install)
RM_ROOT = $(if $(DESTDIR),rm,sudo rm)

build:
	$(CARGO) build --release

# The file set the AUR packages ship (PKGBUILD-git's package()).
install: build
	$(CARGO) build --release -p mbv-owner -p mbv-gui
	$(INSTALL) -Dm755 target/release/$(BINARY) $(DESTDIR)$(PREFIX)/bin/$(BINARY)
	# The GUI Client was `mbv-gui` before it became `mbv`; a stale copy would
	# start an older build that the Owner refuses.
	$(RM_ROOT) -f $(DESTDIR)$(PREFIX)/bin/mbv-gui
	$(INSTALL) -Dm755 target/release/mbvd $(DESTDIR)$(PREFIX)/bin/mbvd
	$(INSTALL) -Dm755 target/release/mbv-tui $(DESTDIR)$(PREFIX)/bin/mbv-tui
	$(INSTALL) -Dm755 target/release/mbv-owner $(DESTDIR)$(PREFIX)/bin/mbv-owner
	$(INSTALL) -Dm644 contrib/mbv.desktop $(DESTDIR)$(PREFIX)/share/applications/mbv.desktop
	$(INSTALL) -Dm644 assets/icon.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/mbv.svg
	$(INSTALL) -d $(DESTDIR)$(PREFIX)/share/mbv/scripts
	$(INSTALL) -m644 scripts/mbv.lua scripts/mbv_*.lua $(DESTDIR)$(PREFIX)/share/mbv/scripts/
	$(INSTALL) -Dm644 fonts/Material-Design-Iconic-Font.ttf $(DESTDIR)$(PREFIX)/share/mbv/fonts/Material-Design-Iconic-Font.ttf
	$(INSTALL) -Dm644 dist/config.toml $(DESTDIR)$(PREFIX)/share/mbv/config.toml
	$(INSTALL) -Dm644 contrib/mbvd.service $(DESTDIR)$(PREFIX)/lib/systemd/system/mbvd.service
	$(INSTALL) -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/mbv/LICENSE
	$(INSTALL) -Dm644 LICENSES/MPL-2.0.txt $(DESTDIR)$(PREFIX)/share/licenses/mbv/LICENSES/MPL-2.0.txt
	$(INSTALL) -m644 crates/mbv-gui/assets/fonts/*-OFL-1.1.txt crates/mbv-gui/assets/vendor/*.txt $(DESTDIR)$(PREFIX)/share/licenses/mbv/LICENSES/
	@if [ -f $(DESTDIR)/etc/mbv/config.toml ]; then \
		echo "Kept existing $(DESTDIR)/etc/mbv/config.toml"; \
	else \
		$(INSTALL) -Dm640 dist/mbvd.toml $(DESTDIR)/etc/mbv/config.toml; \
	fi

uninstall:
	$(RM_ROOT) -f $(DESTDIR)$(PREFIX)/bin/$(BINARY) $(DESTDIR)$(PREFIX)/bin/mbvd $(DESTDIR)$(PREFIX)/bin/mbv-tui $(DESTDIR)$(PREFIX)/bin/mbv-gui $(DESTDIR)$(PREFIX)/bin/mbv-owner
	$(RM_ROOT) -rf $(DESTDIR)$(PREFIX)/share/mbv $(DESTDIR)$(PREFIX)/share/licenses/mbv
	$(RM_ROOT) -f $(DESTDIR)$(PREFIX)/share/applications/mbv.desktop
	$(RM_ROOT) -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/mbv.svg
	$(RM_ROOT) -f $(DESTDIR)$(PREFIX)/lib/systemd/system/mbvd.service
	@echo "Left $(DESTDIR)/etc/mbv/config.toml in place."
	# Legacy copies from the user-local installer removed in June 2026.
	rm -f $(HOME)/.local/bin/$(BINARY)
	rm -rf $(HOME)/.local/share/mbv

clean:
	cargo clean

# Only the dev profile (`target/debug`): the part of `target/` that grows with
# every dependency or flag change, because cargo never removes stale artifacts.
# It keeps the release build. Run it between large changes, not before every build:
# the next build recompiles the whole dependency graph.
clean-dev:
	cargo clean --profile dev

## Agent gates
# Agents run these make targets instead of cargo directly, so the flags live
# in one place. PKG=<package> narrows `checks` and `test` to one package; the
# default is the whole workspace (which includes mbv-gui). Where a target
# mirrors a .github/workflows/build.yml step, its comment names that step.
PKG ?=
SCOPE = $(if $(PKG),-p $(PKG),--workspace)

# Typecheck and lint in one pass: clippy compiles every target it lints.
# (CI: Lint)
checks:
	$(CARGO) clippy $(SCOPE) --all-targets -- -D warnings

format:
	$(CARGO) fmt --all

# (CI: Check formatting)
format-check:
	$(CARGO) fmt --all -- --check

test:
	$(CARGO) nextest run $(SCOPE)

# (CI: Ast-grep tripwires)
ast:
	ast-grep test --skip-snapshot-tests
	ast-grep scan .

# (CI: Audit dependencies)
audit:
	$(CARGO) audit

# The release-profile build of every target, GUI included.
# (CI: Build release + Build GUI Client)
build-release:
	$(CARGO) build --release --all-targets
	$(CARGO) build --release -p mbv-gui

# The throttled invocation CI uses: a third of the suite stands up a real
# libmpv handle or a loopback fixture per test, and unthrottled runs fail on
# fd/thread budget. The default --workspace scope covers mbv-gui too, so one
# run is the union of CI's "Test release" and "Test mbv-gui" steps.
test-release:
	$(CARGO) nextest run --release --test-threads=4 $(SCOPE)

# The slow one: --each-feature re-checks the workspace once per feature.
# (CI: Check feature matrix)
feature-matrix:
	$(CARGO) hack check --each-feature --workspace --all-targets

## Run

# The GUI (`mbv`) starts its Owner from the sibling `mbv-owner` program, and
# the handshake refuses an Owner whose build id differs. `cargo run -p mbv-gui`
# alone leaves `mbv-owner` stale after a commit, so build all three (`mbv`,
# `mbv-tui`, `mbv-owner`) before the window starts.
run:
	$(CARGO) build -p mbv-tui -p mbv-owner -p mbv-gui
	$(CARGO) run -p mbv-gui

dev: run

# The same as `run`, with the release profile: an optimized build that maps
# far less code than the debug binary, for memory and speed checks.
dev-release:
	$(CARGO) build --release -p mbv-tui -p mbv-owner -p mbv-gui
	$(CARGO) run --release -p mbv-gui

# The GUI's stylesheet builds from `assets/app.css` through the Tailwind
# standalone CLI (replace-tauri-with-blitz design D11). The CLI is not in the
# repository; the developer installs the v4.3.5 release asset on PATH as
# `tailwindcss`.
gui-css:
	@command -v $(TAILWINDCSS) >/dev/null 2>&1 || { \
		echo 'tailwindcss is missing from PATH.'; \
		echo 'Install the Tailwind CSS v4.3.5 release asset:'; \
		echo '  https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.5/tailwindcss-linux-x64'; \
		echo '  SHA-256 e47695cee3f0d90623a91f3939b08bdee610eb5ad99340adff9d4145b284d6cc'; \
		echo 'and put it on PATH as tailwindcss (make TAILWINDCSS=/path names it instead).'; \
		exit 1; }; \
	$(TAILWINDCSS) -i crates/mbv-gui/assets/app.css -o crates/mbv-gui/assets/app.generated.css --minify

check-code-file-lines:
	./scripts/check-code-file-lines.sh

test-check-code-file-lines:
	./scripts/check-code-file-lines-test.sh

## Release
# `make release VERSION=0.8.9 SUMMARY="short summary"` runs the CI gate set
# (the make targets above, each leaning on the others rather than open-coded
# cargo commands), bumps Cargo.toml + PKGBUILD, updates Cargo.lock, commits,
# and pushes. On main it also tags and pushes the tag; the tag push triggers
# the PKGBUILD publish run (tarball, debs, GitHub release, AUR hand-off). On
# a feature branch it only commits the version bump: merge, then
# `make release-tag VERSION=...`. PKGBUILD sha256sums becomes 'SKIP' here;
# build.yml fills in the real hash after building the release tarball.
#
# The gates are prerequisites of `release` and run in the order listed, the
# same order as the CI steps. Never run with -j: their order is the point.
# A gate failure means the tag push would fail CI the same way — fix the
# cause, never bypass the gate.
VERSION ?=
SUMMARY ?=
# Cargo.toml carries the bare version; the tag and the commit message carry
# the v prefix.
RELVERSION := $(patsubst v%,%,$(VERSION))
TAG := v$(RELVERSION)

release-preflight:
	@test -n "$(VERSION)" || { echo 'usage: make release VERSION=0.8.9 SUMMARY="short summary"'; exit 1; }
	@test -n "$(SUMMARY)" || { echo 'error: SUMMARY is empty (make release VERSION=0.8.9 SUMMARY="short summary")'; exit 1; }
	@test -z "$$(git status --porcelain --untracked-files=no)" || { echo 'error: uncommitted changes to tracked files'; echo 'commit them before running a release (untracked *.md files are fine)'; exit 1; }
	@untracked="$$(git ls-files --others --exclude-standard | grep -vE '\.md$$' || true)"; \
	test -z "$$untracked" || { echo 'error: untracked non-markdown files would be left out of the release'; echo "$$untracked"; echo 'commit or remove them before running a release (untracked *.md files are fine)'; exit 1; }
	@test -n "$$(git branch --show-current)" || { echo 'error: detached HEAD is not supported for release prep'; exit 1; }
	@! git rev-parse -q --verify "refs/tags/$(TAG)" >/dev/null 2>&1 || { echo 'error: tag $(TAG) already exists locally'; exit 1; }
	@git fetch origin --quiet
	@! git ls-remote --exit-code --tags origin "refs/tags/$(TAG)" >/dev/null 2>&1 || { echo 'error: tag $(TAG) already exists on origin'; exit 1; }
	@echo "==> Releasing $(TAG): $(SUMMARY)"

release: release-preflight format-check checks ast audit build-release test-release feature-matrix
	@echo "==> Bumping Cargo.toml to $(RELVERSION)..."
	sed -i 's/^version = ".*"/version = "$(RELVERSION)"/' Cargo.toml
	@echo "==> Bumping PKGBUILD to $(RELVERSION)..."
	sed -i 's/^pkgver=.*/pkgver=$(RELVERSION)/' PKGBUILD
	sed -i "s/^sha256sums=.*/sha256sums=('SKIP')/" PKGBUILD
	@echo "==> cargo build (updates Cargo.lock)..."
	$(CARGO) build
	@echo "==> Committing..."
	git add Cargo.toml Cargo.lock PKGBUILD
	git commit --no-verify --allow-empty -m "Release $(TAG): $(SUMMARY)"
	@if [ "$$(git branch --show-current)" = "main" ]; then \
		echo "==> git push origin main..."; \
		git push origin main; \
		echo "==> git tag $(TAG)..."; \
		git tag "$(TAG)"; \
		echo "==> git push origin $(TAG)..."; \
		git push origin "$(TAG)"; \
	else \
		echo "==> Release committed on $$(git branch --show-current) (not main)."; \
		echo "    PR-merge into main, then tag with: make release-tag VERSION=$(RELVERSION)"; \
	fi

# For a release committed on a feature branch, once that branch has merged:
# fetches origin/main, tags it, pushes the tag. On main `make release`
# already tagged; never run this there.
release-tag:
	@test -n "$(VERSION)" || { echo 'usage: make release-tag VERSION=0.8.9'; exit 1; }
	@! git rev-parse -q --verify "refs/tags/$(TAG)" >/dev/null 2>&1 || { echo 'error: tag $(TAG) already exists locally'; exit 1; }
	@git fetch origin --quiet
	@! git ls-remote --exit-code --tags origin "refs/tags/$(TAG)" >/dev/null 2>&1 || { echo 'error: tag $(TAG) already exists on origin'; exit 1; }
	@echo "==> Tagging origin/main as $(TAG)..."
	git tag "$(TAG)" "$$(git rev-parse origin/main)"
	@echo "==> git push origin $(TAG)..."
	git push origin "$(TAG)"
	@echo "==> Pushed $(TAG). The publish run: https://github.com/slatkin/mbv/actions"
