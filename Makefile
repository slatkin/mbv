BINARY     := mbv
INSTALL_DIR := $(HOME)/.local/bin
DATA_DIR    := $(HOME)/.local/share/mbv
CONFIG_DIR  := $(HOME)/.config/mbv

.PHONY: all build uninstall clean check-code-file-lines test-check-code-file-lines \
	check-media-list-ownership test-check-media-list-ownership

all: build

build:
	cargo build --release

uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY)
	rm -rf $(DATA_DIR)

clean:
	cargo clean

check-code-file-lines:
	./scripts/check-code-file-lines.sh

test-check-code-file-lines:
	./scripts/check-code-file-lines-test.sh

check-media-list-ownership:
	./scripts/check-media-list-ownership.sh

test-check-media-list-ownership:
	./scripts/check-media-list-ownership-test.sh
