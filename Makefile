BINARY     := mbv
# The AUR layout: the binaries resolve /usr/share/mbv and /etc/mbv at runtime,
# so PREFIX stays /usr. DESTDIR stages the tree, as makepkg's $pkgdir does.
PREFIX     ?= /usr
TAILWINDCSS ?= tailwindcss

.PHONY: all build install uninstall clean clean-dev gui-css check-code-file-lines test-check-code-file-lines dev dev-release

all: build

# Make passes DESTDIR from the command line into MAKEFLAGS, and every child
# inherits it. OpenSSL's vendored build reads DESTDIR and installs its libs into
# the staging tree, where openssl-sys cannot find them. Hide the make variables
# from cargo.
build:
	env -u DESTDIR -u MAKEFLAGS -u MAKEOVERRIDES -u MFLAGS cargo build --release

# The file set the AUR packages ship (PKGBUILD-git's package()).
install: build
	env -u DESTDIR -u MAKEFLAGS -u MAKEOVERRIDES -u MFLAGS cargo build --release -p mbv-gui
	install -Dm755 target/release/$(BINARY) $(DESTDIR)$(PREFIX)/bin/$(BINARY)
	install -Dm755 target/release/mbvd $(DESTDIR)$(PREFIX)/bin/mbvd
	install -Dm755 target/release/mbv-gui $(DESTDIR)$(PREFIX)/bin/mbv-gui
	install -Dm644 contrib/mbv.desktop $(DESTDIR)$(PREFIX)/share/applications/mbv.desktop
	install -Dm644 assets/icon.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/mbv.svg
	install -d $(DESTDIR)$(PREFIX)/share/mbv/scripts
	install -m644 scripts/mbv.lua scripts/mbv_*.lua $(DESTDIR)$(PREFIX)/share/mbv/scripts/
	install -Dm644 fonts/Material-Design-Iconic-Font.ttf $(DESTDIR)$(PREFIX)/share/mbv/fonts/Material-Design-Iconic-Font.ttf
	install -Dm644 dist/config.toml $(DESTDIR)$(PREFIX)/share/mbv/config.toml
	install -Dm644 contrib/mbvd.service $(DESTDIR)$(PREFIX)/lib/systemd/system/mbvd.service
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/mbv/LICENSE
	install -Dm644 LICENSES/MPL-2.0.txt $(DESTDIR)$(PREFIX)/share/licenses/mbv/LICENSES/MPL-2.0.txt
	install -m644 crates/mbv-gui/assets/fonts/*-OFL-1.1.txt crates/mbv-gui/assets/vendor/*.txt $(DESTDIR)$(PREFIX)/share/licenses/mbv/LICENSES/
	@if [ -f $(DESTDIR)/etc/mbv/config.toml ]; then \
		echo "Kept existing $(DESTDIR)/etc/mbv/config.toml"; \
	else \
		install -Dm640 dist/mbvd.toml $(DESTDIR)/etc/mbv/config.toml; \
	fi

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/$(BINARY) $(DESTDIR)$(PREFIX)/bin/mbvd $(DESTDIR)$(PREFIX)/bin/mbv-gui
	rm -rf $(DESTDIR)$(PREFIX)/share/mbv $(DESTDIR)$(PREFIX)/share/licenses/mbv
	rm -f $(DESTDIR)$(PREFIX)/share/applications/mbv.desktop
	rm -f $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/mbv.svg
	rm -f $(DESTDIR)$(PREFIX)/lib/systemd/system/mbvd.service
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

# The GUI starts its Owner from the sibling `mbv` binary, and the handshake
# refuses an Owner whose build id differs. `cargo run -p mbv-gui` alone leaves
# `mbv` stale after a commit, so build both before the window starts.
dev:
	cargo build -p mbv -p mbv-gui
	cargo run -p mbv-gui

# The same as `dev`, with the release profile: an optimized build that maps
# far less code than the debug binary, for memory and speed checks.
dev-release:
	cargo build --release -p mbv -p mbv-gui
	cargo run --release -p mbv-gui

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
