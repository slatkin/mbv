BINARY     := mbv
INSTALL_DIR := $(HOME)/.local/bin
DATA_DIR    := $(HOME)/.local/share/mbv
CONFIG_DIR  := $(HOME)/.config/mbv
UI_DIR      := crates/mbv-gui/ui

.PHONY: all build uninstall clean check-code-file-lines test-check-code-file-lines ui-install ui-build ui-check ui-lint ui-test

all: build

build:
	cargo build --release

uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY)
	rm -rf $(DATA_DIR)

clean:
	cargo clean

ui-install:
	pnpm -C $(UI_DIR) install --frozen-lockfile

ui-build: ui-install
	pnpm -C $(UI_DIR) build

ui-check: ui-install
	pnpm -C $(UI_DIR) check

ui-lint: ui-install
	pnpm -C $(UI_DIR) lint

ui-test: ui-install
	pnpm -C $(UI_DIR) test

check-code-file-lines:
	./scripts/check-code-file-lines.sh

test-check-code-file-lines:
	./scripts/check-code-file-lines-test.sh
