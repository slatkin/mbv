BINARY     := mbv
INSTALL_DIR := $(HOME)/.local/bin
DATA_DIR    := $(HOME)/.local/share/mbv
CONFIG_DIR  := $(HOME)/.config/mbv
TAILWINDCSS ?= tailwindcss

.PHONY: all build uninstall clean gui-css check-code-file-lines test-check-code-file-lines dev dev-release

all: build

build:
	cargo build --release

uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY)
	rm -rf $(DATA_DIR)

clean:
	cargo clean

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
