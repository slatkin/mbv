BINARY     := mbv
INSTALL_DIR := $(HOME)/.local/bin
DATA_DIR    := $(HOME)/.local/share/mbv
CONFIG_DIR  := $(HOME)/.config/mbv

.PHONY: all build uninstall clean

all: build

build:
	cargo build --release

uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY)
	rm -rf $(DATA_DIR)

clean:
	cargo clean
